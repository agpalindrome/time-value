# ADR-0051: `Installment`'s fields are private, read through accessors

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0027](0027-amortization-schedule.md) (the amortization
  schedule — the row's *content* is unchanged; only how it is read)
- **Follows:** [ADR-0042](0042-serde-support.md) /
  [ADR-0044](0044-schemars-support.md) (the wire format and its schema),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (make illegal states
  unrepresentable; test the class, not the instance),
  [ADR-0050](0050-role-newtypes-for-ambiguous-arguments.md) (role newtypes for
  transposable arguments)

## Context

An adversarial pre-publication review of the public API — reading the crate as a
downstream consumer would, and asking what the surface commits us to — found that
`amortization::Installment` was the **only public struct in the crate with public
fields**. All five were public:

```rust
pub struct Installment {
    pub period: u32,
    pub payment: Money,
    pub interest: Money,
    pub principal: Money,
    pub balance: Money,
}
```

Every other public type in the crate — `Money`, `Rate<P>`, `Period<P>`,
`FxRate`, `DatedCashflow`, `Schedule<P>` — keeps its fields private and exposes
`#[must_use] const fn` accessors. `Installment` was the outlier, and the
inconsistency was not the real cost.

Public fields on a struct that is neither `#[non_exhaustive]` nor sealed grant
downstream code two capabilities that outlive the decision to grant them:

1. **Construction by struct literal.** Any consumer can write
   `Installment { period, payment, interest, principal, balance }`.
2. **Exhaustive pattern matching.** `let Installment { period, payment, interest,
   principal, balance } = row;` compiles, and *stays* compiling only while the
   field set is exactly those five.

Both mean **adding a sixth field is a breaking change**. The field set is frozen
for the life of the major version.

That matters more here than it would for a typical value type, because an
`Installment` is **yielded**, not constructed: it is `Schedule<P>`'s
`Iterator::Item`. The only way a consumer obtains one is by iterating a schedule.
So the construction capability is of no use to them — nobody needs to fabricate a
row of a schedule they did not compute — while the pattern-matching capability is
exactly what they *will* reach for (`for Installment { period, balance, .. } in
schedule`), and it is the one that pins the layout. The crate gives away the
capability it cannot afford in exchange for one nobody wants.

And the layout is plausibly going to move. An amortization row has obvious
candidate additions: `cumulative_interest` and `cumulative_principal` (the
running totals every amortization table in practice shows alongside the split),
and the **opening** balance (the row currently reports only the closing one, so a
consumer wanting both must remember the previous row — the properties tests
already do exactly that bookkeeping). None of these is being added now. The point
is that the current shape offers no way to add them later without a major bump.

The crate's own `serde_impls.rs` also constructed and destructured `Installment`
by literal, so the crate itself depended on the public layout.

ADR-0045 asks of every new decision whether the wrong state can be made
unrepresentable at the chokepoint. The wrong state here is "downstream code
depends on the exact field set". The chokepoint is field visibility.

## Decision

**Make all five fields `pub(crate)` and add five accessors**, following the
crate's existing convention exactly — `#[must_use]`, `const fn`, taking `self`
(the type is `Copy`), a one-line rustdoc, and no `get_` prefix:

```rust
impl Installment {
    pub const fn period(self) -> u32;
    pub const fn payment(self) -> Money;
    pub const fn interest(self) -> Money;
    pub const fn principal(self) -> Money;
    pub const fn balance(self) -> Money;
}
```

The rustdoc that described each field moves verbatim onto its accessor, so
nothing documented is lost.

### `pub(crate)`, not module-private

`serde_impls.rs` is a **sibling module**, so strictly module-private fields would
be unreachable from the `Deserialize` impl, which must build an `Installment`
from the wire form. The two candidate fixes are a `pub(crate) fn new` constructor
or `pub(crate)` fields; we take the fields.

A `new` here would take **four positional `Money` arguments** — `payment`,
`interest`, `principal`, `balance` — which is precisely the transposable
signature [ADR-0050](0050-role-newtypes-for-ambiguous-arguments.md) was written
to eliminate, and it would need four role newtypes (two of which do not exist) to
be safe. Named-field construction at the two in-crate sites carries the same
information with no ambiguity and no new API. `pub(crate)` is invisible outside
the crate, so the external commitment is identical either way.

### Not `#[non_exhaustive]`

We **do not** add `#[non_exhaustive]`. With no public fields it is entirely
redundant: private fields already block both literal construction and exhaustive
matching from any other crate, which is the whole of what the attribute buys for
a struct. Adding it would suggest it is doing work that field visibility is not,
and no other type in this crate carries it. (`Currency` is `#[non_exhaustive]`,
but that is an *enum*, where the attribute does something visibility cannot —
keep the variant list open.)

### Testing (ADR-0045 rule 2)

The decision is pinned by a `compile_fail` doctest on `Installment` showing that
the struct literal no longer compiles — the instrument ADR-0045 names for an
invariant that lives in the type system, already used to lock the periodicity
mismatch and the role transpositions (ADR-0050) — plus a runnable doctest
exercising the accessors on a real schedule row.

## Consequences

- A field can be added to `Installment` in a minor release. Downstream code reads
  through accessors, which are additive; adding `cumulative_interest()` breaks
  nothing.
- The public API is uniform: every public type in the crate is now read through
  accessors, so there is one rule to learn rather than one type's exception.
- **This is a breaking change.** `installment.balance` becomes
  `installment.balance()`, and a struct literal or exhaustive `let`-destructuring
  of `Installment` no longer compiles. Nothing is published (ADR-0038), so no
  released API moves.
- **The wire format is unchanged.** `Installment` still serialises as
  `{ period, payment, interest, principal, balance }` via `InstallmentWire`
  (ADR-0042); the schema `schemars` emits is byte-identical (ADR-0044).
  `tests/serde.rs::installment_round_trips` passes with its expectations
  untouched.
- **The binaries' surfaces are unchanged.** Both `results.rs` DTOs read the same
  five values through accessors instead of fields; the CLI's plain and `--json`
  amortization output and the MCP `amortize` tool's description, input schema and
  output schema are byte-identical, verified by running both binaries before and
  after and diffing.
- Follow-on obligation: **a new public struct in this crate keeps its fields
  private** and exposes accessors, whether or not it currently has an invariant to
  protect. The reason is semver headroom, not validation.

## Alternatives considered

- **Leave it as-is, and accept the field set frozen at 1.0.** The honest version
  of this is "we will never want a sixth field", which the list of plausible
  additions above contradicts. It also costs nothing to fix *now*, while nothing
  is published, and cannot be fixed at all later without a major bump — the
  asymmetry decides it.
- **Add `#[non_exhaustive]` and keep the fields public.** This does block literal
  construction and exhaustive matching, so it recovers the ability to add a field.
  Rejected because it still publishes the field *set* as API: every existing field
  name and type stays a permanent commitment, `installment.balance` remains a
  supported read path, and a field could then never be *renamed*, *retyped*, or
  *removed* — only added. Accessors leave the representation genuinely free (the
  cumulative totals could be computed rather than stored, say). It is also the
  less uniform outcome: the type would still read differently from every other
  public type in the crate.
- **Make it generic — `Installment<P>`, tagged with the schedule's periodicity.**
  Superficially attractive, since `Schedule<P>` is already tagged and the rows come
  from a periodicity-tagged schedule. Rejected: **none of the five fields is
  periodicity-tagged** — four are `Money` and one is a bare `u32` index — so `P`
  would be a pure `PhantomData` marker carrying no arithmetic that could go wrong,
  which is the opposite of the ADR-0033 test for a compile-time tag (tag what the
  compiler can *check*). It would also ripple outward for nothing: `Installment` is
  serialised (ADR-0042), and a generic type would have to pick a representation for
  `P` on the wire or erase it, and both binaries' `ScheduleRow` DTOs would need the
  parameter threaded through purely to discard it.
- **Expose the row as a tuple or a `[Money; 4]`.** Smaller API, but it loses the
  names — the whole readability of `installment.interest()` — and is no more
  extensible than the struct.
