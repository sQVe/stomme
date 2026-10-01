# ADR 0003: One environment switch for house rules

- Status: Accepted
- Date: 2026-10-01

## Context

- House rules enforce style: naming, helper order, type placement, condition size, comment content,
  and blank lines. Most of them have no autofix or need a rename.
- Shown in an editor while code is being written, these diagnostics bury the ordinary ones that
  point at bugs.
- Both source projects load house rules only for their style command, through a project-specific
  environment variable.

## Options considered

- Always load house rules. Rejected: editors would show style diagnostics on every keystroke.
- Ship house rules as a preset. Rejected: a consumer could leave it out, which breaks zero
  configuration.
- Load house rules only when `STICKLER_STYLE=1`, set by the `stickler` command. Chosen: one shared
  name replaces the per-project variables, and editors keep ordinary diagnostics.

## Decision

The exported `lint` config adds house rules only when `STICKLER_STYLE` is `1`.

- The `stickler` command sets `STICKLER_STYLE=1` for its lint child and `0` for the formatter child
  in fix mode.
- House rules are the `stickler` plugin rules, the Stylistic padding rules, `eslint/one-var`, and
  `eslint/no-cond-assign` with `always`.
- `options.reportUnusedDisableDirectives` is on only with house rules. Without the plugin loaded, a
  directive for a house rule looks unused.
- Project checks and hooks run `stickler`, not `vp lint`.

## Tradeoffs

- Editors show ordinary diagnostics, and checks enforce the full style.
- Cost: house rule failures appear only when `stickler` runs, not while typing.
- Cost: the config reads the environment when it loads, so a tool that loads it in a long-running
  process keeps the mode it started with.
