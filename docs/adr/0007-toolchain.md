# ADR 0007: Vite Plus, pnpm, and tsc as the toolchain

**Date**: 2026-10-01\
**Status**: Accepted

## Context

stomme-ts-style is an Oxlint configuration and plugin. Consumers run it through Vite Plus, which
bundles Oxlint, Oxfmt, and Vitest. The package must lint and format itself with its own rules, so
it needs the same tools its consumers use.

Consumers use pnpm with a strict, non-hoisted tree, or Bun. The package must load under both.

## Decision

The `packages/style` package uses pnpm, Vite Plus, and `tsc`.

These toolchain rules apply only to `packages/style`; repository-wide gates follow
[ADR 0003](./0003-toolchain-and-quality-gates.md).

pnpm's strict tree proves the package resolves its own dependencies. `pnpm-lock.yaml` is the only
lockfile, and `packageManager` pins the pnpm version. Vite Plus provides lint, format, and tests.
The package lints itself through its own `stomme-ts-style` command.

`tsc` builds JavaScript with declarations, one file per module. Package checks include typechecking,
self-lint, formatting, the build, and packed package tests under Node and Bun.

## Consequences

### Positive

- The package checks itself with the tools and the strict install its consumers use.

### Negative

- Packed package tests need the pnpm store or network access, and Bun on the path.
- Declaration files keep `.ts` import specifiers. TypeScript resolves them to the emitted `.d.ts`
  files, but other tools that read declarations may not.

## Alternatives considered

### Bun as runtime, package manager, and test runner

Rejected because Bun loads TypeScript inside `node_modules`, hiding the Node failure that shipping
built JavaScript exists to avoid.

### Vite Plus for the build

Use `vp pack` for the build. Rejected because `tsc` already typechecks the source and emits one
file per module with declarations, keeping the plugin layout readable in `node_modules`.
