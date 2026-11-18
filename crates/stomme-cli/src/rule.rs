use std::fmt;
#[cfg(test)]
use std::fs;
#[cfg(test)]
use std::io;
#[cfg(test)]
use std::path::Path;

use crate::layer::Language;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Owner {
    Common,
    Language(Language),
    Layer(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Enforcement {
    Enforced(Check),
    Partial {
        check: Check,
        unenforced: &'static str,
    },
    TypeScriptImports(Check),
    RustAndTypeScript {
        rust: Check,
        typescript: Check,
    },
    GuidanceOnly,
    ProjectLocal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Check {
    pub(crate) checker: &'static str,
    pub(crate) fixtures: &'static str,
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
    pub(crate) enforcement: Enforcement,
}

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
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check.rs",
            fixtures: "tests/fixtures/rules/no-stomme",
        }),
    },
    BrokenInstallation => Metadata {
        id: "broken-installation",
        summary: "`stomme.toml` is a regular file of valid TOML that matches the schema, selects a valid layer set, and has a closed module table.",
        owner: Owner::Common,
        applies: "`stomme.toml` exists",
        fields: &["schema", "version", "language", "layers", "roles", "violations", "modules"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check.rs",
            fixtures: "tests/fixtures/rules/broken-installation",
        }),
    },
    RoleName => Metadata {
        id: "role-name",
        summary: "Every role name in `[roles]` is one of the shared roles.",
        owner: Owner::Common,
        applies: "`stomme.toml` is valid",
        fields: &["roles"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/roles.rs",
            fixtures: "tests/fixtures/rules/role-name",
        }),
    },
    RolePath => Metadata {
        id: "role-path",
        summary: "Every role path is a relative folder inside the project.",
        owner: Owner::Common,
        applies: "`stomme.toml` is valid",
        fields: &["roles"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/roles.rs",
            fixtures: "tests/fixtures/rules/role-path",
        }),
    },
    RoleFolder => Metadata {
        id: "role-folder",
        summary: "A role folder is named after the role or ends with `-<role>`.",
        owner: Owner::Common,
        applies: "`stomme.toml` is valid",
        fields: &["roles"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/roles.rs",
            fixtures: "tests/fixtures/rules/role-folder",
        }),
    },
    ViolationStale => Metadata {
        id: "violation-stale",
        summary: "Every entry in `[[violations]]` matches a reported problem.",
        owner: Owner::Common,
        applies: "`stomme.toml` is valid",
        fields: &["violations"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/violations.rs",
            fixtures: "tests/fixtures/rules/violation-stale",
        }),
    },
    ViolationGrowth => Metadata {
        id: "violation-growth",
        summary: "`[[violations]]` does not grow against the target branch.",
        owner: Owner::Common,
        applies: "`stomme.toml` is valid and has entries",
        fields: &["violations"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/violations.rs",
            fixtures: "tests/fixtures/rules/violation-growth",
        }),
    },
    RequiredPart => Metadata {
        id: "required-part",
        summary: "Every required file exists.",
        owner: Owner::Common,
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/required_parts.rs",
            fixtures: "tests/fixtures/rules/required-part",
        }),
    },
    SyncedFiles => Metadata {
        id: "synced-files",
        summary: "`docs/stomme/` equals the owned set, and its record matches the files and the version.",
        owner: Owner::Common,
        applies: "`stomme.toml` is valid",
        fields: &["version", "language", "layers"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/synced_files.rs",
            fixtures: "tests/fixtures/rules/synced-files",
        }),
    },
    AgentsBlock => Metadata {
        id: "agents-block",
        summary: "`AGENTS.md` holds the managed block between its markers, and its record matches the block.",
        owner: Owner::Common,
        applies: "`AGENTS.md` exists",
        fields: &["version", "language", "layers"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/synced_files.rs",
            fixtures: "tests/fixtures/rules/agents-block",
        }),
    },
    PackagePin => Metadata {
        id: "package-pin",
        summary: "A shared package in the root `package.json` matches the pinned version.",
        owner: Owner::Common,
        applies: "the root `package.json` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/package_pins.rs",
            fixtures: "tests/fixtures/rules/package-pin",
        }),
    },
    LayerDeclared => Metadata {
        id: "layer-declared",
        summary: "A project lists the layer of each framework it uses, so removing a layer cannot switch its rules off.",
        owner: Owner::Common,
        applies: "the project uses a framework that has a layer",
        fields: &["layers"],
        enforcement: Enforcement::Partial {
            check: Check {
                checker: "src/check/layer_use.rs",
                fixtures: "tests/fixtures/rules/layer-declared",
            },
            unenforced: "framework use without a `gpui` or `gpui-component` package in `Cargo.lock`, a `react` dependency or `pi` field in the root `package.json`, or a Herdr `mod.json` beside `main.ts`",
        },
    },
    TauFullCheck => Metadata {
        id: "tau-full-check",
        summary: "`.pi/tau.json` names the full check as a non-empty `fullCheck.argv` of strings.",
        owner: Owner::Common,
        applies: "`.pi/tau.json` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/required_parts.rs",
            fixtures: "tests/fixtures/rules/tau-full-check",
        }),
    },
    AdrFileName => Metadata {
        id: "adr-file-name",
        summary: "An ADR file is named `NNNN-slug.md` with a non-empty slug.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-file-name",
        }),
    },
    AdrDuplicateNumber => Metadata {
        id: "adr-duplicate-number",
        summary: "No two ADR files share a number.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-duplicate-number",
        }),
    },
    AdrHeading => Metadata {
        id: "adr-heading",
        summary: "An ADR's first heading is `# ADR NNNN: Title` with its own number, indented by at most 3 spaces.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-heading",
        }),
    },
    AdrRetiredNumber => Metadata {
        id: "adr-retired-number",
        summary: "No ADR file uses a number from the Retired table.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-retired-number",
        }),
    },
    AdrIndexCount => Metadata {
        id: "adr-index-count",
        summary: "The ADR index lists each ADR file once.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-index-count",
        }),
    },
    AdrIndexLabel => Metadata {
        id: "adr-index-label",
        summary: "An ADR index entry's label is the ADR's number and title.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-index-label",
        }),
    },
    AdrIndexMissingFile => Metadata {
        id: "adr-index-missing-file",
        summary: "Every file the ADR index lists exists.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-index-missing-file",
        }),
    },
    AdrIndexEntry => Metadata {
        id: "adr-index-entry",
        summary: "Every link line in the ADR index is a `-`, `*`, or `+` list entry `[NNNN: Title](./NNNN-slug.md)`.",
        owner: Owner::Common,
        applies: "`docs/adr/README.md` exists",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/adr_index.rs",
            fixtures: "tests/fixtures/rules/adr-index-entry",
        }),
    },
    RustConfigFile => Metadata {
        id: "rust-config-file",
        summary: "The root `Cargo.toml` exists, and every Cargo manifest and `clippy.toml` the checks read is valid.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/cargo_manifest.rs",
            fixtures: "tests/fixtures/rules/rust-config-file",
        }),
    },
    RustLintLevel => Metadata {
        id: "rust-lint-level",
        summary: "`[workspace.lints]` keeps every shared lint at its shared priority and at least its shared level.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/rust_lints.rs",
            fixtures: "tests/fixtures/rules/rust-lint-level",
        }),
    },
    RustLintInheritance => Metadata {
        id: "rust-lint-inheritance",
        summary: "Every workspace member has `[lints] workspace = true`.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/rust_members.rs",
            fixtures: "tests/fixtures/rules/rust-lint-inheritance",
        }),
    },
    RustRequiredBan => Metadata {
        id: "rust-required-ban",
        summary: "`clippy.toml` holds each shared Rust `disallowed-methods` ban with its reason.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    RustFileCase => Metadata {
        id: "rust-file-case",
        summary: "A Rust file name is snake_case.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/rust.rs",
            fixtures: "tests/fixtures/rules/rust-file-case",
        }),
    },
    RustCatchAllName => Metadata {
        id: "rust-catch-all-name",
        summary: "A Rust file or folder is not named after a catch-all, and no file is `mod.rs`.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/rust.rs",
            fixtures: "tests/fixtures/rules/rust-catch-all-name",
        }),
    },
    RustOwnerFolder => Metadata {
        id: "rust-owner-folder",
        summary: "A file that shares an owner's noun sits in the owner's folder.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/rust.rs",
            fixtures: "tests/fixtures/rules/rust-owner-folder",
        }),
    },
    RustTestPlacement => Metadata {
        id: "rust-test-placement",
        summary: "A Rust unit test lives in a `#[cfg(test)] mod tests` in the owning file, not below `src`.",
        owner: Owner::Language(Language::Rust),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/rust.rs",
            fixtures: "tests/fixtures/rules/rust-test-placement",
        }),
    },
    GpuiRequiredBan => Metadata {
        id: "gpui-required-ban",
        summary: "`clippy.toml` holds each GPUI `disallowed-methods` ban with its reason.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    ModuleImports => Metadata {
        id: "module-imports",
        summary: "Imports follow the module table: a module imports only the modules its row allows.",
        owner: Owner::Common,
        applies: "the project has a module table",
        fields: &["modules"],
        enforcement: Enforcement::RustAndTypeScript {
            rust: Check {
                checker: "src/check/modules.rs",
                fixtures: "tests/fixtures/rules/module-imports",
            },
            typescript: Check {
                checker: "../../packages/imports/src/rules/moduleBoundaries.ts",
                fixtures: "../../packages/imports/src/rules/moduleBoundaries.test.ts",
            },
        },
    },
    ModuleListed => Metadata {
        id: "module-listed",
        summary: "Every source file belongs to a module in the module table.",
        owner: Owner::Common,
        applies: "the project has a module table",
        fields: &["modules"],
        enforcement: Enforcement::RustAndTypeScript {
            rust: Check {
                checker: "src/check/modules.rs",
                fixtures: "tests/fixtures/rules/module-listed",
            },
            typescript: Check {
                checker: "../../packages/imports/src/rules/moduleBoundaries.ts",
                fixtures: "../../packages/imports/src/rules/moduleBoundaries.test.ts",
            },
        },
    },
    ModuleTableOrder => Metadata {
        id: "module-table-order",
        summary: "Module registries and their lists are sorted and free of duplicates.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    ModuleTableDocumented => Metadata {
        id: "module-table-documented",
        summary: "The import table in the module ADR equals the enforced module table.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    ModulePrivateFile => Metadata {
        id: "module-private-file",
        summary: "Other modules use a folder module through its entry or public files only.",
        owner: Owner::Common,
        applies: "the project has a module table",
        fields: &["modules"],
        enforcement: Enforcement::RustAndTypeScript {
            rust: Check {
                checker: "src/check/modules.rs",
                fixtures: "tests/fixtures/rules/module-private-file",
            },
            typescript: Check {
                checker: "../../packages/imports/src/rules/moduleBoundaries.ts",
                fixtures: "../../packages/imports/src/rules/moduleBoundaries.test.ts",
            },
        },
    },
    CompositionRootImport => Metadata {
        id: "composition-root-import",
        summary: "No module imports the composition root.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a composition root",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    ExtensionIsolation => Metadata {
        id: "extension-isolation",
        summary: "An extension does not import another extension.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has extensions",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    ExtensionDirection => Metadata {
        id: "extension-direction",
        summary: "A shared module does not import an extension.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has extensions",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    LiteralImport => Metadata {
        id: "literal-import",
        summary: "A dynamic import names a literal path, not a computed one.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project has a module table",
        fields: &["modules"],
        enforcement: Enforcement::TypeScriptImports(Check {
            checker: "../../packages/imports/src/rules/moduleBoundaries.ts",
            fixtures: "../../packages/imports/src/rules/moduleBoundaries.test.ts",
        }),
    },
    CrateRootPath => Metadata {
        id: "crate-root-path",
        summary: "A module uses other modules through their module paths, not items at the crate root.",
        owner: Owner::Language(Language::Rust),
        applies: "the project has a module table",
        fields: &["modules"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/modules.rs",
            fixtures: "tests/fixtures/rules/crate-root-path",
        }),
    },
    PureDependencies => Metadata {
        id: "pure-dependencies",
        summary: "A module uses only pure modules; no module uses an I/O module.",
        owner: Owner::Language(Language::Rust),
        applies: "the project has a module table",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    PureStdIo => Metadata {
        id: "pure-std-io",
        summary: "A pure module reaches no I/O through `std`, such as `std::fs` or `std::time::SystemTime::now`.",
        owner: Owner::Language(Language::Rust),
        applies: "the project has a module table",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    PureModuleEffects => Metadata {
        id: "pure-module-effects",
        summary: "A registered pure module performs no reads or effects and imports only types and other pure modules.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project registers pure modules",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    TypeScriptFileCase => Metadata {
        id: "typescript-file-case",
        summary: "A TypeScript file name is camelCase, or PascalCase for a file that exports one class.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/typescript.rs",
            fixtures: "tests/fixtures/rules/typescript-file-case",
        }),
    },
    TypeScriptCatchAllName => Metadata {
        id: "typescript-catch-all-name",
        summary: "A TypeScript file or folder is not named after a catch-all.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/typescript.rs",
            fixtures: "tests/fixtures/rules/typescript-catch-all-name",
        }),
    },
    TypeScriptOwnerFolder => Metadata {
        id: "typescript-owner-folder",
        summary: "A file that shares an owner's noun sits in the owner's folder.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/typescript.rs",
            fixtures: "tests/fixtures/rules/typescript-owner-folder",
        }),
    },
    NoIndexFile => Metadata {
        id: "no-index-file",
        summary: "Source folders hold no index files.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/typescript.rs",
            fixtures: "tests/fixtures/rules/no-index-file",
        }),
    },
    OneFileFolder => Metadata {
        id: "one-file-folder",
        summary: "A module with one production source file is a flat file, not a folder.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    FolderModuleEntry => Metadata {
        id: "folder-module-entry",
        summary: "A folder module has an entry file named after its folder.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    TestFileName => Metadata {
        id: "test-file-name",
        summary: "A source test file is named after a module beside it.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/typescript.rs",
            fixtures: "tests/fixtures/rules/test-file-name",
        }),
    },
    FixtureImport => Metadata {
        id: "fixture-import",
        summary: "Production code does not load fixtures.",
        owner: Owner::Language(Language::TypeScript),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    VersionedRecord => Metadata {
        id: "versioned-record",
        summary: "A versioned record format has a version literal and current, newer, and malformed fixtures.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project writes versioned records",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    TestProject => Metadata {
        id: "test-project",
        summary: "Every test file belongs to exactly one test project.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project splits tests into projects",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    TestEnvironment => Metadata {
        id: "test-environment",
        summary: "The cached test tasks declare every environment variable that test code reads.",
        owner: Owner::Language(Language::TypeScript),
        applies: "the project caches test tasks",
        fields: &[],
        enforcement: Enforcement::ProjectLocal,
    },
    GpuiPlainCore => Metadata {
        id: "gpui-plain-core",
        summary: "The `core` and `cli` role packages depend on no GPUI package and no `ui`, `kit`, or `app` role package, directly or through other packages.",
        owner: Owner::Layer("gpui"),
        applies: "the project has a `core` or `cli` role",
        fields: &["roles"],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/package_boundaries.rs",
            fixtures: "tests/fixtures/rules/gpui-plain-core",
        }),
    },
    GpuiAppLifetime => Metadata {
        id: "gpui-app-lifetime",
        summary: "One module of the `app` role owns the GPUI application, windows, tasks, and one shutdown path.",
        owner: Owner::Layer("gpui"),
        applies: "the project has an `app` role",
        fields: &["roles"],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiContextNames => Metadata {
        id: "gpui-context-names",
        summary: "Name a GPUI context `cx` and a window `window`, with `window` before `cx`.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiBuilderChain => Metadata {
        id: "gpui-builder-chain",
        summary: "Write an element builder as one fluent chain, with `.when_some(...)` for an optional child.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiBuilderSize => Metadata {
        id: "gpui-builder-size",
        summary: "Do not split a builder chain only to meet size review signals.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiElementId => Metadata {
        id: "gpui-element-id",
        summary: "Element IDs come from stable domain data, never a list index.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiUpdateClosure => Metadata {
        id: "gpui-update-closure",
        summary: "Inside `entity.update`, use the closure's `state` and `cx`.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiReentrantUpdate => Metadata {
        id: "gpui-reentrant-update",
        summary: "Never read or update an entity through its handle while it is being updated.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiWeakParent => Metadata {
        id: "gpui-weak-parent",
        summary: "A child refers to its parent through a `WeakEntity` and treats a failed upgrade as normal.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiPureRender => Metadata {
        id: "gpui-pure-render",
        summary: "`render` starts no work, subscribes to nothing, and changes no state.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiNotify => Metadata {
        id: "gpui-notify",
        summary: "Call `cx.notify()` once after a visible change, never from `render`.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiTaskOwner => Metadata {
        id: "gpui-task-owner",
        summary: "Await a `Task`, store it in its owner, or detach it on purpose.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiSubscriptionOwner => Metadata {
        id: "gpui-subscription-owner",
        summary: "Store a `Subscription` in its owner, or detach it on purpose.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiNoDiscardBinding => Metadata {
        id: "gpui-no-discard-binding",
        summary: "Never bind a `Task` or `Subscription` with `let _ =`.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiStaleResult => Metadata {
        id: "gpui-stale-result",
        summary: "Drop a background result when the state it started from is no longer current.",
        owner: Owner::Layer("gpui"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    GpuiComponentKit => Metadata {
        id: "gpui-component-kit",
        summary: "Only the `kit` role imports gpui-component; other code uses the project's own widget types.",
        owner: Owner::Layer("gpui-component"),
        applies: "always",
        fields: &["roles"],
        enforcement: Enforcement::Partial {
            check: Check {
                checker: "src/check/package_boundaries.rs",
                fixtures: "tests/fixtures/rules/gpui-component-kit",
            },
            unenforced: "components that a package other than `kit` uses through a bundle crate that re-exports them",
        },
    },
    ReactJsxExtension => Metadata {
        id: "react-jsx-extension",
        summary: "Only a file that contains JSX uses `.tsx`.",
        owner: Owner::Layer("react"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    ReactComponentFile => Metadata {
        id: "react-component-file",
        summary: "A file that mainly exports one component is named after it in PascalCase.",
        owner: Owner::Layer("react"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/react.rs",
            fixtures: "tests/fixtures/rules/react-component-file",
        }),
    },
    ReactTestPlacement => Metadata {
        id: "react-test-placement",
        summary: "A component's unit test sits beside it as `<Component>.test.tsx`.",
        owner: Owner::Layer("react"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::Enforced(Check {
            checker: "src/check/layout/react.rs",
            fixtures: "tests/fixtures/rules/react-test-placement",
        }),
    },
    PiDiscoveredPath => Metadata {
        id: "pi-discovered-path",
        summary: "`package.json` declares every path Pi discovers, and renaming code keeps those paths.",
        owner: Owner::Layer("pi"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    PiRegisteredName => Metadata {
        id: "pi-registered-name",
        summary: "Renaming code keeps every name registered with Pi.",
        owner: Owner::Layer("pi"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    HerdrModBundle => Metadata {
        id: "herdr-mod-bundle",
        summary: "Each mod builds into one `main.js` bundle; imports of Herdr's own modules stay imports.",
        owner: Owner::Layer("herdr"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    HerdrModFiles => Metadata {
        id: "herdr-mod-files",
        summary: "A mod keeps the fixed names `main.js` and `mod.json`, and its install script copies every file Herdr needs at run time.",
        owner: Owner::Layer("herdr"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
    HerdrDeclarations => Metadata {
        id: "herdr-declarations",
        summary: "Mods typecheck against declarations that the pinned Herdr generates, and the repository never commits them.",
        owner: Owner::Layer("herdr"),
        applies: "always",
        fields: &[],
        enforcement: Enforcement::GuidanceOnly,
    },
}

impl Rule {
    pub(crate) fn id(self) -> &'static str {
        self.metadata().id
    }
}

#[cfg(test)]
impl Enforcement {
    pub(crate) fn check(self) -> Option<Check> {
        match self {
            Self::Enforced(check) | Self::Partial { check, .. } => Some(check),
            Self::RustAndTypeScript { rust, .. } => Some(rust),
            Self::TypeScriptImports(_) | Self::GuidanceOnly | Self::ProjectLocal => None,
        }
    }

    fn files(self) -> Vec<Check> {
        match self {
            Self::Enforced(check)
            | Self::Partial { check, .. }
            | Self::TypeScriptImports(check) => vec![check],
            Self::RustAndTypeScript { rust, typescript } => vec![rust, typescript],
            Self::GuidanceOnly | Self::ProjectLocal => Vec::new(),
        }
    }

    fn label(self) -> String {
        match self {
            Self::Enforced(_) => "enforced".to_string(),
            Self::Partial { unenforced, .. } => {
                format!("partial; not enforced: {unenforced}")
            }
            Self::TypeScriptImports(_) => "enforced by `@sqve/stomme-ts-imports`".to_string(),
            Self::RustAndTypeScript { .. } => {
                "enforced; TypeScript by `@sqve/stomme-ts-imports`".to_string()
            }
            Self::GuidanceOnly => "guidance only".to_string(),
            Self::ProjectLocal => "project-local".to_string(),
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
pub(crate) fn compare_or_rewrite_table(
    path: &Path,
    generated: &str,
    rewrite: bool,
) -> io::Result<bool> {
    if rewrite {
        fs::write(path, generated)?;
    }

    let committed = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };

    Ok(committed == generated)
}

#[cfg(test)]
fn join_checks(checks: &[Check], field: impl Fn(&Check) -> &'static str) -> String {
    if checks.is_empty() {
        return "none".to_string();
    }

    checks.iter().map(field).collect::<Vec<_>>().join(", ")
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
    let checks = metadata.enforcement.files();
    let checker = join_checks(&checks, |check| check.checker);
    let fixtures = join_checks(&checks, |check| check.fixtures);

    format!(
        "| `{}` | {} | {owner} | {} | {fields} | {checker} | {fixtures} | {} |\n",
        metadata.id,
        metadata.summary,
        metadata.applies,
        metadata.enforcement.label()
    )
}

#[cfg(test)]
mod tests;
