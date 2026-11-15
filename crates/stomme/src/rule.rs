use std::fmt;

use crate::layer::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Common,
    Language(Language),
    Layer(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Status {
    Enforced,
    #[expect(dead_code, reason = "no rule is partly enforced yet")]
    Partial,
    GuidanceOnly,
    ProjectLocal,
}

#[derive(Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the rules table test reads the descriptive fields"
    )
)]
pub(crate) struct Metadata {
    pub(crate) id: &'static str,
    pub(crate) summary: &'static str,
    pub(crate) owner: Owner,
    pub(crate) applies: &'static str,
    pub(crate) fields: &'static [&'static str],
    pub(crate) checker: &'static str,
    pub(crate) fixtures: Option<&'static str>,
    pub(crate) status: Status,
}

const PROJECT_LOCAL: &str = "project-local";

#[cfg(test)]
const TABLE_HEADER: &str = "\
# Rules

The stomme binary generates this file from its rule catalog. Do not edit it. Checker and fixture
paths are relative to the stomme crate.

| ID | Rule | Owner | Applies | `stomme.toml` fields | Checker | Fixtures | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
";

// One table generates the enum, `Rule::ALL`, and `Rule::metadata`, so a new rule cannot be left out
// of `ALL`, which the layer catalog uses to find two owners of one rule ID.
macro_rules! rules {
    ($($variant:ident => $metadata:expr,)*) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub(crate) enum Rule {
            $($variant,)*
        }

        impl Rule {
            pub(crate) const ALL: &[Self] = &[$(Self::$variant,)*];

            pub(crate) fn metadata(self) -> &'static Metadata {
                match self {
                    $(Self::$variant => {
                        const METADATA: Metadata = $metadata;

                        &METADATA
                    })*
                }
            }
        }
    };
}

rules! {
    NoStomme => Metadata {
        id: "no-stomme",
        summary: "The project has a `stomme.toml`.",
        owner: Owner::Common,
        applies: "always",
        fields: &[],
        checker: "src/check.rs",
        fixtures: Some("tests/fixtures/rules/no-stomme"),
        status: Status::Enforced,
    },
    BrokenInstallation => Metadata {
        id: "broken-installation",
        summary: "`stomme.toml` is valid TOML that matches the schema and selects a valid layer set.",
        owner: Owner::Common,
        applies: "`stomme.toml` exists",
        fields: &["schema", "language", "layers", "roles"],
        checker: "src/check.rs",
        fixtures: Some("tests/fixtures/rules/broken-installation"),
        status: Status::Enforced,
    },
    RequiredPart => Metadata {
        id: "required-part",
        summary: "Every required file exists.",
        owner: Owner::Common,
        applies: "always",
        fields: &[],
        checker: "src/check/required_parts.rs",
        fixtures: Some("tests/fixtures/rules/required-part"),
        status: Status::Enforced,
    },
    TauFullCheck => Metadata {
        id: "tau-full-check",
        summary: "`.pi/tau.json` names the full check as a non-empty `fullCheck.argv` of strings.",
        owner: Owner::Common,
        applies: "`.pi/tau.json` exists",
        fields: &[],
        checker: "src/check/required_parts.rs",
        fixtures: Some("tests/fixtures/rules/tau-full-check"),
        status: Status::Enforced,
    },
    AdrDuplicateNumber => Metadata {
        id: "adr-duplicate-number",
        summary: "No two ADR files share a number.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-duplicate-number"),
        status: Status::Enforced,
    },
    AdrHeading => Metadata {
        id: "adr-heading",
        summary: "An ADR's first heading is `# ADR NNNN: Title` with its own number.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-heading"),
        status: Status::Enforced,
    },
    AdrRetiredNumber => Metadata {
        id: "adr-retired-number",
        summary: "No ADR file uses a number from the Retired table.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-retired-number"),
        status: Status::Enforced,
    },
    AdrIndexCount => Metadata {
        id: "adr-index-count",
        summary: "The ADR index lists each ADR file once.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-index-count"),
        status: Status::Enforced,
    },
    AdrIndexLabel => Metadata {
        id: "adr-index-label",
        summary: "An ADR index entry's label is the ADR's number and title.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-index-label"),
        status: Status::Enforced,
    },
    AdrIndexMissingFile => Metadata {
        id: "adr-index-missing-file",
        summary: "Every file the ADR index lists exists.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-index-missing-file"),
        status: Status::Enforced,
    },
    AdrIndexEntry => Metadata {
        id: "adr-index-entry",
        summary: "Every link line in the ADR index is `- [NNNN: Title](./NNNN-slug.md)`.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        checker: "src/check/adr_index.rs",
        fixtures: Some("tests/fixtures/rules/adr-index-entry"),
        status: Status::Enforced,
    },
    RustConfigFile => Metadata {
        id: "rust-config-file",
        summary: "The root `Cargo.toml` exists, and every Cargo manifest and `clippy.toml` the checks read is valid.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        checker: "src/check/cargo_manifest.rs",
        fixtures: Some("tests/fixtures/rules/rust-config-file"),
        status: Status::Enforced,
    },
    RustLintLevel => Metadata {
        id: "rust-lint-level",
        summary: "`[workspace.lints]` keeps every shared lint at its shared priority and at least its shared level.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        checker: "src/check/rust_lints.rs",
        fixtures: Some("tests/fixtures/rules/rust-lint-level"),
        status: Status::Enforced,
    },
    RustLintInheritance => Metadata {
        id: "rust-lint-inheritance",
        summary: "Every workspace member has `[lints] workspace = true`.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        checker: "src/check/rust_members.rs",
        fixtures: Some("tests/fixtures/rules/rust-lint-inheritance"),
        status: Status::Enforced,
    },
    RustRequiredBan => Metadata {
        id: "rust-required-ban",
        summary: "`clippy.toml` holds each shared Rust `disallowed-methods` ban with its reason.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        checker: "none",
        fixtures: None,
        status: Status::GuidanceOnly,
    },
    GpuiRequiredBan => Metadata {
        id: "gpui-required-ban",
        summary: "`clippy.toml` holds each GPUI `disallowed-methods` ban with its reason.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        checker: "none",
        fixtures: None,
        status: Status::GuidanceOnly,
    },
    ModuleImports => Metadata {
        id: "module-imports",
        summary: "Imports follow the module table: a module imports only the modules its row allows.",
        owner: Owner::Common,
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ModuleListed => Metadata {
        id: "module-listed",
        summary: "Every source file belongs to a module in the module table.",
        owner: Owner::Common,
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ModuleTableClosed => Metadata {
        id: "module-table-closed",
        summary: "Every module the module table allows as an import has its own row.",
        owner: Owner::Common,
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ModuleTableOrder => Metadata {
        id: "module-table-order",
        summary: "Module registries and their lists are sorted and free of duplicates.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ModuleTableDocumented => Metadata {
        id: "module-table-documented",
        summary: "The import table in the module ADR equals the enforced module table.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ModulePrivateFile => Metadata {
        id: "module-private-file",
        summary: "Other modules import a folder module through its entry or public files only.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    CompositionRootImport => Metadata {
        id: "composition-root-import",
        summary: "No module imports the composition root.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a composition root",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ExtensionIsolation => Metadata {
        id: "extension-isolation",
        summary: "An extension does not import another extension.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has extensions",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    ExtensionDirection => Metadata {
        id: "extension-direction",
        summary: "A shared module does not import an extension.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has extensions",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    LiteralImport => Metadata {
        id: "literal-import",
        summary: "A dynamic import names a literal path, not a computed one.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    CrateRootPath => Metadata {
        id: "crate-root-path",
        summary: "A module uses other modules through their module paths, not items at the crate root.",
        owner: Owner::Language(Language::Rust),
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    PureDependencies => Metadata {
        id: "pure-dependencies",
        summary: "A module uses only pure modules; no module uses an I/O module.",
        owner: Owner::Language(Language::Rust),
        applies: "the project has a module table",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    PureModuleEffects => Metadata {
        id: "pure-module-effects",
        summary: "A registered pure module performs no reads or effects and imports only types and other pure modules.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project registers pure modules",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    UiDependency => Metadata {
        id: "ui-dependency",
        summary: "Core and CLI packages do not depend on UI packages, directly or through other packages.",
        owner: Owner::Language(Language::Rust),
        applies: "the workspace has UI packages",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    NoIndexFile => Metadata {
        id: "no-index-file",
        summary: "Source folders hold no index files.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    OneFileFolder => Metadata {
        id: "one-file-folder",
        summary: "A module with one production source file is a flat file, not a folder.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    FolderModuleEntry => Metadata {
        id: "folder-module-entry",
        summary: "A folder module has an entry file named after its folder.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    TestFileName => Metadata {
        id: "test-file-name",
        summary: "A source test file is named after a module beside it.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    FixtureImport => Metadata {
        id: "fixture-import",
        summary: "Production code does not load fixtures.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    VersionedRecord => Metadata {
        id: "versioned-record",
        summary: "A versioned record format has a version literal and current, newer, and malformed fixtures.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project writes versioned records",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    TestProject => Metadata {
        id: "test-project",
        summary: "Every test file belongs to exactly one test project.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project splits tests into projects",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
    TestEnvironment => Metadata {
        id: "test-environment",
        summary: "The cached test tasks declare every environment variable that test code reads.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project caches test tasks",
        fields: &[],
        checker: PROJECT_LOCAL,
        fixtures: None,
        status: Status::ProjectLocal,
    },
}

impl Rule {
    pub(crate) fn id(self) -> &'static str {
        self.metadata().id
    }
}

#[cfg(test)]
impl Status {
    fn label(self) -> &'static str {
        match self {
            Self::Enforced => "enforced",
            Self::Partial => "partial",
            Self::GuidanceOnly => "guidance only",
            Self::ProjectLocal => PROJECT_LOCAL,
        }
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

#[cfg(test)]
pub(crate) fn markdown_table() -> String {
    let mut table = TABLE_HEADER.to_string();

    for rule in Rule::ALL {
        table.push_str(&table_row(rule.metadata()));
    }

    table
}

#[cfg(test)]
fn table_row(metadata: &Metadata) -> String {
    let owner = match metadata.owner {
        Owner::Common => "common".to_string(),
        Owner::Language(language) => language.to_string(),
        Owner::Layer(layer) => format!("layer `{layer}`"),
    };
    let fields = if metadata.fields.is_empty() {
        "none".to_string()
    } else {
        let quoted: Vec<String> = metadata
            .fields
            .iter()
            .map(|field| format!("`{field}`"))
            .collect();

        quoted.join(", ")
    };
    let fixtures = metadata.fixtures.unwrap_or("none");

    format!(
        "| `{}` | {} | {owner} | {} | {fields} | {} | {fixtures} | {} |\n",
        metadata.id,
        metadata.summary,
        metadata.applies,
        metadata.checker,
        metadata.status.label()
    )
}

#[cfg(test)]
mod tests;
