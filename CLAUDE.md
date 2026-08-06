# CLAUDE.md — time_value

## Purpose

`time_value` is a type-safe time-value-of-money library for Rust, published on
crates.io as `time_value` (the GitHub repo is `time-value`, kebab-cased per the
org ruleset). It is being **rebuilt from nothing**, one formula at a time.

The workspace currently holds one crate — `crates/time_value`, the `no_std`
core. The CLI and MCP server return as separate efforts once there is an
operation for them to expose.

## The Knowledge Bundle is the documentation

`docs/knowledge/` is an OKF Knowledge Bundle and the authoritative record — of
what the library knows and of how it is built. **There are no ADRs.** A decision
lives in a Concept there, not in a comment, a commit message, or this file.

Read `docs/knowledge/process/` before changing anything:

- `workflow.md` — one formula per effort, what a formula effort contains
- `toolchain.md` — the single pin, and why nightly rustfmt is a component
- `lints.md` — manifest-authoritative levels, and what CI must run
- `dependencies.md` — the bar, and why `[workspace.dependencies]` is empty
- `testing.md` — placement, properties, and the open `float_cmp` question

A formula that is not in the bundle is not in the library. When work changes a
decision, change the Concept in the same pull request.

## Design principles

- **Make TVM mistakes compile errors.** The bug this domain actually produces is
  applying a rate of one periodicity to cashflows of another. Encode what the
  compiler can check.
- **Earn each type.** A formula lands in `f64` first, with tests capturing the
  behaviour its Concept states; the types follow in the same pull request, as
  separate commits. A type that catches no real failure mode *for that formula*
  does not belong. The pressure comes from the formula, never from a design
  decided in advance.
- **`no_std`, zero dependencies by default.** Transcendental functions are
  `std`-only; prefer an optional `libm` feature over an unconditional dependency.

## Verification

Everything runs through the flake, so local and CI are the same commands:

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c cargo clippy --workspace --all-targets --locked
nix develop -c cargo nextest run --workspace --locked --no-tests=pass
nix develop -c cargo test --doc --workspace --locked
nix develop -c cargo doc -p time_value --no-deps --locked
nix develop -c cargo deny check all
nix develop -c tv-bundle-check
```

No `-D warnings` anywhere: lint levels live in `Cargo.toml`, so the local verdict
equals the CI verdict. Do not reintroduce it.

Run all of them. Three are silent no-ops if skipped — `cargo test --doc` (nextest
does not run doc tests), `cargo doc` (every `[lints.rustdoc]` entry is otherwise
inert), and `cargo deny`.

## CI and releases

CI is one job whose id is **`ci`** — the required status check. Do not rename it,
give it a custom `name:`, or drop the `merge_group` trigger.

`main` merges go through a merge queue, so `gh pr merge <n> --squash` *enqueues*:
a pull request needs green CI and a clean rebase to land.

**There is no release and none scheduled.** Bumping a version, flipping
`publish`, tagging, or adding release machinery is the owner's call and is never
inferred from the work looking finished.

## Deletion & creation

Layered on the global floor in `~/.claude/CLAUDE.md`.

- **Ask before deleting** anything under `docs/knowledge/` (removing a Concept
  removes the reason for the code that implements it), `LICENSE-*`, `Cargo.lock`,
  or `rust-toolchain.toml`.
- **Never rename** the `ci` job or the published `time_value` crate.
- **New crates** join under `crates/`, inherit `[workspace.package]` and
  `[workspace.dependencies]`, and **must** carry `[lints] workspace = true` — a
  member without it silently gets none of the lint configuration. Non-core crates
  start `publish = false`.
- **New dependencies** arrive with the formula that needs them, never in advance.

## Conventions

- Never commit to `main`; branch and open a pull request. Branch names match
  `^(feat|fix|chore|docs|refactor)/.*`. Commits are Conventional Commits.
- Comments supplement the code, the bundle, and the structure — they earn their
  place by naming a trap, not by restating the line below them.
