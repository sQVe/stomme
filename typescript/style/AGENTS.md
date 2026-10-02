# Seam

Zero-config Oxlint house style for TypeScript projects, published as `@sqve/seam`. Read the
[README](README.md) for usage.

- Run `pnpm run check` before finishing changes. It runs typechecking, the self-lint through `seam`,
  the format check, the build, and all tests, including the end-to-end tests on Node and Bun.
- Fix style with `pnpm style:fix`; it lints with the house rules, then formats.
- Follow the decisions in [docs/adr](docs/adr/README.md), and record new decisions there. Read that
  guide before adding an ADR. Do not write documents that explain how a rule works.
- Every rule in the package is on for every consumer. Do not add a rule that a consumer would need
  to turn off, and do not add options for turning rules off. Area-specific rules go in a preset.
- Put each plugin rule in its own file under `src/rules/`, with its `RuleTester` test beside it.
  Shared helpers go in `src/shared/`.
- Pin `vite-plus`, `@stylistic/eslint-plugin`, `@oxlint/plugins`, `oxlint`, `oxlint-tsgolint`, and
  `vitest` to exact versions, and upgrade them together. `tests/pins.test.ts` fails when a pin
  differs from the version Vite Plus declares.
- Name values in camelCase and types in PascalCase. Never SCREAMING_CASE, not even for module
  constants.
- Declare a helper before the code that uses it. Join at most three checks in one condition, and do
  not mix `&&` with `||`; name the inner group instead.
- Comment only what the code cannot say, such as a constraint or a workaround. Do not cite tickets,
  ADRs, or reviews, and do not describe the code's history.

## Tests

- Test behavior a caller can observe: a rule's diagnostics and fixes, or the command's exit status
  and output.
- Rule tests run in process with `RuleTester`. Behavior that depends on loading the package, such as
  plugin paths, the style switch, presets, and the runner, goes in `tests/package.test.ts`, which
  runs the packed package.
- Use temporary directories for fixtures and remove them when the test finishes.
- Never drop assertions or failure cases to save time.
