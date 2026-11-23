# ADR 0016: Pin the sync workflow by release commit

**Date**: 2026-11-23\
**Status**: Accepted\
**Related**:
[ADR 0012 (Release the binary through GitHub Releases)](./0012-release-train.md),
[ADR 0014 (Compose new projects from owned starter files)](./0014-starter-files.md)

## Context

A project's sync workflow calls a reusable workflow in the stomme repository by tag, as in
`sQVe/stomme/.github/workflows/sync-update.yml@v0.2.3`. The workflow runs `stomme sync` with an app
token, so it writes to the project. A project's OCR caller calls a second reusable workflow the same
way, as in `sQVe/stomme/.github/workflows/ocr-review.yml@v0.2.3`, and reviews every pull request
push.

A tag is a moving name. Anyone who can push a tag in stomme, or a compromised account, can point
`v0.2.3` at different code, and every project then runs that code on its next sync or review. The
project's review sees no change, because its own file still says `v0.2.3`.

`stomme sync` writes the shared rules and guides into the project. It must not rewrite the workflows
that call it, or every sync would produce a diff on a file the project owns.

## Decision

Both calls are pinned by the commit of the release tag, with the version as a comment:
`uses: sQVe/stomme/.github/workflows/sync-update.yml@<40-hex commit> # v0.2.3` and
`uses: sQVe/stomme/.github/workflows/ocr-review.yml@<40-hex commit> # v0.2.3`.

### Where the commit comes from

`stomme init` writes the calls. A release build embeds the commit of the tag it is built from,
taken from `GITHUB_SHA` when `GITHUB_REF` is `refs/tags/v<version>`. The build script, not the
process environment, decides this, so an exported value cannot embed an arbitrary commit.

A binary built another way, such as from a checkout, reads `STOMME_RELEASE_COMMIT`. `stomme init`
refuses without either, because a wrong or empty commit would write a call that does not resolve.

### Keeping the pin current

A pin would otherwise stay on an old commit forever. The shared Renovate preset finds both forms,
the tag and the commit with its comment, for each reusable workflow, and rewrites a tag call to the
commit form. It matches only the unquoted `uses:` form that `stomme init` writes, because Renovate's
replacement template cannot carry a quote into its output. A hand-quoted call stays unchanged
rather than being corrupted. The preset resolves the commit through the `github-releases`
datasource, which looks up the commit of the exact tag name, so the version keeps its `v` prefix. A
`packageRules` entry with `pinDigests` adds the commit to a tag call that has none, for workflow
files only.

### The release gate

Every release runs the candidate binary's `stomme init` in an empty folder and requires each written
workflow, sync and OCR, to equal its common skeleton with `{{commit}}` and `{{version}}` replaced. A
binary that embeds the wrong commit, or writes any other difference, fails before the release is
hosted.

## Consequences

### Positive

- A project runs the code of the commit it reviewed, not whatever the tag names later.
- The comment keeps the version readable in each workflow file.
- The preset keeps each pin current for the unquoted `uses:` form on the project's next stomme
  update, so that form needs no hand edit.

### Negative

- The commit is not readable as a version, so the comment must stay next to it.
- A hand-written workflow must look up the commit, because 0.2.3 was not tagged when this decision
  was recorded and no real commit is shown in the README.
- A release build outside the tag context must set `STOMME_RELEASE_COMMIT`, or `stomme init`
  refuses.

## Alternatives considered

### Pin by tag

Keep `@v0.2.3` and rely on the tag. Rejected because the tag can be moved after a project reviewed
it, and the project's file does not change when it is.

### Look up the tag in the build script

Run `git` in `build.rs` to resolve `v<version>` to a commit. Rejected because the release build is
not a full clone with tags, and a build script that needs the network or a Git repository fails for
a consumer building from a source archive.

### Check in a commit one release behind

Write the previous release's commit into the starter file and update it at each release. Rejected
because the written pin would name code that is not the version the project installs, and a missed
update leaves it further behind with no check.

### Resolve the commit with `git ls-remote` at init time

Ask GitHub for the tag's commit when `stomme init` runs. Rejected because it needs network access
and a repository URL, and the result would depend on the tag at that moment instead of the release
the binary came from.
