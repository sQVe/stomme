# ADR 0017: Review pull requests with Open Code Review

**Date**: 2026-11-23\
**Status**: Accepted\
**Related**:
[ADR 0002 (One versioned foundation for every project)](./0002-purpose-and-design.md),
[ADR 0005 (Separate languages, framework layers, and roles)](./0005-languages-and-layers.md)

## Context

A reviewer checks each pull request against the project's coding guides, and the review covers what
the reviewer remembers to read. A review bot applies the same rules to every push, but each project
that sets one up keeps its own copy of those rules, and the copies drift from the guides.

Open Code Review (OCR) reviews a change and reads its rules from a rule file. A rule file holds path
globs with rule text, and OCR applies the first entry that matches a file. A run reads one rule
file.

stomme already generates a project's coding guides from one foundation, and the project names its
language and layers in `stomme.toml`. The review rules follow the same selection.

## Decision

stomme owns OCR's settings, generates OCR's rule file from the guides, and runs the review as a
required check whose findings are advice.

### The workflow

- One reusable workflow in stomme reviews the pull request, and a project calls it by tag. The
  project holds one reference and no copy.
- Renovate bumps that tag in its `stomme` group, with the `stomme.toml` version and the binary pin,
  so the review settings move with the rules they enforce.

### The rule file

- `stomme sync` writes `docs/stomme/ocr-rules.json`, and the workflow passes it as OCR's `rule`
  input. It is the project's only rule file, so OCR never chooses between two.
- The rule text inlines the general guide, the language guide, and the selected layer guides.
  Inlined text reaches the model in the request, while a pointer to a guide depends on a read that
  the model may skip.

### Project rules

A project may extend the rules in an optional `.opencodereview/rule.json`. It takes a subset of
OCR's format: `rules` entries with `path` and `rule`, plus `include` and `exclude`. Other keys,
`merge_system_rule` included, are refused.

- stomme sets `merge_system_rule: true` on every entry, so OCR keeps its built-in rule. Every entry
  takes the same value, so a project cannot set it.
- OCR uses the first matching rule and never combines rule files. A separate project file would
  hide one of the two, so `sync` merges the project's entries into the synced file: the project's
  entries first, each with the guide text, then the entry for the project's language.
- OCR's default path filter skips test files, and the guides apply to tests too. So the synced
  `include` lists the project's own patterns, then the test paths that filter drops for the
  language. Rust adds `**/*_test.rs`, `**/*_tests.rs`, `**/tests.rs`, `**/tests/**/*.rs`, and
  `**/benches/**/*.rs`. TypeScript adds `.test`, `.spec`, and `__tests__` files for every
  TypeScript and JavaScript extension. An `include` match only admits a file past that filter, so
  the list narrows nothing. It does not list the whole language glob, which would also admit build
  output, generated files, and test data that the guides do not govern.
- `exclude` passes through unchanged. OCR checks it before `include`, so a project can drop a test
  path, such as fixtures that break the rules on purpose. That exclude is the project's own
  exception to the guides.
- Rules for a library or framework are layer guide text. A layer already holds those rules for its
  language, and a second rule system would state them twice.

### The model

The review uses DeepSeek V4.1 Flash through Morph's OpenAI-compatible endpoint, because our
unlimited DeepSeek usage runs through Morph. The exact model ID and URL live in the workflow, and
the token comes from the `MORPH_API_KEY` repository secret.

### The check

- The OCR job is a required check that runs on every pull request push. It passes when the review
  ran, whatever OCR found. It fails when OCR cannot run, such as on a missing secret, a timeout, or
  a model error.
- Findings stay advisory, as with the size and mutation signals in
  [ADR 0009 (Advisory mutation testing and size as a review signal)](../stomme/shared/adr/0009-advisory-review-signals.md).
- Low-severity findings go to the summary comment. Tau needs no review-bot entry, because OCR
  reviews every push without a request.

## Consequences

### Positive

- Every project reviews each push against the same guides, with no rule copy to maintain.
- A finding never blocks a merge, so the review stays a signal and not a step to clear.
- A project's own rule text joins the shared rules in one file, so OCR applies both.

### Negative

- After a project edits its OCR rules, `check` fails while the synced rule file is stale, until
  `stomme sync` runs.
- Each repository sets the secret and the required check by hand.
- An OCR or provider outage fails the check, so merges wait until the review runs.
- Findings depend on a model, so the review does not replace a person's review.

## Alternatives considered

### A copy of the workflow and rules in each project

Each project keeps its own OCR workflow and rule file. Rejected because the copies drift from the
guides, and each fix must be repeated in every project.

### A stomme rule file next to a project rule file

Sync one shared rule file and let the project keep its own beside it. Rejected because OCR uses the
first matching rule and never combines rule files, so one of the two files would be hidden.

### Rule text that points to the guides

Name the guides' paths in the rule and let the model read them. Rejected because a model may skip
the read, and the review then runs without the rules.

### Compose the rules in CI with the stomme binary

Build the rule file in the workflow with the `stomme` binary instead of syncing it. Rejected because
CI must install the binary, and stomme's own pull requests would review with the rules of the last
release, not the ones in the change.
