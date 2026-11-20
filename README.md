# stomme

A shared foundation for project structure, coding rules, and documentation.

stomme will provide versioned rules, starter files, and a Rust command-line tool to keep
projects in sync. Each project keeps its own modules and decisions. Drift from the shared rules
will fail the project's full check.

stomme is in early development. `stomme check` checks the project in the current directory: its
`stomme.toml`, its required files, and its ADR index. `stomme init` creates a project in an empty
directory:

```sh
stomme init --name demo --language rust --layers gpui --roles core,cli
```

`--layers` and `--roles` take comma-separated lists. An invalid request writes nothing.

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
"github:sQVe/stomme" = "0.2.3"
```

Set the same version in `stomme.toml`:

```toml
version = "0.2.3"
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
    uses: sQVe/stomme/.github/workflows/sync-update.yml@v0.2.3
    secrets:
      app-id: ${{ secrets.STOMME_APP_ID }}
      app-private-key: ${{ secrets.STOMME_APP_PRIVATE_KEY }}
```

The workflow takes an optional `guides-directory` input. Leave it unset for the behavior above. When
it is set, the job runs `stomme guides <directory>` after `stomme sync` and commits that folder with
the rest of the sync, so the folder holds the current release's shared guides. The folder must be
inside the repository, and Git must be able to stage its files, so the folder must not be ignored.
It must not be inside another Git repository. It must not be `docs/stomme` or a folder inside it:

```yaml
name: Sync stomme

on:
  push:
    branches: [renovate/stomme]

permissions:
  contents: read

jobs:
  sync:
    uses: sQVe/stomme/.github/workflows/sync-update.yml@v0.2.3
    with:
      guides-directory: docs/guides
    secrets:
      app-id: ${{ secrets.STOMME_APP_ID }}
      app-private-key: ${{ secrets.STOMME_APP_PRIVATE_KEY }}
```

The TypeScript style package in [packages/style](packages/style/README.md) is published as
`@sqve/stomme-ts-style` with its own version.

See [the documentation](docs/README.md) for the development guide and architecture decisions.
