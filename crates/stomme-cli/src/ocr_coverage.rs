use std::collections::HashSet;

use serde::Deserialize;
use thiserror::Error;

const SCHEMA_VERSION: &str = "ocr.run-manifest/v1";
const UNKNOWN_CLASSIFICATION: &str = "unknown";
const MISSING_REASON: &str = "no coverage record";

#[derive(Debug, Deserialize)]
struct RunResult {
    status: String,
    manifest: Manifest,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: String,
    terminal_state: String,
    coverage: CoverageRecord,
}

#[derive(Debug, Deserialize)]
struct CoverageRecord {
    selected: Vec<CoverageItem>,
    completed: Vec<CoverageItem>,
    reused: Vec<CoverageItem>,
    failed: Vec<CoverageItem>,
    waived: Vec<CoverageItem>,
    excluded: Vec<ExcludedItem>,
}

#[derive(Debug, Deserialize)]
struct CoverageItem {
    item_id: String,
    path: String,
    #[serde(default)]
    classification: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExcludedItem {
    path: String,
    exclude_reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    Complete,
    Partial,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Reviewed,
    Partial,
    Failed,
}

#[derive(Debug)]
struct Unreviewed {
    path: String,
    classification: String,
    reason: String,
}

#[derive(Debug)]
struct Excluded {
    path: String,
    reason: String,
}

#[derive(Debug)]
pub(crate) struct Coverage {
    selected: usize,
    reviewed: usize,
    unreviewed: Vec<Unreviewed>,
    excluded: Vec<Excluded>,
    outcome: Outcome,
}

#[derive(Debug, Error)]
pub(crate) enum OcrCoverageError {
    #[error("{path} is not a usable OCR result: {reason}")]
    InvalidJson { path: String, reason: String },
    #[error("{path} has unsupported schema {schema_version}, expected {SCHEMA_VERSION}")]
    UnsupportedSchema {
        path: String,
        schema_version: String,
    },
    #[error("{path} has unknown status {status}")]
    UnknownStatus { path: String, status: String },
    #[error("{path} status {status} differs from manifest terminal state {terminal_state}")]
    StatusMismatch {
        path: String,
        status: String,
        terminal_state: String,
    },
    #[error("{path} is partial but every selected item is reviewed")]
    InconsistentPartial { path: String },
}

pub(crate) fn parse(bytes: &[u8], label: &str) -> Result<Coverage, OcrCoverageError> {
    let result: RunResult =
        serde_json::from_slice(bytes).map_err(|error| OcrCoverageError::InvalidJson {
            path: label.to_string(),
            reason: error.to_string(),
        })?;

    check_schema(label, &result.manifest)?;
    let status = parse_status(label, &result.status)?;
    check_status_matches(label, &result.status, &result.manifest.terminal_state)?;

    let coverage = Coverage::from_record(status, &result.manifest.coverage);

    if status == Status::Partial && coverage.unreviewed.is_empty() {
        return Err(OcrCoverageError::InconsistentPartial {
            path: label.to_string(),
        });
    }

    Ok(coverage)
}

impl Coverage {
    pub(crate) fn exit_code(&self) -> u8 {
        match self.outcome {
            Outcome::Reviewed => 0,
            Outcome::Failed => 1,
            Outcome::Partial => 2,
        }
    }

    pub(crate) fn fragment(&self) -> String {
        let mut lines = vec![
            "### Review coverage".to_string(),
            String::new(),
            self.summary_sentence(),
        ];

        if !self.unreviewed.is_empty() {
            lines.push(String::new());
            lines.push("Not reviewed:".to_string());
            lines.push(String::new());

            for item in &self.unreviewed {
                let path = code_span(&item.path);
                let line = format!("- {path}: {}, {}", item.classification, item.reason);
                lines.push(line);
            }
        }

        if !self.excluded.is_empty() {
            lines.push(String::new());
            lines.extend(self.excluded_block());
        }

        let mut text = lines.join("\n");
        text.push('\n');

        text
    }

    pub(crate) fn status_line(&self) -> String {
        match self.outcome {
            Outcome::Failed => format!("OCR review failed: {}", self.reviewed_counts()),
            Outcome::Partial => format!("OCR review partial: {}", self.reviewed_counts()),
            Outcome::Reviewed if self.selected == 0 => {
                "OCR review skipped: no files selected".to_string()
            }
            Outcome::Reviewed => format!("OCR review complete: {}", self.reviewed_counts()),
        }
    }

    pub(crate) fn unreviewed_lines(&self) -> Vec<String> {
        self.unreviewed
            .iter()
            .map(|item| {
                let classification = &item.classification;
                let reason = &item.reason;
                format!("{}: not reviewed ({classification}: {reason})", item.path)
            })
            .collect()
    }

    fn from_record(status: Status, record: &CoverageRecord) -> Self {
        let reviewed_ids = reviewed_ids(record);
        let mut unreviewed: Vec<Unreviewed> = record
            .selected
            .iter()
            .filter(|item| !reviewed_ids.contains(item.item_id.as_str()))
            .map(|item| unreviewed_item(record, item))
            .collect();

        unreviewed.sort_by(|left, right| left.path.cmp(&right.path));

        let reviewed = record.selected.len() - unreviewed.len();
        let excluded = excluded_items(record);
        let outcome = outcome(status, &unreviewed);

        Coverage {
            selected: record.selected.len(),
            reviewed,
            unreviewed,
            excluded,
            outcome,
        }
    }

    fn summary_sentence(&self) -> String {
        if self.outcome == Outcome::Failed {
            return format!("The OCR review failed. {}.", self.reviewed_sentence());
        }

        if self.selected == 0 {
            return "OCR selected no files to review.".to_string();
        }

        if self.unreviewed.is_empty() {
            return format!("{}.", self.reviewed_sentence());
        }

        let sentence = self.reviewed_sentence();
        format!("{sentence}. The check fails until every selected file is reviewed.")
    }

    fn reviewed_sentence(&self) -> String {
        format!("OCR {}", self.reviewed_counts())
    }

    fn reviewed_counts(&self) -> String {
        let noun = if self.selected == 1 { "file" } else { "files" };
        let reviewed = self.reviewed;
        let selected = self.selected;
        format!("reviewed {reviewed} of {selected} selected {noun}")
    }

    fn excluded_block(&self) -> Vec<String> {
        let noun = if self.excluded.len() == 1 {
            "file"
        } else {
            "files"
        };
        let count = self.excluded.len();
        let mut lines = vec![
            "<details>".to_string(),
            format!("<summary>{count} {noun} excluded by the review rules</summary>"),
            String::new(),
        ];

        for item in &self.excluded {
            let path = code_span(&item.path);
            lines.push(format!("- {path}: {}", item.reason));
        }

        lines.push("</details>".to_string());

        lines
    }
}

fn check_schema(path: &str, manifest: &Manifest) -> Result<(), OcrCoverageError> {
    if manifest.schema_version == SCHEMA_VERSION {
        return Ok(());
    }

    Err(OcrCoverageError::UnsupportedSchema {
        path: path.to_string(),
        schema_version: manifest.schema_version.clone(),
    })
}

fn parse_status(path: &str, status: &str) -> Result<Status, OcrCoverageError> {
    match status {
        "complete" => Ok(Status::Complete),
        "partial" => Ok(Status::Partial),
        "failed" => Ok(Status::Failed),
        "skipped" => Ok(Status::Skipped),
        other => Err(OcrCoverageError::UnknownStatus {
            path: path.to_string(),
            status: other.to_string(),
        }),
    }
}

fn check_status_matches(
    path: &str,
    status: &str,
    terminal_state: &str,
) -> Result<(), OcrCoverageError> {
    if status == terminal_state {
        return Ok(());
    }

    Err(OcrCoverageError::StatusMismatch {
        path: path.to_string(),
        status: status.to_string(),
        terminal_state: terminal_state.to_string(),
    })
}

fn reviewed_ids(record: &CoverageRecord) -> HashSet<&str> {
    record
        .completed
        .iter()
        .chain(&record.reused)
        .map(|item| item.item_id.as_str())
        .collect()
}

fn unreviewed_item(record: &CoverageRecord, item: &CoverageItem) -> Unreviewed {
    if let Some(failed) = find_item(&record.failed, &item.item_id) {
        return Unreviewed {
            path: item.path.clone(),
            classification: failed
                .classification
                .clone()
                .unwrap_or_else(|| UNKNOWN_CLASSIFICATION.to_string()),
            reason: failed
                .reason
                .clone()
                .unwrap_or_else(|| MISSING_REASON.to_string()),
        };
    }

    if let Some(waived) = find_item(&record.waived, &item.item_id) {
        return Unreviewed {
            path: item.path.clone(),
            classification: "waived".to_string(),
            reason: waived
                .reason
                .clone()
                .unwrap_or_else(|| MISSING_REASON.to_string()),
        };
    }

    Unreviewed {
        path: item.path.clone(),
        classification: UNKNOWN_CLASSIFICATION.to_string(),
        reason: MISSING_REASON.to_string(),
    }
}

fn find_item<'a>(items: &'a [CoverageItem], item_id: &str) -> Option<&'a CoverageItem> {
    items.iter().find(|item| item.item_id == item_id)
}

fn excluded_items(record: &CoverageRecord) -> Vec<Excluded> {
    let mut excluded: Vec<Excluded> = record
        .excluded
        .iter()
        .map(|item| Excluded {
            path: item.path.clone(),
            reason: item.exclude_reason.clone(),
        })
        .collect();

    excluded.sort_by(|left, right| left.path.cmp(&right.path));

    excluded
}

fn outcome(status: Status, unreviewed: &[Unreviewed]) -> Outcome {
    if status == Status::Failed {
        return Outcome::Failed;
    }

    if unreviewed.is_empty() {
        Outcome::Reviewed
    } else {
        Outcome::Partial
    }
}

fn code_span(text: &str) -> String {
    let fence = "`".repeat(longest_backtick_run(text) + 1);

    if needs_padding(text) {
        return format!("{fence} {text} {fence}");
    }

    format!("{fence}{text}{fence}")
}

fn needs_padding(text: &str) -> bool {
    if starts_or_ends_with_backtick(text) {
        return true;
    }

    starts_or_ends_with_space(text) && !is_only_spaces(text)
}

fn starts_or_ends_with_backtick(text: &str) -> bool {
    text.starts_with('`') || text.ends_with('`')
}

fn starts_or_ends_with_space(text: &str) -> bool {
    text.starts_with(' ') || text.ends_with(' ')
}

fn is_only_spaces(text: &str) -> bool {
    text.chars().all(|character| character == ' ')
}

fn longest_backtick_run(text: &str) -> usize {
    let mut longest = 0;
    let mut current = 0;

    for character in text.chars() {
        if character == '`' {
            current += 1;
            longest = longest.max(current);
        } else {
            current = 0;
        }
    }

    longest
}

#[cfg(test)]
mod tests;
