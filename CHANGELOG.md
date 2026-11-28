# Changelog

## 0.2.6

- `stomme` 0.2.6.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/stomme-ts-imports` 0.2.6.
- `@sqve/mutation` 0.2.0.

Changes:

- Add `stomme ocr-scope --reviewed <ref> --reviewed-base <ref> --base <ref> --head <ref>`, which
  builds the review base since the last complete Open Code Review run and grafts it onto the head,
  so a force push or a rebase leaves only the new lines in scope.
- Normalize a module entry and public path when the config is read, so `./src/a.rs` and
  `src//a.rs` reach the module check instead of matching no file and passing silently.
- Reject an invalid dependency, lock, and Clippy config value, and follow the documented precedence
  between the config files.
- Count a package in `optionalDependencies` as a layer dependency, as for the other dependency
  fields.
- Name `.clippy.toml` first as the effective Clippy config, and describe the runtime search from the
  crate manifest directory or `CLIPPY_CONF_DIR`.
- Accept an acronym stem as PascalCase, so `ReportUI` sits in the family of `Report`.
- Reject a TOML date or timestamp in place of a Cargo dependency, which Cargo refuses when a
  package inherits it.
- Read a fence after the list markers of a line and inside a block quote, so a fenced link on a
  list marker line is not an ADR index entry and its sample is not an invalid entry.
- Read the ADR index section and the retired table through the shared scanner, so a heading or a
  table row inside a fence or an HTML comment no longer ends the section or retires a number. The
  check also reads the index lines once and hands them to both readers.
- Report an ADR file-name problem only for a file, and read the ADR heading only from a line that is
  a heading, so a folder named like an ADR and a `#hashtag` line above the title are not problems.
- Start the Release workflow only on a `v` tag, so a Changesets package tag such as
  `@sqve/stomme-ts-imports@0.2.6` no longer starts a run whose plan job cannot parse its version.
- Read a trigger form the release tag guard used to skip, such as a `tags-ignore:` filter or a
  commented trigger key, as a tag push, and fail on a trigger line it cannot read, so the guard
  rejects every workflow that can start on a tag outside the `v` family.
- Run the sync workflow's branch code without the app token, and push from a separate job that
  reads only the bundle entry from the artifact, so a push by another actor cannot use the token.
- Add timeouts, concurrency groups, and least-privilege tokens to the workflows.

## 0.2.5

- `stomme` 0.2.5.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/stomme-ts-imports` 0.2.5.
- `@sqve/mutation` 0.2.0.

Changes:

- Leave a synced file alone when its content would not change, so `stomme sync` no longer replaces
  `docs/stomme/.sync-record.toml` on every run and a file watcher sees no change.
- Read the current directory only to resolve a relative `stomme guides` destination, so an absolute
  destination works in a folder that has been deleted.
- Lock the folder a run writes for its whole plan and write, so two syncs on one folder wait for
  each other instead of removing a staging file the other run is about to publish.
- Fail the sync workflow when Git ignores a file the guides refresh wrote, instead of committing an
  incomplete folder.
- End the OCR coverage section only at a marker on a line of its own, so a path that quotes the
  marker cannot end the section early and leave its tail after the new section.
- Retry a provider failure in the OCR check before the check fails. A retry needs every unreviewed
  file to have failed on the provider, and a failure that survives the retries names the provider
  and the files it left unreviewed.
- Read the ADR index and its fences from the content column of each open list item, so a link that
  Markdown renders as indented code is not an entry and a fenced sample is not a heading.
- Follow a glob in a private file, a glob of the crate root or a parent, and a glob module beside a
  value export of the same name in `module-private-file`.
- Add the rule `weakened-test`, which compares the working tree with the target branch and reports
  an added test skip, an added suppression, and a deleted test case.
- Add one clause to every Open Code Review rule entry for each role the project declares, built
  from the role duty blocks in its ADRs and from role-scoped layer rules.
- Match a layout test with an owner of either TypeScript extension, so a `.test.ts` file may sit
  beside a `.tsx` owner.
- Add the rule `workflow-pin`, which refuses a stomme reusable workflow call that pins no release
  commit, and move a project's workflow calls to the release commit in the release gate.

## 0.2.4

- `stomme` 0.2.4.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/stomme-ts-imports` 0.2.4.
- `@sqve/mutation` 0.2.0.

Changes:

- Read arguments as OS strings, so `stomme ocr-coverage <path>` and `stomme guides <folder>` accept
  a path that is not valid UTF-8. A non-text argument is refused with a `stomme:` error that names
  the rejected bytes.
- Escape a path before `stomme ocr-coverage` prints it, so a path with a line break or a backslash
  stays one readable value in the coverage fragment, the excluded block, and the `stomme:` lines.
- Report coverage on a same-head no-op run. The OCR workflow reads the coverage section of that
  head's summary comment and passes only when it reports every selected file as reviewed.
- Post every OCR finding as an inline comment, whatever its severity. The workflow no longer routes
  a low-severity finding into the summary comment. A finding with no line, or an inline post GitHub
  rejects, still lands in the summary comment with the reason.
- Review the TypeScript packages under `packages/` against the TypeScript guide, tests included. A
  project rule that names a language inlines the general guide and that language's guide.
- Write a synced file through a staging file and a rename instead of truncating it in place, so a
  hard link to the file keeps its own contents.

## 0.2.3

- `stomme` 0.2.3.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/stomme-ts-imports` 0.2.3.
- `@sqve/mutation` 0.2.0.

Changes:

- Take an optional `guides-directory` input in the reusable sync workflow. When it is set, the job
  refreshes the shared guides in that folder and commits them with the rest of the sync.
- `stomme init` pins the reusable sync workflow by commit with a version comment. The Renovate
  preset keeps the pin current for the unquoted `uses:` form by converting an existing `@vX.Y.Z`
  call on the project's next stomme update. `stomme sync` does not change the file.
- `stomme sync` writes `docs/stomme/ocr-rules.json`, which composes the selected guides and the
  project's own `.opencodereview/rule.json` into one Open Code Review rule file, and `stomme check`
  reports the file when it is stale or the project file is invalid.
- `stomme ocr-coverage` classifies an Open Code Review result file and fails when the review left
  files unreviewed.
- stomme ships a reusable workflow that reviews a pull request with Open Code Review and leaves a
  summary comment that starts with the review coverage. `stomme init` writes the caller workflow,
  which calls it by release commit, and the release-pin gate checks both callers.
- The Renovate preset keeps both reusable workflow calls current in the `stomme` group.

## 0.2.2

- `stomme` 0.2.2.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/stomme-ts-imports` 0.2.2.
- `@sqve/mutation` 0.2.0.

Changes:

- Follow glob re-exports and crate root and parent aliases when `stomme check` resolves a Rust
  module path for `module-private-file`.
- Make `@sqve/stomme-ts-imports` public, with the same publish config as the style package. Its
  version equals the binary's, and the release script checks it before publishing.
- Say in the rules table that the TypeScript module table is recorded but its rules are not checked
  yet.

## 0.2.1

- `stomme` 0.2.1.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/mutation` 0.2.0.

Changes:

- Add `stomme init` to create a project that passes `stomme check` and its own full check.
- Fix the Herdr starter: its mod now passes Herdr's load check, types, and build, and the project
  has a command that installs the mod and docks it on the right.
- Tell new developers in the starter guides to trust `mise.toml` before `mise install`.
- Compare a pull request with its target branch in `stomme check`, not always with `main`.
- Refuse a `stomme.toml` that is not a regular file.
- Add the rule `adr-file-name`: reject an ADR file with no slug, and decide the ADR index shapes.
- Reject in `stomme sync` a relative `.md` guide link whose `#anchor` matches no heading, and test
  each layer's guide links against the smallest setup that sync accepts.
- Report `module-private-file` in `stomme check` when a Rust path reaches a private file through an
  alias of a whole module, such as `pub use self::attention as renamed;`.
- Exempt only code that compiles under `cfg(test)` from `module-private-file`, found by
  `#[cfg(test)]` attributes. A production module named `tests` is no longer exempt.
- Accept every `stomme.toml` version pin the CLI accepts in the release gate, including a trailing
  comment.
- Fail the release gate on a broken `stomme.toml` link instead of skipping the project.
- Give the release gate token read-only access to contents.
- Write a bare version, such as `0.2.1`, into `mise.toml` from the Renovate preset.

## 0.2.0

- `stomme` 0.2.0.
- `@sqve/stomme-ts-style` 0.4.0.
- `@sqve/mutation` 0.2.0.

Changes:

- Add `stomme guides` to write the shared guides into a directory for a consumer that has no
  `stomme.toml`.
- Turn off `react/react-in-jsx-scope` in the React preset, because the automatic JSX runtime puts
  no `React` in scope.
- Open the release pull request with a GitHub App token, so GitHub starts its required checks.

## 0.1.0

- `stomme` 0.1.0.
- `@sqve/stomme-ts-style` 0.3.0.
- `@sqve/mutation` 0.2.0.
