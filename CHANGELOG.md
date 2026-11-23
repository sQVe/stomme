# Changelog

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
