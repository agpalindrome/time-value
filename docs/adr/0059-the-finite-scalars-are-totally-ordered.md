# ADR-0059: The finite-by-construction scalars are totally ordered

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0035](0035-periodicity-tagged-time.md) (`Period<P>` — whose
  derived `PartialOrd` turns out never to have worked, and is replaced here)
- **Follows:** [ADR-0005](0005-domain-modelling-and-strong-typing.md) (the
  validated newtypes and their periodicity tag — **extended**, not reversed: this
  adds the comparison surface the tag was always meant to make safe),
  [ADR-0032](0032-ergonomic-convenience-impls.md) (an impl earns its
  place by removing ceremony from a real call site),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (pin every stated
  assumption; a compile error is a test),
  [ADR-0033](0033-core-domain-model-two-axes-and-an-f64-engine.md) (the `f64`
  approximate-real contract)
- **Closes:** issue #103

## Context

`Rate<P>` derived `Clone, Copy, PartialEq` and nothing else, so

```rust
if irr > hurdle { … }   // E0369: binary operation `>` cannot be applied to `Rate<Monthly>`
```

did not compile. Comparing a solved rate with a threshold — an IRR against a
hurdle, a quoted rate against a cap — is close to the most common thing anyone
does with a rate, and the only route was `irr.value() > hurdle.value()`, which
drops to bare `f64` and discards the periodicity tag that is the crate's whole
point (ADR-0005). An adversarial pre-publication API review raised it as issue
#103.

The issue framed this as an asymmetry — `Rate<P>` being the one scalar without an
ordering, since `Period<P>` and `ContinuousRate` both derive `PartialOrd` and
`Money` has a hand-written one. **Implementing it showed the asymmetry was worse
than reported, and in a different place.**

### `Period<P>`'s derived `PartialOrd` never worked

A `#[derive]` on a generic type bounds every type parameter by the trait being
derived. `Period<P>` holds a `PhantomData<P>`, so its derive expands to

```rust
impl<P: Periodicity + PartialOrd> PartialOrd for Period<P> { … }
```

and **no periodicity marker implements `PartialOrd`** — the markers derive
`Debug, Clone, Copy, PartialEq, Eq, Hash` (`periodicity.rs`), and `Periodicity`
does not require it. So the bound is unsatisfiable for every `P` in the sealed
set, and

```rust
let a = Period::<Monthly>::new(1.0)?;
let b = Period::<Monthly>::new(2.0)?;
a < b   // E0369, with a note: the foreign item type `Monthly` doesn't implement `PartialOrd`
```

fails with the *same* error issue #103 reports for `Rate`. The derive was
decorative: it appeared in the type's declaration, satisfied nobody, and could not
be called. Verified directly rather than reasoned about.

The same trap has already been met once from the other side: `Period<P>`'s
*derived* `PartialEq` carries a `P: PartialEq` bound, which the markers happen to
satisfy today — so equality works, by luck of what the marker macro chose to
derive, not by anything the `Periodicity` contract promises.

### Why the ordering is total, not partial

`Money`'s ordering is genuinely partial and its hand-written impl says so: two
distinct non-`Xxx` currencies do not combine, so the comparison has no answer and
returns `None` (ADR-0034). Nothing analogous applies to the three scalars here:

- **Finite by construction.** `Rate::new` rejects any non-finite value (and
  anything `<= -1.0`), `Period::new` rejects non-finite and negative counts, and
  `ContinuousRate::new` rejects non-finite forces. `NaN` — the single `f64` value
  that makes a comparison undefined and equality irreflexive — is therefore
  unrepresentable in all three.
- **No cross-tag comparison to worry about.** For `Rate<P>` and `Period<P>` the
  periodicity is part of the type, so `Rate<Monthly>` against `Rate<Annual>` is a
  type mismatch (E0308) before any trait is consulted. `ContinuousRate` is
  periodicity-free (ADR-0036), so any two are comparable.

So these are exactly the types for which `Eq` and `Ord` — normally wrong on a float
newtype — are sound.

## Decision

**`Rate<P>`, `Period<P>` and `ContinuousRate` implement `PartialEq`, `Eq`,
`PartialOrd` and `Ord`, by hand, bounded only by `Periodicity` where there is a
tag. `Money` keeps its partial order. Nothing gets `Hash`.**

### One `cmp`, everything else delegating

```rust
impl<P: Periodicity> Ord for Rate<P> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.per_period.partial_cmp(&other.per_period).unwrap_or(Ordering::Equal)
    }
}

impl<P: Periodicity> PartialOrd for Rate<P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) }
}

impl<P: Periodicity> PartialEq for Rate<P> {
    fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}

impl<P: Periodicity> Eq for Rate<P> {}
```

Four properties of that shape are deliberate:

- **`f64::partial_cmp`, not `f64::total_cmp`.** `total_cmp` orders `-0.0` strictly
  below `+0.0`, while `PartialEq` calls them equal — which would make `cmp` return
  `Less` for a pair `eq` reports as equal, breaking the consistency `Ord`'s
  contract requires between the two. `partial_cmp` gives `Equal` for the signed
  zeros and so agrees with equality everywhere.
- **`unwrap_or(Ordering::Equal)`, not `expect`.** The branch is unreachable
  (both operands are finite), and a fallback keeps the impl panic-free by
  construction rather than by a message nobody will read.
- **`PartialOrd` is `Some(self.cmp(other))`.** The canonical form once `Ord`
  exists, and what `clippy::non_canonical_partial_ord_impl` asks for.
- **`PartialEq` routes through `cmp`** rather than `self.per_period ==
  other.per_period`, so there is no float `==` in the crate to allow
  `clippy::float_cmp` around.

### Hand-written, because the derive cannot get there

`Ord` on these types can never be derived: a derive would require `f64: Ord`. That
settles the question for `Ord`, and once `Ord` is hand-written the rest has to
follow it:

- `Ord: Eq + PartialOrd`, so an `impl<P: Periodicity> Ord` needs `Rate<P>: Eq` for
  *every* `P: Periodicity`.
- `Eq: PartialEq`, so it needs `Rate<P>: PartialEq` on the same bound — which the
  **derived** `PartialEq` does not provide, because it carries `P: PartialEq`.

So the derived `PartialEq` on `Rate<P>` and `Period<P>` is replaced by a
hand-written one. This is not a behaviour change: the derive compared the `f64`
field and a `PhantomData` (always equal), which is what `cmp(…).is_eq()` computes
for every representable value — the two differ only on `NaN`, which no constructor
admits. What changes is the *bound*, from `P: Periodicity + PartialEq` to
`P: Periodicity`, which is strictly wider, so no call site that compiled before
stops compiling.

`ContinuousRate` has no type parameter and so no bound to shed; its derived
`PartialEq` is kept, and only `Eq`/`Ord`/`PartialOrd` are added. Its `PartialOrd`
is hand-written too, because a derived `PartialOrd` beside a manual `Ord` is what
`clippy::derive_ord_xor_partial_ord` exists to catch.

The alternative — adding `PartialOrd, Ord` to the periodicity markers' derive, so
the existing derives would start working — is rejected below.

### `Money` keeps `PartialOrd` only

The rule this ADR states reads off the value's domain: **a scalar whose
constructor admits no `NaN` and whose comparison cannot be refused is totally
ordered; a value whose comparison can be refused is not.** `Money`'s can be
refused — `100 USD` against `100 EUR` has no answer — so `Money` stays partial and
gets no `Eq` and no `Ord`. The asymmetry between `Money` and the three scalars is
therefore the *stated* one, not the accident issue #103 found.

### No `Hash`

`Eq` invites `Hash`, and `Hash` is declined. `+0.0 == -0.0` while their bit
patterns differ, so a correct `Hash` would have to normalise the zero before
hashing — a small amount of code carrying a permanent obligation to stay in step
with `eq`, in exchange for a `HashMap<Rate<P>, _>` nothing in the crate or its
binaries wants. `Currency` remains the crate's one hashable type, where the key is
a fieldless enum and the question does not arise.

### Testing (ADR-0045 rule 2)

- The **class**, via proptest: for arbitrary pairs, each type's `cmp` equals the
  `partial_cmp` of the wrapped `f64`, `PartialOrd` agrees with `Ord` and never
  answers `None`, and `<`/`>`/`==` agree with the raw comparison. Over arbitrary
  *triples* of rates, the total-order laws themselves — reflexivity, totality
  (exactly one of less / equal / greater), antisymmetry via
  `x.cmp(&y) == y.cmp(&x).reverse()`, and transitivity — because those are the
  obligations that make `Eq`/`Ord` on a float newtype sound, and they are sound
  only as long as `NaN` stays unrepresentable.
- The **signed zeros** are pinned for all three: equal under `PartialEq` *and*
  `Equal` under `cmp`. That pair is the one place `total_cmp` would have split the
  two traits, so it is the regression test for the choice above.
- The **ergonomic** the issue asked for is pinned at a call site and in a doctest:
  `irr > hurdle`, plus the `Ord`-only surface (`max`, `min`, `clamp`,
  `sort_unstable`) that `PartialOrd` alone would not provide.
- The **compile error** that keeps the tag honest is pinned as a `compile_fail`
  doctest on each tagged type — `monthly > annual` must not compile. The failure
  was checked to be the intended one (E0308, mismatched `Rate<Monthly>` /
  `Rate<Annual>`) rather than an unrelated error, which is the standing hazard with
  `compile_fail`.

## Consequences

- `irr > hurdle` compiles, and so do `rate.max(cap)`, `rate.clamp(floor, cap)`,
  `rates.sort_unstable()` and `BTreeMap<Rate<P>, _>` — without unwrapping to `f64`
  and without a periodicity mismatch becoming possible.
- **`Period<P>`'s ordering works for the first time.** No code can have depended on
  the derive it replaces, because no code could call it.
- Purely additive. Four trait impls appear on three types; no signature, behaviour
  or rendering changes, and the CLI and MCP surfaces are untouched (neither
  compares a scalar).
- The generic bounds on `Rate<P>`'s and `Period<P>`'s comparison traits are now
  `P: Periodicity` alone, so a future periodicity marker cannot silently lose
  equality by omitting a derive from the macro.
- `Eq` says equality is exact, which for an `f64` it is: two rates that are
  mathematically equal but computed by different routes may still compare unequal.
  That was already true of the derived `PartialEq` and is inherent to ADR-0033's
  approximate-real contract; `Eq` adds a law about the relation, not a claim about
  numerical accuracy.
- The soundness of `Eq`/`Ord` rests entirely on the constructors rejecting
  non-finite values. That is now a **load-bearing invariant**: admitting a `NaN`
  into any of these three types — through a new constructor, or by loosening
  `new` — would silently break reflexivity and the `Ord` contract. The internal
  `from_valid` constructors are the ones to watch, and the total-order properties
  are the test that would notice.
- `ContinuousRate` is included even though its `PartialOrd` was functional, so the
  rule holds uniformly across the three finite scalars rather than leaving one of
  them a case to remember.

## Alternatives considered

- **`PartialOrd` only, no `Eq`/`Ord`.** The minimum that closes issue #103, and the
  conservative choice: it leaves the "never put `Eq` on a float" habit intact and
  avoids the load-bearing invariant above. Rejected because the ordering *is* total
  and saying otherwise costs the caller real API — `max`, `min`, `clamp`, the slice
  sorts and `BTreeMap` all require `Ord`, and a caller who wants `rate.max(cap)`
  would be pushed back to `f64` for exactly the operation the newtype should serve.
  The invariant it depends on is already enforced at the chokepoint the crate
  designed for it (ADR-0045), and is now under test.
- **Add `PartialOrd, Ord` to the periodicity markers' derive.** One line in
  `periodicity.rs` makes `Period<P>`'s existing derive start working and would let
  `Rate<P>` derive `PartialOrd` too. Rejected on three counts: it fixes the symptom
  by satisfying a bound rather than by stating an intent; it puts a public ordering
  on `Annual`, `Monthly` and the rest — a comparison of *type tags*, which means
  nothing and would be a promise to keep; and it does not reach `Ord` on the
  scalars anyway, since that needs `f64: Ord`. The bound would remain in the impls'
  signatures, waiting for the next marker whose derive list drifts.
- **`f64::total_cmp`.** The idiomatic way to get a total order out of a float, and
  wrong here: it separates `-0.0` from `+0.0`, contradicting the `PartialEq` these
  types already have, so `cmp` and `eq` would disagree and `Ord`'s contract would
  be violated on a pair both constructors accept. Making `PartialEq` bit-exact to
  match would be a real behaviour change, for the sake of distinguishing two values
  that are the same number.
- **A newtype ordering key** (`rate.ordering_key() -> OrderedF64`) instead of the
  traits. Explicit, and unusable: it does not make `irr > hurdle` compile, which is
  the entire request.
- **`Hash` alongside `Eq`.** Discussed above; declined for want of a use case, and
  because the signed-zero normalisation is a standing correctness obligation with
  nothing on the other side of the ledger.
- **Give `Money` `Eq` too** (its derived `PartialEq` is reflexive, so the law
  holds). Rejected as an impl with no purpose: `Money` cannot have `Ord`, so `Eq`
  would unlock only `Hash`, which is declined here for every type. Leaving `Money`
  at `PartialEq`/`PartialOrd` also keeps the boundary legible — partial order,
  partial equality vocabulary, one story.
- **Fix `Rate` alone and leave `Period` and `ContinuousRate` for a follow-up
  issue.** The narrowest reading of #103. Rejected because #103's own premise —
  "every sibling type has it" — is false for `Period`, so closing the issue without
  touching `Period` would leave the reported asymmetry in place with the roles
  swapped, and would leave an ADR stating a rule that two of the three types it
  covers do not follow.
