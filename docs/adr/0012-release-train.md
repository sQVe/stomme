# ADR 0012: Release the binary through GitHub Releases

**Date**: 2026-11-16\
**Status**: Accepted

## Context

The stomme binary embeds shared rules, guides, layer files, and `packages.toml` from folders outside
its crate folder. `cargo package` leaves those files out, so a crate published to crates.io would
not build.

Projects pin a stomme version and declare the npm packages they use. A release must therefore carry
both the binary and the compatible package versions, and an incompatible update must fail before a
project adopts it.

## Decision

stomme releases the binary through GitHub Releases built by cargo-dist. crates.io is not a
publishing target. The binary embeds the guides that `stomme guides` writes, so a guide bundle's
version is the binary version of the release that carries it.

### Compatibility record

`packages.toml` is the release's compatibility record. It names the stomme npm packages and the
version of each that a project selects at that release. The record ships with the binary and in the
release notes, so a project's pinned stomme version identifies one tested package set.

### Release gate

Every release runs the candidate binary against each adopted project's `main` and a fresh consumer
fixture. An adopted project that fails stops the release. A project that has not adopted stomme is
reported without blocking. The gate runs before hosting, so a GitHub Release is created only after
every gate job passes.

### Retrying and rolling back

A partial publish is retried from the same commit and never replaces a published version. A bad
release is rolled back by pointing projects at the previous version; published versions stay
immutable.

## Consequences

### Positive

- The binary, its embedded rules, and the package record are released together.
- A project learns about an incompatible release before it reaches a GitHub Release.

### Negative

- Users install with mise or `cargo binstall --git`, not `cargo install stomme`.
- A release waits for the gate to run the built archive against every project.

## Alternatives considered

### Publish to crates.io

Rejected because the binary embeds files outside its crate folder, so a crates.io build would lack
part of its behavior.

### Publish without a gate

Rejected because a breaking change would reach adopted projects as a published version, and the
project's own full check would fail only after it updated.

### Replace a published version

Rejected because GitHub Releases and package registries treat a version as immutable; consumers that
already fetched it would keep different bytes.
