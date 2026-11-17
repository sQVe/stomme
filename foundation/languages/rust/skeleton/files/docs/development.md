# Development

## Set up

1. Install [mise](https://mise.jdx.dev) and [rustup](https://rustup.rs).
2. Run `mise install`. It installs the `stomme` binary that `mise.toml` pins.
3. Run `rustup toolchain install`. It installs the toolchain that `rust-toolchain.toml` pins.

## Run the full check

Run the full check before you finish a change:

```sh
cargo xtask check
```

The check runs these steps in order and reports every step that fails:

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `stomme check`

Format the code with `cargo fmt`.
