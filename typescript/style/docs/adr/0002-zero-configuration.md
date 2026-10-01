# ADR 0002: Zero configuration, with every rule on

- Status: Accepted
- Date: 2026-10-01

## Context

- Stickler exists to make agent-written TypeScript consistent across projects. A rule that one
  project turns off no longer gives that guarantee.
- Some rules only make sense for one area, such as React hooks or Vitest tests.
- Both source projects keep function size, file size, and parameter count advisory. With
  `--deny-warnings`, a warning rule fails the check like an error.
- Oxlint's native test rules recognize Vitest and Jest imports and test globals, but not imports
  from `bun:test`. A probe with the `jest` plugin reported nothing for a `bun:test` file that broke
  five rules, while the same file importing from `vitest` broke all five.

## Options considered

- Ship a base set and let consumers turn rules off. Rejected: each project would drift into its own
  style.
- Turn every rule on and accept only rules that no project needs to turn off. Chosen: a rule that
  needs an exception does not belong in the package.
- Keep size rules as warnings. Rejected: under `--deny-warnings` a warning is not advisory.

## Decision

A consumer passes the exported `lint` config unchanged, and every rule in it is on.

- The package owns the test-file override for `**/*.test.{ts,tsx}`, `**/fixtures/**`, and
  `tests/*.ts`, with the relaxations both source projects use.
- Presets exist only for specific areas: `react` and `vitest`. Consumers add them with `extends`.
- There are no size rules: no `max-lines`, `max-lines-per-function`, or `max-params`.
- There is no Bun preset until Oxlint's test rules recognize `bun:test`.
- `typescript/explicit-function-return-type` and `typescript/prefer-readonly-parameter-types` stay
  out of the package.
- `stickler/no-abbreviations` replaces `eslint/id-denylist`. It splits names into words, so it
  catches `onBtnClick` as well as `btn`. It checks only names the file declares, so object keys that
  external APIs require stay allowed.

## Tradeoffs

- Every consumer gets the same rules with no configuration.
- Cost: a rule that is right for most projects but wrong for one cannot ship. That project must add
  the rule itself.
- Cost: Bun test files get no test-specific lint.
- Cost: long functions and files pass the check. Reviews must catch them.
