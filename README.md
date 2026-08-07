# time-value

Type-safe time-value-of-money calculations in Rust.

A Cargo workspace whose published crate is [`time_value`](crates/time_value).

## Status

Foundations only. The toolchain, lints, supply-chain checks and dev shell are
in place; no library code is written yet, and nothing is published — the
`0.1.0`–`0.8.0` series on crates.io is a separate, immutable history this line
does not continue.

## Development

```sh
nix develop           # toolchain, bacon, nextest, cargo-deny, pre-commit hooks
bacon                 # continuous clippy
```

Everything CI runs, runs the same way locally:

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c cargo clippy --workspace --all-targets --locked
nix develop -c cargo nextest run --workspace --locked --no-tests=pass
nix develop -c cargo test --doc --workspace --locked
nix develop -c cargo doc -p time_value --no-deps --locked
nix develop -c cargo deny check all
```

Run all of them. Three are silent no-ops if skipped: `cargo test --doc` (nextest
does not run doc tests), `cargo doc` (every `[lints.rustdoc]` entry is otherwise
inert), and `cargo deny`.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
