# Stomme-only Rust rules

Apply these rules with the [shared Rust guide](./stomme/languages/rust/guide.md) and the
[general coding guide](./stomme/shared/coding.md) to every Rust file in stomme, tests included.
This guide holds only rules that apply to this repository alone. The reasons behind them are in
[ADR 0004](./adr/0004-rust-coding-guide.md).

## Errors

- Treat `xtask` as app code: its tasks report errors with `anyhow`, not with `String`.

## Tests

- Put unit tests in a `#[cfg(test)] mod tests` at the end of the file that owns the behavior.
- A test in `xtask` gets its temporary folder from the shared helper in
  `xtask/src/test_support.rs`.
