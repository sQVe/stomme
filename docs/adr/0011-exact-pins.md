# ADR 0011: Exact tool pins and dependency kinds

**Date**: 2026-10-01\
**Status**: Accepted

## Context

Oxlint's JS plugin API is alpha and has no semver promise. A minor release can break the plugin or
Stylistic. Oxlint resolves `jsPlugins` specifiers from the consumer's config. Under pnpm, a
consumer cannot resolve the dependencies of a package it installed.

The `stomme-ts-style` command must run the consumer's Vite Plus. A second copy could load a
different Oxlint than the one the consumer's editor uses.

## Decision

stomme-ts-style pins `vite-plus`, `@stylistic/eslint-plugin`, `@oxlint/plugins`, `oxlint`,
`oxlint-tsgolint`, and `vitest` to exact versions.

### Dependency ownership

- `vite-plus` is an exact peer dependency. The consumer owns it, and the `stomme-ts-style` command
  must use the consumer's copy.
- `@stylistic/eslint-plugin` is an exact dependency. The config passes its absolute path to Oxlint,
  resolved from the package, so pnpm and Bun consumers need not install it.
- `@oxlint/plugins` is an exact dependency because the plugin's declaration files import its types.
- `oxlint-tsgolint` is an exact development dependency. Vite Plus supplies it to consumers. The pin
  keeps this package on the same version.
- `oxlint` and `vitest` are exact development dependencies. Vite Plus depends on exact versions of
  both. Rule tests must use the same Oxlint and Vitest versions as consumers.

### Version alignment and loading

`oxlint`, `oxlint-tsgolint`, `vitest`, and `@oxlint/plugins` match the versions Vite Plus declares,
and a test fails when they differ. The `seam` plugin loads by absolute path from the package,
under the name `seam`.

A Vite Plus upgrade requires a stomme-ts-style release. All pins change together and must pass
the full check.

## Consequences

### Positive

- Consumers get a tested combination of tools.

### Negative

- Consumers must use the Vite Plus version stomme-ts-style pins and wait for a package release to
  upgrade it.

## Alternatives considered

### Version ranges

Rejected because an alpha API change would reach consumers without a stomme-ts-style release.

### Consumers install Stylistic

Rejected because it breaks zero configuration.
