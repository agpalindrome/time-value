# ADR-0050: Role newtypes for transposable arguments

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Follows:** [ADR-0005](0005-domain-modelling-and-strong-typing.md) (domain
  modelling and strong typing — **extended**, not reversed, by this ADR),
  [ADR-0033](0033-core-domain-model-two-axes-and-an-f64-engine.md) /
  [ADR-0034](0034-money-and-currency.md) (the two axes: periodicity as a
  compile-time tag, currency as a runtime value),
  [ADR-0042](0042-serde-support.md) (the wire format),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (make illegal states
  unrepresentable; test the class, not the instance)

## Context

ADR-0005 names the bug this crate exists to catch: a **periodicity mismatch**,
silent because the arithmetic runs and returns a plausible number. Periodicity is
now a compile-time tag, so that mismatch does not compile.

A second bug of exactly the same shape is still wide open. Twelve public
functions take **two adjacent arguments of the same type** — two `Money` amounts,
or two `Rate<P>`s — and nothing distinguishes the positions:

```rust
annuity::periods(rate, payment, present)      // Rate<P>, Money, Money
single_sum::periods(rate, present, future)    // Rate<P>, Money, Money
annuity::growing_present_value(rate, growth, periods, payment) // two Rate<P>
Schedule::with_payment(rate, payment, principal)               // Money, Money
```

Transposing the pair compiles and returns a number, not an error:

- `annuity::periods(1%, PMT 100, PV 1125.508)` returns `Ok(12.000003)`; with the
  two amounts swapped it returns `Ok(0.0893)`. Both are "a number of periods".
- `annuity::growing_present_value(r = 5%, g = 2%, …)` returns `Ok(979.32)`;
  discounting at the growth rate and growing at the discount rate returns
  `Ok(1386.73)`. Both are "a present value".

Worse, the argument order is **not consistent between modules**, so a caller's
correct habit in one place is a bug in another: `single_sum::periods` takes
`(rate, present, future)` while the same-named `annuity::periods` takes
`(rate, payment, present)`. Nothing warns; both compile.

This is the periodicity mismatch's twin — a *semantic* error that survives type
checking, in the same call sites, with the same failure mode (a wrong answer that
looks right). ADR-0045 asks of every new decision whether the wrong state can be
made unrepresentable at the chokepoint. Here it can: the chokepoint is the
function signature.

## Decision

**Tag the ambiguous argument positions with zero-cost role newtypes**, in
`crates/time_value/src/roles.rs`, re-exported from the crate root:

```rust
pub struct Payment(pub Money);
pub struct PresentValue(pub Money);
pub struct FutureValue(pub Money);
pub struct Principal(pub Money);
pub struct Growth<P: Periodicity>(pub Rate<P>);
```

They apply to exactly the twelve functions where a swap is *expressible*:

| Ambiguity | Functions |
| --- | --- |
| two `Money` | `amortization::Schedule::with_payment(rate, Payment, Principal)`; `annuity::periods(rate, Payment, PresentValue)`; `annuity::periods_from_future(rate, Payment, FutureValue)`; `annuity::rate(periods, Payment, PresentValue)`; `annuity::rate_from_future(periods, Payment, FutureValue)`; `single_sum::periods(rate, PresentValue, FutureValue)`; `single_sum::rate(periods, PresentValue, FutureValue)` |
| two `Rate<P>` | `annuity::growing_perpetuity`, `annuity::growing_present_value`, `annuity::growing_future_value`, and the two `annuity::due::growing_*` — the **second** rate becomes `Growth<P>`; the first stays `Rate<P>` |

Four rules fix the shape:

1. **No `From<Money>` for any `Money` role, and no `From<Rate<P>>` for
   `Growth<P>`.** A blanket injection would let both argument orders compile
   again, which is the entire point. Construction is written out at the call
   site: `Payment(amount)`.
2. **Extraction is unrestricted** — the field is public, `From<Payment> for Money`
   (and `From<Growth<P>> for Rate<P>`) exists, and an inherent `.money()` /
   `.rate()` accessor reads it inline.
3. **No validation.** The inner value was already validated by `Money::new` /
   `Rate::new`; a role is a marker, not a second constructor. Each wrapper is the
   size of what it wraps (pinned by a test).
4. **Only ambiguous positions are tagged.** A function with at most one `Money`
   argument — `annuity::present_value`, `annuity::payment`, `Schedule::for_term`,
   the whole `single_sum` value pair — keeps taking a plain `Money`.

`modified_internal_rate_of_return` (borrowed and owned) is **deliberately left
unchanged**.

**Testing (ADR-0045 rule 2).** Each role pair gets a `compile_fail` doctest
proving the transposed call no longer compiles — the same instrument that already
locks the periodicity mismatch — plus one proving no `Money` → role conversion
exists.

## Consequences

- The transposition is a compile error, in the same way and by the same mechanism
  as the periodicity mismatch. The two silent-wrong-answer classes this crate
  cares about are now both caught by the compiler.
- Call sites read as their own documentation:
  `annuity::periods(r, Payment(pmt), PresentValue(pv))` states which amount is
  which, so the cross-module inconsistency between `single_sum::periods` and
  `annuity::periods` stops being a trap — the compiler enforces the difference
  the names describe.
- **This is a breaking change to the twelve signatures.** Existing calls must wrap
  their arguments; there is no conversion that would let them compile unchanged,
  by design. Nothing is published (ADR-0038), so no released API moves.
- **The wire format is untouched.** Roles exist only in function signatures; they
  are never serialised, never appear in a schema, and `serde_impls.rs` /
  `schemars_impls.rs` / `wire.rs` are unmodified (ADR-0042, ADR-0044).
- **The binaries' surfaces are untouched.** The CLI grammar and the MCP tool names,
  descriptions, and input/output schemas are identical; the roles are internal
  plumbing at the call sites, verified by running both binaries across every
  affected operation before and after and diffing the output.
- Follow-on obligation: a **new** operation taking two same-typed arguments should
  reach for an existing role, or add one — the same question ADR-0045 asks.

## Alternatives considered

- **Leave it, on ADR-0005's "no dimensional analysis" grounds.** ADR-0005 rejected
  full dimensional analysis because "TVM stays entirely in *money*; the extra
  machinery adds ceremony without catching a *semantic* error the marker approach
  misses". Role tagging is the case that **passes** that test rather than failing
  it: the argument swap *is* a semantic error the periodicity marker misses, it is
  demonstrated above with concrete wrong answers, and the ceremony is one
  constructor call at the few sites where the confusion is possible. So this
  extends ADR-0005's reasoning to a case it did not have in view; it does not
  reverse the decision. (Dimensional analysis proper — units on *every* quantity —
  remains rejected.)
- **A universal `Amount<Role>` parameterised over a role marker**, applied to
  every monetary quantity. Rejected on two counts. Roles are **not conserved
  through arithmetic** the way periodicity is: a periodicity survives every
  operation that touches a `Rate<Monthly>`, but the sum of a payment and an
  interest amount has no role at all, and a present value compounded forward
  becomes a future value — so the parameter would have to be erased and
  reintroduced constantly, and every `try_add` would need a rule for combining
  roles. And it would put a type parameter on `Money` itself, changing the
  ADR-0042 wire format (`{ amount, currency }`) and every schema built from it,
  for a distinction that is meaningless once the value leaves the call.
- **A parameters struct per operation** (`annuity::periods(PeriodsArgs { rate,
  payment, present })`). It does prevent the transposition — named fields cannot
  be reordered — but it costs a struct per operation (a dozen new public types,
  each mirroring one signature), makes every call multi-line, and gives the
  binaries a second layer to construct. The role newtypes are reusable across
  operations, where a params struct is not.
- **Tag only the `Rate`/`Growth` pair** (the confusion with the largest numeric
  divergence) and leave the `Money` pairs. Rejected: the `Money` transpositions
  are the more likely mistake, because the argument order genuinely differs
  between `single_sum` and `annuity`.
- **Runtime checks or debug assertions on the ordering.** There is nothing to
  check — both orders are numerically legitimate inputs. Only the *intent*
  distinguishes them, and intent is what a type records.
