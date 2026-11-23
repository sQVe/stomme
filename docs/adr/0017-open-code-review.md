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

- One reusable workflow in stomme reviews the pull request, and a project calls it by release
  commit with a version comment, as
  [ADR 0016 (Pin the sync workflow by release commit)](./0016-pin-the-sync-workflow-by-release-commit.md)
  says. The project holds one reference and no copy.
- Renovate bumps that pin in its `stomme` group, with the `stomme.toml` version and the binary pin,
  so the review settings move with the rules they enforce.
- The job classifies the review with the `stomme` binary that the project pins in its `mise.toml`.
  Renovate moves that pin with the release commit the workflow calls, so the classification matches
  the workflow. stomme's own workflow builds the binary from the same checkout, so its pull requests
  classify with the code in the change.
- The Action's checkout step passes no `ref`, so a pull request run checks out the merge commit.
  The rule file and stomme's own build come from the change, not from the base branch.
- A fork pull request gets no secrets, so it cannot run the review. Fork pull requests are not
  supported.

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

- The OCR job is a required check that runs on every pull request push. It passes only when OCR
  reviewed every file it selected. Findings never change the result.
- `stomme ocr-coverage` decides the result from the coverage record in OCR's JSON result, not from
  OCR's exit code or its warnings. OCR exits 0 on a partial review that ran out of budget, and a
  warning names its whole review group, not the files it left unreviewed.
- A selected file counts as reviewed when the record lists it as completed, or as reused from an
  earlier run of the same session. A failed or waived file counts as unreviewed. A file that the
  rules exclude was never selected, so it leaves no gap.
- When a run leaves files unreviewed, the workflow resumes the session once, and OCR reviews only
  those files. The resume uses the same range, rule file, provider, and model as the first run, and
  it passes no token budget, so the resumed run does not stop at the same budget again. The check
  fails when files stay unreviewed after the resume, because a pass would let a change merge with
  code that no one read.
- A partial review and a failed run both fail the check, and the command's exit code tells them
  apart. Every partial review is resumed once, whether a budget stop or a timeout cut it short.
  A failed run, such as on a missing secret or an unusable result, stops the review and may keep
  the files it already recorded as reviewed. It needs a fix to the setup, so the workflow does not
  resume it.
- The job passes only when the last classification reports every selected file as reviewed. A
  partial result fails and names the unreviewed files, and a failed run fails with its error.
- The resumed review posts its result once, including the comments of the first run for the files
  it reused. Its summary keeps the previous checkpoint marker, so the next push reviews from the
  old head: a wider range, never a wrong one.
- The summary comment starts with the coverage section that the command prints. It names each
  unreviewed file with OCR's reason, and it lists the excluded files in a collapsed block. The
  workflow puts the same section at the top of the job summary, and inserts it after the summary
  comment's marker so the comment stays findable and keeps its single checkpoint marker.
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
- A review that leaves files unreviewed after its resume blocks the merge until a rerun reviews
  every file.
- A model that stops early while OCR records the file as completed passes the check, because the
  check trusts OCR's coverage record.
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

Build the rule file in the workflow with the `stomme` binary instead of syncing it. Rejected
because stomme's own pull requests would review with the rules of the last release, not the ones in
the change.

### Pass a partial review with a list of unreviewed files

Let the check pass and list the unreviewed files in the comment. Rejected because the list is
advice, and the check then passes on code that no one reviewed.

### Pass above a reviewed share

Let the check pass when OCR reviewed most of the selected files. Rejected because any threshold
lets an unread file through, and it counts a large file the same as a small one.

### Read the result with a script in the workflow

Classify the result with `jq` in the workflow instead of the binary. Rejected because the
classification then has no tests against real result files, and a change in OCR's format breaks
the check only at run time.
