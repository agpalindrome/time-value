---
type: Reference
title: Toolchain
status: stable
generated: { by: claude/opus-5, at: 2026-08-06T23:40:05Z }
sources:
  - id: rust-defaults
    resource: https://github.com/ojhermann-org/claude/blob/main/rust.md
    author: human:ojhermann
---

# Toolchain

`rust-toolchain.toml` is the only pin. `flake.nix` reads it through
`rust-bin.fromRustupToolchainFile`, so nix and rustup resolve the same
toolchain and there is nothing to reconcile between local and CI.

Channel `1.97.1`, stable, edition 2024. `clippy`, `rustfmt` and `rust-src` are
named explicitly: components are part of the pin, and an unnamed `cargo clippy`
is simply not there to run.

## `rust-version` is not the pin

`rust-version` is the support floor promised to consumers; the pin is what we
compile with. They are independent, and conflating them loses lint coverage
silently — clippy's `msrv` defaults to `rust-version` and gates around 80 lints,
so declaring a low floor disables lints you believe are enabled. `rust-version`
is latest stable (`1.97`), bumped deliberately.

This is the deliberate cost: a downstream user on an older compiler cannot build
the crate. Revisit if the crate acquires consumers who need an older floor; the
escape is a `clippy.toml` `msrv` key, which overrides `rust-version` in both
directions without lowering the promise.

## Nightly rustfmt is a component, not a second pin

Two thirds of rustfmt's configuration surface is nightly-only, and on stable
those options are **silently ignored** — a warning, then exit 0. A repo with
`group_imports` in `rustfmt.toml` and stable rustfmt has a format check that
verifies almost nothing and reports success.

There is no `cargo +nightly fmt` under nix, since `+toolchain` is rustup syntax.
`flake.nix` sets `RUSTFMT` to the pinned nightly binary instead, so every
command stays literally `cargo fmt` and CI inherits it through `nix develop`.

The failure mode this creates is a **false green, not a false failure**: anyone
running `cargo fmt --check` outside the devshell gets a pass that checked
nothing. The pre-commit hook therefore runs the wrapped binary rather than
git-hooks' own `rustfmt` hook, which would use stable.

Nightly rustfmt output carries no stability guarantee, so a date bump can
reformat the tree. That is why the date is pinned.

## Everything runs through the flake

`nix develop -c cargo …`, locally and in CI, so there is one definition of each
tool. See [Lints](lints.md) for what CI must run and why three of the commands
are silent no-ops if omitted.
