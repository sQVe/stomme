# ADR 0005: Separate languages, framework layers, and roles

**Date**: 2026-11-14\
**Status**: Accepted\
**Related**: [ADR 0002 (One versioned foundation for every project)](./0002-purpose-and-design.md)

## Context

Language, framework, and application shape are independent choices. A project can combine
frameworks or provide both an app and a CLI. Treating each combination as a separate category
would make the shared foundation grow with combinations rather than with the rules it owns.

Combined rules must have a stable meaning. Configuration order must not weaken a rule or hide an
incompatible framework choice.

## Decision

Each project declares one language, an unordered set of framework layers, and structural roles.

### Independent choices

The language is `rust` or `typescript`. The first layers are `gpui`, `gpui-component`, `react`,
`pi`, and `herdr`. Native `gpui` and `gpui-component` belong to Rust; `react`, `pi`, and `herdr`
belong to TypeScript. Herdr's TypeScript `gpui-kit` bridge belongs to `herdr`, not native `gpui`,
so a shared framework name does not blur the language boundary.

There is no shape field. The `[roles]` map assigns one path per role and says whether a project
has an `app`, a `cli`, both, or neither. This avoids a second declaration of the same choice.

### Layer ownership

A layer owns one folder, `layers/<name>/`, containing its manifest and, later, its guide and
starter files. Its manifest declares `language`, `requires`, and `conflicts`. One Rust module
embeds the folder, and one registry entry registers the layer. This keeps a layer's shared
material together and makes its inclusion in the distributed binary explicit.

Layers only add or narrow rules. Each rule has one owner and one ID. A rule that needs two parts
still has one named owner, so combining layers cannot create competing definitions of that rule.

### Explicit compatibility

Conflicts count in both directions, even if only one layer declares the conflict. Missing
required layers are errors; stomme never adds them silently. For example, `gpui-component`
requires an explicit selection of `gpui`. This keeps the project's selection under its owner's
control.

A wrong language, a conflicting pair, or an unknown layer is a broken installation, not an
absent one. Rejecting these choices prevents fallback behavior from hiding invalid shared rules.
Layer order never changes meaning.

## Consequences

### Positive

- Projects can combine frameworks without a new category for every combination.
- Roles describe application shape without duplicating configuration.
- Rule ownership and compatibility remain explicit when layers are combined.

### Negative

- Project owners must list required layers themselves.
- Layers cannot override shared rules to accommodate incompatible choices.
- Each new layer needs both its owned folder and explicit registration in the binary.

## Alternatives considered

### One kind per project

Give each language, framework, and shape combination one kind. Rejected because this mixes
independent choices and requires a new kind for each new combination.

### Ordered layers with last-writer-wins rules

Let later layers override earlier rules. Rejected because order would change meaning and could
silently weaken shared constraints.
