# ADR 0009: One environment switch for house rules

**Date**: 2026-10-01\
**Status**: Accepted

## Context

House rules enforce style: naming, helper order, type placement, condition size, comment content,
and blank lines. Most of them have no autofix or need a rename. In an editor while code is being
written, these diagnostics bury the ordinary ones that point at bugs.

## Decision

The exported `lint` config adds house rules only when `SEAM_STYLE` is `1`.

The `stomme-ts-style` command sets this shared switch for lint, so editors keep ordinary
diagnostics without project-specific environment variables.

House rules are the `seam` plugin rules, the Stylistic padding rules, `eslint/one-var`, and
`eslint/no-cond-assign` with `always`. Unused disable directives are reported only with house
rules. Without the plugin loaded, a directive for a house rule looks unused.

Within `packages/style`, checks and hooks that lint run `stomme-ts-style`, not `vp lint`.
This does not replace the repository-wide hook rules in
[ADR 0003](./0003-toolchain-and-quality-gates.md). Consumers must also use `stomme-ts-style` in
their checks and lint hooks to enforce the full style.

## Consequences

### Positive

- Editors show ordinary diagnostics, and checks enforce the full style.

### Negative

- House rule failures appear only when `stomme-ts-style` runs, not while typing.
- The config reads the environment when it loads, so a tool that loads it in a long-running
  process keeps the mode it started with.

## Alternatives considered

### Always load house rules

Rejected because editors would show style diagnostics on every keystroke.

### House rules as a preset

Rejected because a consumer could leave it out, which breaks zero configuration.
