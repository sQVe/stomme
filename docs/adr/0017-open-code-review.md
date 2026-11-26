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
- Every caller, stomme included, classifies the review with the released `stomme` binary that
  its `mise.toml` pins. Renovate moves that pin with the release commit the workflow calls, so the
  classification matches the workflow. No step in the job runs code built from the change; OCR
  reads the change only as data.
- The Action's checkout step passes no `ref`, so a pull request run checks out the merge commit.
  The rule file comes from the change, not from the base branch.
- A fork pull request gets no secrets, so it cannot run the review. Fork pull requests are not
  supported.
- The workflow trusts whoever can push to the repository, because a branch pusher can edit the
  caller workflow in the change. It does not trust the code that a dependency update builds, so a
  dependency update cannot run next to the model token and a write token.

### The rule file

- `stomme sync` writes `docs/stomme/ocr-rules.json`, and the workflow passes it as OCR's `rule`
  input. It is the project's only rule file, so OCR never chooses between two.
- The rule text inlines the general guide, the language guide, and the selected layer guides.
  Inlined text reaches the model in the request, while a pointer to a guide depends on a read that
  the model may skip.
- The rule file holds one entry per language, not one entry per role. OCR groups files across
  roles, and each distinct rule text is sent as its own block, so per-role entries repeat the
  shared guides. For a project with language `rust`, layers `gpui` and `gpui-component`, and all
  five roles under `crates/plugga-<role>`, `stomme sync` produced 14,546 rule bytes with one entry
  and clauses. Five single-role renders, one per role with the full guide text, sum to 68,578
  bytes.
- Each declared role adds one clause, in the closed role order `core`, `ui`, `kit`, `app`, `cli`:
  `If the file path starts with <root>/, apply: <role text>`. `<root>` is the role path without
  empty parts or `.`, with exactly one trailing slash. The slash keeps `src/ui/` from matching
  `src/uikit`. OCR puts `<file path="...">` above each diff, so the model sees the path the clause
  names. An undeclared role gets no clause. A role path that normalizes to nothing gets no clause,
  because `stomme check` owns role path validity.
- Role text has one owner. The blocks marked with an inline `<!-- role:<role> -->` comment in
  ADR 0004 and in the role-scoped layer rules are extracted when the rule file renders. No Rust
  copy holds the prose and no scope table restates it. A layer the project did not select
  contributes no role text.
- A role-scoped layer rule leaves the global guide text and is read once in its role's clause only
  when every file the rule governs is under that role's root. A rule that restricts other roles,
  such as `gpui-component-kit`, stays in the global guide text once.
- The review text keeps a rule that `stomme check` covers only in part, and the rules table marks
  each such rule partial with the part it cannot check. A rule the check enforces exactly, such as
  the role folder names, stays out of the clause text.
- A project entry keeps its composition: its own text first, then the same guide text with the
  clauses. An entry with `language` inlines the general and that language's guides and no layer
  guides, so it gets only the clauses those guides produce.
- Rules that change findings stay inline. A file-valued rule saves no prompt space, because OCR
  loads it into the prompt, and a prose pointer depends on a read that the model may skip.
- The rule text for this repository measured 16,696 bytes before this change and 16,968 after,
  summing the `rule` strings of `docs/stomme/ocr-rules.json` with
  `jq '[.rules[].rule|length]|add'`.

### Project rules

A project may extend the rules in an optional `.opencodereview/rule.json`. It takes a subset of
OCR's format: `rules` entries with `path`, `rule`, and an optional `language`, plus `include` and
`exclude`. Other keys, `merge_system_rule` included, are refused. `language` is `rust` or
`typescript`, and any other value is refused.

- stomme sets `merge_system_rule: true` on every entry, so OCR keeps its built-in rule. Every entry
  takes the same value, so a project cannot set it.
- OCR uses the first matching rule and never combines rule files. A separate project file would
  hide one of the two, so `sync` merges the project's entries into the synced file: the project's
  entries first, each with its guide text, then the entry for the project's language.
- A rule with `language` covers files of another language, such as a TypeScript package in a Rust
  repository. Its text inlines the general guide and that language's guide, without layer guides:
  the layers follow the project's language, and the rule's files do not use them. A rule without
  `language` keeps the project's guides, layer guides included.
- OCR's default path filter skips test files, and the guides apply to tests too. So the synced
  `include` lists the project's own patterns, then the test paths that filter drops for the project
  language, then the test paths of every language a project rule names. Rust adds `**/*_test.rs`,
  `**/*_tests.rs`, `**/tests.rs`, `**/tests/**/*.rs`, and `**/benches/**/*.rs`. TypeScript adds
  `.test`, `.spec`, and `__tests__` files for every TypeScript and JavaScript extension. An
  `include` match only admits a file past that filter, so the list narrows nothing. It does not
  list the whole language glob, which would also admit build output, generated files, and test
  data that the guides do not govern.
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
- `stomme ocr-coverage` decides which files the review left unreviewed from the coverage record in
  OCR's JSON result, not from OCR's warnings. OCR exits 0 on a partial review that ran out of
  budget and nonzero on any other failure, so the workflow reads the exit code only to tell those
  two apart. A warning names its whole review group, not the files it left unreviewed.
- A selected file counts as reviewed when the record lists it as completed, or as reused from an
  earlier run of the same session. A failed or waived file counts as unreviewed. A file that the
  rules exclude was never selected, so it leaves no gap.
- A budget-only partial review resumes once. A provider failure is retried with the same provider
  and model, up to three attempts in total, because a provider error is a failure of the review
  service and says nothing about the change. A provider failure on the budget resume is retried the
  same way. OCR exits 0 for the budget partial and nonzero for every other failure, so the workflow
  resumes a budget partial only when OCR exits 0, the coverage command reports a partial review,
  and the result holds a session. The resume reviews only the unreviewed files. It uses the same
  range, rule file, provider, and model as the first run, and it passes no token budget, so the
  resumed run does not stop at the same budget again. The check fails when files stay unreviewed
  after the resume, because a pass would let a change merge with code that no one read.
- A provider failure that survives the retries fails the check with an error that names the provider
  as the cause and lists the files it left unreviewed, so a reader can tell an outage from a problem
  with the change.
- Only a result that `stomme ocr-coverage` classifies is retried, and only when every unreviewed file
  failed on the provider. A result the command rejects, a run failure, or another classification is
  reported at once: it needs a fix to the setup, so the workflow does not resume it. A failed run
  may keep the files it already recorded as reviewed.
- The workflow classifies a resumed result whenever it holds usable JSON, even when the resume
  exits nonzero, and then fails the check with its reason. A missing or unusable resumed result is
  a failed run: the check fails with that attempt's reason, the coverage stays the last usable one,
  and that result is not posted.
- The job passes only when the last classification reports every selected file as reviewed. A
  partial result fails and names the unreviewed files, and a failed run fails with its error.
- The resumed review posts its result once, including the comments of the first run for the files
  it reused. Its summary keeps the previous checkpoint marker, so the next push reviews from the
  old head: a wider range, never a wrong one.
- A push whose head the checkpoint already records is a no-op run: OCR reviews an empty range and
  writes nothing. The checkpoint moves on OCR's own completeness, which is a different measure from
  the coverage record: `stomme ocr-coverage` counts a selected file that the record does not list
  as reviewed, so a checkpointed head can hold files that the check counts as unreviewed. A rerun
  of that head therefore has no new evidence to offer. Instead the workflow reads the coverage
  section that this head's run inserted into the summary comment, copies that section to the top of
  the job summary, and passes only when the section reports every selected file as reviewed. A
  missing section, unreviewed files, or a failed run fails the check, and its error says that the
  same head cannot be reviewed again, so the change needs a new commit.
- The summary comment starts with the coverage section that the command prints. The workflow writes
  a hidden verdict line above it that names the run's classification and the head the section
  covers, so a later run reads the result and its head from the comment instead of its prose. A
  section that names another head is not evidence for this one. The section names each unreviewed
  file with OCR's reason, and it lists the excluded files in a collapsed block. The workflow puts
  the same section at the top of the job summary, and inserts it after the marker of the summary
  comment that this run posted, found by that post's URL, so the comment stays findable and keeps
  its single checkpoint marker. When the run posted no summary comment, the coverage goes only to
  the job summary.
- Findings stay advisory, as with the size and mutation signals in
  [ADR 0009 (Advisory mutation testing and size as a review signal)](../stomme/shared/adr/0009-advisory-review-signals.md).
- Every finding is posted as an inline comment, whatever its severity. The workflow sets no
  severity or category routing, and the resumed post passes none, because the posting helper routes
  nothing without a threshold. Routing would move a finding into the summary comment, away from the
  line it describes. A finding with no line, and an inline post that GitHub rejects, still land in
  the summary comment with the reason.
- Tau needs no review-bot entry, because OCR reviews every push without a request.

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
- A review that leaves files unreviewed after its resume blocks the merge until a new commit
  reviews them, because a rerun of the same head reviews nothing.
- A model that stops early while OCR records the file as completed passes the check, because the
  check trusts OCR's coverage record.
- Findings depend on a model, so the review does not replace a person's review.
- A file under two role roots matches two clauses, and a role root that is a symlink matches no
  clause for the real Git path, because the clause is a lexical prefix of the configured path.
  `stomme check` accepts both shapes today.

## Alternatives considered

### A copy of the workflow and rules in each project

Each project keeps its own OCR workflow and rule file. Rejected because the copies drift from the
guides, and each fix must be repeated in every project.

### One rule entry per role

Give each role its own entry, each with the role's clause. Rejected because OCR sends each distinct
rule text as its own block, so every entry repeats the shared guides: the plugga-like project above
measured 68,578 rule bytes with per-role entries against 14,546 with one entry and clauses. Revisit
this choice when OCR can share one base rule across entries, when it stops sending each distinct
rule text as its own block, or when the closed role list in ADR 0004 grows.

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

### Read the verdict from the classification command

Let `stomme ocr-coverage` print the hidden verdict line, so one program owns the classification.
Rejected because the workflow is live at merge while the classifier comes from the release pin that
`mise.toml` holds. A project, stomme included, would then run the new reader against an older
binary that prints no verdict and fail every no-op run until the next release. The workflow writes
the line from the classification it already reads, so the writer and the reader ship in one file.

### Route findings by severity to the summary comment

Move findings at or below a severity threshold into the summary comment. Rejected because a routed
finding loses the line it describes, so the reader has to find that line by hand.

### Read the result with a script in the workflow

Classify the result with `jq` in the workflow instead of the binary. Rejected because the
classification then has no tests against real result files, and a change in OCR's format breaks
the check only at run time.
