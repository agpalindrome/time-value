---
type: Reference
title: Testing
status: stable
generated: { by: claude/opus-5, at: 2026-08-06T23:40:05Z }
sources:
  - id: rust-defaults
    resource: https://github.com/ojhermann-org/claude/blob/main/rust.md
    author: human:ojhermann
---

# Testing

`tests/` owns the public contract; `#[cfg(test)] mod tests` owns internal logic;
doc tests are examples first.

The argument for insisting on some `tests/`: a unit test **cannot detect a
breaking public-API change**. It lives inside the crate and sees everything, so
you can gut the exports and every unit test still passes. `tests/` compiles as a
separate crate and can only reach the public API. That constraint is the value.

Any `.rs` file directly in `tests/` becomes its own test binary, so shared
helpers go in `tests/common/mod.rs`. Group by area — twenty tiny files is twenty
link steps.

## Names are assertions

`rejects_negative_periods`, not `test_parse`. The name is the diagnostic; it is
what you read in failure output. `redundant_test_prefix` enforces the absence of
the prefix, which also wastes the scan position where the eye lands.

## Comparing floats — the open question

`float_cmp` is a `pedantic` lint and `pedantic` is denied, so `assert_eq!` on an
`f64` result is expected to be a merge blocker throughout a numerics crate. The
`allow-*-in-tests` options in `clippy.toml` have no `float_cmp` member, so the
test exemptions do not cover it.

**Not yet probed.** This is reasoned from the lint's group membership, not
observed — there is no code to run it against yet. It is the first thing to
measure when the first formula lands, and the answer decides between a shared
approximate-comparison helper and a per-module `#[expect]` with a reason. Record
what is actually observed here, and delete this paragraph.

## Properties over points, where the property is real

Three shapes earn their keep: **round-trip** (`pv(fv(x)) == x`), **invariant**
(the output always satisfies P), and **oracle** (a slow, obviously-correct
reference). The inverse relationships between these formulas are round-trips,
which is the strongest shape because it can be stated without reimplementing
anything.

The failure mode is an oracle that duplicates the implementation — then you have
tested that two copies of the same bug agree. Easy to do accidentally when the
property is written after the code.

Where the domain is small, enumerate instead: it is clearer, faster, and the
failure names itself.

Commit `proptest-regressions/`. It converts a lucky random failure into a
permanent regression test.

## Pin every stated assumption

When a rustdoc line or a Concept in this bundle *asserts* a behaviour, that
assertion earns a test which fails the moment the code stops honouring it. A
claim in prose with nothing checking it is the thing this bundle exists to
prevent.
