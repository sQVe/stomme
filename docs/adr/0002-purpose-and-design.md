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
links keep working. The owned set also holds one generated file, `docs/stomme/ocr-rules.json`,
built from the selection and the project's optional `.opencodereview/rule.json`. It is not part of
the tree that mirrors `foundation/`, and link validation covers only the Markdown files.
[ADR 0016](./0016-open-code-review.md) records the review and the rule file's format.
`docs/stomme/.sync-record.toml` records the pinned version and a content hash for each synced path,
and `sync` writes it last. `AGENTS.md` holds one managed block between `<!-- stomme:start -->` and
`<!-- stomme:end -->` markers, and the record holds a hash of that block. Text outside the markers
belongs to the project. `docs/adr/` holds only project ADRs, with independent numbering.

`stomme.toml` is a regular file. `check` and `sync` refuse a symbolic link or any other kind of
entry as a broken installation, and the release gate refuses it before it writes. None of them
follows the link. The gate rewrites the version pin, and a followed link would let it write
outside the project. The checks assume the checkout does not change while they run. The
`@sqve/stomme-ts-imports` reader still follows links, because `check` already fails such a
project.

Side effects in Rust are records of functions declared by the consuming module, not
single-implementation traits. Seam lives in stomme as `packages/style`, published as
`@sqve/stomme-ts-style` with its own version. `@sqve/mutation` stays separate, and stomme pins
its version.

### Distribution and enforcement

A Rust binary provides `init`, `sync`, `guides`, and `check`. The `@sqve/stomme-ts-imports` package
supplies the import rule for TypeScript projects. Both read the same project configuration. The
package version equals the binary's, so one release names a matching pair, and a test in the package
keeps the two versions equal. A consumer loads coding guides from the project's synced text and uses
its bundled guides only when stomme is absent, not broken. [ADR 0013](./0013-consumer-guides.md)
defines the bundle and its record.

`sync` writes only the owned set for the project's version, language, and layers, which includes
the generated `docs/stomme/ocr-rules.json`. It also writes the managed `AGENTS.md` block and leaves
the text outside its markers alone. It sets every stomme npm package the project already declares
in `package.json` to the version recorded for the release, and never adds a package. It validates
every relative link in that set's Markdown files before writing, refuses a file whose bytes match
neither the record nor the new content, and writes the record last.

Checks reject changed synced text, missing required files, invalid ADR indexes, broken layout or
imports, and dependency version drift. Existing projects may record violations by path and rule,
but the list may only shrink.

Rule metadata in the binary generates the shared rules table, so the table cannot drift from the
checks. Each row names the rule's ID, owner, and status. Each enforced rule has a passing and a
failing fixture. A local check is removed only after stomme passes its fixtures; rules not enforced
remain explicitly identified as such.

Renovate updates the `stomme.toml` version, the binary pin, and the sync workflow reference in one
update, and leaves the stomme npm packages alone. The `stomme.toml` version and the `mise.toml`
binary pin both use the same bare version, such as `0.2.0`; the sync workflow reference is a commit,
and its trailing comment keeps the `v` tag prefix. The sync workflow then writes synced files and
package pins, and runs CI on the final commit. Sync must be idempotent and stop on a push conflict.

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

### Links that resolve inside the project

Accept a `stomme.toml` link whose target resolves inside the project. Rejected because each
reader would resolve targets itself, and the gate would still write through the link.
