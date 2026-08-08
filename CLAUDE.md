# CLAUDE.md — time_value

## Purpose

`time_value` is a type-safe time-value-of-money library for Rust, published on
crates.io as `time_value` (the GitHub repo is `time-value`, kebab-cased per the
org ruleset).

One published crate, `crates/time_value`, built against the Knowledge Bundle
described below. Beside it, unpublished: `crates/bundle-check`, whose tests
assert the bundle's own invariants, and `crates/time-value-cli`, which installs
the `time-value` binary.

**The cadence is library, then CLI, then MCP.** A feature is modelled and built
in `time_value`, exposed in the CLI once it is ready, and exposed in an MCP
server after that. There is no schedule and no cycle — we build when we want to
and expose a feature when it is ready. The MCP surface is tracked in
[#153](https://github.com/ojhermann-org/time-value/issues/153) and does not
exist yet.

A surface never validates. Every value is built by the library's constructors,
so a binary parses text, calls one operation and renders the answer — putting a
rule in two places is how the two come to disagree.

## The Knowledge Bundle comes first

`knowledge/` is an
[OKF](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md)
bundle and the authoritative record of what this library knows and why. There
are no ADRs. **Read `knowledge/index.md` before changing anything** — it opens
with a routing table saying which concepts a given kind of task needs.

Its principles (`knowledge/principles/`) are read once and govern everything;
its domain concepts (`knowledge/domain/`) are read per task.

The order is concept, then code. A formula is modelled first — what the source
says, what is derived from it, what is decided here, and what is left open — and
the implementation follows the model.

**They change together.** Every change asks three questions, and the third is
the one that pays:

1. Code changed — what does the bundle now say that is false?
2. A concept changed — what code now contradicts it?
3. Something was learned — is it a **lesson**, or only a local fix? A defect is
   usually an instance of a rule nobody wrote down, and fixing the instance
   leaves the rule unlearned. Ask: would a rule have prevented this, and is that
   rule recorded? If yes and no, the fix is not finished.

See
[code and bundle change together](knowledge/principles/code-and-bundle-change-together.md).

The standing rules live there rather than here, so they are linkable and
versioned with everything they govern:
[`knowledge/principles/`](knowledge/principles/). They are not listed here — a
list in two places is a list that goes stale in one of them.

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
- **Run the bundle's checks after every change** —
  `nix develop -c ./scripts/check.sh test`, which is where `okf-graph` now runs;
  see Verification.

## Design principles

- **Make TVM mistakes compile errors.** The bug this domain produces is applying
  a rate of one periodicity to cashflows of another. Encode what the compiler
  can check — and note this is currently narrowed, deliberately and with the
  reason recorded, in [future value](knowledge/domain/future-value.md).
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

```sh
nix develop -c ./scripts/check.sh            # everything CI runs
nix develop -c ./scripts/check.sh clippy     # one check, by name
```

`scripts/check.sh` is the **only** definition of what CI enforces; CI runs the
same script. Do not restate the list here or in the README — a list in two
places is a list that goes stale in one of them, which is how the markdown check
once ran in CI while the docs described six checks and not seven.

It runs every check and reports each, rather than stopping at the first failure.
It also refuses to run against a stable `rustfmt`: most of `rustfmt.toml` is
nightly-only and stable ignores it silently, so that would be a pass which
verified almost nothing.

**Pre-commit hooks in `flake.nix` are a second gate, and five of them run
nowhere else:** `end-of-file-fixer`, `trim-trailing-whitespace`, `check-toml`,
`check-merge-conflicts`, `detect-private-keys`. A clone without hooks armed
pushes past all five and CI does not notice. They stay hooks because the first
two rewrite files, and a script whose job is to verify must not edit your tree.

The hooks that check **content** are in the script instead, in their
non-mutating form — `typos`, and `nixfmt --check` rather than the rewriting
`nixfmt` — since `typos` and nix formatting running only on a developer's
machine is a real gap, where a missing trailing newline is not.

The sentence above said "the only definition of what must **pass**" until
2026-08-07, which was false in exactly the way it warns against: it named one
definition where there were two.

The bundle is checked entirely by the tests in `crates/bundle-check`, which are
not listed here because the tests are their definition. Both halves are there:
the OKF spec's own conformance rules come from
[`okf-graph`](https://crates.io/crates/okf-graph), which that crate depends on,
and the house rules — stricter than the spec, and each labelled as such where it
is defined — are its own.

There was a second half until 2026-08-08: the same `okf-graph`, as a binary from
an `okf-tools` flake input, run as a `bundle` step in `check.sh`. The crate does
that job too now, so the checker's version is pinned in `Cargo.lock` beside
every other dependency rather than in `flake.lock`. Update it deliberately,
never as a side effect:

```sh
cargo update okf-graph
```

One behaviour changed with the move, and it is the strictest thing in the repo.
`okf-graph` exits zero on a **report** — a dangling cross-link, an out-of-order
log entry — because §6 and §11 say a consumer MUST NOT reject a bundle for one,
and the old step printed it. A passing test prints nothing, so `bundle-check`
**fails** on a report instead. That is a claim about this bundle and not about
the spec; accepting a report means editing `Rule::SpecReport` on purpose.

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
- **Documentation states what is durable, not what is currently true.** A
  "Status" section, a count, or a list of what is implemented so far is correct
  on the day it is written and wrong soon after, and keeping it right is work
  nobody remembers to do. Progress belongs in `knowledge/log.md` and in git,
  which record it without being asked. The exception is a state claim that
  carries its own resolution — "not gated; tracked in #136" tells a reader both
  the fact and where it stops being true.
- **Do not enumerate a growing set in two places.** Naming the standing rules in
  both `CLAUDE.md` and the bundle guaranteed one of them would fall behind, and
  it did. Link to the index instead.
