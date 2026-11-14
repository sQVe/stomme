use std::fmt;
use std::path::Path;

use crate::config::{self, CONFIG_FILE, Installation};

mod adr_index;
mod required_parts;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) path: String,
    pub(crate) rule: Rule,
    pub(crate) message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Rule {
    NoStomme,
    BrokenInstallation,
    RequiredPart,
    TauFullCheck,
    AdrDuplicateNumber,
    AdrHeading,
    AdrRetiredNumber,
    AdrIndexCount,
    AdrIndexLabel,
    AdrIndexMissingFile,
    AdrIndexEntry,
}

impl Rule {
    fn id(self) -> &'static str {
        match self {
            Self::NoStomme => "no-stomme",
            Self::BrokenInstallation => "broken-installation",
            Self::RequiredPart => "required-part",
            Self::TauFullCheck => "tau-full-check",
            Self::AdrDuplicateNumber => "adr-duplicate-number",
            Self::AdrHeading => "adr-heading",
            Self::AdrRetiredNumber => "adr-retired-number",
            Self::AdrIndexCount => "adr-index-count",
            Self::AdrIndexLabel => "adr-index-label",
            Self::AdrIndexMissingFile => "adr-index-missing-file",
            Self::AdrIndexEntry => "adr-index-entry",
        }
    }
}

impl fmt::Display for Problem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} [{}]",
            self.path,
            self.message,
            self.rule.id()
        )
    }
}

pub(crate) fn run(root: &Path) -> anyhow::Result<Vec<Problem>> {
    let mut problems: Vec<Problem> = installation_problem(root).into_iter().collect();

    problems.extend(required_parts::check(root)?);
    problems.extend(adr_index::check(root)?);

    Ok(problems)
}

fn installation_problem(root: &Path) -> Option<Problem> {
    let (rule, message) = match config::read(root) {
        Installation::Valid(_) => return None,
        Installation::NoStomme => (
            Rule::NoStomme,
            "is missing; add one with `schema = 1` and a `kind`".to_string(),
        ),
        Installation::Broken(reason) => (Rule::BrokenInstallation, reason),
    };

    Some(Problem {
        path: CONFIG_FILE.to_string(),
        rule,
        message,
    })
}
