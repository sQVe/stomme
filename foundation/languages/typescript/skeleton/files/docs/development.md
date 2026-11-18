# Development

## Set up

1. Install [mise](https://mise.jdx.dev).
2. Run `mise trust mise.toml` after you open `mise.toml` and check the tools it pins. The command
   marks the file as trusted, so mise reads it.
3. Run `mise install`. It installs the `stomme` binary, Node, and pnpm that `mise.toml` pins.
4. For each layer in the `layers` list of `stomme.toml`, follow the steps in `docs/<layer>.md` when
   that file exists. A layer can need tools that `mise install` does not install on every system,
   and `pnpm install` can fail without them.
5. Run `pnpm install`.
6. On the first run, commit the `pnpm-lock.yaml` that `pnpm install` writes. The CI workflow runs
   `pnpm install` because a new project has no lockfile. Once the lockfile is committed, CI can use
   `pnpm install --frozen-lockfile` and the `cache: pnpm` option of `actions/setup-node`.

## Run the full check

Run the full check before you finish a change:

```sh
pnpm run check
```

The check runs these steps in order and stops at the first step that fails:

1. `tsc --noEmit`
2. `stomme-ts-style`
3. `vp fmt --check`
4. `vp test`
5. `pnpm run --if-present "/^check:layer:/"`: the checks that layers add as `check:layer:<layer>`
   scripts
6. `stomme check`

Fix style and format problems with `pnpm run style:fix`.
