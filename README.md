# time-value

Type-safe time-value-of-money calculations in Rust.

A Cargo workspace whose published crate is [`time_value`](crates/time_value).

## The Knowledge Bundle

[`knowledge/`](knowledge/) is an [Open Knowledge Format][okf] bundle and this
project's authoritative documentation: what each formula is, where its
definition came from, which convention was followed where sources disagree, and
what has deliberately been left open. There are no ADRs.

Concepts come before code. A formula is modelled first, and the implementation
follows the model — so the bundle is worth reading before the source, not after.

Each claim says where it came from. A footnote means a source supports it;
otherwise it is labelled **Decided** or **Derived**, so a reader can tell a
choice from a consequence. Most of it is ours: the sources give formulas and
little else.

[okf]:
  https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md

## Status

**Modelled, not yet implemented.** `FV = PV(1 + rt)` — future value under simple
interest — is fully described in the bundle, down to the shape of the operation
and the two distinct ways it can fail. No library code is written yet.

Nothing is published. The `0.1.0`–`0.8.0` series on crates.io is a separate,
immutable history this line does not continue.

## Development

```sh
nix develop           # toolchain, bacon, nextest, cargo-deny, prettier, hooks
bacon                 # continuous clippy
```

Everything CI runs, runs the same way locally:

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c prettier --check "**/*.md"
nix develop -c cargo clippy --workspace --all-targets --locked
nix develop -c cargo nextest run --workspace --locked --no-tests=pass
nix develop -c cargo test --doc --workspace --locked
nix develop -c cargo doc -p time_value --no-deps --locked
nix develop -c cargo deny check all
```

Run all of them. Three are silent no-ops if skipped: `cargo test --doc` (nextest
does not run doc tests), `cargo doc` (every `[lints.rustdoc]` entry is otherwise
inert), and `cargo deny`.

Changes to `knowledge/` are validated separately, from `okf-tools`' own flake:

```sh
nix run github:ojhermann-org/okf-tools#okf-graph -- knowledge
```

This is not yet part of CI — see
[#136](https://github.com/ojhermann-org/time-value/issues/136).

## Repo settings

This repo's own GitHub rulesets are version-controlled in
[`.github/rulesets/`](.github/rulesets/) and reconciled by
`scripts/settings.sh --check` / `--apply`.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
