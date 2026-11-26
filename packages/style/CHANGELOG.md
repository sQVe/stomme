# @sqve/seam

## 1.0.0

### Major Changes

- [#72](https://github.com/sQVe/stomme/pull/72)
  [`e7e219b`](https://github.com/sQVe/stomme/commit/e7e219b610207bf95bf73b717515f785f9b1aa35) Thanks
  [@sQVe](https://github.com/sQVe)! - Upgrade `vite-plus` from 0.3.0 to 1.6.0, and the tools pinned
  with it: `oxlint` and `@oxlint/plugins` to 1.93.0, `oxlint-tsgolint` to 7.0.2006, and `vitest` to
  5.1.0. `vite-plus` is a peer dependency, so consumers must upgrade to 1.6.0 as well.

### Minor Changes

- [#68](https://github.com/sQVe/stomme/pull/68)
  [`b32937c`](https://github.com/sQVe/stomme/commit/b32937c092f6c4cc9f5d79fb192ebda71c23a7b4) Thanks
  [@sQVe](https://github.com/sQVe)! - Pin `vitest/no-focused-tests` and `vitest/no-disabled-tests`
  in the Vitest preset, so the bans stay on if oxlint moves either rule out of the default
  `correctness` category.

## 0.4.0

### Minor Changes

- [#20](https://github.com/sQVe/stomme/pull/20)
  [`fcfa0f4`](https://github.com/sQVe/stomme/commit/fcfa0f472af39398e0076fc11405e3c630f95dae) Thanks
  [@sQVe](https://github.com/sQVe)! - Turn off `react/react-in-jsx-scope` in the React preset,
  because the automatic JSX runtime puts no `React` in scope.

## 0.3.0

### Minor Changes

- [#9](https://github.com/sQVe/stomme/pull/9)
  [`be7ed11`](https://github.com/sQVe/stomme/commit/be7ed11935813c2482c9809e27c14afe75984d97) Thanks
  [@sQVe](https://github.com/sQVe)! - Add `seam/no-unnamed-arguments`, which rejects a ternary or
  call chain passed as a call argument. It also checks values nested in logical, binary, unary, and
  template expressions, and in computed object and member keys.

### Patch Changes

- [#5](https://github.com/sQVe/stomme/pull/5)
  [`4f9556d`](https://github.com/sQVe/stomme/commit/4f9556d53890f03b127cb3f6aeda41ddc7854b90) Thanks
  [@sQVe](https://github.com/sQVe)! - Treat `--fix` after a path as a path, so
  `stomme-ts-style <file> --fix` no longer fixes the file.

## 0.2.0

### Minor Changes

- [#5](https://github.com/sQVe/seam/pull/5)
  [`649e212`](https://github.com/sQVe/seam/commit/649e2124846ed9d20eb1bdb57b8a3fc41db8c66b) Thanks
  [@sQVe](https://github.com/sQVe)! - A type assertion no longer needs a `SAFETY:` comment when a
  one-line disable comment for `typescript/no-unsafe-type-assertion` gives a reason.
