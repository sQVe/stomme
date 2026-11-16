# stomme

A shared foundation for project structure, coding rules, and documentation. Read
[the development guide](docs/development.md) for local setup and verification.

- Run `cargo xtask check` before finishing changes.
- Follow [the Rust coding guide](docs/coding.md) in Rust code. It changes and adds to Tau's coding
  instructions.
- Format with `cargo fmt`. rustfmt runs with its default settings.
- Follow the decisions in [docs/adr](docs/adr/README.md), and record new decisions there. Read that
  guide before adding or changing an ADR. When details of a decision change, edit its ADR in place
  in the same change. Do not write documents that explain how a feature works; see
  [ADR 0001](docs/adr/0001-documentation-scope.md).
- Comment only what the code cannot say, such as a constraint or a workaround. Do not describe the
  code's history.
- Before finishing a document, check its local links and verify the commands it gives against the
  repository.

## Architecture

- stomme owns shared rules and checks. Projects own their modules, nouns, and project decisions.
  See [ADR 0002](docs/adr/0002-purpose-and-design.md).
- `crates/stomme` is the `stomme` CLI, with one command: `stomme check`. `xtask` holds repository checks and mutation tooling.
- `typescript/style` is the TypeScript style package, published as `@sqve/stomme-ts-style`.
- The toolchain is pinned in `rust-toolchain.toml`. Lint levels live in `[workspace.lints]`, and CI
  fails on warnings. Add single clippy lints, not whole groups. See
  [ADR 0003](docs/adr/0003-toolchain-and-quality-gates.md).
- stomme is MIT. Never depend on a GPL crate. See
  [ADR 0003](docs/adr/0003-toolchain-and-quality-gates.md).
<!-- stomme:start -->
# Shared rules

- Run the full check before finishing changes: `cargo xtask check`.
- Format with `cargo fmt`.
- Follow the decisions in [docs/adr](docs/adr/README.md).
- Follow the shared ADRs in [docs/stomme/shared/adr](docs/stomme/shared/adr/README.md).
- Record new decisions in [docs/adr](docs/adr/README.md).
- Edit an ADR in place when its details change.
- Comment only what the code cannot say. Never describe history.
- Document decisions, not features. See [ADR 0001](docs/stomme/shared/adr/0001-documentation-scope.md).
- Read [the general guide](docs/stomme/shared/coding.md).
- Read [the Rust guide](docs/stomme/languages/rust/guide.md).
<!-- stomme:end -->
