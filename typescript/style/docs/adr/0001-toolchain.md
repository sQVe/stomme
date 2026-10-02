# ADR 0001: Vite Plus, pnpm, and tsc as the toolchain

- Status: Accepted
- Date: 2026-10-01

## Context

- Seam is an Oxlint configuration and plugin. Consumers run it through Vite Plus, which bundles
  Oxlint, Oxfmt, and Vitest.
- The package must lint and format itself with its own rules, so it needs the same tools its
  consumers use.
- Consumers use pnpm with a strict, non-hoisted tree, or Bun. The package must load under both.

## Options considered

- Bun as runtime, package manager, and test runner. Rejected: Bun loads TypeScript inside
  `node_modules`, so it would hide the Node failure that shipping built JavaScript exists to avoid.
- pnpm with Vite Plus for lint, format, and tests, and `tsc` for the build. Chosen: pnpm's strict
  tree proves the package resolves its own dependencies, and the end-to-end tests still run the
  built package under Bun.
- `vp pack` for the build. Rejected: `tsc` already typechecks the source, and it emits one file per
  module with declarations, which keeps the plugin layout readable in `node_modules`.

## Decision

Seam uses pnpm, Vite Plus, and `tsc`.

- `pnpm-lock.yaml` is the only lockfile. `packageManager` pins the pnpm version.
- `vp lint` runs through the package's own `seam` command, `vp fmt` formats, and `vp test` runs
  Vitest.
- `tsc -p tsconfig.build.json` builds `dist/`. Source imports use `.ts` extensions, and the build
  rewrites them to `.js`.
- `pnpm run check` runs typechecking, the self-lint, the format check, the build, and all tests,
  including the end-to-end tests under Node and Bun.

## Tradeoffs

- The package checks itself with the tools and the strict install its consumers use.
- Cost: the end-to-end tests install the packed tarball into temporary projects, so the test suite
  needs the pnpm store or network access, and Bun on the path.
- Cost: declaration files keep `.ts` import specifiers. TypeScript resolves them to the emitted
  `.d.ts` files, but other tools that read declarations may not.
