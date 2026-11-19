use std::fs;
use std::path::Path;

use super::*;

fn layers(names: &[&str]) -> Vec<String> {
    names.iter().map(ToString::to_string).collect()
}

fn source(name: &'static str, manifest: &'static str) -> Source {
    Source {
        name,
        manifest,
        bans: None,
        guide: "",
        refinements: &[],
        detection: Detection::PackageField(name),
        starter: None,
    }
}

#[test]
fn language_parse_reads_the_displayed_name() {
    for language in [Language::Rust, Language::TypeScript] {
        assert_eq!(Language::parse(&language.to_string()), Some(language));
    }

    assert_eq!(Language::parse("Rust"), None);
}

#[test]
fn select_reports_each_bad_layer_set() {
    let cases = [
        (
            "unknown layer",
            Language::Rust,
            &["egui"][..],
            SelectionError::UnknownLayer {
                layer: "egui".to_string(),
            },
        ),
        (
            "duplicate layer",
            Language::Rust,
            &["gpui", "gpui"][..],
            SelectionError::DuplicateLayer {
                layer: "gpui".to_string(),
            },
        ),
        (
            "wrong language",
            Language::TypeScript,
            &["gpui"][..],
            SelectionError::WrongLanguage {
                layer: "gpui".to_string(),
                layer_language: Language::Rust,
                language: Language::TypeScript,
            },
        ),
        (
            "missing required layer",
            Language::Rust,
            &["gpui-component"][..],
            SelectionError::MissingRequirement {
                layer: "gpui-component".to_string(),
                required: "gpui".to_string(),
            },
        ),
    ];

    for (case, language, listed, expected) in cases {
        assert_eq!(
            select(catalog(), language, layers(listed)),
            Err(expected),
            "{case}"
        );
    }
}

#[test]
fn select_reports_a_conflicting_pair_in_either_order() {
    let sources = [
        source("one", "language = \"rust\"\nconflicts = [\"two\"]\n"),
        source("two", "language = \"rust\"\n"),
    ];
    let catalog = Catalog::new(&sources, &RUST, &[]).expect("the test catalog is valid");

    for listed in [["one", "two"], ["two", "one"]] {
        assert_eq!(
            select(&catalog, Language::Rust, layers(&listed)),
            Err(SelectionError::Conflict {
                layer: "one".to_string(),
                conflict: "two".to_string(),
            }),
            "{listed:?}"
        );
    }
}

#[test]
fn select_gives_the_same_set_for_layers_in_either_order() {
    let forward = select(
        catalog(),
        Language::Rust,
        layers(&["gpui", "gpui-component"]),
    );
    let backward = select(
        catalog(),
        Language::Rust,
        layers(&["gpui-component", "gpui"]),
    );

    assert_eq!(
        forward,
        Ok(BTreeSet::from([
            "gpui".to_string(),
            "gpui-component".to_string()
        ]))
    );
    assert_eq!(forward, backward);
}

#[test]
fn catalog_accepts_the_compiled_in_layers_and_rules() {
    assert!(Catalog::new(REGISTRY, &RUST, &rule_owners()).is_ok());
}

#[test]
fn catalog_reports_a_requirement_cycle() {
    let sources = [
        source("one", "language = \"rust\"\nrequires = [\"two\"]\n"),
        source("two", "language = \"rust\"\nrequires = [\"one\"]\n"),
    ];

    assert_eq!(
        Catalog::new(&sources, &RUST, &[]).map(|_| ()),
        Err(CatalogError::RequirementCycle {
            layers: layers(&["one", "two", "one"]),
        })
    );
}

#[test]
fn catalog_reports_a_manifest_naming_an_unknown_layer() {
    let cases = [
        ("requires", "language = \"rust\"\nrequires = [\"egui\"]\n"),
        ("conflicts", "language = \"rust\"\nconflicts = [\"egui\"]\n"),
    ];

    for (case, manifest) in cases {
        assert_eq!(
            Catalog::new(&[source("one", manifest)], &RUST, &[]).map(|_| ()),
            Err(CatalogError::UnknownLayer {
                layer: "one",
                unknown: "egui".to_string(),
            }),
            "{case}"
        );
    }
}

#[test]
fn catalog_reports_an_invalid_manifest() {
    let sources = [source("one", "language = \"go\"\n")];

    assert!(matches!(
        Catalog::new(&sources, &RUST, &[]),
        Err(CatalogError::InvalidManifest { layer: "one", .. })
    ));
}

#[test]
fn catalog_reports_two_rules_with_one_id_naming_both_owners() {
    let sources = [source("one", "language = \"rust\"\n")];
    let rules = [
        (Owner::Common, "required-part"),
        (Owner::Language(Language::Rust), "rust-edition"),
        (Owner::Layer("one"), "required-part"),
    ];

    assert_eq!(
        Catalog::new(&sources, &RUST, &rules).map(|_| ()),
        Err(CatalogError::DuplicateRuleId {
            id: "required-part",
            first: Owner::Common,
            second: Owner::Layer("one"),
        })
    );
}

#[test]
fn duplicate_rule_id_message_names_both_owners() {
    let cases = [
        (
            Owner::Common,
            Owner::Layer("gpui"),
            "rule `required-part` belongs to the common rules and to layer `gpui`",
        ),
        (
            Owner::Language(Language::TypeScript),
            Owner::Layer("react"),
            "rule `required-part` belongs to the typescript language and to layer `react`",
        ),
    ];

    for (first, second, expected) in cases {
        let error = CatalogError::DuplicateRuleId {
            id: "required-part",
            first,
            second,
        };

        assert_eq!(error.to_string(), expected);
    }
}

#[test]
fn catalog_check_sees_the_compiled_in_rules() {
    let mut rules = rule_owners();

    rules.push((Owner::Layer("gpui"), "required-part"));

    assert_eq!(
        Catalog::new(REGISTRY, &RUST, &rules).map(|_| ()),
        Err(CatalogError::DuplicateRuleId {
            id: "required-part",
            first: Owner::Common,
            second: Owner::Layer("gpui"),
        })
    );
}

#[test]
fn registry_matches_the_layer_folders() {
    let folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../foundation/layers");
    let folders: BTreeSet<String> = fs::read_dir(&folder)
        .expect("read the layers folder")
        .map(|entry| {
            let entry = entry.expect("read a layers folder entry");

            entry.file_name().to_string_lossy().into_owned()
        })
        .collect();
    let registered: BTreeSet<String> = REGISTRY
        .iter()
        .map(|source| source.name.to_string())
        .collect();

    assert_eq!(
        folders.difference(&registered).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "folders under foundation/layers/ without a registered module"
    );
    assert_eq!(
        registered.difference(&folders).collect::<Vec<_>>(),
        Vec::<&String>::new(),
        "registered modules without a folder under foundation/layers/"
    );
}

#[test]
fn catalog_reports_an_invalid_rust_policy_file() {
    let cases = [
        (
            "lints",
            RustSource {
                lints: PolicyFile {
                    text: "[workspace.lints.clippy]\ntodo = \"loud\"\n",
                    ..RUST.lints
                },
                clippy: RUST.clippy,
            },
            "foundation/languages/rust/lints.toml",
        ),
        (
            "bans",
            RustSource {
                lints: RUST.lints,
                clippy: PolicyFile {
                    text: "disallowed-methods = [\"std::process::exit\"]\n",
                    ..RUST.clippy
                },
            },
            "foundation/languages/rust/clippy.toml",
        ),
    ];

    for (case, rust, expected) in cases {
        assert!(
            matches!(
                Catalog::new(REGISTRY, &rust, &[]),
                Err(CatalogError::InvalidRustPolicy { file, .. }) if file == expected
            ),
            "{case}"
        );
    }
}

#[test]
fn catalog_reports_invalid_layer_bans() {
    let sources = [Source {
        name: "one",
        manifest: "language = \"rust\"\n",
        guide: "",
        refinements: &[],
        detection: Detection::PackageField("one"),
        starter: None,
        bans: Some(LayerBans {
            text: "disallowed-methods = [{ path = \"gpui::App::run\" }]\n",
            rule: Rule::GpuiRequiredBan,
        }),
    }];

    assert!(matches!(
        Catalog::new(&sources, &RUST, &[]),
        Err(CatalogError::InvalidLayerBans { layer: "one", .. })
    ));
}

#[test]
fn smallest_selection_follows_requirements_through_a_second_hop() {
    let catalog = test_catalog(&[
        ("one", "language = \"rust\"\nrequires = [\"two\"]\n"),
        ("two", "language = \"rust\"\nrequires = [\"three\"]\n"),
        ("three", "language = \"rust\"\n"),
        ("other", "language = \"rust\"\n"),
    ]);

    assert_eq!(
        catalog.smallest_selection("one"),
        Some((
            Language::Rust,
            BTreeSet::from(["one".to_string(), "two".to_string(), "three".to_string()])
        ))
    );
}
