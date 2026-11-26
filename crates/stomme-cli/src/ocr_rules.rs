use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::config::{ROLES, normalize};
use crate::layer::Language;

pub(crate) const RULE_FILE: &str = "docs/stomme/ocr-rules.json";
pub(crate) const PROJECT_RULE_FILE: &str = ".opencodereview/rule.json";

const ROLE_ADR: &str =
    include_str!("../../../foundation/shared/adr/0004-roles-languages-and-layers.md");
const ROLE_MARKER_START: &str = "<!-- role:";
const ROLE_MARKER_END: &str = " -->";

const GUIDE_INTRO: &str = "Apply stomme's coding guides below. Each guide starts with its repository path. Resolve its relative links from that path, and read linked files with file_read.";
const GUIDE_OUTRO: &str = "Also apply the rules in the project's AGENTS.md and the decisions in docs/adr/ that cover the change. Read them with file_read.";

#[derive(Debug)]
pub(crate) struct ProjectRules {
    rules: Vec<ProjectRule>,
    include: Vec<String>,
    exclude: Vec<String>,
}

#[derive(Debug)]
struct ProjectRule {
    path: String,
    rule: String,
    language: Option<Language>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRulesFile {
    #[serde(default)]
    rules: Vec<ProjectRuleFile>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRuleFile {
    path: String,
    rule: String,
    #[serde(default, deserialize_with = "present_value")]
    language: Option<Value>,
}

fn present_value<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

pub(crate) struct Guide {
    pub(crate) path: String,
    pub(crate) text: &'static str,
}

pub(crate) struct LanguageGuides {
    pub(crate) rust: Guide,
    pub(crate) typescript: Guide,
}

pub(crate) struct Guides {
    pub(crate) general: Guide,
    pub(crate) languages: LanguageGuides,
    pub(crate) layers: Vec<Guide>,
}

#[derive(Serialize)]
struct Rendered {
    rules: Vec<Entry>,
    include: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    exclude: Vec<String>,
}

#[derive(Serialize)]
struct Entry {
    path: String,
    rule: String,
    merge_system_rule: bool,
}

#[derive(Debug, Error)]
pub(crate) enum OcrRulesError {
    #[error("could not read {path}: {source}")]
    Read { path: String, source: io::Error },
    #[error("{path} is not valid JSON: {reason}")]
    InvalidJson { path: String, reason: String },
    #[error("{path} is not a rule file OCR accepts: {reason}")]
    Shape { path: String, reason: String },
    #[error("{path} rule {index} has an empty path")]
    EmptyPath { path: String, index: usize },
    #[error("{path} rule {index} has an empty rule")]
    EmptyRule { path: String, index: usize },
    #[error("{path} rule {index} has an unknown language {language}; use rust or typescript")]
    UnknownLanguage {
        path: String,
        index: usize,
        language: String,
    },
    #[error(
        "{path} rule {index} has a language {value} that is not a string; use rust or typescript"
    )]
    LanguageNotString {
        path: String,
        index: usize,
        value: String,
    },
    #[error("{path} is not a regular file")]
    NotAFile { path: String },
}

pub(crate) fn read_project(root: &Path) -> Result<Option<ProjectRules>, OcrRulesError> {
    if !check_folder(root)? {
        return Ok(None);
    }

    let path = root.join(PROJECT_RULE_FILE);
    let Some(metadata) = entry_metadata(&path)? else {
        return Ok(None);
    };

    if !metadata.is_file() {
        return Err(not_a_file());
    }

    let bytes = fs::read(&path).map_err(read_error)?;
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|error| OcrRulesError::InvalidJson {
            path: PROJECT_RULE_FILE.to_string(),
            reason: error.to_string(),
        })?;

    let object = value.as_object().ok_or_else(not_an_object)?;

    if let Some(Value::Array(rules)) = object.get("rules") {
        for rule in rules {
            if !rule.is_object() {
                return Err(not_an_object());
            }
        }
    }

    let file: ProjectRulesFile = serde_json::from_slice(&bytes).map_err(shape_error)?;
    let project = ProjectRules::from_file(file)?;

    Ok(Some(project))
}

pub(crate) fn render(
    language: Language,
    guides: &Guides,
    roles: &BTreeMap<String, String>,
    project: Option<&ProjectRules>,
) -> String {
    let selected = guides.selected(language);
    let selected_text = guide_text(&selected, &role_clauses(roles, &selected));
    let mut rules = Vec::new();
    let mut include = Vec::new();
    let mut exclude = Vec::new();

    if let Some(project) = project {
        for rule in &project.rules {
            rules.push(Entry {
                path: rule.path.clone(),
                rule: rule_text(rule, guides, roles, &selected_text),
                merge_system_rule: true,
            });
        }

        for pattern in &project.include {
            push_unique(&mut include, pattern);
        }

        exclude.clone_from(&project.exclude);
    }

    for pattern in test_includes(language) {
        push_unique(&mut include, pattern);
    }

    if let Some(project) = project {
        for rule in &project.rules {
            let Some(rule_language) = rule.language else {
                continue;
            };

            for pattern in test_includes(rule_language) {
                push_unique(&mut include, pattern);
            }
        }
    }

    rules.push(Entry {
        path: language_glob(language).to_string(),
        rule: selected_text,
        merge_system_rule: true,
    });

    let document = Rendered {
        rules,
        include,
        exclude,
    };

    let mut text = serde_json::to_string_pretty(&document)
        .expect("a document of strings, lists, and booleans serializes");
    text.push('\n');

    text
}

fn read_error(source: io::Error) -> OcrRulesError {
    OcrRulesError::Read {
        path: PROJECT_RULE_FILE.to_string(),
        source,
    }
}

fn check_folder(root: &Path) -> Result<bool, OcrRulesError> {
    let folder = Path::new(PROJECT_RULE_FILE)
        .parent()
        .expect("the project rule file lies inside a folder");

    match entry_metadata(&root.join(folder))? {
        Some(metadata) if metadata.is_dir() => Ok(true),
        Some(_) => Err(not_a_file()),
        None => Ok(false),
    }
}

fn entry_metadata(path: &Path) -> Result<Option<fs::Metadata>, OcrRulesError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(read_error(source)),
    }
}

fn not_a_file() -> OcrRulesError {
    OcrRulesError::NotAFile {
        path: PROJECT_RULE_FILE.to_string(),
    }
}

fn shape_error(error: serde_json::Error) -> OcrRulesError {
    OcrRulesError::Shape {
        path: PROJECT_RULE_FILE.to_string(),
        reason: error.to_string(),
    }
}

fn not_an_object() -> OcrRulesError {
    OcrRulesError::Shape {
        path: PROJECT_RULE_FILE.to_string(),
        reason: "expected an object".to_string(),
    }
}

impl ProjectRules {
    fn from_file(file: ProjectRulesFile) -> Result<Self, OcrRulesError> {
        let mut rules = Vec::with_capacity(file.rules.len());

        for (position, rule) in file.rules.into_iter().enumerate() {
            rules.push(ProjectRule::from_file(rule, position + 1)?);
        }

        Ok(Self {
            rules,
            include: file.include,
            exclude: file.exclude,
        })
    }
}

impl ProjectRule {
    fn from_file(file: ProjectRuleFile, index: usize) -> Result<Self, OcrRulesError> {
        if file.path.trim().is_empty() {
            return Err(OcrRulesError::EmptyPath {
                path: PROJECT_RULE_FILE.to_string(),
                index,
            });
        }

        if file.rule.trim().is_empty() {
            return Err(OcrRulesError::EmptyRule {
                path: PROJECT_RULE_FILE.to_string(),
                index,
            });
        }

        let language = match file.language {
            None => None,
            Some(Value::String(name)) => {
                Some(
                    Language::parse(&name).ok_or_else(|| OcrRulesError::UnknownLanguage {
                        path: PROJECT_RULE_FILE.to_string(),
                        index,
                        language: name,
                    })?,
                )
            }
            Some(value) => {
                return Err(OcrRulesError::LanguageNotString {
                    path: PROJECT_RULE_FILE.to_string(),
                    index,
                    value: value.to_string(),
                });
            }
        };

        Ok(Self {
            path: file.path,
            rule: file.rule,
            language,
        })
    }
}

impl Guides {
    fn selected(&self, language: Language) -> Vec<(&str, &str)> {
        let mut guides = self.general_and_language(language);
        guides.extend(
            self.layers
                .iter()
                .map(|guide| (guide.path.as_str(), guide.text)),
        );

        guides
    }

    fn general_and_language(&self, language: Language) -> Vec<(&str, &str)> {
        let guide = self.language(language);

        vec![
            (self.general.path.as_str(), self.general.text),
            (guide.path.as_str(), guide.text),
        ]
    }

    pub(crate) fn language(&self, language: Language) -> &Guide {
        match language {
            Language::Rust => &self.languages.rust,
            Language::TypeScript => &self.languages.typescript,
        }
    }
}

fn rule_text(
    rule: &ProjectRule,
    guides: &Guides,
    roles: &BTreeMap<String, String>,
    selected_text: &str,
) -> String {
    let Some(language) = rule.language else {
        return format!("{}\n\n{selected_text}", rule.rule);
    };
    let language_guides = guides.general_and_language(language);
    let language_text = guide_text(&language_guides, &role_clauses(roles, &language_guides));

    format!("{}\n\n{language_text}", rule.rule)
}

fn role_clauses(roles: &BTreeMap<String, String>, guides: &[(&str, &str)]) -> Vec<String> {
    let mut clauses = Vec::new();

    for role in ROLES {
        let Some(path) = roles.get(*role) else {
            continue;
        };
        let root = normalize(path);

        if root.is_empty() {
            continue;
        }

        let mut blocks = marked_blocks(ROLE_ADR, role);

        assert!(
            !blocks.is_empty(),
            "ADR 0004 marks no block for the declared role `{role}`"
        );

        for (_, text) in guides {
            blocks.extend(marked_blocks(text, role));
        }

        clauses.push(format!(
            "If the file path starts with {root}/, apply: {}",
            blocks.join("\n\n")
        ));
    }

    clauses
}

fn marked_blocks(text: &str, role: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let Some((marker_role, content)) = marker(line) else {
            continue;
        };

        assert!(
            ROLES.contains(&marker_role),
            "a block is marked for the unknown role `{marker_role}`"
        );

        if marker_role == role {
            blocks.push(block_text(&lines, index, content));
        }
    }

    blocks
}

fn marker(line: &str) -> Option<(&str, &str)> {
    let start = line.find(ROLE_MARKER_START)?;
    let rest = &line[start + ROLE_MARKER_START.len()..];
    let end = rest.find(ROLE_MARKER_END)?;

    Some((&rest[..end], &line[..start]))
}

fn block_text(lines: &[&str], marker_index: usize, content: &str) -> String {
    let start = block_start(lines, marker_index);
    let mut block: Vec<&str> = lines[start..marker_index].to_vec();

    block.push(content.trim_end());

    let first = block[0]
        .strip_prefix("- ")
        .expect("a marked block starts with a list item");

    block[0] = first;

    block.join("\n")
}

fn block_start(lines: &[&str], marker_index: usize) -> usize {
    if lines
        .get(marker_index + 1)
        .is_some_and(|line| line.starts_with("  "))
    {
        panic!("a marked block is a list item whose marker ends its last line");
    }

    let mut start = marker_index;

    while start > 0 && lines[start].starts_with("  ") {
        start -= 1;
    }

    if !lines[start].starts_with("- ") {
        panic!("a marked block is a list item whose marker ends its last line");
    }

    start
}

pub(crate) fn without_marked_blocks(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut dropped = vec![false; lines.len()];

    for (index, line) in lines.iter().enumerate() {
        if marker(line).is_none() {
            continue;
        }

        let start = block_start(&lines, index);

        for flag in &mut dropped[start..=index] {
            *flag = true;
        }
    }

    lines
        .iter()
        .zip(dropped)
        .filter(|(_, drop)| !drop)
        .map(|(line, _)| *line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn guide_text(guides: &[(&str, &str)], clauses: &[String]) -> String {
    let mut text = String::from(GUIDE_INTRO);

    for (path, content) in guides {
        text.push_str("\n\nFile: ");
        text.push_str(path);
        text.push_str("\n\n");
        text.push_str(without_marked_blocks(content).trim_end());
    }

    if !clauses.is_empty() {
        text.push_str("\n\n");
        text.push_str(&clauses.join("\n\n"));
    }

    text.push_str("\n\n");
    text.push_str(GUIDE_OUTRO);

    text
}

fn language_glob(language: Language) -> &'static str {
    match language {
        Language::Rust => "**/*.rs",
        Language::TypeScript => "**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
    }
}

fn test_includes(language: Language) -> &'static [&'static str] {
    match language {
        Language::Rust => &[
            "**/*_test.rs",
            "**/*_tests.rs",
            "**/tests.rs",
            "**/tests/**/*.rs",
            "**/benches/**/*.rs",
        ],
        Language::TypeScript => &[
            "**/*.test.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "**/*.spec.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
            "**/__tests__/**/*.{ts,tsx,js,jsx,mjs,cjs,mts,cts}",
        ],
    }
}

fn push_unique(patterns: &mut Vec<String>, pattern: &str) {
    if patterns.iter().any(|existing| existing == pattern) {
        return;
    }

    patterns.push(pattern.to_string());
}

#[cfg(test)]
mod tests;
