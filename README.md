# time-value

Type-safe time-value-of-money calculations in Rust.

A Cargo workspace whose published crate is [`time_value`](crates/time_value).

Nothing here is published. The `0.1.0`–`0.8.0` series on crates.io is a
separate, immutable history that this line does not continue.

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

## Development

```sh
nix develop           # toolchain, bacon, nextest, cargo-deny, prettier, hooks
bacon                 # continuous clippy
```

Everything CI runs, runs the same way locally — CI runs this exact script:

```sh
nix develop -c ./scripts/check.sh            # everything
nix develop -c ./scripts/check.sh clippy     # one check, by name
```

It reports every check rather than stopping at the first failure, and refuses to
run against a stable `rustfmt`, which would silently ignore most of
`rustfmt.toml` and pass anyway.

Changes to `knowledge/` are validated separately, from `okf-tools`' own flake:

```sh
nix run github:ojhermann-org/okf-tools#okf-graph -- knowledge
```

It does not gate merges; wiring it in is tracked in
[#136](https://github.com/ojhermann-org/time-value/issues/136).

## Repo settings

This repo's own GitHub rulesets are version-controlled in
[`.github/rulesets/`](.github/rulesets/) and reconciled by
`scripts/settings.sh --check` / `--apply`.

## License

Licensed under either of [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at
your option.
