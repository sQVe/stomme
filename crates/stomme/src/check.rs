use std::fmt;
use std::path::Path;

use crate::config::{self, CONFIG_FILE, Installation};
use crate::layer::Language;

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "no language or layer owns a rule yet; only tests build those owners"
)]
pub(crate) enum Owner {
    Common,
    Language(Language),
    Layer(&'static str),
}

impl Rule {
    pub(crate) const ALL: &[Self] = &[
        Self::NoStomme,
        Self::BrokenInstallation,
        Self::RequiredPart,
        Self::TauFullCheck,
        Self::AdrDuplicateNumber,
        Self::AdrHeading,
        Self::AdrRetiredNumber,
        Self::AdrIndexCount,
        Self::AdrIndexLabel,
        Self::AdrIndexMissingFile,
        Self::AdrIndexEntry,
    ];

    pub(crate) fn id(self) -> &'static str {
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

    pub(crate) fn owner(self) -> Owner {
        match self {
            Self::NoStomme
            | Self::BrokenInstallation
            | Self::RequiredPart
            | Self::TauFullCheck
            | Self::AdrDuplicateNumber
            | Self::AdrHeading
            | Self::AdrRetiredNumber
            | Self::AdrIndexCount
            | Self::AdrIndexLabel
            | Self::AdrIndexMissingFile
            | Self::AdrIndexEntry => Owner::Common,
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

impl fmt::Display for Owner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Common => formatter.write_str("the common rules"),
            Self::Language(language) => write!(formatter, "the {language} language"),
            Self::Layer(layer) => write!(formatter, "layer `{layer}`"),
        }
    }
}

pub(crate) fn run(root: &Path) -> anyhow::Result<Vec<Problem>> {
    let installation = config::read(root);
    let mut problems: Vec<Problem> = installation_problem(&installation).into_iter().collect();

    problems.extend(required_parts::check(root)?);
    problems.extend(adr_index::check(root)?);

    Ok(problems)
}

fn installation_problem(installation: &Installation) -> Option<Problem> {
    let (rule, message) = match installation {
        Installation::Valid(_) => return None,
        Installation::NoStomme => (
            Rule::NoStomme,
            "is missing; add one with `schema = 1` and a `language`".to_string(),
        ),
        Installation::Broken(reason) => (Rule::BrokenInstallation, reason.clone()),
    };

    Some(Problem {
        path: CONFIG_FILE.to_string(),
        rule,
        message,
    })
}
