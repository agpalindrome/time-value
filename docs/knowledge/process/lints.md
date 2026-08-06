---
type: Reference
title: Lints
status: stable
generated: { by: claude/opus-5, at: 2026-08-06T23:40:05Z }
sources:
  - id: rust-defaults
    resource: https://github.com/ojhermann-org/claude/blob/main/rust.md
    author: human:ojhermann
---

# Lints

Levels are **manifest-authoritative**. Every group carries its real level in
`Cargo.toml`, and CI adds no `-D warnings`. A local verdict therefore equals the
CI verdict, and `warn` stays available as a genuine "advisory, will not block a
merge" level.

The rejected alternative is manifest-`warn` plus CI `-D warnings`, which is what
this repo used before the rewrite: the manifest stops describing policy, since a
group marked `warn` is actually merge-blocking and you have to read the workflow
to know it.

`deny` over `warn` for groups, because a warning that always fires stops being
read. Once forty are standing nobody reads the forty-first; `deny` holds the
count at zero by construction.

## Groups

`correctness`, `suspicious`, `complexity`, `perf`, `style`, `pedantic` and
`cargo` are denied. `restriction` and `nursery` are **never** enabled as groups
— `restriction` contains mutually contradictory lints (`implicit_return` against
the denied `needless_return`; no source satisfies both), and `nursery` is by
definition the churning group, so denying it turns `#[expect]` into a treadmill
against the denied `unfulfilled_lint_expectations`. Both are cherry-picked.

`multiple_crate_versions` is the one allowed member of a denied group: it fires
on a transitive dependency, which is unactionable, and an unactionable lint must
not block a merge.

## Suppression

`#[expect(..., reason = "...")]` over `#[allow]` — an `#[expect]` errors once it
stops applying, so it deletes itself. `allow_attributes` and
`allow_attributes_without_reason` make that machine-checked.

A disagreement with a `style` or `pedantic` lint is systematic, not situational,
so it belongs in the manifest as a per-lint `allow` under the denied group —
that is what `priority = -1` buys. Scattering `#[expect]`s for a lint you always
disagree with is the wrong mechanism.

## Thresholds are pinned

`clippy.toml` sets every configurable threshold explicitly. Denying a group whose
lints have configurable thresholds while inheriting the defaults delegates a
merge-blocking decision to clippy's constants.

`excessive_nesting` deserves a note: its default is `0`, which means **off** — a
lint sitting inert inside a denied group. It is set to 5.

## What CI must run

Three of these are silent no-ops if omitted, which is the recurring shape:

| Command | Omitting it means |
|---|---|
| `cargo clippy --all-targets` | lints never see test code |
| `cargo nextest run` | — |
| `cargo test --doc` | **doc tests never run** — nextest skips them |
| `cargo doc --no-deps` | **every `[lints.rustdoc]` entry is inert** |
| `cargo deny check all` | licences and advisories unchecked |
| `cargo fmt --check` | formatting undiffed — and see [Toolchain](toolchain.md) |

## Divergences from the shared Rust defaults

The shared defaults are a starting point, and a repo wins. Two apply here:

- **`thiserror` is not used.** The defaults call for it in libraries; the core is
  `no_std` and zero-dependency, so the error type is hand-written. The message
  conventions still hold — lowercase gerund, no trailing period, never restate
  what the chain already says.
- **`disallowed-methods` is empty.** The usual starting entries
  (`RefCell::borrow`/`borrow_mut`) would be inert in a `no_std` numeric crate
  with no interior mutability, and an entry that never fires is indistinguishable
  from one that is misspelled — a bad path only warns and bans nothing.

`float_cmp` is expected to be the first real friction point; see
[Testing](testing.md).
