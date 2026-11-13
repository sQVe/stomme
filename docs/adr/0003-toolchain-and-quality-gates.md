# ADR 0003: Toolchain and quality gates

**Date**: 2026-11-13\
**Status**: Accepted

## Context

stomme needs consistent formatting, linting, testing, and license checks before application code
is added. The source project ruta provides a Rust workspace with these checks. Reusing that base
keeps the development workflow familiar without carrying over its application-specific tooling.

stomme ships under MIT, so every dependency must have a compatible license.

## Decision

stomme pins a stable Rust toolchain, uses default rustfmt settings and selected clippy lints,
checks dependencies with cargo-deny, and runs mutation testing as advice.

### Toolchain and formatting

- `rust-toolchain.toml` pins one stable release with clippy and rustfmt for developers and CI.
- Every crate uses edition 2024 and shared workspace lints.
- rustfmt uses its default widths. Review checks blank lines between logical steps.

### Linting

- Clippy's `all` group warns and `correctness` denies. Add individual lints as needed, not whole
  pedantic, nursery, or restriction groups.
- Unsafe code is denied. CI fails on warnings.
- A suppression uses `#[expect(lint, reason = "...")]` on the smallest item that needs it.
- Use `#[allow(lint, reason = "...")]` only when the lint fires in some builds but not others.

### Dependencies and hooks

- cargo-deny checks licenses, advisories, sources, and duplicate crates. It allows licenses
  compatible with an MIT tool and rejects GPL.
- prek runs formatting and lint checks before commits, without changing files.
- `cargo xtask check` is the entry point for rustfmt, typos, taplo, cargo-deny, cargo-shear,
  clippy, nextest, rustdoc, and `stomme check` on the repository. It reports all failed steps,
  rather than stopping at the first.
- Cargo builds use the lockfile. Build warnings fail the check.
- CI keeps jobs named `Check` and `Test (Linux)` so branch rules can require them.

### Mutation testing

- cargo-mutants mutates workspace code. `.cargo/mutants.toml` excludes test code and test helpers,
  with a reason for each exclusion.
- CI mutates each pull request's diff. Scheduled and manual runs on `main` audit the workspace.
- Survivors are advice, because equivalent mutants cannot be caught by tests. Errors, unmutated
  files, and a failing baseline fail the job.

## Consequences

### Positive

- Developers and CI use the same compiler and checks.
- License and advisory problems are checked before code ships.
- Hooks catch formatting and lint failures before a commit.
- Mutation reports show code that tests may not defend.

### Negative

- The toolchain pin needs updates when Rust changes.
- Selected lints leave some style rules to review.
- A new dependency license needs review before the check can pass.
- Hooks and mutation runs add time, and the check tools need installation.
- Survivors can remain unfixed because they do not fail CI.

## Alternatives considered

### A floating stable toolchain

Build with whichever stable release is installed. Rejected because compiler and lint changes can
make the same code pass on one machine and fail on another.

### Loosened rustfmt widths

Change line-breaking widths to force shorter lines. Rejected because default formatting is
familiar and avoids another policy to maintain.

### The whole clippy pedantic group

Enable every pedantic lint and suppress unwanted ones. Rejected because individual lints make
the chosen rules explicit without adding unrelated restrictions.

### Survivors fail CI

Reject every surviving mutant. Rejected because equivalent mutants would require exclusions or
tests written only to satisfy mutation tooling.

### No license and advisory checks

Rely on review alone. Rejected because transitive dependency licenses and advisories are easy to
miss.
