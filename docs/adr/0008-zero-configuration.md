# ADR 0008: Zero configuration, with every rule on

**Date**: 2026-10-01\
**Status**: Accepted

## Context

stomme-ts-style exists to make agent-written TypeScript consistent across projects. A rule that
one project turns off no longer gives that guarantee. Some rules only make sense for one area,
such as React hooks or Vitest tests.

Both source projects keep function size, file size, and parameter count advisory. With
`--deny-warnings`, a warning rule fails the check like an error. Oxlint's native test rules
recognize Vitest and Jest imports and test globals, but not imports from `bun:test`.

## Decision

A consumer passes the exported `lint` config unchanged, and every rule in it is on.

A rule that needs a project exception does not belong in the package. The package owns shared
test-file relaxations. Presets exist only for specific areas: `react` and `vitest`.

There are no size rules: no `max-lines`, `max-lines-per-function`, or `max-params`.
There is no Bun preset until Oxlint's test rules recognize `bun:test`.
`typescript/explicit-function-return-type` and `typescript/prefer-readonly-parameter-types` stay
out of the package.

`seam/no-abbreviations` replaces `eslint/id-denylist` so abbreviation checks cover words within
names, while allowing object keys required by external APIs.

## Consequences

### Positive

- Every consumer gets the same rules with no configuration.

### Negative

- A rule that is right for most projects but wrong for one cannot ship. That project must add
  the rule itself.
- Bun test files get no test-specific lint.
- Long functions and files pass the check. Reviews must catch them.

## Alternatives considered

### A base set with optional rules

Let consumers turn rules off. Rejected because each project would drift into its own style.

### Size rules as warnings

Rejected because under `--deny-warnings` a warning is not advisory.
