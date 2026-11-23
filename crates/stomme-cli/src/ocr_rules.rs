use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::layer::Language;

pub(crate) const RULE_FILE: &str = "docs/stomme/ocr-rules.json";
pub(crate) const PROJECT_RULE_FILE: &str = ".opencodereview/rule.json";

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

pub(crate) struct Guides {
    pub(crate) general: (String, &'static str),
    pub(crate) languages: Vec<(Language, (String, &'static str))>,
    pub(crate) layers: Vec<(String, &'static str)>,
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
    project: Option<&ProjectRules>,
) -> String {
    let selected_text = guide_text(&guides.selected(language));
    let mut rules = Vec::new();
    let mut include = Vec::new();
    let mut exclude = Vec::new();

    if let Some(project) = project {
        for rule in &project.rules {
            rules.push(Entry {
                path: rule.path.clone(),
                rule: rule_text(rule, guides, &selected_text),
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
                .map(|(path, text)| (path.as_str(), *text)),
        );

        guides
    }

    fn general_and_language(&self, language: Language) -> Vec<(&str, &str)> {
        let (path, text) = self.language(language);

        vec![
            (self.general.0.as_str(), self.general.1),
            (path.as_str(), *text),
        ]
    }

    pub(crate) fn language(&self, language: Language) -> &(String, &'static str) {
        &self
            .languages
            .iter()
            .find(|(name, _)| *name == language)
            .expect("the guides hold every language")
            .1
    }
}

fn rule_text(rule: &ProjectRule, guides: &Guides, selected_text: &str) -> String {
    let Some(language) = rule.language else {
        return format!("{}\n\n{selected_text}", rule.rule);
    };
    let language_text = guide_text(&guides.general_and_language(language));

    format!("{}\n\n{language_text}", rule.rule)
}

fn guide_text(guides: &[(&str, &str)]) -> String {
    let mut text = String::from(GUIDE_INTRO);

    for (path, content) in guides {
        text.push_str("\n\nFile: ");
        text.push_str(path);
        text.push_str("\n\n");
        text.push_str(content.trim_end());
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
