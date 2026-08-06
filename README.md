# time-value

Type-safe time-value-of-money calculations in Rust.

A Cargo workspace whose published crate is [`time_value`](crates/time_value) —
a `no_std`, zero-dependency core. The CLI and MCP server follow, one operation
at a time.

## The Knowledge Bundle

[`docs/knowledge/`](docs/knowledge/) is an [Open Knowledge Format][okf] bundle
and the project's living documentation: what each formula is, where its
definition came from, which convention was followed where sources disagree, and
how the code is built. The code implements the bundle; there are no ADRs.

`okf-graph` validates its structure on every commit that touches it and in CI.

[okf]: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md

## Status

Rebuilt from nothing, one formula at a time. Nothing is implemented yet, and
nothing is published — the `0.1.0`–`0.8.0` series on crates.io is a separate,
immutable history this line does not continue.

## Development

```sh
nix develop           # toolchain, bacon, nextest, cargo-deny, okf-graph, hooks
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
nix develop -c tv-bundle-check
```

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
