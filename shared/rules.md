# Rules

The stomme binary generates this file from its rule catalog. Do not edit it. Checker and fixture
paths are relative to the stomme crate.

| ID | Rule | Owner | Applies | `stomme.toml` fields | Checker | Fixtures | Status |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `no-stomme` | The project has a `stomme.toml`. | common | always | none | src/check.rs | tests/fixtures/rules/no-stomme | enforced |
| `broken-installation` | `stomme.toml` is valid TOML that matches the schema and selects a valid layer set. | common | `stomme.toml` exists | `schema`, `language`, `layers`, `roles` | src/check.rs | tests/fixtures/rules/broken-installation | enforced |
| `required-part` | Every required file exists. | common | always | none | src/check/required_parts.rs | tests/fixtures/rules/required-part | enforced |
| `tau-full-check` | `.pi/tau.json` names the full check as a non-empty `fullCheck.argv` of strings. | common | `.pi/tau.json` exists | none | src/check/required_parts.rs | tests/fixtures/rules/tau-full-check | enforced |
| `adr-duplicate-number` | No two ADR files share a number. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-duplicate-number | enforced |
| `adr-heading` | An ADR's first heading is `# ADR NNNN: Title` with its own number. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-heading | enforced |
| `adr-retired-number` | No ADR file uses a number from the Retired table. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-retired-number | enforced |
| `adr-index-count` | The ADR index lists each ADR file once. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-index-count | enforced |
| `adr-index-label` | An ADR index entry's label is the ADR's number and title. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-index-label | enforced |
| `adr-index-missing-file` | Every file the ADR index lists exists. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-index-missing-file | enforced |
| `adr-index-entry` | Every link line in the ADR index is `- [NNNN: Title](./NNNN-slug.md)`. | common | `docs/adr/README.md` exists | none | src/check/adr_index.rs | tests/fixtures/rules/adr-index-entry | enforced |
| `rust-config-file` | The root `Cargo.toml` exists, and every Cargo manifest and `clippy.toml` the checks read is valid. | rust | always | none | src/check/cargo_manifest.rs | tests/fixtures/rules/rust-config-file | enforced |
| `rust-lint-level` | `[workspace.lints]` keeps every shared lint at its shared priority and at least its shared level. | rust | always | none | src/check/rust_lints.rs | tests/fixtures/rules/rust-lint-level | enforced |
| `rust-lint-inheritance` | Every workspace member has `[lints] workspace = true`. | rust | always | none | src/check/rust_members.rs | tests/fixtures/rules/rust-lint-inheritance | enforced |
| `rust-required-ban` | `clippy.toml` holds each shared Rust `disallowed-methods` ban with its reason. | rust | always | none | none | none | guidance only |
| `gpui-required-ban` | `clippy.toml` holds each GPUI `disallowed-methods` ban with its reason. | layer `gpui` | always | none | none | none | guidance only |
| `module-imports` | Imports follow the module table: a module imports only the modules its row allows. | common | the project has a module table | none | none | none | project-local |
| `module-listed` | Every source file belongs to a module in the module table. | common | the project has a module table | none | none | none | project-local |
| `module-table-closed` | Every module the module table allows as an import has its own row. | common | the project has a module table | none | none | none | project-local |
| `module-table-order` | Module registries and their lists are sorted and free of duplicates. | typescript | the project has a module table | none | none | none | project-local |
| `module-table-documented` | The import table in the module ADR equals the enforced module table. | typescript | the project has a module table | none | none | none | project-local |
| `module-private-file` | Other modules import a folder module through its entry or public files only. | typescript | the project has a module table | none | none | none | project-local |
| `composition-root-import` | No module imports the composition root. | typescript | the project has a composition root | none | none | none | project-local |
| `extension-isolation` | An extension does not import another extension. | typescript | the project has extensions | none | none | none | project-local |
| `extension-direction` | A shared module does not import an extension. | typescript | the project has extensions | none | none | none | project-local |
| `literal-import` | A dynamic import names a literal path, not a computed one. | typescript | the project has a module table | none | none | none | project-local |
| `crate-root-path` | A module uses other modules through their module paths, not items at the crate root. | rust | the project has a module table | none | none | none | project-local |
| `pure-dependencies` | A module uses only pure modules; no module uses an I/O module. | rust | the project has a module table | none | none | none | project-local |
| `pure-module-effects` | A registered pure module performs no reads or effects and imports only types and other pure modules. | typescript | the project registers pure modules | none | none | none | project-local |
| `ui-dependency` | Core and CLI packages do not depend on UI packages, directly or through other packages. | rust | the workspace has UI packages | none | none | none | project-local |
| `no-index-file` | Source folders hold no index files. | typescript | always | none | none | none | project-local |
| `one-file-folder` | A module with one production source file is a flat file, not a folder. | typescript | always | none | none | none | project-local |
| `folder-module-entry` | A folder module has an entry file named after its folder. | typescript | always | none | none | none | project-local |
| `test-file-name` | A source test file is named after a module beside it. | typescript | always | none | none | none | project-local |
| `fixture-import` | Production code does not load fixtures. | typescript | always | none | none | none | project-local |
| `versioned-record` | A versioned record format has a version literal and current, newer, and malformed fixtures. | typescript | the project writes versioned records | none | none | none | project-local |
| `test-project` | Every test file belongs to exactly one test project. | typescript | the project splits tests into projects | none | none | none | project-local |
| `test-environment` | The cached test tasks declare every environment variable that test code reads. | typescript | the project caches test tasks | none | none | none | project-local |
