# ADR 0010: Ship built JavaScript

**Date**: 2026-10-01\
**Status**: Accepted

## Context

Node strips TypeScript types, but refuses TypeScript files inside `node_modules` with
`ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING`. Bun loads them. Consumers run stomme-ts-style on
Node or Bun, and Oxlint imports the plugin with the consumer's runtime.

## Decision

The package publishes only JavaScript built by `tsc` in `dist/`, with `.d.ts` files.

Both runtimes load plain JavaScript. Packing must build first so releases never ship stale
output. The package runs its own source with Node type stripping, while packed package tests
run the published form under Node and Bun.

## Consequences

### Positive

- The package loads on both runtimes without a loader.

### Negative

- Every release needs a build step, and the release workflow must run it.

## Alternatives considered

### Publish TypeScript source

Rejected because Node consumers cannot load it.
