# ADR 0002: One versioned foundation for every project

**Date**: 2026-11-13\
**Status**: Accepted

## Context

Projects copy coding rules, documentation policies, and tooling when they start. The copies miss
parts and drift apart. A person must then find and repair the same gap in each project.

Shared packages already provide a way to distribute rules, but version drift remains possible
unless each project's full check enforces its pin. Projects also need different modules and
capability names, so sharing a foundation must not force them to share application architecture.

## Decision

stomme owns one versioned foundation of shared rules, starter files, checks, and a sync command for
personal Rust and TypeScript projects. Each project pins a version, and its full check fails on
drift. This is the target design; the repository currently holds `stomme check`, repository
tooling, and the TypeScript style package in `packages/style`.

### Shared rules and local choices

stomme owns general, Rust, and TypeScript coding guides, shared ADRs, the ADR template, structural
role names, and starter files. Projects select languages and framework layers as defined in
[ADR 0005](./0005-languages-and-layers.md). Shared roles have shared names in each language's normal
case.

Projects own their capability names, module import tables, and project decisions. `stomme.toml`
holds the pinned version, language, layers, role paths, module table, and bounded exceptions.
`docs/stomme/` holds synced shared text that projects never edit. Its layout mirrors the source
tree under `foundation/`, with `shared/`, `languages/<language>/`, and `layers/<name>/`, so relative
links keep working.
`docs/stomme/.sync-record.toml` records the pinned version and a content hash for each synced path,
and `sync` writes it last. `AGENTS.md` holds one managed block between `<!-- stomme:start -->` and
`<!-- stomme:end -->` markers, and the record holds a hash of that block. Text outside the markers
belongs to the project. `docs/adr/` holds only project ADRs, with independent numbering.

Side effects in Rust are records of functions declared by the consuming module, not
single-implementation traits. Seam lives in stomme as `packages/style`, published as
`@sqve/stomme-ts-style` with its own version. `@sqve/mutation` stays separate, and stomme pins
its version.

### Distribution and enforcement

A Rust binary provides `init`, `sync`, and `check`. The `@sqve/stomme-ts-imports` package supplies
the import rule for TypeScript projects. Both read the same project configuration. The package
version equals the binary's, so one release names a matching pair, and a test in the package keeps
the two versions equal. Tau loads coding guides from
the project's synced text and uses its bundled guides only when stomme is absent, not broken.

`sync` writes only the owned set for the project's version, language, and layers. It also writes the
managed `AGENTS.md` block and leaves the text outside its markers alone. It validates every relative
link in that set before writing, refuses a file whose bytes match neither the record nor the new
content, and writes the record last.

Checks reject changed synced text, missing required files, invalid ADR indexes, broken layout or
imports, and dependency version drift. Existing projects may record violations by path and rule,
but the list may only shrink.

Rule metadata in the binary generates the shared rules table, so the table cannot drift from the
checks. Each row names the rule's ID, owner, and status. Each enforced rule has a passing and a
failing fixture. A local check is removed only after stomme passes its fixtures; rules not enforced
remain explicitly identified as such.

Renovate groups version pins into one update. A workflow syncs shared files on that branch and
runs CI on the final commit. Sync must be idempotent and stop on a push conflict.

stomme follows its own rules. Project adoption and enforcement are separate work from the initial
repository setup.

## Consequences

### Positive

- A shared rule has one owner instead of a copy maintained in each project.
- A full check catches drift instead of relying on a person to notice it.
- Familiar structural names do not replace project-owned capabilities or decisions.
- Version pins make updates explicit and allow rollback.

### Negative

- A faulty shared rule can break several projects at once.
- Starter files and language-specific checks need continued maintenance.
- Adoption requires checking enforcement parity before removing existing checks.
- Automated sync needs permission to push to update branches.

## Alternatives considered

### Tau owns the foundation

Put shared rules and checks in Tau. Rejected because Rust projects would need Tau tooling, and
foundation changes would require a Tau update.

### A starting template and a drift report

Copy a template at project creation and report differences later. Rejected because reporting does
not prevent drift, and each copied check still needs separate maintenance.

### Identical application structure everywhere

Require every project to use the same modules and capability names. Rejected because purposes and
languages differ; only shared structural roles need shared names.
