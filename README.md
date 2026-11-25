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
"github:sQVe/stomme" = "0.2.4"
```

Set the same version in `stomme.toml`:

```toml
version = "0.2.4"
```

Extend the Renovate preset in `renovate.json`:

```json
{
  "extends": ["github>sQVe/stomme//renovate/stomme"]
}
```

Add a workflow that syncs the update branch. `stomme init` writes the commit, and a reader copying
by hand uses the commit of tag `v0.2.4`, from
`gh api repos/sQVe/stomme/commits/v0.2.4 --jq .sha`, which works for annotated and lightweight
tags. 0.2.4 is not tagged yet, so no real SHA exists to show:

```yaml
name: Sync stomme

on:
  push:
    branches: [renovate/stomme]

permissions:
  contents: read

jobs:
  sync:
    uses: sQVe/stomme/.github/workflows/sync-update.yml@<commit> # v0.2.4
    secrets:
      app-id: ${{ secrets.STOMME_APP_ID }}
      app-private-key: ${{ secrets.STOMME_APP_PRIVATE_KEY }}
```

The workflow takes an optional `guides-directory` input. Leave it unset for the behavior above. When
it is set, the job runs `stomme guides <directory>` after `stomme sync` and commits that folder with
the rest of the sync, so the folder holds the current release's shared guides. The folder must be
inside the repository, and Git must be able to stage its files, so Git must not ignore the folder or
any file inside it. When Git ignores a file the refresh wrote, the job fails instead of committing
an incomplete folder. It must not be inside another Git repository. It must not be `docs/stomme` or
a folder inside it:

```yaml
name: Sync stomme

on:
  push:
    branches: [renovate/stomme]

permissions:
  contents: read

jobs:
  sync:
    uses: sQVe/stomme/.github/workflows/sync-update.yml@<commit> # v0.2.4
    with:
      guides-directory: docs/guides
    secrets:
      app-id: ${{ secrets.STOMME_APP_ID }}
      app-private-key: ${{ secrets.STOMME_APP_PRIVATE_KEY }}
```

The TypeScript style package in [packages/style](packages/style/README.md) is published as
`@sqve/stomme-ts-style` with its own version.

## Review pull requests with OCR

`stomme init` writes `.github/workflows/ocr.yml`, which calls the reusable OCR workflow in stomme by
release commit with a version comment:

```yaml
name: OCR

on:
  pull_request:
    types: [opened, synchronize, reopened, ready_for_review]

jobs:
  review:
    permissions:
      contents: write
      pull-requests: write
    uses: sQVe/stomme/.github/workflows/ocr-review.yml@<commit> # v0.2.4
    secrets:
      model-token: ${{ secrets.MORPH_API_KEY }}
```

Set the `MORPH_API_KEY` repository secret to the Morph token. The caller grants `contents: write`
and `pull-requests: write`. The review posts every finding as an inline comment on the pull request,
beside the line it describes, whatever its severity. A finding with no line, or an inline post that
GitHub rejects, still lands in the summary comment with the reason.

Require the `review / OCR review` check in branch protection, so a pull request merges only after
the review covers every selected file. The check fails until the secret is set. A fork pull request
gets no secrets, so it fails the check too; fork pull requests are not supported.

See [ADR 0017](docs/adr/0017-open-code-review.md) for the review rules and the coverage check.

See [the documentation](docs/README.md) for the development guide and architecture decisions.
