# Architecture decision records

Record lasting decisions and the reasons behind them, not how a feature works.

## Before writing

Before opening the [template](./TEMPLATE.md), answer:

- What choice are we making, and what lasting reason stands behind it?
- What credible alternative did we consider, and why did we reject it?

If the answers only restate what the code does, do not write an ADR. Code and tests hold behavior. A
rule change does not require a new document.

Use a title that names the choice, and state that choice at the start of the Decision section. An
ADR is not a feature summary, implementation plan, or acceptance checklist.

A new ADR is Accepted. Merging its PR is the approval, so there is no Proposed stage. When a later
decision replaces it, change its status to Superseded with a link to the replacement.

## Index

- [0001: Vite Plus, pnpm, and tsc as the toolchain](./0001-toolchain.md)
- [0002: Zero configuration, with every rule on](./0002-zero-configuration.md)
- [0003: One environment switch for house rules](./0003-house-rule-switch.md)
- [0004: Ship built JavaScript](./0004-built-javascript.md)
- [0005: Exact tool pins and dependency kinds](./0005-exact-pins.md)
