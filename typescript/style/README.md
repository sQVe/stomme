# Stickler

Stickler is a zero-config Oxlint house style for TypeScript projects. It runs through
[Vite Plus](https://viteplus.dev) on Node 24 and Bun 1.4.

It ships three parts:

- `lint`: the rule set for `defineConfig({ lint })`. Every rule in it is on.
- `format`: the Oxfmt settings for `defineConfig({ fmt })`.
- `stickler`: the command that runs lint with the house rules, and formats in fix mode.

## Install

```sh
pnpm add --save-dev --save-exact @sqve/stickler vite-plus@0.3.0
```

With Bun:

```sh
bun add --dev --exact @sqve/stickler vite-plus@0.3.0
```

Stickler pins the Vite Plus version it supports. Install that exact version.

## Use

Pass the configs to Vite Plus in `vite.config.ts`:

```ts
import { format, lint } from '@sqve/stickler';
import { defineConfig } from 'vite-plus';

export default defineConfig({ lint, fmt: format });
```

Add scripts to `package.json`:

```json
{
  "scripts": {
    "lint": "stickler",
    "lint:fix": "stickler --fix",
    "format": "vp fmt",
    "format:check": "vp fmt --check"
  }
}
```

`stickler [--fix] [paths...]` runs `vp lint --deny-warnings` with the house rules on. Warnings fail
the check. With `--fix` as the first argument, it fixes what it can, then runs `vp fmt` on the same
paths, even when some lint errors remain. Paths default to the whole project.

The command runs Vite Plus with the runtime that started it. Under Bun, run it with
`bun run --bun lint` so Vite Plus also runs on Bun.

### House rules

House rules check style: names, abbreviations, helper order, type placement, condition size,
comments that cite tickets or reviews, disable comments without a reason, and blank lines between
statements. They load only when `STICKLER_STYLE=1`, which the `stickler` command sets. Editors and
plain `vp lint` show the ordinary rules only.

### Presets

Add a preset with `extends`:

```ts
import { format, lint, react, vitest } from '@sqve/stickler';
import { defineConfig } from 'vite-plus';

export default defineConfig({
  lint: { extends: [lint, react, vitest] },
  fmt: format,
});
```

- `react`: `react/rules-of-hooks` and `react/exhaustive-deps`, plus the React plugin's correctness
  rules.
- `vitest`: Oxlint's native Vitest rules for test files.

There is no Bun test preset. Oxlint's test rules do not recognize imports from `bun:test`.

### Project settings

Settings that belong to one project, such as ignore patterns, go next to the shared config:

```ts
export default defineConfig({
  lint: { extends: [lint], ignorePatterns: ['generated/**'] },
  fmt: { ...format, ignorePatterns: ['pnpm-lock.yaml'] },
});
```

### Plugin

The `stickler` plugin is also exported on its own, as `@sqve/stickler/plugin`. Its rules are named
`stickler/<rule>`.

## Development

```sh
pnpm install
pnpm run check
```

`check` runs typechecking, the self-lint through `stickler`, the format check, the build, and all
tests. The end-to-end tests pack the package, install it into temporary projects with pnpm and Bun,
and run it on both runtimes. They need Bun on the path.

Decisions and their reasons are in [docs/adr](docs/adr/README.md).

## Release

Pushing a `v*` tag runs the release workflow. It runs the full check, builds, and publishes with
`npm publish --access public --provenance` through npm trusted publishing.

Trusted publishing needs the package to exist on npm first. Publish the first version by hand from a
clean checkout with `npm publish --access public`, then add this repository's release workflow as a
trusted publisher in the package settings on npmjs.com.

## License

[MIT](LICENSE)
