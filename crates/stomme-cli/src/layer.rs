use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use serde::Deserialize;
use thiserror::Error;

use crate::rule::{Owner, Rule};
use detection::Detection;
use policy::{LintPolicy, RequiredBan};

pub(crate) mod detection;
mod gpui;
mod gpui_component;
mod herdr;
mod pi;
pub(crate) mod policy;
mod react;

const REGISTRY: &[Source] = &[
    gpui::LAYER,
    gpui_component::LAYER,
    herdr::LAYER,
    pi::LAYER,
    react::LAYER,
];

macro_rules! policy_file {
    ($path:literal) => {
        PolicyFile {
            path: $path,
            text: include_str!(concat!("../../../", $path)),
        }
    };
}

pub(crate) const RUST: RustSource = RustSource {
    lints: policy_file!("foundation/languages/rust/lints.toml"),
    clippy: policy_file!("foundation/languages/rust/clippy.toml"),
};

static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    Catalog::new(REGISTRY, &RUST, &rule_owners())
        .expect("the layer tests check the compiled-in catalog")
});

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Language {
    Rust,
    TypeScript,
}

impl fmt::Display for Language {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
        };

        formatter.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Refinement {
    TsxComponents,
}

pub(crate) struct Source {
    name: &'static str,
    manifest: &'static str,
    guide: &'static str,
    bans: Option<LayerBans>,
    refinements: &'static [Refinement],
    detection: Detection,
}

pub(crate) struct LayerBans {
    text: &'static str,
    rule: Rule,
}

pub(crate) struct RustSource {
    pub(crate) lints: PolicyFile,
    pub(crate) clippy: PolicyFile,
}

#[derive(Clone, Copy)]
pub(crate) struct PolicyFile {
    pub(crate) path: &'static str,
    pub(crate) text: &'static str,
}

#[derive(Debug)]
pub(crate) struct Catalog {
    layers: BTreeMap<&'static str, Layer>,
    layer_bans: BTreeMap<&'static str, (Rule, Vec<RequiredBan>)>,
    rust_lints: LintPolicy,
    rust_bans: Vec<RequiredBan>,
}

#[derive(Debug)]
struct Layer {
    manifest: Manifest,
    guide: &'static str,
    refinements: &'static [Refinement],
    detection: Detection,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    language: Language,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    conflicts: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum CatalogError {
    #[error("layer `{layer}` has an invalid layer.toml: {reason}")]
    InvalidManifest { layer: &'static str, reason: String },
    #[error("layer `{layer}` has an invalid clippy.toml: {reason}")]
    InvalidLayerBans { layer: &'static str, reason: String },
    #[error("the Rust policy file {file} is invalid: {reason}")]
    InvalidRustPolicy { file: &'static str, reason: String },
    #[error("layer `{layer}` names unknown layer `{unknown}`")]
    UnknownLayer {
        layer: &'static str,
        unknown: String,
    },
    #[error("layers form a requirement cycle: {}", layers.join(" requires "))]
    RequirementCycle { layers: Vec<String> },
    #[error("rule `{id}` belongs to {first} and to {second}")]
    DuplicateRuleId {
        id: &'static str,
        first: Owner,
        second: Owner,
    },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub(crate) enum SelectionError {
    #[error("lists unknown layer `{layer}`")]
    UnknownLayer { layer: String },
    #[error("lists layer `{layer}` twice")]
    DuplicateLayer { layer: String },
    #[error("lists layer `{layer}`, which is for {layer_language}, but `language` is {language}")]
    WrongLanguage {
        layer: String,
        layer_language: Language,
        language: Language,
    },
    #[error(
        "lists layer `{layer}`, which requires layer `{required}`; add `{required}` to `layers`"
    )]
    MissingRequirement { layer: String, required: String },
    #[error("lists layers `{layer}` and `{conflict}`, which conflict")]
    Conflict { layer: String, conflict: String },
}

pub(crate) fn catalog() -> &'static Catalog {
    &CATALOG
}

pub(crate) fn select(
    catalog: &Catalog,
    language: Language,
    listed: Vec<String>,
) -> Result<BTreeSet<String>, SelectionError> {
    let mut layers = BTreeSet::new();

    for layer in listed {
        let Some(entry) = catalog.layers.get(layer.as_str()) else {
            return Err(SelectionError::UnknownLayer { layer });
        };

        if layers.contains(&layer) {
            return Err(SelectionError::DuplicateLayer { layer });
        }

        if entry.manifest.language != language {
            return Err(SelectionError::WrongLanguage {
                layer,
                layer_language: entry.manifest.language,
                language,
            });
        }

        layers.insert(layer);
    }

    for layer in &layers {
        let manifest = &catalog.layers[layer.as_str()].manifest;

        if let Some(required) = manifest
            .requires
            .iter()
            .find(|name| !layers.contains(*name))
        {
            return Err(SelectionError::MissingRequirement {
                layer: layer.clone(),
                required: required.clone(),
            });
        }

        if let Some(conflict) = manifest
            .conflicts
            .iter()
            .find(|name| layers.contains(*name))
        {
            return Err(SelectionError::Conflict {
                layer: layer.clone(),
                conflict: conflict.clone(),
            });
        }
    }

    Ok(layers)
}

impl Catalog {
    pub(crate) fn rust_lints(&self) -> &LintPolicy {
        &self.rust_lints
    }

    pub(crate) fn required_bans(&self, layers: &BTreeSet<String>) -> Vec<(Rule, &RequiredBan)> {
        let base = self
            .rust_bans
            .iter()
            .map(|ban| (Rule::RustRequiredBan, ban));
        let selected = layers
            .iter()
            .filter_map(|layer| self.layer_bans.get(layer.as_str()))
            .flat_map(|(rule, bans)| bans.iter().map(|ban| (*rule, ban)));

        base.chain(selected).collect()
    }

    fn new(
        sources: &[Source],
        rust: &RustSource,
        rules: &[(Owner, &'static str)],
    ) -> Result<Self, CatalogError> {
        let mut layers = BTreeMap::new();
        let mut layer_bans = BTreeMap::new();

        for source in sources {
            let manifest = toml::from_str(source.manifest).map_err(|error: toml::de::Error| {
                CatalogError::InvalidManifest {
                    layer: source.name,
                    reason: error.message().to_string(),
                }
            })?;

            layers.insert(
                source.name,
                Layer {
                    manifest,
                    guide: source.guide,
                    refinements: source.refinements,
                    detection: source.detection,
                },
            );

            if let Some(bans) = &source.bans {
                let required = policy::parse_required_bans(bans.text).map_err(|reason| {
                    CatalogError::InvalidLayerBans {
                        layer: source.name,
                        reason,
                    }
                })?;

                layer_bans.insert(source.name, (bans.rule, required));
            }
        }

        let (rust_lints, rust_bans) = parse_rust_policy(rust)?;
        let catalog = Self {
            layers,
            layer_bans,
            rust_lints,
            rust_bans,
        };

        catalog.check_names()?;

        if let Some(cycle) = catalog.find_cycle() {
            return Err(CatalogError::RequirementCycle { layers: cycle });
        }

        check_rule_ids(rules)?;

        Ok(catalog)
    }

    pub(crate) fn guide(&self, name: &str) -> Option<&'static str> {
        self.layers.get(name).map(|layer| layer.guide)
    }

    pub(crate) fn refinements(&self, layers: &BTreeSet<String>) -> BTreeSet<Refinement> {
        layers
            .iter()
            .filter_map(|layer| self.layers.get(layer.as_str()))
            .flat_map(|layer| layer.refinements.iter().copied())
            .collect()
    }

    pub(crate) fn detections(
        &self,
        language: Language,
    ) -> impl Iterator<Item = (&'static str, Detection)> + '_ {
        self.layers
            .iter()
            .filter(move |(_, layer)| layer.manifest.language == language)
            .map(|(name, layer)| (*name, layer.detection))
    }

    pub(crate) fn guides(&self) -> impl Iterator<Item = (&'static str, &'static str)> + '_ {
        self.layers.iter().map(|(name, layer)| (*name, layer.guide))
    }

    fn check_names(&self) -> Result<(), CatalogError> {
        for (layer, entry) in &self.layers {
            let names = entry
                .manifest
                .requires
                .iter()
                .chain(&entry.manifest.conflicts);

            for name in names {
                if !self.layers.contains_key(name.as_str()) {
                    return Err(CatalogError::UnknownLayer {
                        layer,
                        unknown: name.clone(),
                    });
                }
            }
        }

        Ok(())
    }

    fn find_cycle(&self) -> Option<Vec<String>> {
        let mut finished = BTreeSet::new();

        self.layers
            .keys()
            .find_map(|layer| self.cycle_from(layer, &mut Vec::new(), &mut finished))
    }

    fn cycle_from<'a>(
        &'a self,
        layer: &'a str,
        path: &mut Vec<&'a str>,
        finished: &mut BTreeSet<&'a str>,
    ) -> Option<Vec<String>> {
        if let Some(start) = path.iter().position(|step| *step == layer) {
            let mut cycle: Vec<String> = path[start..].iter().map(ToString::to_string).collect();

            cycle.push(layer.to_string());

            return Some(cycle);
        }

        if finished.contains(layer) {
            return None;
        }

        path.push(layer);

        for required in &self.layers[layer].manifest.requires {
            if let Some(cycle) = self.cycle_from(required, path, finished) {
                return Some(cycle);
            }
        }

        path.pop();
        finished.insert(layer);

        None
    }
}

fn parse_rust_policy(rust: &RustSource) -> Result<(LintPolicy, Vec<RequiredBan>), CatalogError> {
    let invalid = |file: PolicyFile| {
        move |reason| CatalogError::InvalidRustPolicy {
            file: file.path,
            reason,
        }
    };
    let lints = policy::parse_lint_policy(rust.lints.text).map_err(invalid(rust.lints))?;
    let bans = policy::parse_required_bans(rust.clippy.text).map_err(invalid(rust.clippy))?;

    Ok((lints, bans))
}

fn check_rule_ids(rules: &[(Owner, &'static str)]) -> Result<(), CatalogError> {
    let mut owners: BTreeMap<&str, Owner> = BTreeMap::new();

    for &(owner, id) in rules {
        if let Some(&first) = owners.get(id) {
            return Err(CatalogError::DuplicateRuleId {
                id,
                first,
                second: owner,
            });
        }

        owners.insert(id, owner);
    }

    Ok(())
}

fn rule_owners() -> Vec<(Owner, &'static str)> {
    Rule::ALL
        .iter()
        .map(|rule| (rule.metadata().owner, rule.id()))
        .collect()
}

#[cfg(test)]
pub(crate) fn test_catalog(manifests: &[(&'static str, &'static str)]) -> Catalog {
    let sources: Vec<Source> = manifests
        .iter()
        .map(|&(name, manifest)| Source {
            name,
            manifest,
            bans: None,
            guide: "",
            refinements: &[],
            detection: Detection::PackageField(name),
        })
        .collect();

    Catalog::new(&sources, &RUST, &[]).expect("a test catalog is valid")
}

#[cfg(test)]
pub(crate) fn test_policy_catalog(
    rust: &RustSource,
    layer_bans: &[(&str, &'static str)],
) -> Catalog {
    let override_for = |layer: &str| {
        layer_bans
            .iter()
            .find(|(name, _)| *name == layer)
            .map(|&(_, text)| text)
    };
    let sources: Vec<Source> = REGISTRY
        .iter()
        .map(|source| Source {
            name: source.name,
            manifest: source.manifest,
            guide: source.guide,
            bans: source.bans.as_ref().map(|bans| LayerBans {
                text: override_for(source.name).unwrap_or(bans.text),
                rule: bans.rule,
            }),
            refinements: source.refinements,
            detection: source.detection,
        })
        .collect();

    Catalog::new(&sources, rust, &[]).expect("a test policy catalog is valid")
}

#[cfg(test)]
mod tests;
