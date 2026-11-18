# Development

## Set up

1. Install [rustup](https://rustup.rs). The first `cargo` command in the repository installs the
   toolchain that `rust-toolchain.toml` pins.
2. Install the check tools:

   ```sh
   cargo install --locked cargo-nextest cargo-deny cargo-shear cargo-mutants typos-cli taplo-cli prek
   ```

3. Install Node, pnpm, and Bun. Use the Node major and Bun version in
   [CI](../.github/workflows/ci.yml), and the pnpm version in
   [package.json](../package.json). The package's `engines` field sets the minimum Node and Bun
   versions. Bun is needed for the packed package tests, even when developing with Node.
   The init tests also need `herdr-app` on `PATH`, at the version pinned by the `http:herdr-app`
   table in
   [foundation/layers/herdr/skeleton/merge/mise.toml](../foundation/layers/herdr/skeleton/merge/mise.toml).
   Install [mise](https://mise.jdx.dev). Then run these commands in the repository root. They trust
   the mise file, install `herdr-app`, and add its folder to `PATH`, as CI does:

   ```sh
   (cd foundation/layers/herdr/skeleton/merge && mise trust && mise install http:herdr-app)
   export PATH="$(cd foundation/layers/herdr/skeleton/merge && mise where http:herdr-app)/herdr-app:$PATH"
   herdr-app --version
   ```

   `herdr-app --version` prints `herdr-app 0.1.16`. Add the `export` line to your shell profile to
   keep it. On macOS, mise skips this tool. Install the Herdr app with
   `curl -fsSL https://releases.herdr.dev/app/install.sh | sh` instead.
4. Install JavaScript dependencies from the repository root:

   ```sh
   pnpm install --frozen-lockfile
   ```

5. Install the Git hooks:

   ```sh
   prek install
   ```

## Workspace

- `crates/stomme-cli` is the `stomme` binary. `stomme check` checks the project in the current
  directory and prints each problem to stderr as `stomme: <path>: <message> [<rule>]`. It exits 1
  on any problem. Without `--base`, it compares recorded violations with `origin/<GITHUB_BASE_REF>`
  in a pull request and with `origin/main` otherwise; see
  [ADR 0010](./stomme/shared/adr/0010-recorded-violations.md). `stomme guides <dir>` writes every
  shared file and guide, plus `.sync-record.toml`, to `<dir>` for a consumer without a project. Run
  it on this repository with `cargo run -q -p stomme -- check`.
  `stomme init` creates a project in the current directory, which must be empty apart from `.git`:

  ```sh
  stomme init --name <name> --language <rust|typescript> [--layers <layers>] [--roles <roles>]
  ```

  `--layers` and `--roles` take comma-separated lists. An invalid request writes nothing.
- `xtask` holds repository tasks, run as `cargo xtask <task>`.
- `cargo xtask release-gate --stomme <binary> --version <candidate> <project-folder>` runs a
  candidate `stomme` binary against one project as its update would, and exits 1 only when an
  adopted project fails.
- `packages/style` is the `@sqve/stomme-ts-style` package in the pnpm workspace.
- `packages/imports` is the `@sqve/stomme-ts-imports` package, which reads the module table in
  `stomme.toml`.
- `foundation/` holds the guides, ADRs, and manifests that the binary embeds and `stomme sync`
  writes to `docs/stomme/`.
- `foundation/shared/rules.md` is generated from the rule catalog in
  `crates/stomme-cli/src/rule.rs`. A test fails when the two differ.

## Check your work

Run every check before you open a pull request:

```sh
cargo xtask check
```

It changes no tracked files. It runs rustfmt, typos, taplo, cargo-deny, cargo-shear, clippy,
nextest, rustdoc, and `stomme check`, and reports every failed check at the end.
Warnings fail clippy, the Rust tests, and rustdoc. Every Cargo build runs with `--locked`, so a
stale `Cargo.lock` fails too.

It also runs these package steps, each through `pnpm -r run <script>` in both `packages/style`
and `packages/imports`:

| Step | Script |
|---|---|
| `ts-typecheck` | `typecheck` |
| `ts-style` | `style:check` |
| `ts-format` | `format:check` |
| `ts-build` | `build` |
| `ts-test` | `test` |

The tests include packed package tests on Node and Bun. The build may rebuild ignored `dist/`
files. The check does not install dependencies; run `pnpm install --frozen-lockfile` first.

`cargo xtask check build` runs checks that compile code or need the JavaScript toolchain.
`cargo xtask check tidy` runs the remaining checks without that toolchain.

Fix problems with these commands:

| Problem | Command |
|---|---|
| Rust formatting | `cargo fmt --all` |
| TOML formatting | `taplo fmt` |
| Spelling | `typos --write-changes` |
| Unused dependencies | `cargo shear --fix` |
| Some clippy warnings | `cargo clippy --workspace --all-targets --fix` |
| TypeScript style | `pnpm --filter @sqve/stomme-ts-style run style:fix` |
| TypeScript formatting | `pnpm --filter @sqve/stomme-ts-style run format` |
| Import package style | `pnpm --filter @sqve/stomme-ts-imports run style:fix` |
| Import package formatting | `pnpm --filter @sqve/stomme-ts-imports run format` |
| Stale `foundation/shared/rules.md` | `UPDATE=1 cargo nextest run -p stomme rules_table` |

The pre-commit hook runs rustfmt, clippy, typos, and taplo, but only in check mode. Run the fix
commands yourself.

## Mutation testing

Mutation testing changes the code in small ways and reports each change that no test catches.
`cargo xtask mutation` runs [cargo-mutants](https://mutants.rs) and prints one JSON report on stdout
in the format Tau's `mutation_check` tool reads.

```sh
cargo xtask mutation changed main

cargo xtask mutation audit xtask/src/check.rs

cargo xtask mutation changed main --max-mutants 50
```

`.cargo/mutants.toml` leaves out test code and test helpers. A missed mutant is a survivor: add a
test that fails when that line changes. The command exits 0 when every mutant is caught, 2 for
survivors in changed mode, and 1 for an error or an unmutated file. cargo-mutants writes its logs
to `target/mutants/mutants.out`.

## CI

`.github/workflows/ci.yml` runs on pull requests, on pushes to `main`, and weekly, so new security
advisories fail cargo-deny without a code change.

- `Check` runs `cargo xtask check tidy` on Linux.
- `Test (Linux)` sets up Node, pnpm with its cache, and Bun, installs with
  `pnpm install --frozen-lockfile`, then runs `cargo xtask check build` on Linux.
- `Mutation (changed)` runs on pull requests. It runs `cargo xtask mutation changed` against the
  pull request's base, prints the JSON report, and lists survivors in the job summary. Survivors
  pass the job; an error or an unmutated file fails it.
- `Mutation audit` runs on the weekly schedule and on manual runs from `main`. It runs
  `cargo mutants --workspace` and uploads `mutants.out` as an artifact. Survivors and timeouts
  pass the job; other cargo-mutants failures fail it.

Only runs on `main` save the build cache. Renovate updates dependencies, actions, and the toolchain
every Monday, after a release is three days old. The six TypeScript style pins update together,
including major updates, without automerge. `tests/pins.test.ts` in the package checks the pins
for tools declared by Vite Plus.

The dependency audit workflow runs `pnpm audit --audit-level high` weekly. Dependency review
checks the dependencies each pull request adds or changes, including `pnpm-lock.yaml`. Its
`allow-ghsas` list holds the advisories of the pinned `vite-plus` 0.3.0 tool set until that set
is upgraded.

## Package releases

For changes under `packages/style/src`, add a Changeset from the repository root with
`pnpm changeset`. Select `@sqve/stomme-ts-style`, choose the version bump, and describe the change.
Commit the generated file with the code. The Changeset workflow checks that source changes have one
with `pnpm changeset:check`.

The Package release workflow uses Changesets on `main`. It opens a version pull request with
`pnpm changeset:version`. Merging that pull request runs `pnpm run release`, which checks the
package before publishing. Do not publish from your local checkout.

## Binary releases

A `v<version>` tag builds `stomme` with cargo-dist and publishes a GitHub Release. `CHANGELOG.md`
holds the release notes, and `packages.toml` records the npm package versions compatible with that
binary.

The release runs the gate jobs in `.github/workflows/release-gate.yml` before hosting. They run the
candidate binary against every adopted project's `main` and against `release/consumer/`. An adopted
project that fails stops the host job, so the GitHub Release is created only when every gate job
passes. A project without `stomme.toml` is reported without blocking the release.

Install a published binary with mise:

```sh
mise use -g github:sQVe/stomme
```

Or install through cargo-binstall:

```sh
cargo binstall --git https://github.com/sQVe/stomme stomme
```

Do not create a tag, publish a release, or publish a package from your local checkout without
approval.
