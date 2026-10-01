# ADR 0005: Exact tool pins and dependency kinds

- Status: Accepted
- Date: 2026-10-01

## Context

- Oxlint's JS plugin API is alpha and has no semver promise. A minor release can break the plugin or
  Stylistic.
- Oxlint resolves `jsPlugins` specifiers from the consumer's config. Under pnpm, a consumer cannot
  resolve the dependencies of a package it installed.
- The `stickler` command must run the consumer's Vite Plus. A second copy could load a different
  Oxlint than the one the consumer's editor uses.

## Options considered

- Range versions. Rejected: an alpha API change would reach consumers without a Stickler release.
- Ask consumers to install Stylistic themselves. Rejected: it breaks zero configuration.
- Exact pins, with each tool in the dependency kind that matches who owns it. Chosen.

## Decision

Stickler pins `vite-plus`, `@stylistic/eslint-plugin`, `@oxlint/plugins`, and `oxlint-tsgolint` to
exact versions.

- `vite-plus` is an exact peer dependency. The consumer owns it, and the `stickler` command resolves
  `vite-plus/bin` from the package, which reaches the consumer's copy.
- `@stylistic/eslint-plugin` is an exact dependency. The config passes its absolute path to Oxlint,
  resolved from the package with `import.meta.resolve`, so pnpm and Bun consumers need not install
  it.
- `@oxlint/plugins` is an exact dependency, because the plugin's declaration files import its types.
- `oxlint-tsgolint` is an exact development dependency. Vite Plus depends on its own exact version
  and finds it, so consumers get it with Vite Plus. The pin keeps this repository on the same
  version.
- The `stickler` plugin is loaded by absolute path from the package, under the name `stickler`.
- A Vite Plus upgrade is a Stickler release: update all pins together and run the full check.

## Tradeoffs

- Consumers get a tested combination of tools.
- Cost: consumers must use the Vite Plus version Stickler pins, and wait for a Stickler release to
  upgrade it.
