# stomme

A shared foundation for project structure, coding rules, and documentation.

stomme will provide versioned rules, starter files, and a Rust command-line tool to keep
projects in sync. Each project keeps its own modules and decisions. Drift from the shared rules
will fail the project's full check.

stomme is in early development. `stomme check` checks the project in the current directory: its
`stomme.toml`, its required files, and its ADR index.

Install a released binary with mise:

```sh
mise use -g github:sQVe/stomme
```

Or install through cargo-binstall:

```sh
cargo binstall --git https://github.com/sQVe/stomme stomme
```

## Use stomme in a project

Pin the binary in `mise.toml`:

```toml
[tools]
"github:sQVe/stomme" = "0.2.0"
```

Set the same version in `stomme.toml`:

```toml
version = "0.2.0"
```

Extend the Renovate preset in `renovate.json`:

```json
{
  "extends": ["github>sQVe/stomme//renovate/stomme"]
}
```

Add a workflow that syncs the update branch:

```yaml
name: Sync stomme

on:
  push:
    branches: [renovate/stomme]

permissions:
  contents: read

jobs:
  sync:
    uses: sQVe/stomme/.github/workflows/sync-update.yml@v0.2.0
    secrets:
      app-id: ${{ secrets.STOMME_APP_ID }}
      app-private-key: ${{ secrets.STOMME_APP_PRIVATE_KEY }}
```

The TypeScript style package in [packages/style](packages/style/README.md) is published as
`@sqve/stomme-ts-style` with its own version.

See [the documentation](docs/README.md) for the development guide and architecture decisions.
