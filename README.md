# time-value

Type-safe time-value-of-money (TVM) calculations in Rust — a core library, a
command-line interface, and a Model Context Protocol server.

The headline goal: make TVM mistakes — applying an annual rate to monthly
cashflows, discounting with an economically meaningless rate — into *compile
errors*, while keeping the common path ergonomic.

## Workspace

| Crate | Kind | Description |
|-------|------|-------------|
| [`time_value`](crates/time_value) | library (`no_std`) | The TVM calculations. Published on crates.io. |
| [`time-value-cli`](crates/time-value-cli) | binary `time-value` | Command-line interface over the library. |
| [`time-value-mcp`](crates/time-value-mcp) | binary `time-value-mcp` | MCP server exposing the calculations as tools. |
| [`time-value-daycount`](crates/time-value-daycount) | library (internal) | ACT/365 day-count shared by the binaries. Unpublished. |

Dependencies point one way, toward the library; the binaries depend on
`time_value` (and the internal `time-value-daycount`) by workspace path (see
[ADR-0002](docs/adr/0002-workspace-layout.md)).
Architecture decisions are recorded under [`docs/adr/`](docs/adr).

## Quick look

As a library ([`crates/time_value`](crates/time_value)):

```rust
use time_value::{Cashflows, Money, Monthly, Rate};

// Pure-number TVM is currency-agnostic; `Money::new(amount, currency)` denominates it.
let flows = [Money::agnostic(-100.0)?, Money::agnostic(60.0)?, Money::agnostic(60.0)?];
let project = Cashflows::<Monthly>::new(&flows);
let npv = project.net_present_value(Rate::<Monthly>::new(0.01)?)?; // ≈ 18.22
let irr = project.internal_rate_of_return()?;                      // ≈ 0.1307
```

From the shell ([`time-value` CLI](crates/time-value-cli)):

```sh
time-value series npv --rate 0.01 -100 60 60   # 18.2237…
time-value series irr -100 60 60               # 0.1307… per period
```

The [`time-value-mcp` server](crates/time-value-mcp) exposes the same operations
as MCP tools for assistants.

## Development

This repo is Nix-native. `direnv` activates the dev shell automatically via
`.envrc`; otherwise enter it manually:

```sh
nix develop
```

The dev shell provides the pinned Rust toolchain (see `rust-toolchain.toml`),
[`bacon`](https://dystroy.org/bacon/), `cargo-nextest`, `cargo-deny`, and
`nixfmt`, and installs the [git-hooks.nix](https://github.com/cachix/git-hooks.nix)
managed pre-commit hooks on entry.

Run the checks exactly as CI does — every check goes through the flake, so there
is no second source of truth for the toolchain:

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings
nix develop -c cargo nextest run --workspace --all-features
nix develop -c cargo test --doc --workspace --all-features
nix develop -c cargo deny check
```

The workspace builds on Rust **1.88** (`rust-toolchain.toml`); the published
`time_value` library keeps a lower **1.85** MSRV, which CI verifies separately
(`nix develop .#msrv -c cargo build -p time_value --all-features`). The canonical
check list is [`.github/workflows/ci.yml`](.github/workflows/ci.yml); `CLAUDE.md`
documents the full workflow.

`bacon` runs the workspace `clippy` job by default (see `bacon.toml`); a project
Zellij layout wires it into a dedicated pane:

```sh
zellij --layout .zellij/layout.kdl
```

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the full check gate, the commit and
branch conventions, and how to add an operation across the library, CLI, and MCP
server.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this workspace by you, as defined in the Apache-2.0 license,
shall be dual licensed as above, without any additional terms or conditions.
