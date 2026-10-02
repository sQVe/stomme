# ADR 0004: Ship built JavaScript

- Status: Accepted
- Date: 2026-10-01

## Context

- Node 24 strips TypeScript types, but refuses TypeScript files inside `node_modules` with
  `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING`. Bun loads them.
- Consumers run Seam on Node or Bun, and Oxlint imports the plugin with the consumer's runtime.

## Options considered

- Publish the TypeScript source. Rejected: Node consumers cannot load it.
- Build to JavaScript with declarations, and publish only `dist/`. Chosen: both runtimes load plain
  JavaScript.

## Decision

The package publishes JavaScript built by `tsc`, with `.d.ts` files.

- `prepack` builds, so `pnpm pack` and `npm publish` never ship a stale `dist/`.
- The repository runs its own source with Node type stripping. The config finds the plugin beside
  itself with its own file extension, so the same code loads `plugin.ts` from source and `plugin.js`
  from the package.
- End-to-end tests install the packed tarball and run it under Node and Bun.

## Tradeoffs

- The package loads on both runtimes without a loader.
- Cost: every release needs a build step, and the release workflow must run it.
