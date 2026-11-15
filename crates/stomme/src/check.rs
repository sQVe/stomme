use std::fmt;
use std::path::Path;

use crate::config::{self, CONFIG_FILE, Installation};
use crate::layer::{Catalog, Language};

mod adr_index;
mod required_parts;

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) path: String,
    pub(crate) rule: Rule,
    pub(crate) message: String,
}

// One table generates the enum, `Rule::ALL`, and `Rule::id`, so a new rule cannot be left out of
// `ALL`, which the catalog uses to find two owners of one rule ID.
macro_rules! rules {
    ($($variant:ident => $id:literal,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Rule {
            $($variant,)*
        }

        impl Rule {
            pub(crate) const ALL: &[Self] = &[$(Self::$variant,)*];

            pub(crate) fn id(self) -> &'static str {
                match self {
                    $(Self::$variant => $id,)*
                }
            }
        }
    };
}

rules! {
    NoStomme => "no-stomme",
    BrokenInstallation => "broken-installation",
    RequiredPart => "required-part",
    TauFullCheck => "tau-full-check",
    AdrDuplicateNumber => "adr-duplicate-number",
    AdrHeading => "adr-heading",
    AdrRetiredNumber => "adr-retired-number",
    AdrIndexCount => "adr-index-count",
    AdrIndexLabel => "adr-index-label",
    AdrIndexMissingFile => "adr-index-missing-file",
    AdrIndexEntry => "adr-index-entry",
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

pub(crate) fn run(root: &Path, catalog: &Catalog) -> anyhow::Result<Vec<Problem>> {
    let installation = config::read(root, catalog);
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

#[cfg(test)]
mod tests;
