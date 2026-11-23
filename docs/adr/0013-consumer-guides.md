# ADR 0013: Publish a pinned guide bundle for consumers without a project

**Date**: 2026-11-17\
**Status**: Accepted\
**Related**: [ADR 0002 (One versioned foundation for every project)](./0002-purpose-and-design.md)

## Context

An agent that works in many projects needs stomme's coding guides even when the project has no
`stomme.toml`. ADR 0002 says a consumer uses its bundled guides only when stomme is absent, not
broken, but it does not say how a consumer obtains the guides, or how a bundle states which release
it came from. Only `stomme sync` writes the guides today, and it refuses without a project.

A guide is not a standalone file. The general guide links into `shared/adr/`, and every layer guide
links to `shared/rules.md`. A copy of the guides alone leaves those links broken. A consumer must
also not edit synced text, because a check compares it with the release.

The ownership record is internal today. It holds the stomme version and a hash per synced path, but
it has no format version, and its keys are relative to the project root.

## Decision

`stomme guides <dir>` writes the guides of one stomme release and their record to `<dir>`, and it
needs no `stomme.toml`.

### Channel

The binary writes the bundle because it already embeds every guide with `include_str!`. The output
is the source of truth with no second copy to keep equal. The command is idempotent, and it refuses
to replace a file that differs from the bundle's record.

The guides are not an npm package. A consumer that builds from source commits the bundle beside its
own code, the same way a project commits `docs/stomme/`. A package can run the command in its pack
step if a consumer ever needs one.

### Contents

The bundle holds the full synced set, minus the managed `AGENTS.md` block and the generated
`docs/stomme/ocr-rules.json`: every shared file, every language guide, and every layer guide, in
the `docs/stomme/` layout. A consumer without a project has no language or layer selection, so the
bundle holds every guide. It omits the OCR rule file, whose text depends on a project's language
and layer selection. The links inside the text then resolve without a rewrite.

### Record

`.sync-record.toml` is public API, and a consumer may read it. It holds `schema`, the record
format's version, whose current value is `1`; `version`, the stomme release; an optional `agents`
hash for the managed block; and `[files]`, a SHA-256 for each bundled file. A file's key is its path
from the synced root, so a project and a bundle use one shape. A consumer record holds only the
files it bundles, so it may omit `agents`.

### Fallback

Only a missing `stomme.toml` permits the bundle. A project with a broken configuration, unreadable
synced guides, a record that does not match its files, or a dangling link is broken, and a consumer
reports the error instead of falling back. A consumer compares a bundle with its own record, not
with the project's version, because a project may pin another release.

### Version

The bundle declares the version of the binary that wrote it, and it ships with that binary's
release. One number names the binary and the guides it embeds.

## Consequences

### Positive

- A consumer gets guides for one known release without a project or a second release pipeline.
- The bundle's links resolve, because it holds the files the guides link to.
- One record shape and one reader cover a project and a bundle.

### Negative

- A consumer's bundle goes stale until someone runs the command again.
- The record becomes a public format, so its fields need a version on every change.
- The bundle carries every language and layer guide, including guides a consumer does not load.

## Alternatives considered

### Publish an npm package of the guides

Ship the release's guides as a package, like `@sqve/stomme-ts-style`. Rejected because it adds a
second copy of the guides, a publish job, and an identity test, and because a consumer that installs
from Git without devDependencies would still copy the files into its own tree.

### Keep a separate bundle record

Give the bundle its own record file, with keys from the bundle root, and keep the project record
internal. Rejected because two shapes need two readers and two contracts for the same hashes.

### Bundle only the coding guides

Ship `shared/coding.md` and the language and layer guides, without the shared ADRs and rules table.
Rejected because the links inside those guides would break, and a consumer must not edit synced text
to repair them.
