use serde_json::{Value, json};

use super::request::Mode;
use super::survivors::Survivor;

pub(super) fn report_value(
    status: &str,
    error: Option<&str>,
    survivors: &[Survivor],
    unmutated: &[String],
) -> Value {
    let survivor_values: Vec<Value> = survivors
        .iter()
        .map(|survivor| {
            json!({
                "file": survivor.file,
                "line": survivor.line,
                "mutator": survivor.mutator,
                "original": survivor.original,
                "replacement": survivor.replacement,
                "covered": true,
            })
        })
        .collect();

    let mut report = json!({
        "version": 1,
        "status": status,
        "survivors": survivor_values,
        "unmutated": unmutated,
    });

    if let Some(error) = error {
        report["error"] = json!(error);
    }

    report
}

pub(super) fn ran_report(mode: Mode, survivors: &[Survivor], unmutated: &[String]) -> (Value, u8) {
    let (status, exit, error) = match (survivors.is_empty(), unmutated.is_empty(), mode) {
        (true, true, _) => ("pass", 0, None),
        (false, true, Mode::Changed) => ("survivors", 2, None),
        (false, true, Mode::Audit) => ("survivors", 0, None),
        (true, false, _) => (
            "error",
            1,
            Some("Files over the mutant cap were not mutated."),
        ),
        (false, false, _) => ("survivors", 1, None),
    };

    (report_value(status, error, survivors, unmutated), exit)
}

#[cfg(test)]
mod tests;
