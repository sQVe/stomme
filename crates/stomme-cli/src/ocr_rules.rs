use std::fs;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::layer::Language;

pub(crate) const RULE_FILE: &str = "docs/stomme/ocr-rules.json";
pub(crate) const PROJECT_RULE_FILE: &str = ".opencodereview/rule.json";

const GUIDE_INTRO: &str = "Apply stomme's coding guides below. Each guide starts with its repository path. Resolve its relative links from that path, and read linked files with file_read.";
const GUIDE_OUTRO: &str = "Also apply the rules in the project's AGENTS.md and the decisions in docs/adr/ that cover the change. Read them with file_read.";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProjectRules {
    #[serde(default)]
    rules: Vec<ProjectRule>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectRule {
    path: String,
    rule: String,
}

#[derive(Serialize)]
struct Rendered {
    rules: Vec<Entry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
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

    let project: ProjectRules = serde_json::from_value(value).map_err(shape_error)?;

    for (position, rule) in project.rules.iter().enumerate() {
        check_rule(rule, position + 1)?;
    }

    Ok(Some(project))
}

pub(crate) fn render(
    language: Language,
    guides: &[(&str, &str)],
    project: Option<&ProjectRules>,
) -> String {
    let guide_text = guide_text(guides);
    let mut rules = Vec::new();
    let mut include = Vec::new();
    let mut exclude = Vec::new();

    if let Some(project) = project {
        for rule in &project.rules {
            let text = format!("{}\n\n{guide_text}", rule.rule);

            rules.push(Entry {
                path: rule.path.clone(),
                rule: text,
                merge_system_rule: true,
            });
        }

        include.clone_from(&project.include);
        exclude.clone_from(&project.exclude);
    }

    rules.push(Entry {
        path: language_glob(language).to_string(),
        rule: guide_text,
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

fn check_rule(rule: &ProjectRule, index: usize) -> Result<(), OcrRulesError> {
    if rule.path.trim().is_empty() {
        return Err(OcrRulesError::EmptyPath {
            path: PROJECT_RULE_FILE.to_string(),
            index,
        });
    }

    if rule.rule.trim().is_empty() {
        return Err(OcrRulesError::EmptyRule {
            path: PROJECT_RULE_FILE.to_string(),
            index,
        });
    }

    Ok(())
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

#[cfg(test)]
mod tests;
