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

stomme owns one versioned foundation of shared rules, skeletons, checks, and a sync command for
personal Rust and TypeScript projects. Each project pins a version, and its full check fails on
drift. This is the target design; the initial repository contains only the Rust CLI stub and
repository tooling.

### Shared rules and local choices

stomme owns general, Rust, and TypeScript coding guides, shared ADRs, the ADR template, structural
role names, and project skeletons. The kinds are Rust CLI, Rust GPUI app, TypeScript web app, and
TypeScript package. Shared roles have shared names in each language's normal case.

Projects own their capability names, module import tables, and project decisions. `stomme.toml`
holds the pinned version, project kind, role paths, module table, and bounded exceptions.
`docs/stomme/` holds synced shared text that projects never edit. `docs/adr/` holds only project
ADRs, with independent numbering.

Side effects in Rust are records of functions declared by the consuming module, not
single-implementation traits. Existing lint and mutation packages stay separate; stomme pins
versions instead of absorbing them.

### Distribution and enforcement

A Rust binary provides `init`, `sync`, and `check`. A TypeScript package supplies the import rule
for TypeScript projects. Both read the same project configuration. Tau loads coding guides from
the project's synced text and uses its bundled guides only when stomme is absent, not broken.

Checks reject changed synced text, missing required files, invalid ADR indexes, broken layout or
imports, and dependency version drift. Existing projects may record violations by path and rule,
but the list may only shrink. A local check is removed only after stomme passes its fixtures;
rules not enforced remain explicitly identified as such.

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
- Skeletons and language-specific checks need continued maintenance.
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
