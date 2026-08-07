# CLAUDE.md — time_value

## Purpose

`time_value` is a type-safe time-value-of-money library for Rust, published on
crates.io as `time_value` (the GitHub repo is `time-value`, kebab-cased per the
org ruleset).

The repo was restarted from nothing. It holds the Knowledge Bundle described
below and one crate, `crates/time_value`, which is still empty.

## The Knowledge Bundle comes first

`knowledge/` is an
[OKF](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
bundle and the authoritative record of what this library knows and why. There
are no ADRs. **Read `knowledge/index.md` before changing anything.**

The order is concept, then code. A formula is modelled first — what the source
says, what is derived from it, what is decided here, and what is left open — and
the implementation follows the model. Code that contradicts a concept is wrong,
or the concept is, and either way one of them changes deliberately.

Two standing rules live there rather than here, so they are linkable and
versioned with everything they govern:
[illegal states are unrepresentable](knowledge/concepts/illegal-states-unrepresentable.md)
and [the bundle is revisable](knowledge/concepts/the-bundle-is-revisable.md).

### Writing a concept

- **Mark where a claim comes from.** Sourced claims carry a footnote to a
  `sources[]` id. Everything else is labelled **Decided** (a choice that could
  have gone otherwise) or **Derived** (a consequence of something already
  established). A reader must be able to tell which without guessing.
- **Say when sources disagree**, which one was followed, and why. They do
  disagree here — on the symbol for a simple rate, and on whether a period is
  named.
- **Correct in place, visibly.** A claim that changes says it changed and why
  the earlier one failed. Do not silently rewrite; the reasoning is the
  artifact.
- **Bump `generated.at`** on every substantive edit. That is what makes a
  verification visibly stale, so do not skip it to keep a concept looking fresh.
- **Never write a `verified` entry yourself.** It asserts a human read and
  confirmed the content. Ask.
- **Run `okf-graph` after every change** — see Verification.

## Design principles

- **Make TVM mistakes compile errors.** The bug this domain produces is applying
  a rate of one periodicity to cashflows of another. Encode what the compiler
  can check — and note this is currently narrowed, deliberately and with the
  reason recorded, in [future value](knowledge/concepts/future-value.md).
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
- **Markdown wraps at 80** via prettier, as a pre-commit hook and a CI step.
  Tables are exempt: markdown cannot wrap a cell.
- **Dependencies arrive when something needs them**, never in advance.

## Verification

Everything CI runs, in the order CI runs it:

```sh
nix develop -c cargo fmt --all -- --check
nix develop -c prettier --check "**/*.md"
nix develop -c cargo clippy --workspace --all-targets --locked
nix develop -c cargo nextest run --workspace --locked
nix develop -c cargo test --doc --workspace --locked
nix develop -c cargo doc -p time_value --no-deps --locked
nix develop -c cargo deny check all
```

Run all of them. Three are silent no-ops if skipped — `cargo test --doc`,
`cargo doc`, and `cargo deny`.

**And validate the bundle after touching `knowledge/`:**

```sh
nix run ~/okf-tools#okf-graph -- knowledge
```

That is **not** in CI and not in the devshell — it runs from a sibling repo's
flake, so nothing stops a malformed bundle merging. Issue #136 tracks fixing
that once `okf-graph` is a crate.

## CI and releases

CI runs on pushes to `main` and on pull requests. It is **not** a required
status check — that gate was removed deliberately.

Merging to `main` requires a pull request, with zero required approvals (GitHub
forbids self-approval, so any higher count would block a sole maintainer) and an
`OrganizationAdmin` bypass for deliberate direct pushes.

**There is no release and none scheduled.** Bumping a version, flipping
`publish`, tagging, or adding release machinery is the owner's call and is never
inferred from the work looking finished.

## Repo settings as code

This repo's own rulesets live in `.github/rulesets/` and are reconciled by
`scripts/settings.sh --check` / `--apply` — owner-run, deliberately not in CI,
so settings never change silently. Org-wide rules come from `~/github-settings`
and are invisible to that script; the layers compose and GitHub enforces the
more restrictive.

Change a repo-level GitHub setting by editing the JSON and applying it, never by
clicking. Per the global rules, that change is made by a
`~/github-settings`-seated session, not from here.

## Deletion & creation

Layered on the global floor in `~/.claude/CLAUDE.md`.

- **Ask before deleting** anything under `knowledge/` — removing a concept
  removes the reason for the code implementing it — or `LICENSE-*`,
  `Cargo.lock`, or `rust-toolchain.toml`.
- **Never rename** the published `time_value` crate.
- **New crates** join under `crates/`, inherit `[workspace.package]`, and
  **must** carry `[lints] workspace = true`. Non-core crates start
  `publish = false`.

## Conventions

- Branch names match `^(feat|fix|chore|docs|refactor)/.*` (repo ruleset).
  Commits are Conventional Commits.
- Comments supplement the code and the structure — they earn their place by
  naming a trap, not by restating the line below them.
