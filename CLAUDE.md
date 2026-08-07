# CLAUDE.md — time_value

## Purpose

`time_value` is a type-safe time-value-of-money library for Rust, published on
crates.io as `time_value` (the GitHub repo is `time-value`, kebab-cased per the
org ruleset).

The repo was restarted from nothing. It currently holds **foundations only** —
one empty crate, `crates/time_value`.

## Design principles

- **Make TVM mistakes compile errors.** The bug this domain actually produces is
  applying a rate of one periodicity to cashflows of another. Encode what the
  compiler can check.
- **Earn each type.** Code lands in `f64` first, with tests capturing the stated
  behaviour; types follow. A type that catches no real failure mode does not
  belong — the pressure comes from the problem, never from a design decided in
  advance.

## Rust and Nix conventions

`~/.claude/rust.md` is the source these were copied from; read it before
scaffolding anything new. What is load-bearing here:

- **Lint levels are manifest-authoritative.** They live in `Cargo.toml`, and CI
  adds no `-D warnings`, so the local verdict equals the CI verdict. Do not
  reintroduce it.
- **`resolver = "3"` is explicit.** A virtual workspace does not infer the
  resolver from its members and silently falls back to 1.
- **Every workspace member needs `[lints] workspace = true`.** Without it a
  member silently gets none of the lint configuration.
- **`rustfmt` is a pinned nightly**, wired through `RUSTFMT` in `flake.nix`
  because there is no `cargo +nightly fmt` under nix. Most of `rustfmt.toml` is
  nightly-only and stable ignores it silently — a `--check` that verifies almost
  nothing and exits 0.
- **`rust-toolchain.toml` is the only toolchain pin**; the flake reads it.
- **Dependencies arrive when something needs them**, never in advance.

## Verification

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c cargo clippy --workspace --all-targets --locked
nix develop -c cargo nextest run --workspace --locked --no-tests=pass
nix develop -c cargo test --doc --workspace --locked
nix develop -c cargo doc -p time_value --no-deps --locked
nix develop -c cargo deny check all
```

Run all of them. Three are silent no-ops if skipped — `cargo test --doc`,
`cargo doc`, and `cargo deny`.

`--no-tests=pass` is there only because the crate is empty. **Remove it with the
first real test**; nextest failing on zero tests is the check working.

## CI and releases

CI runs on pushes to `main` and on pull requests. It is **not** a required status
check — that gate was removed deliberately.

**There is no release and none scheduled.** Bumping a version, flipping
`publish`, tagging, or adding release machinery is the owner's call and is never
inferred from the work looking finished.

## Deletion & creation

Layered on the global floor in `~/.claude/CLAUDE.md`.

- **Ask before deleting** `LICENSE-*`, `Cargo.lock`, or `rust-toolchain.toml`.
- **Never rename** the published `time_value` crate.
- **New crates** join under `crates/`, inherit `[workspace.package]`, and **must**
  carry `[lints] workspace = true`. Non-core crates start `publish = false`.

## Conventions

- Branch names match `^(feat|fix|chore|docs|refactor)/.*` (repo ruleset). Commits
  are Conventional Commits.
- Comments supplement the code and the structure — they earn their place by
  naming a trap, not by restating the line below them.
