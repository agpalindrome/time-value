# CLAUDE.md — time_value

## Purpose

`time_value` is a type-safe time-value-of-money (TVM) library for Rust,
published on crates.io as `time_value` (the GitHub repo is `time-value`,
kebab-cased per the org ruleset). It is a deliberately type-heavy redesign — not
a port of the old `0.x` series — developed continuously, with no scheduled
release (ADR-0038).

The repo is a **Cargo workspace** (see `docs/adr/0002-workspace-layout.md`) of
four crates — three primary, plus one internal support crate:

- `crates/time_value` — the `no_std` core library (the published crate).
- `crates/time-value-cli` — the `time-value` CLI binary.
- `crates/time-value-mcp` — the `time-value-mcp` MCP server binary.
- `crates/time-value-daycount` — internal, unpublished ACT/365 day-count shared
  by the two binaries (`docs/adr/0030-shared-day-count-support-crate.md`).

Dependencies point one way, toward the library; the binaries depend on
`time_value` (and on `time-value-daycount`) by workspace path. Async is contained
to `-mcp`; the core stays synchronous and `no_std`. Architecture decisions are
logged under `docs/adr/`.

## Design principles

- **Make TVM mistakes compile errors.** The headline bug TVM cares about is
  applying a rate of one periodicity to cashflows of another (e.g. an annual
  rate on monthly flows). Encode periodicity in the type (`Rate`/`Cashflows`
  tagged with a zero-cost periodicity marker) so the compiler rejects the
  mismatch.
- **Type-heavy *and* friendly.** Validated newtypes (`Rate`, `Money`, `Period`)
  with fallible constructors, but type aliases and inference keep the common
  path one-liner-clean. Avoid full dimensional-analysis types — TVM stays in
  "money", so they add ceremony without catching the real (semantic) errors.
- **Transposable arguments get a role newtype** (ADR-0050). Where an operation
  takes two adjacent arguments of the *same* type — two `Money`, or a rate and a
  growth rate — the ambiguous positions are tagged (`Payment`, `PresentValue`,
  `FutureValue`, `Principal`, `Growth<P>`) so a swap is a compile error. They
  validate nothing and cost nothing; there is deliberately **no** `From<Money>` /
  `From<Rate<P>>` into a role (it would let both orders compile again). Positions
  that cannot be confused — a lone `Money` argument — stay untagged.
- **`#![no_std]` + zero dependencies.** Transcendental functions (`powf`, `ln`,
  `exp`) are `std`-only; when the API needs them, prefer an optional `libm`
  feature over an unconditional dependency.
- **Currency is a runtime *value* on `Money`, not a type tag** (ADR-0033/0034):
  `Money` is an `f64` magnitude plus a runtime `Currency` (a closed
  `#[non_exhaustive]` ISO-4217 enum); `XXX` is the currency-agnostic identity, and a
  mismatch is a runtime `CurrencyMismatch`. Periodicity — static, known when the
  model is written — is the crate's *only* compile-time tag; currency — dynamic,
  chosen at runtime — is a value. (This supersedes ADR-0005's earlier "`Money` is a
  plain untagged newtype" stance.)

## Testing & design discipline (ADR-0045)

Two standing rules; ADR-0045 has the full statement and rationale.

- **Make illegal states unrepresentable.** When a decision could encode an
  invariant in a type the compiler (or serde, at the wire boundary) checks, prefer
  that over a comment, a convention, or a runtime check — at the *chokepoint*, not
  each call site. This is a design test for **new** work, not a retrofit mandate,
  and it has a boundary: genuinely open or runtime-chosen sets stay values (that's
  why currency is a runtime value, not a type tag), and a newtype that catches no
  real failure mode is not an improvement.
- **Test the class, not the instance; pin every stated assumption.** When a
  rustdoc line or an ADR *asserts* a behaviour ("NPV decreases as the rate rises",
  "PV inverts FV", "`ALL` lists every currency", "the periodicities must match"),
  that assertion earns a test that fails the moment the code stops honouring it.
  Where the assumption is a *universal*, prefer a proptest property over a point
  test; where the domain is a small *finite* enum, prefer exhaustive iteration
  over sampling; where the invariant lives in the type system, a `compile_fail`
  doctest is the test.

## Tooling (Nix-native, no prek)

- `flake.nix` is the single source of truth **for the toolchain**: a devShell
  (via `oxalica/rust-overlay`, toolchain pinned in `rust-toolchain.toml`) that
  provides `cargo`, `clippy`, `rustfmt`, `bacon`, `cargo-nextest`, `cargo-deny`,
  and `nixfmt`, plus `git-hooks.nix` pre-commit hooks installed on shell entry.
- Verification runs **through the flake** as `nix develop -c cargo …` (the same
  commands locally and in CI — see `docs/adr/0012-ci-and-release-automation.md`),
  so there is no second definition of the toolchain or the tools:

  ```sh
  nix develop -c cargo fmt --all -- --check
  nix develop -c cargo clippy --workspace --all-targets -- -D warnings              # no_std default
  nix develop -c cargo clippy --workspace --all-targets --all-features -- -D warnings
  nix develop -c cargo clippy -p time_value --no-default-features --features libm --all-targets -- -D warnings  # no_std + libm
  nix develop -c cargo clippy -p time_value --no-default-features --features serde --all-targets -- -D warnings # no_std + serde
  nix develop -c cargo clippy -p time_value --no-default-features --features alloc,libm --all-targets -- -D warnings # no_std + alloc (owned)
  nix develop -c cargo clippy -p time_value --no-default-features --features alloc,serde -- -D warnings # no_std + alloc + serde, LIB ONLY (ADR-0060)
  nix develop -c cargo clippy -p time_value --no-default-features --features schemars --all-targets -- -D warnings # no_std + schemars
  nix develop -c cargo nextest run --workspace --all-features
  nix develop -c cargo test --doc --workspace --all-features                        # doctests
  nix develop .#msrv -c cargo build -p time_value --all-features                    # core MSRV (1.85): build, not test, so dev-deps don't gate it
  nix develop -c cargo deny check
  RUSTDOCFLAGS="-D warnings" nix develop -c cargo doc -p time_value --no-deps                 # rustdoc, default features
  RUSTDOCFLAGS="-D warnings" nix develop -c cargo doc -p time_value --no-deps --all-features  # rustdoc, all features
  ```

  **Run the two `cargo doc` lines too** — CI does (ADR-0055), and they catch a
  class nothing else does: an intra-doc link to an item that exists only behind a
  feature resolves under `--all-features` and breaks under the default `no_std`
  build. When a doc comment on an always-present item (a `TvmError` variant, say)
  needs to name a feature-gated one, add the path to the `docs_rs_links!` macro in
  `lib.rs` and attach it with `#[cfg_attr(not(any(feature = "std", feature =
  "libm")), doc = docs_rs_links!())]`, which substitutes a docs.rs URL when the
  target is absent.

  The core `time_value` crate keeps a conservative **MSRV of 1.85** (declared per
  crate, below the workspace's 1.88 toolchain, which the MCP crate's deps force);
  the `.#msrv` devShell pins rustc 1.85 and CI verifies the core there so the
  promise can't silently regress (`docs/adr/0017-per-crate-msrv-core-1.85.md`).

  Both feature configurations are checked: default features build the `no_std`,
  zero-dep core (catching an accidental `std` dependency), and `--all-features`
  exercises the feature-gated operations and their tests. Doctests are run
  separately because `nextest` does not run them.

  The `alloc,serde` clippy line is **deliberately lib-only — do not add
  `--all-targets`** (ADR-0060). Any build that includes the test targets pulls the
  `serde_json` dev-dependency, which enables `serde/std` and so masks a missing
  `serde/alloc`; `--all-features` masks it the same way. Lib-only is the dependency
  graph a downstream `features = ["alloc", "serde"]` user gets, and the only
  configuration that proves the `alloc = ["serde?/alloc"]` weak feature is present.

- `nix flake check` now only validates the pre-commit hook set (the crane build
  checks were retired when CI moved to `nix develop -c cargo …`).
- `bacon.toml` defines the bacon jobs; the default `clippy` job mirrors the CI
  clippy check across the workspace. `.zellij/layout.kdl` wires bacon into a
  project layout.

## Repo structure

```
flake.nix                 # devShell (toolchain + tools) + git-hooks
rust-toolchain.toml       # pinned toolchain (oxalica/rust-overlay reads this)
Cargo.toml                # [workspace]: members, shared package/deps/lints
deny.toml                 # cargo-deny: licenses + advisories + bans
release-plz.toml          # release-plz: versions/changelogs/tags/GH releases
crates/
  time_value/             # core library (no_std) — the published crate
  time-value-daycount/     # internal ACT/365 day-count (unpublished, ADR-0030)
  time-value-cli/          # binary `time-value`
  time-value-mcp/          # binary `time-value-mcp`
docs/adr/                 # architecture decision records
bacon.toml                # bacon jobs (default: clippy)
.zellij/layout.kdl        # project Zellij layout with a bacon pane
.helix/languages.toml     # Helix: rust (clippy check) + nix (nixfmt)
.github/workflows/
  ci.yml                  # CI: nix develop -c cargo fmt/clippy/nextest/deny
  release-plz.yml         # versions + changelogs + tags + GitHub releases
  publish.yml             # tag-triggered `cargo publish` (crates.io OIDC)
```

## CI / release

- CI runs `nix develop -c cargo fmt/clippy/nextest/deny` on pushes to `main`, on
  PRs, and in the merge queue's `merge_group` (one `ci` job). The job id `ci` is
  the required status check — **do not rename it, set a custom `name:`, or remove
  it**, and keep the `merge_group` trigger (a required-check merge queue with no
  `merge_group` CI deadlocks).
- **`main` merges go through a merge queue** (repo-level `merge-queue` ruleset:
  `required_status_checks = ci` + `merge_queue`, squash method, ALLGREEN). Don't
  merge a PR directly — `gh pr merge <n> --squash` **enqueues** it; GitHub rebases
  it on `main`, runs `ci` against the `merge_group` ref, and squash-merges on
  green. So a PR needs green CI *and* a clean rebase to land; a queued PR that
  fails or conflicts is ejected for a manual rebase. The repo-level ruleset is
  managed by hand (the `github-settings` IaC repo manages only *org-level*
  rulesets — `imports.tf`: "Repositories are no longer managed here").
- **No scheduled release (ADR-0038).** The project is developed **continuously, for
  its own sake** — there is no release target, no version goal, and no need to
  classify work as release-relevant. Open issues are a flat, label-prioritized
  backlog (no milestones; the old `1.0.0` / "Post-1.0 backlog" milestones and the
  "Road to 1.0.0" epic are dissolved/closed). If and when it feels like a good spot,
  the owner decides to cut a release; until then, just do the next useful work and
  keep docs + ADRs current with each change.
- Release *machinery* is wired but **inert**: `release-plz.yml` (per-crate versions,
  changelogs, tags, GitHub releases from Conventional Commits; `release-plz.toml`,
  `publish = false`) and `publish.yml` (`cargo publish` via crates.io OIDC trusted
  publishing on a version tag) are ready but not driven. Crates keep their in-tree
  versions; nothing is published; old `0.1.0`–`0.8.0` remain the separate, immutable
  published history. The held `release-plz` release PR (#28) stays held.
- **Cutting a release is solely the owner's call, whenever they choose it** — bumping
  versions, flipping any crate's `publish = false`, extending
  `release-plz`/`publish.yml`, tagging, or merging a release PR are out of scope for
  ordinary development and never inferred from "the work looks done". (Remaining
  publish setup, for whenever that day comes, is tracked in issue #20.)

## Deletion & creation

Layered on the global floor (`~/.claude/CLAUDE.md`). What is sensitive *here*:

- **Ask before deleting** ADRs (`docs/adr/*` — an append-only decision log; supersede
  with a new ADR rather than deleting), `LICENSE-*`, `Cargo.lock`, or the pinned
  `rust-toolchain.toml`.
- **Never rename or delete** the `ci` job / status check (see above), and never
  rename the published `time_value` crate.
- **New crates** join the workspace under `crates/`, inherit `[workspace.package]`
  /`[workspace.dependencies]`/`[workspace.lints]` (`field.workspace = true`,
  `[lints] workspace = true`), and get a `README.md`; non-core crates start
  `publish = false`.
- **New ADRs** are the next free number under `docs/adr/`, copied from
  `0000-adr-template.md`.

## Conventions

- Never commit to `main`; branch + PR, squash-merge. Branch names must match
  `^(feat|fix|chore|docs|refactor)/.*` (repo ruleset). Commits are Conventional
  Commits (they feed `release-plz`).
- Run the `nix develop -c cargo …` checks (or let bacon / the pre-commit hooks
  run) before pushing.
