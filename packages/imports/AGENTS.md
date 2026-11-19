# stomme-ts-imports

Oxlint import rules for the module table in `stomme.toml`. Read the [README](README.md) for usage.

- Run `pnpm --filter @sqve/stomme-ts-imports run check` before finishing changes. It runs
  typechecking, the self-lint through `stomme-ts-style`, the format check, the build, and all tests.
- Fix style with `pnpm --filter @sqve/stomme-ts-imports style:fix`; it lints with the house rules,
  then formats.
- The package is public but not on npm yet. The release publishes it.
- Follow the decisions in [docs/adr](../../docs/adr/README.md), and record new decisions there. Read
  that guide before adding an ADR. Do not write documents that explain how a rule works.
- Name values in camelCase and types in PascalCase. Never SCREAMING_CASE, not even for module
  constants.
- Declare a helper before the code that uses it. Join at most three checks in one condition, and do
  not mix `&&` with `||`; name the inner group instead.
- Comment only what the code cannot say, such as a constraint or a workaround. Do not cite tickets,
  ADRs, or reviews, and do not describe the code's history.

## Tests

- Test behavior a caller can observe: the config loader's result, or the command's exit status and
  output.
- Use temporary directories for fixtures and remove them when the test finishes.
- Never drop assertions or failure cases to save time.
