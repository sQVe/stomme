# ADR 0015: Generate Herdr declarations from a pinned herdr-app

**Date**: 2026-11-18\
**Status**: Accepted\
**Related**:
[ADR 0005 (Separate languages, framework layers, and roles)](./0005-languages-and-layers.md),
[ADR 0014 (Compose new projects from owned starter files)](./0014-starter-files.md)

## Context

A Herdr mod imports host modules such as `gpui-kit`. TypeScript needs type declarations for them.
`herdr-app` writes those declarations into the mod folder.

The declarations are large and are the same for one Herdr version. Their own header says to
gitignore them. `herdr-app check` rewrites them in the mod folder. A committed copy broke the
project's lint and format check (TS2499).

The declarations and the build must match the `herdr-app` that loads the mod. A project therefore
needs one agreed Herdr version, and every machine that runs the checks needs that version.

## Decision

The herdr layer pins one `herdr-app` version and generates Herdr's type declarations from it on
`pnpm install`; projects never commit them.

### The pin

- The pin lives once, in the herdr layer's `mise.toml` fragment, as an `http:herdr-app` tool with a
  checksum per Linux platform. stomme's CI reads the version from that fragment and repeats no
  version or URL.
- The pin covers Linux only. Herdr's macOS release is a signed `Herdr.app` bundle whose binary is
  `Herdr.app/Contents/MacOS/Herdr`, not a `herdr-app` executable. On macOS the person installs the
  app with Herdr's installer. The installer links `~/.local/bin/herdr-app` to the app's binary, the
  app repairs that link on launch, and Herdr's own guide says to keep signed bundles unchanged.

### The declarations

- `postinstall` runs `herdr-app types mods/<name>`.
- Git, lint, and format ignore `mods/**/gpui-kit.d.ts`.
- A `tsconfig.json` in the mod folder stops Herdr from writing a `jsconfig.json`.

### The externals list

The build's `--external` list is written out by hand. A test compares it with the `declare module`
names in the generated declarations, because Herdr has no command that lists its host modules.

## Consequences

### Positive

- A fresh clone typechecks after `pnpm install`.
- On Linux, the declarations always match the pinned Herdr.

### Negative

- `pnpm install` needs `herdr-app` on PATH.
- macOS has no version pin, so the installed app's version may differ from the pin.
- A Herdr upgrade means editing the pin, its checksums, and possibly the externals list.

## Alternatives considered

### Commit a copy of the declarations

Keep the generated file in Git. Rejected because the file says not to, `herdr-app check` overwrites
it, it is large, and lint and format must ignore it anyway.

### An explicit `pnpm run types` script

Generate the declarations with a script that a person runs, not on `postinstall`. Rejected because
a fresh clone fails typecheck until someone runs a step that nothing enforces.

### Install `herdr-app` only in CI with a pinned download

Pin the download in the CI workflow and leave local machines alone. Rejected because local runs
would use any `herdr-app` on PATH, and the version would live in two places.

### Pin the macOS app through mise too

Download the macOS release through mise as well. Rejected because the macOS release is a signed app
bundle without a `herdr-app` executable, and changing the bundle breaks its signature.

### Mark every package external

Pass `--packages external` to the build instead of a list. Rejected because npm dependencies would
stay unbundled, and Herdr resolves imports only inside the mod folder.
