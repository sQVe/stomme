# @sqve/seam

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
