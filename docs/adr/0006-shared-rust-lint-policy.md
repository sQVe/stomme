# ADR 0006: Check the shared Rust lint policy as a floor

**Date**: 2026-11-15\
**Status**: Accepted\
**Related**: [ADR 0003 (Toolchain and quality gates)](./0003-toolchain-and-quality-gates.md),
[ADR 0005 (Separate languages, framework layers, and roles)](./0005-languages-and-layers.md)

## Context

Every Rust project that uses stomme kept its own copy of the same `[workspace.lints]` block. The
copies drift when one project changes a level and the others do not. A project can also weaken the
block, or add a crate that does not inherit it, without anyone noticing.

Clippy reads one `clippy.toml` per project. That file holds the project's own bans and settings
next to any shared bans, so no single owner can write it.

## Decision

stomme owns the shared Rust lint policy as a floor that `stomme check` enforces and never writes.

### Lint levels

The Rust base owns one `[workspace.lints]` block, embedded in the binary. `check` compares the
project's block with it after normalizing each value. A project may add lints or raise a level. A
missing lint, a lower level, or a different priority fails, naming the lint. A different priority
fails because it changes which of two overlapping lints wins.

Every workspace member must have `[lints] workspace = true`, so no crate escapes the block.
Members include the path dependencies that Cargo makes members because they lie inside the
workspace root and are not excluded.

### Required bans

`check` requires a set of `disallowed-methods` bans in `clippy.toml`, each with its reason. A
project may add its own bans and settings. stomme never overwrites the file, so the project's own
entries stay under its owner's control.

The Rust base owns general bans. A framework layer owns the bans for its framework, such as GPUI
bans in the `gpui` layer, and they apply only when the project selects that layer. Each lint rule
has one ID and one owner, as ADR 0005 requires.

## Consequences

### Positive

- One policy replaces the copies, and `check` reports a project that drifts below it.
- Projects keep their own lints, levels, bans, and clippy settings.
- A framework's bans reach only the projects that use the framework.

### Negative

- A project must copy a new shared lint or ban by hand when `check` reports it missing.
- `check` must read Cargo workspace members itself, and it rejects member globs beyond `*` inside
  one path segment.

## Alternatives considered

### Sync and overwrite the files

Write the lint block and `clippy.toml` into each project on every update. Rejected because Clippy
reads one file that also holds the project's own bans and settings, and overwriting it would erase
them.

### Require an exact match

Fail when the project's block differs from the policy in any way. Rejected because projects need to
add lints and raise levels for their own code, and an exact match would push them to suppress the
check instead.
