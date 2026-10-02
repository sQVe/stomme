# Seam

Seam is a zero-config Oxlint house style for TypeScript projects. It runs through
[Vite Plus](https://viteplus.dev) on Node 24 and Bun 1.4.

It ships three parts:

- `lint`: the rule set for `defineConfig({ lint })`. Every rule in it is on.
- `format`: the Oxfmt settings for `defineConfig({ fmt })`.
- `seam`: the command that runs lint with the house rules, and formats in fix mode.

## Install

```sh
pnpm add --save-dev --save-exact @sqve/seam vite-plus@0.3.0
```

With Bun:

```sh
bun add --dev --exact @sqve/seam vite-plus@0.3.0
```

Seam pins the Vite Plus version it supports. Install that exact version.

## Use

Pass the configs to Vite Plus in `vite.config.ts`:

```ts
import { format, lint } from '@sqve/seam';
import { defineConfig } from 'vite-plus';

export default defineConfig({ lint, fmt: format });
```

Add scripts to `package.json`:

```json
{
  "scripts": {
    "lint": "seam",
    "lint:fix": "seam --fix",
    "format": "vp fmt",
    "format:check": "vp fmt --check"
  }
}
```

`seam [--fix] [paths...]` runs `vp lint --deny-warnings` with the house rules on. Warnings fail the
check. With `--fix` as the first argument, it fixes what it can, then runs `vp fmt` on the same
paths, even when some lint errors remain. Paths default to the whole project.

The command runs Vite Plus with the runtime that started it. Under Bun, run it with
`bun run --bun lint` so Vite Plus also runs on Bun.

### House rules

House rules check style: names, abbreviations, helper order, type placement, condition size,
comments that cite tickets or reviews, disable comments without a reason, and blank lines between
statements. Rules ported from [anti-slop](https://github.com/dmmulroy/anti-slop) reject object
parameters, accumulator copies in `reduce`, `Reflect.apply` and `Reflect.get`, type aliases that
hide `unknown`, and widen-then-assert casts. Outside tests, a type assertion needs a `SAFETY:`
comment. They load only when `SEAM_STYLE=1`, which the `seam` command sets. Editors and plain
`vp lint` show the ordinary rules only.

### Presets

Add a preset with `extends`:

```ts
import { format, lint, react, vitest } from '@sqve/seam';
import { defineConfig } from 'vite-plus';

export default defineConfig({
  lint: { extends: [lint, react, vitest] },
  fmt: format,
});
```

- `react`: `react/rules-of-hooks` and `react/exhaustive-deps`, plus the React plugin's correctness
  rules.
- `vitest`: Oxlint's native Vitest rules for test files, plus `vitest/prefer-import-in-mock`.

There is no Bun test preset. Oxlint's test rules do not recognize imports from `bun:test`.

### Project settings

Settings that belong to one project, such as ignore patterns, go next to the shared config:

```ts
export default defineConfig({
  lint: { extends: [lint], ignorePatterns: ['generated/**'] },
  fmt: { ...format, ignorePatterns: ['pnpm-lock.yaml'] },
});
```

A project can add its own JS plugin next to Seam's. The plugin path is relative to `vite.config.ts`,
and its rules run in plain `vp lint` and through `seam`:

```ts
export default defineConfig({
  lint: {
    extends: [lint],
    jsPlugins: ['./lint/localPlugin.js'],
    rules: { 'local/no-forbidden-name': 'error' },
  },
});
```

### Plugin

The `seam` plugin is also exported on its own, as `@sqve/seam/plugin`. Its rules are named
`seam/<rule>`.

## Development

```sh
pnpm install
pnpm run check
```

`check` runs typechecking, the self-lint through `seam`, the format check, the build, and all tests.
The end-to-end tests pack the package, install it into temporary projects with pnpm and Bun, and run
it on both runtimes. They need Bun on the path.

Decisions and their reasons are in [docs/adr](docs/adr/README.md).

## Release

Pushing a `v*` tag runs the release workflow. It runs the full check, builds, and publishes with
`npm publish --access public --provenance` through npm trusted publishing.

Trusted publishing needs the package to exist on npm first. Publish the first version by hand from a
clean checkout with `npm publish --access public`, then add this repository's release workflow as a
trusted publisher in the package settings on npmjs.com.

## License

[MIT](LICENSE). Notices for ported code are in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
