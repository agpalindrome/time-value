# ADR-0043: Owned cashflows — `OwnedCashflows` behind an `alloc` feature

- **Status:** Accepted (amended by [0060](0060-owned-cashflows-on-the-wire.md) — the
  wire format it deferred; and by [0065](0065-dated-counterparts.md) — an
  `OwnedDatedCashflows` built to the same pattern, for the dated series)
- **Date:** 2026-07-14
- **Deciders:** Project owner
- **Follows:** [ADR-0013](0013-core-api-values-and-discrete-operations.md) (the
  borrowed `Cashflows` and its operations), [ADR-0009](0009-no_std-and-optional-libm.md)
  (the feature set / `no_std` posture)

## Context

[`Cashflows<'a, P>`](../../crates/time_value/src/cashflows.rs) **borrows** a
`&[Money]`, which keeps the core `no_std` and allocation-free (ADR-0013). That is
the right default, but it forces the caller to own the backing slice for the
series' whole lifetime — awkward when the flows are **produced by an iterator**,
returned from a function, or otherwise not already sitting in a slice the caller
can lend. Issue #24 asks for an owned complement, gated so the default build stays
allocation-free.

## Decision

**Add an off-by-default `alloc` feature and an owned `OwnedCashflows<P>`** that
complements — does not replace — the borrowed `Cashflows`.

- **`alloc` feature.** Pulls in the `alloc` crate (`extern crate alloc`) without
  requiring `std`. **`std` implies `alloc`** (`std = ["alloc"]`) — `std` is a
  superset. The default build is unchanged: `no_std`, no allocation.
- **`OwnedCashflows<P>`** owns a `Vec<Money>` with the same periodicity tag `P`. It
  is built from a `Vec` (`new` / `From<Vec<Money>>`), an **iterator**
  (`FromIterator<Money>`), or a borrowed series (`From<Cashflows<'_, P>>`), and
  yields its data back via `as_slice` / `into_vec`.
- **Operations are not reimplemented.** `OwnedCashflows` lends a borrowed
  [`Cashflows`] view through `as_cashflows(&self) -> Cashflows<'_, P>`, and its
  operation methods (`net_present_value`, `net_future_value`,
  `internal_rate_of_return[_from]`, and `modified_internal_rate_of_return`) are
  **one-line forwards** to that view. The borrowed type stays the single source of
  truth for the math (ADR-0013); the owned type is storage + ergonomic forwarding.
  A new `Cashflows` operation should gain a matching forward here.
- **`mirr` stays feature-gated.** The owned `modified_internal_rate_of_return`
  forward is gated `all(feature = "alloc", any(feature = "std", feature = "libm"))`,
  matching the borrowed `mirr`'s transcendental requirement (ADR-0026).

**`no_std` is preserved and verified.** `OwnedCashflows` uses `alloc::vec::Vec`
only; the feature composes with a `no_std` build. A CI clippy check
(`--no-default-features --features alloc,libm`) compiles the owned type — including
its `libm`-gated `mirr` forward — without `std`.

**serde is out of scope here.** ADR-0042 excluded the cashflow aggregates from the
`serde` wire format. `OwnedCashflows` *could* round-trip (it owns its data), but a
serialized series shape is a separate, additive decision; this ADR is the owned
collection only.

## Consequences

- Callers can build a series from an iterator or hold one without keeping a slice
  alive, at the cost of one allocation — opt-in behind `alloc`.
- No behavioural or API change to the default `no_std` build, nor to the borrowed
  `Cashflows`; this is purely additive.
- The owned type introduces **no new numeric code** (it delegates), so there is no
  std/libm numeric-divergence surface to test separately (unlike ADR-0021's
  concern for the arithmetic itself) — the clippy compile check suffices, and the
  forwards are exercised under `--all-features`.
- A new feature configuration to keep green: `no_std + alloc(+libm)` (CI clippy).
- Forwarding couples the two types' operation surfaces by hand (a new `Cashflows`
  op must be forwarded), accepted as cheaper and clearer than a shared trait or a
  storage-generic `Cashflows`.

## Alternatives considered

- **A storage-generic `Cashflows<S>`** (over `&[Money]` vs `Vec<Money>`) — unifies
  the two but complicates the type that ADR-0013 deliberately kept simple, and
  churns the existing borrowed API. Rejected; a separate owned type is additive and
  leaves `Cashflows` untouched.
- **View-only owner (no forwarding), operations via `as_cashflows()` only** —
  smaller and avoids the hand-coupling, but an owned series you cannot call
  `.net_present_value()` on directly is clunky, against the crate's "type-heavy
  *and* friendly" principle. Rejected; the forwards are thin and worth it.
- **`Deref<Target = Cashflows>`** — the natural "owned derefs to borrowed" pattern
  (à la `Vec`/`[T]`), but impossible here: `Cashflows` carries a lifetime, so there
  is no stored `Cashflows` value to hand back a `&`. Rejected as unrepresentable.
- **Reimplement the operations on the owned `Vec`** — duplicates the NPV/NFV/IRR/
  MIRR math and invites drift. Rejected; forward to the one implementation.
- **Include `serde` for `OwnedCashflows` now** — plausible (it can round-trip), but
  it is a separable wire-format decision beyond this issue. Deferred.
