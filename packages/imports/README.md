# stomme-ts-imports

stomme-ts-imports reads the module table in `stomme.toml` and checks TypeScript imports against it.
It runs through [Vite Plus](https://viteplus.dev) on Node 24 and Bun 1.4.

The package is private and not published yet.

## Module table

`stomme.toml` declares each module with an `entry`, the files it makes `public`, and the modules it
`imports`:

```toml
schema = 1
version = "0.1.0"
language = "typescript"

[modules.errors]
entry = "src/errors.ts"

[modules.config]
entry = "src/config/config.ts"
public = ["src/config/models.ts"]
imports = ["errors"]
```

Paths are relative to the folder that holds `stomme.toml`.

## Development

From the repository root:

```sh
pnpm install
pnpm --filter @sqve/stomme-ts-imports run check
```

`check` runs typechecking, the self-lint through `stomme-ts-style`, the format check, the build, and
all tests.

Decisions and their reasons are in [docs/adr](../../docs/adr/README.md).

## License

[MIT](LICENSE).
