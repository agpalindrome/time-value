---
type: Quantity
title: Amount
description: A quantity of money located at a point in time.
tags: [money, numerics]
status: stable
verified:
  - { by: human:ojhermann, at: 2026-08-07T02:37:35Z }
  - { by: human:ojhermann, at: 2026-08-07T15:13:35Z }
  - { by: human:ojhermann, at: 2026-08-07T15:40:47Z }
  - { by: human:ojhermann, at: 2026-08-07T18:50:30Z }
  - { by: human:ojhermann, at: 2026-08-07T19:07:03Z }
  - { by: human:ojhermann, at: 2026-08-07T22:22:09Z }
  - { by: human:ojhermann, at: 2026-08-08T15:26:25Z }
generated: { by: claude/opus-5, at: 2026-08-08T15:21:30Z }
sources:
  - id: wikipedia-fv
    resource: https://en.wikipedia.org/wiki/Future_value
    title: Future value — Wikipedia
    author: team:wikipedia-contributors
    last_modified: 2026-08-04
---

# What it is

A quantity of money located at a point in time.

`PV` and `FV` in [future value](future-value.md) are the same kind of thing,
differing only in where they sit. One Amount enters and one leaves.

**Decided — the location is not carried by the Amount.** It is supplied by
whatever operates on it. Were it intrinsic, `PV` and `FV` would be
distinguishable as values and could not be one kind; keeping it extrinsic is
precisely what lets a single Amount serve both roles.

**Most of what follows is derived or decided here, not stated by the source.**
The source gives the formula and nothing else — no domain, no arithmetic, no
representation. Claims that do come from it carry a footnote; everything else is
ours, and is open to revision on evidence.

# The unit is forced, not chosen

`FV` necessarily carries `PV`'s unit. This is not a convention: `(1 + rt)` is
dimensionless,[^wikipedia-fv] so multiplying by it cannot change the dimension
of what it multiplies. A formula that returned a different unit would be a
different formula.

What the unit _is_ — whether a currency is named, and whether that name is
checked — is a separate question, answered next.

# Both readings of "same unit" stay admissible

**Decided — an Amount's unit may be a named, checked currency _or_ an assumption
the caller honours, and nothing built here may foreclose either.** Both are
legitimate. A model working throughout in one implied currency has nothing to
check and should not pay for a check. One combining amounts drawn from different
places needs exactly that check.

Nothing forces the choice while a single Amount enters and a single Amount
leaves, as it does in [future value](future-value.md). It becomes forced where
two Amounts meet: addition, subtraction, comparison, and Amount ÷ Amount.

**Derived — whatever represents an Amount keeps its magnitude private.** A
private magnitude is what allows a currency to be added later without altering a
single existing signature. Exposing it directly would forfeit that, and with it
the flexibility above. This is a constraint on the implementation, not a
preference.

**Decided — a read accessor exists, and is a deliberate hole.** The field stays
private, so the invariant holds and cannot be circumvented. But a method handing
the number out puts every operation listed below as _unimplemented_ — including
the meaningless ones — within reach of a caller willing to do the arithmetic
themselves.

The accessor stays because a value type nobody can get a value out of does not
get used, and the alternative of routing every caller through rendering and
parsing is worse. What it costs is worth stating exactly: once an Amount records
a currency, the accessor returns a value stripped of its unit while every
existing call site goes on compiling, and adding two magnitudes drawn from
different currencies is precisely the mistake this type exists to prevent. So
"without altering a single existing signature" is true of the signatures and not
of their meaning.

**Decided — the accessor is renamed when a currency lands.** Once the number it
returns is a partial view, its name is the only warning left, and it should read
wrong at a call site doing arithmetic with it. That is the trigger; there is
nothing to rename while the number is still the whole value.

**Derived — reaching through the hatch is evidence an operation is missing, not
that the hatch is acceptable.** This library's own test used it to divide two
Amounts — an operation named in the table below as true and merely
unimplemented. That was the argument for implementing it rather than for leaving
the hatch to absorb the need. The same reading applies to the next one.

**Derived — when a currency is named, it is a value the Amount carries, not a
tag in its type.** A currency is chosen while the program runs: parsed from an
argument, read from a record, received over a wire. A type-level tag cannot
express something unknown until run time, so it would serve hand-written models
and fail every interface that accepts input.

The _period_ shared by `r` and `t` was recorded here as the opposite case — a
period being fixed when a model is written, and so a candidate for a type-level
tag. **That was too strong.** A command line takes a period as an argument and a
wire protocol takes it as a field, exactly as either does a currency, so the
same objection applies to both. The two questions turned out to be alike after
all. See [future value](future-value.md), where the period is settled.

# An absent currency means unrecorded, never unitless

**Decided — absence records that nobody said, and asserts nothing.** Money is
always denominated; there is no unitless sum of money. So an Amount carrying no
currency is not claiming to be unitless — that claim would be false about the
domain. It is only reporting that the unit was not written down.

**Decided — an unknown unit is not silently resolved.** An Amount bearing a
currency does not combine with one bearing none. Adopting the named currency
would turn "nobody said" into "it is USD" with no signal, and if the unrecorded
amount was in fact EUR, the result is a wrong number that never announces
itself.

The cost of this lands precisely where it should. A caller who names no
currencies never mixes and pays nothing. A caller who names them all pays
nothing. Only _partial_ recording is refused — and partial recording is the
ambiguity worth catching, not an inconvenience the rule creates.

**Derived — this does not oblige an Amount to carry a currency.** That money
always _has_ a unit is a fact about the domain; whether a program _records_ it
is an engineering choice with a known cost. A model working throughout in
implied dollars is not wrong — its unit lives in the modeller's head rather than
in the data. Requiring the attribute would conflate the two and reverse the
decision above.

# Domain

- **Always a definite quantity.** A representation's non-values are not Amounts.
  In binary floating point that excludes NaN and the infinities, which propagate
  silently through every subsequent operation, so a single one voids every claim
  downstream of it without announcing itself. A decimal representation has no
  such values, and satisfies the rule with nothing to exclude. "Must be finite"
  is one representation's spelling of this rule, not the rule.
- **May be negative.** An outflow, or a liability. The source does not forbid it
  and the formula stays coherent under it; excluding it would be a policy
  nothing here supports.
- **May be zero.** It is the fixed point — `FV = 0` for any accumulation factor.
- **Unbounded in the domain, bounded in any representation.** Nothing about
  money caps its magnitude. This gap between the domain and its representation
  is the entire source of the partiality below.

# Arithmetic

What makes sense, given two Amounts of the same unit and a dimensionless number:

| operation              | result        | note                                                          |
| ---------------------- | ------------- | ------------------------------------------------------------- |
| Amount + Amount        | Amount        |                                                               |
| Amount − Amount        | Amount        | `FV − PV` is the interest earned, itself an Amount            |
| Amount × dimensionless | Amount        | the only operation [future value](future-value.md) requires   |
| Amount ÷ dimensionless | Amount        |                                                               |
| Amount ÷ Amount        | dimensionless | the route to inverting for `r` or `t`, since `FV/PV = 1 + rt` |
| Amount vs Amount       | ordering      |                                                               |

What does not make sense:

- **Amount × Amount.** Money squared has no referent. An Amount is therefore not
  a number that happens to be labelled; it is arithmetically a different kind of
  thing.
- **Amount + dimensionless.** No referent either. It is why the `1 +` sits
  _inside_ the parentheses: the formula adds a dimensionless number to a
  dimensionless number, then multiplies.
- **Amounts of different units**, without an explicit conversion.

**Derived — changing the unit and keeping it are the same operation.** What
distinguishes them is the dimension the multiplier carries:

| multiplier's dimension | effect on the unit | example          |
| ---------------------- | ------------------ | ---------------- |
| 1 (dimensionless)      | unchanged          | `(1 + rt)`       |
| `unitA / unitB`        | changed to `unitA` | an exchange rate |

So converting between currencies is not a new kind of operation needing its own
machinery. It is Amount × multiplier, with a multiplier that happens to carry a
ratio of units — and the dimensionless case is the one where that ratio is 1.

This also locates the quantity correctly. A currency is a unit and has no
magnitude; the _rate_ is the quantity, and it is what does the converting.

# Every Amount-returning operation is partial

Because the domain is unbounded and any representation is not, each operation
above that yields an Amount can leave the domain. Amount ÷ Amount is separately
partial at a zero divisor.

**Leaving it at the bottom counts as much as leaving it at the top.** Overflow
is conspicuous — the result is an infinity, which the rules here already
exclude. Underflow is not: a small enough amount scaled by a valid factor
becomes zero, which is a perfectly ordinary Amount, so nothing announces that
the value was lost. Worse, it is _signed_ zero, and a liability that underflows
stops comparing as less than zero — so a guarantee about carrying the sign
quietly stops holding. Both ends are failures and both are reported.

This is a property of the domain meeting a representation, not of the
mathematics. It is stated here so that whatever represents an Amount cannot
quietly pretend the operations are total.

**Derived — the required operations are stated as _checked_, not in terms of
finiteness.** The two representations fail differently: binary floating point
produces NaN or an infinity, a decimal one overflows. Phrasing the requirement
as "is finite" describes only the first. Phrasing it as arithmetic that can
report failure describes both, and is the only phrasing under which a single set
of requirements covers more than one representation.

# Order holds within a unit, not across units

Excluding non-values is what makes ordering possible at all: NaN is unordered
against everything, itself included, so admitting it would cost comparison and
not merely arithmetic. A representation without non-values is ordered already.

**Decided — Amounts are ordered within a unit, and not across units.** Two
Amounts in the same named currency compare. Two Amounts that both record no
currency compare, since the caller's assumption is that they share one. An
Amount in one currency against an Amount in another does not compare, and
neither does a currency-bearing Amount against one carrying none.

Comparing across currencies is not a comparison that this library declines to
make. It is not a comparison at all until the units agree, and making them agree
is the conversion above — an exchange rate applied first, then an ordinary
comparison within the resulting unit.

**This amends an earlier claim.** Ordering was first recorded here as _total_,
which was decided with currency out of view. It is not: it is total within a
unit and undefined across units. The earlier statement was right about what
excluding non-values buys and wrong about how far it reaches.

# Equality

**Decided — equality is exact, and inherited from the representation.** Two
Amounts are equal when their representations hold the same value.

**Decided — equality includes the unit, and stays total across units.** An
amount in one currency is not equal to the same magnitude in another; they are
unequal, which is a definite answer. This is why equality survives the unit
question that [ordering](#order-holds-within-a-unit-not-across-units) does not:
"are these the same?" has an answer across currencies, and "which is larger?"
does not.

This is deliberately **not** the claim that they are the same amount of money.
Two amounts reached by mathematically identical routes can differ in the last
bits — `PV(1 + rt)` against `PV + PV·rt` is exactly such a pair — and this
equality calls them different. Nothing weaker is available without giving up
more than it buys.

**Derived — barring non-values makes equality reflexive.** NaN is the only
binary floating-point value unequal to itself, so excluding it gives an Amount
full equality rather than the partial form floating point normally forces.

This was first written as also yielding a _total order_, and as making "sorting,
deduplication and use as a key available at all". Both were too strong.
[Ordering holds within a unit and not across](#order-holds-within-a-unit-not-across-units),
so a total order is a promise this bundle has already declined to make — and
hash-keyed use needs a hash that agrees with equality, which means one that maps
negative zero and zero together, since they are equal here. Ordered collections
work; hashed ones need that hash written deliberately, and a derived one would
be wrong.

**Derived — the order must be the one that agrees with equality.** The obvious
tool for totally ordering binary floats places negative zero below zero, while
equality calls them the same. An order that disagrees with equality is a broken
contract, and it fails quietly — in lookups and sorts, far from the call that
caused it. Negative zero and zero are the same sum of money, and the order must
say so.

**Decided — approximate comparison exists, is named, and is never the
equality.** Comparison within a tolerance is not transitive, so it is not an
equivalence relation and cannot serve as equality without breaking every
structure that assumes one. It is a separate operation, asked for explicitly.

This is what a test comparing a computed Amount against an expected one wants.
Equality is the wrong instrument there, and reaching for it is the common error.

**Measured 2026-08-07 — the toolchain enforces this, but not everywhere.**
Clippy's `float_cmp` fires on `assert_eq!` between two binary floats inside a
`#[test]`, and under a denied `pedantic` group that is a hard error, not a
warning. The `allow-*-in-tests` options in `clippy.toml` have no member covering
it, so the usual test exemptions do not apply.

**Corrected 2026-08-07 — the earlier claim that a test suite here "cannot reach
for equality even by accident" was too strong**, and it failed in the way an
unconditional claim about a tool usually does: the tool has a condition nobody
looked for. `float_cmp` suppresses itself based on the **enclosing item's
name**. Measured on clippy 0.1.97 under this repository's configuration, a
`#[test] fn` named `eq`, `ne` or `is_nan`, or whose name starts `eq_` or ends
`_eq`, compiles clean while comparing two computed `f64` — and the identical
body in a function named anything else is a hard error. `float_cmp_const` does
not close the gap: comparing a computed value against a literal inside an
exempt-named function is silent too.

Those five forms are the whole of the condition, not a sample of it: clippy's
`float_cmp.rs` at the `rust-1.97.0` tag reads the parent item's name and returns
early on exactly `eq`, `ne`, `is_nan`, a `eq_` prefix or a `_eq` suffix. Each
was also run here — the list is read from the source and confirmed against the
compiler, since either alone has been wrong before.

So the lint agrees with the reasoning above, from an entirely different
direction, **for as long as the test is not named after the thing it is
testing** — which is exactly what a test of equality would be called. Nothing in
`crates/` currently sits in the exempt shape, so this is latent rather than
live; it is recorded because a guarantee believed to be unconditional is the
kind that is never re-checked.

# Approximate comparison is two operations, not one

"Close enough" is two different questions wearing one word, and they take
different tolerances for different reasons.

**Decided — they are separate, named operations rather than one with a
configurable tolerance.** A single parameterised comparison hides which question
a call site is asking, and the two are not interchangeable:

| question                           | tolerance                        | kind of fact                       |
| ---------------------------------- | -------------------------------- | ---------------------------------- |
| Is this the same sum of money?     | a minor unit — "to the penny"    | domain, and specific to a currency |
| Did the arithmetic lose precision? | relative, or representable steps | representation                     |

The first is absolute and its size is a property of the currency: `0.01` for
USD, `1` for JPY, `0.001` for KWD. It has nothing to do with floating point. The
second is what a test asking whether `FV` was computed correctly wants.

**Decided — the settlement comparison is deferred with currency**, since its
tolerance is a currency's minor unit and no currency is represented yet.

# The numerical comparison is a hybrid, and neither pure form works

**Decided — `|a − b| ≤ max(absolute, relative × max(|a|, |b|))`.** Both terms
are load-bearing, and both failure modes they guard against are reachable here
rather than hypothetical:

- **Absolute alone breaks at scale.** Near `1e20` a single representable step is
  roughly `16384`, so an absolute tolerance of `0.01` sits far below one step
  and the comparison silently degenerates into exact equality — rejecting values
  that are already as close as the representation permits.
- **Relative alone breaks at zero.** Comparing a computed `1e-300` against an
  exact `0` is a relative error of 1, so it always rejects. And zero is a
  legitimate Amount: `PV = 0` gives `FV = 0` exactly, and `FV − PV` at a one-day
  rate is a near-zero amount.

Amounts legitimately span cents to trillions, so both ends are in range in
ordinary use.

**Derived — the tolerance cannot be counted only in representable steps.** That
measure is precise and scale-free, and it is meaningful solely in binary
floating point. Adopting it as _the_ definition would quietly narrow the
decision that the representation is the caller's choice. It stays available as a
way to express a tolerance, never as the only one.

**Decided — no default tolerance is supplied.** A tolerance is a judgement about
what error is acceptable, and this library cannot know whether a caller is
reconciling a ledger or checking that a solve converged. The caller names it.
That costs one argument and prevents a silently wrong answer, which is the trade
made everywhere else here.

**Decided — the two terms are carried by a type, built one named term at a
time.** Passed as two adjacent numbers they can be transposed, and a transposed
pair does not fail — it changes which comparisons pass. Near zero the absolute
term decides and at scale the relative one does, so a swap is wrong in opposite
directions depending on the magnitudes, and both directions occur in ordinary
use. This is the same argument that gives a
[rate](simple-interest-rate.md#it-is-a-fraction-and-the-formula-forces-that) two
named constructors rather than one, applied to a hazard the first version of
this Concept did not notice it had created.

**Decided — a tolerance is validated like every other quantity here.** A NaN
tolerance is silently discarded by the obvious implementation, because taking
the maximum of two numbers returns the one that is not NaN — so the comparison
answers confidently under a tolerance the caller never supplied. A negative
tolerance costs the comparison its reflexivity. Both are refused at
construction.

# Rendering is two operations, and only one belongs here

The two purposes are opposed. Displaying for a person means discarding digits;
reconstructing a value means discarding none. One operation cannot do both, so
this splits exactly as
[comparison](#approximate-comparison-is-two-operations-not-one) does.

**Decided — this library renders exactly, and human presentation belongs to
whatever sits in front of it.** Symbols, digit grouping, locale, negatives in
parentheses — those are an application's policy, and a library that fixes them
has claimed a decision that was not its to make.

**Decided — the rendering is unambiguous and unlocalized.** The shortest text
that reconstructs the value, plus the currency **code** when one is recorded,
and nothing that implies a currency when none is. A code rather than a symbol:
`USD` identifies one currency, `$` identifies more than a dozen.

**Decided — exactness wins over looking tidy.** An exact rendering shows
`0.1 + 0.2` as `0.30000000000000004`. That is ugly and people will ask for it to
be rounded. Rounding at the boundary would conceal the representation error this
Concept exists to make visible, and the standing posture is that a wrong number
which says nothing is worse than an honest number that looks awkward.

**Decided — a rounded rendering is the caller's to specify, and the library's to
enable.** It needs two things, not one: how many places, _and_ which rounding
mode — half-even, half-up, half-away-from-zero, truncate. Those differ by
jurisdiction and by contract, and picking one would be the library making a
judgement it cannot make on the caller's behalf. It is the same shape as the
[tolerance](#the-numerical-comparison-is-a-hybrid-and-neither-pure-form-works):
supply the mechanism, require the caller to state the intent. Both the place
count and the mode defer with currency, since the place count is a currency's
minor unit.

**Derived — an exact rendering has an inverse, and the pair states a law.**
Parsing reconstructs what rendering wrote, so `parse(render(a)) == a` holds for
every Amount. This is a round-trip property, the strongest shape available
because it can be stated without reimplementing anything, and it is the first
real property this library can be tested against. It exists as a consequence of
choosing exactness, not as something added on top.

# The representation is a parameter

**Decided — which numeric representation carries an Amount is the caller's
choice, not this library's.** Binary floating point is fast, ubiquitous, and
wrong for settlement; a decimal representation is right for money and slower.
Neither is correct for every use, so the library states the operations it
requires and lets the caller supply something providing them.

**Derived — the required operations differ between formulas, and that is
acceptable.** Simple interest needs addition and multiplication. Compound
interest needs `(1 + i)^n`, which for a non-integer `n` is a transcendental
operation that most decimal representations do not offer. So the requirements
are not one set but several, and a representation may satisfy the requirements
of one formula and not another. This is a true statement about the
representations rather than a defect: a tool that claimed to compute compound
interest exactly in a representation that cannot do so would be worse than one
that declines.

**Derived — one requirement is a constraint on rounding rather than on
arithmetic, and it is the sharpest one.** `1 + rt` must be computed with **at
most one rounding** between the operands and the factor. Not a precision
preference: the [simple accumulation factor](simple-accumulation-factor.md)
refuses a factor whose sign is indistinguishable from the rounding error in its
operands, and a second rounding can drive a product that is merely near `-1` to
exactly `-1`. Measured there over 216,000 pairs, the one-rounding and
two-rounding forms disagree about **which inputs are legal** for 63% of those
near cancellation. A representation that rounds twice does not compute the same
answers less precisely; it computes a different domain.

**Derived — how a representation meets that is its own business, and the
requirement is not "supply a fused multiply-add".** A 64-bit binary float needs
fusion, because a rate like `0.05` is not representable and the intermediate
product must not round before the addition. A decimal representation holds that
rate exactly and multiplies it exactly within its scale, so it meets the same
requirement with nothing fused. Stating the requirement as FMA would write a
binary-specific remedy into a representation-neutral contract, and would exclude
the decimal representations this parameter exists to admit.

**Derived — so this guarantee is per-representation, and that is worth saying
plainly.** Two representations may each round once and still disagree about a
factor near cancellation, because each stores the rate with a different error
and the guard's verdict is a question about that error. Choosing a
representation is therefore not only a choice about precision or speed: for
inputs at the boundary it can change which are accepted. The [domain](#domain)
below is stable under a change of representation; the boundary is not.

**Derived — a representation choice does not decide the
[known and unaddressed](#known-and-unaddressed) items below.** Selecting a
decimal representation removes binary rounding error; it does not supply a
rounding rule the formula never had.

**Decided — the first concrete representation is a 64-bit binary float.** It is
ubiquitous, needs no dependency, and supplies every operation any formula here
will want, including the transcendental ones compound interest needs. It is also
the wrong representation for settlement, which is the whole reason the choice is
a parameter rather than a fixture.

Several claims in this Concept are specific to that choice and would need
restating under another: that non-values are NaN and the infinities, that
excluding NaN is what earns reflexive equality, and that negative zero must
compare equal to zero.

**Decided — the parameter is not built until something needs it.** One concrete
representation ships first. Made a parameter later with a default, the plain
name goes on meaning what it means today, so deferring costs almost nothing
while adopting early taxes every signature, every error message and every page
of documentation. The caveat is that the change is near-, not perfectly,
non-breaking: it can disturb inference and any implementation a caller wrote
against the concrete type. With no callers, that is free; it will not stay free.

# What the library exposes

**Decided — the library implements a subset of the algebra above, and grows it
when a formula requires it.** [Future value](future-value.md) needs Amount ×
dimensionless and nothing else. The rest are true of Amounts and unimplemented,
and writing them down here is what makes their absence a choice rather than an
oversight.

This is the asymmetry between a Concept and its implementation. The Concept
states what is true; the implementation states what has been earned.

**Derived — the operations are named methods that can fail, not arithmetic
operators.** An operator must return a value, and the honest return here is one
that can report failure. Overloading it would either lie about totality or yield
a result that no longer composes with a second operator, so the operator earns
nothing a named method does not.

# How this Concept satisfies the standing rule

[Illegal states are unrepresentable](../principles/illegal-states-unrepresentable.md)
applies here, at rung 2 of its ladder: a representation's non-values exist and
cannot be removed by any type discipline, so **an Amount holding one must not be
constructible.** Construction validates and can fail, and no path skips it.

This gives the private magnitude required in
[both readings](#both-readings-of-same-unit-stay-admissible) a second job.
Privacy is not only what keeps a currency addable later; it is the sole
mechanism by which any invariant here is enforceable. An exposed magnitude can
be written to directly, and every guarantee becomes a comment.

Because the operations above are partial, they too return something that can
fail — the rule propagates past the constructor rather than stopping at it.

# Known and unaddressed

- **Binary representation, decimal money.** A representation in binary floating
  point cannot hold `0.1` exactly, so amounts carry representation error that
  accumulates through arithmetic.
- **The formula carries no rounding rule.** `PV(1 + rt)` is a real number.
  Rounding to a minor unit is a settlement concern that sits outside the
  formula, and nothing here decides it.

Both are recorded as known rather than solved. Choosing a representation does
not decide them.

# Not decided

- How a currency is represented, and how an exchange rate is. Both are deferred
  until two Amounts actually meet, which no operation here yet does. Neither is
  an Amount: a currency is a unit and carries no magnitude, and a rate is a
  quantity whose dimension is a ratio of two units.

Everything else once parked here has collapsed into that one question. A
currency's minor unit is what supplies a settlement tolerance and a rounded
rendering's place count alike, so both wait on the same answer and neither is
needed before it.

## What the currency work must revisit

Recorded here rather than only where each was decided, because this section is
where that work starts and a trigger nobody finds is not a trigger.

- **The read accessor.** It returns the whole value today and a value stripped
  of its unit the moment a currency exists, while every call site goes on
  compiling. Rename it then, to something that reads wrong at a call site doing
  arithmetic with it. See
  [both readings](#both-readings-of-same-unit-stay-admissible).
- **Ordering.** Already `PartialOrd` and deliberately not a total order, so
  nothing needs withdrawing — but the day a currency lands is the day the
  comparison must start returning "these do not compare" rather than an answer.
  See [order holds within a unit](#order-holds-within-a-unit-not-across-units).
- **What an absent currency means when it meets a named one.** Decided already —
  it is not silently resolved — but the mechanism is not built, and it is the
  first thing the representation has to express. See
  [an absent currency](#an-absent-currency-means-unrecorded-never-unitless).
- **Equality.** It survives unchanged and should be checked rather than assumed:
  including the unit keeps it total, because "are these the same?" has an answer
  across currencies where "which is larger?" does not.
- **The settlement comparison and the rounded rendering**, both of which take a
  currency's minor unit as their parameter and are deferred on exactly that.

[^wikipedia-fv]: _Future value_, Wikipedia, revision last modified 2026-08-04.
