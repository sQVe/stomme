# ADR 0014: Compose new projects from owned starter files

**Date**: 2026-11-17\
**Status**: Accepted\
**Related**: [ADR 0002 (One versioned foundation for every project)](./0002-purpose-and-design.md),
[ADR 0005 (Separate languages, framework layers, and roles)](./0005-languages-and-layers.md)

## Context

A new project needs its required parts before `stomme check` passes: `stomme.toml`, `AGENTS.md`,
the ADR index, build files, and a full check. Projects combine one language, any set of layers,
and any set of roles, as [ADR 0005](./0005-languages-and-layers.md) says. A template per
combination would grow with the combinations, not with the parts each choice owns.

Several choices add to the same file. The language and a layer both add to `package.json` or
`clippy.toml`. ADR 0005 forbids an order that changes meaning, so a later source must not
silently replace what an earlier one set. A starter must also never overwrite a person's work.

## Decision

`stomme init` composes a project from embedded starter files, with one source for the common base,
one for the language, one for each selected layer, and one for each selected role.

### Skeleton layout

Each source owns one `skeleton/` folder next to the shared material it belongs to:

- `foundation/common/skeleton/` for files every project needs.
- `foundation/languages/<language>/skeleton/` for the language base.
- `foundation/languages/<language>/roles/<role>/skeleton/` for a role.
- `foundation/layers/<layer>/skeleton/` for a layer.

A skeleton holds `files/`, whose files keep their project-relative path, and `merge/`, whose files
are fragments of a shared target at the same path. `{{name}}` and `{{version}}` are replaced in
every path and text. The binary embeds each file, so a release carries its starters as it carries
its synced text.

A layer that needs set-up steps puts them in `docs/<layer>.md` in its skeleton. The language's
`docs/development.md` links that path pattern, because whole-file ownership stops a layer from
adding a step to the language's file.

### Ownership and merge rules

- A whole file has one owner. Two sources that both own a path are an error, even with equal
  bytes, so no source relies on another source's copy.
- A merge target takes fragments from any number of sources, but no source may also own it as a
  whole file. Only JSON and TOML targets merge.
- Objects merge key by key. Arrays keep every distinct item in first-seen order. Equal values
  merge. Two different values at one key are an error that names both owners and the dotted key.
- Sources apply in a fixed order: common, language, layers by name, then roles by name. Merged
  keys are written sorted, so the selection order on the command line never changes the output.
- `stomme.toml` is generated from the request, not copied from a skeleton.
- After writing, `init` runs `sync`, so `docs/stomme/`, the record, and the managed `AGENTS.md`
  block come from the same code as every later update. The common skeleton provides `AGENTS.md`,
  because `sync` writes the block only into an existing file.

### Roles are Rust only

Rust has a starter for each role, at `crates/<name>-<role>`. TypeScript projects start as one
package and have no role starters, so a TypeScript role is an error.

### Empty folders only

`init` writes only into an empty folder; an existing `.git` is allowed. It validates the request
and plans every file before the first write, so an invalid request writes nothing.

### One full check per language

Each language base names one full check in `.pi/tau.json`: `cargo xtask check` for Rust and
`pnpm run check` for TypeScript. The starter's check runs `stomme check`, so a new project fails
on drift from its first commit. The TypeScript check runs every `check:layer:<layer>` script after
the tests and before `stomme check`. A layer adds a check this way because merge refuses a second
value for `scripts.check`, so no layer can extend the language's check script itself. Each layer
uses its own key, such as `check:layer:herdr`, and pnpm runs every matching script, fails when one
fails, and passes when none match.

## Consequences

### Positive

- A new language, layer, or role adds one skeleton, not a template for each combination.
- A clash between two choices fails before any write and names both owners.
- A new project passes `stomme check` and its full check without hand edits.

### Negative

- Only JSON and TOML files can take fragments. Other shared files need one owner.
- Merged JSON and TOML lose comments and key order, because they are parsed and rewritten.
- `init` cannot add a layer or role to an existing project; that change is made by hand.
- TypeScript roles need a later decision on their folders.

## Alternatives considered

### One template per combination

Keep a complete project template for each language, layer, and role combination. Rejected because
the templates grow with the combinations and copy the same files many times.

### Later sources override earlier ones

Let a later source replace a whole file or a value an earlier source set. Rejected because order
would change meaning, which [ADR 0005](./0005-languages-and-layers.md) rejects for rules.

### Init into an existing project

Merge the starters into a folder that already has files. Rejected because `init` would have to
decide which of a person's edits to keep. Adopting an existing project stays a manual step.
