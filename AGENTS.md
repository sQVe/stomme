# stomme

A shared foundation for project structure, coding rules, and documentation. Read
[the development guide](docs/development.md) for local setup and verification.

- Read [the stomme-only Rust rules](docs/coding.md) for rules that apply to this repository alone.
- rustfmt runs with its default settings.
- Read [the ADR guide](docs/adr/README.md) before adding or changing an ADR.
- Before finishing a document, check its local links and verify the commands it gives against the
  repository.

## Architecture

- stomme owns shared rules and checks. Projects own their modules, nouns, and project decisions.
  See [ADR 0002](docs/adr/0002-purpose-and-design.md).
- `crates/stomme-cli` is the `stomme` CLI, with six commands: `stomme check`, `stomme sync`,
  `stomme guides`, `stomme init`, `stomme ocr-coverage`, and `stomme ocr-scope`.
  `stomme ocr-coverage <result-file>`
  reads an Open Code Review result and exits 0 when every selected file was reviewed, 2 for a
  partial review, and 1 for a failed run or an unusable file.
  `stomme ocr-scope --reviewed <ref> --reviewed-base <ref> --base <ref> --head <ref>` writes the
  review base since the last complete OCR review and grafts it onto the head.
  `xtask` holds repository checks and mutation tooling.
- `packages/style` is the TypeScript style package, published as `@sqve/stomme-ts-style`.
- `packages/imports` is the `@sqve/stomme-ts-imports` package, which reads the module table in
  `stomme.toml`.
- `foundation/` holds the guides, ADRs, and manifests that the binary embeds and `stomme sync`
  writes to `docs/stomme/`.
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
