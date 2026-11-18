# Architecture decision records

Record lasting decisions and the reasons behind them, not how a feature works.

## Before writing

Follow [ADR 0001](./0001-documentation-scope.md) for documentation scope. Before opening the
[template](./TEMPLATE.md), answer:

- What choice are we making, and what lasting reason stands behind it?
- What credible alternative did we consider, and why did we reject it?

If the answers only restate what the code does, do not write an ADR. Code and tests hold behavior. A
feature change does not require a new document.

Use a title that names the choice, and state that choice in the first sentence of the Decision
section. Give each rejected or deferred alternative its own heading under Alternatives considered.
An ADR is not a feature summary, implementation plan, or acceptance checklist.

A new ADR is Accepted. Merging its PR is the approval, so there is no Proposed stage.

## Keeping ADRs current

An ADR states the current decision. When its details change, edit it in place. Supersede it only
when the core choice reverses. The new ADR then states the whole current rule and has a Supersedes
line that names the old ADR without a link. Delete the old file, and add a row for it to the Retired
table. Numbers are never reused. To read a retired ADR, run `git show <commit>:docs/adr/<file>` with
the commit in its row.

## Index

### Documentation

- [0001: Documentation scope](./0001-documentation-scope.md)

### Purpose

- [0002: One versioned foundation for every project](./0002-purpose-and-design.md)
- [0005: Separate languages, framework layers, and roles](./0005-languages-and-layers.md)
- [0013: Publish a pinned guide bundle for consumers without a project](./0013-consumer-guides.md)
- [0014: Compose new projects from owned starter files](./0014-starter-files.md)

### Project and toolchain

- [0003: Toolchain and quality gates](./0003-toolchain-and-quality-gates.md)
- [0004: A Rust coding guide adapts Tau's coding rules](./0004-rust-coding-guide.md)
- [0006: Check the shared Rust lint policy as a floor](./0006-shared-rust-lint-policy.md)

### TypeScript style package

- [0007: Vite Plus, pnpm, and tsc as the toolchain](./0007-toolchain.md)
- [0008: Zero configuration, with every rule on](./0008-zero-configuration.md)
- [0009: One environment switch for house rules](./0009-house-rule-switch.md)
- [0010: Ship built JavaScript](./0010-built-javascript.md)
- [0011: Exact tool pins and dependency kinds](./0011-exact-pins.md)

### Releases

- [0012: Release the binary through GitHub Releases](./0012-release-train.md)

## Retired

| ADR | Title | Replaced by | Last commit |
| --- | ----- | ----------- | ----------- |
