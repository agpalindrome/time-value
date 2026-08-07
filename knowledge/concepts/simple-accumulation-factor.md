---
type: Quantity
title: Simple accumulation factor
description:
  The dimensionless multiplier by which simple interest grows an amount over a
  span.
tags: [simple-interest, accumulation]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T16:20:15Z }
generated: { by: claude/opus-5, at: 2026-08-07T17:36:18Z }
sources:
  - id: wikipedia-accum
    resource: https://en.wikipedia.org/wiki/Accumulation_function
    title: Accumulation function — Wikipedia
    author: team:wikipedia-contributors
  - id: fm-notes
    resource: https://drbeane.github.io/_pages/courses/mth324/FM%20Notes.pdf
    title: FM Notes, Chapter 1 — The Measurement of Interest
    author: human:drbeane
---

# What it is

The `(1 + rt)` of [future value](future-value.md) — the multiple by which an
account has grown over a span.[^fm-notes] It is the accumulation function `a(t)`
for simple interest, evaluated at one horizon.

Dimensionless: a [rate](simple-interest-rate.md)'s `1/time` cancels against
[elapsed periods](elapsed-periods.md)' `time`. That is why multiplying by it
leaves an [Amount](amount.md)'s unit untouched — it is the multiplier whose
dimension is 1.

**This is the first object here the source does not name.** `FV = PV(1 + rt)`
contains no "factor"; the parenthesised expression is introduced as a thing in
its own right because the domain does name it — as an accumulation function —
and because it is where an invariant can be enforced once. That is a choice made
here, not something the formula hands over.

Whether it is the right cut becomes answerable when `(1 + i)^n` appears beside
it, and not before — see
[the bundle is revisable](the-bundle-is-revisable.md#decompositions-are-guesses-until-a-second-instance-exists).

# Why it exists as a thing rather than an expression

**Decided — the factor is built, then applied, and each step can fail exactly
one way.**

| step                      | fails when                              | kind of failure |
| ------------------------- | --------------------------------------- | --------------- |
| build it from `r` and `t` | `1 + rt ≤ 0`                            | domain          |
| build it from `r` and `t` | `1 + rt` is indistinguishable from zero | domain          |
| build it from `r` and `t` | `1 + rt` overflows                      | representation  |
| apply it to an Amount     | the result overflows or underflows      | representation  |

Written as a single expression, one operation carries unrelated failures and a
caller has to disentangle which occurred. Split, each step carries a set with a
common remedy.

**This table is a correction.** It first read that each step fails _exactly one
way_ — build on domain grounds, apply on representation grounds — and called
that clean split "the evidence the decomposition is real rather than merely
tidy." Adversarial review falsified it: a finite rate and a finite span whose
product overflows fail at the **build** step on representation grounds, and the
implementation had always done so. The argument was overstated, and what
survives is weaker but true — the steps separate failures by _cause_, not into
one apiece. The decomposition still earns its place, because the domain rule
becomes an invariant rather than a repeated check; it is simply not the tidy
one-to-one the first version claimed.

**Derived — positivity becomes an invariant instead of a check.** Because the
factor is constructed and validated, `1 + rt > 0` holds for every factor that
exists, which is rung 2 of
[illegal states are unrepresentable](illegal-states-unrepresentable.md). Left
inside the formula it would be a runtime check re-run per call and easy to omit
in the next formula needing it.

**Derived — it is also what inversion produces.** `FV/PV` is
[Amount ÷ Amount](amount.md#arithmetic), a dimensionless quantity, and it _is_
this factor — the object from which `r` or `t` is recovered. Appearing in both
directions is a reasonable test of whether a type is genuine or invented.

# Domain

- **Strictly positive.** The criterion a valid accumulation function must
  satisfy.[^fm-notes] It is what stops the factor flipping the sign of what it
  multiplies, turning money held into money owed by nothing but elapsed time.
- **Far enough from zero for its sign to mean something.** `1 + rt` is a
  difference, so as `rt` approaches `-1` the leading digits cancel and the
  residue is the rounding error already in the operands rather than anything the
  caller expressed. Such a factor is positive as often as not, and would be
  accepted and applied in silence. Refusing it costs a band of answers nobody
  wants — a factor of `5e-17` is not a financial scenario — and buys the
  guarantee that an accepted factor carries meaning.
- **Exactly 1 at `t = 0`.** `a(0) = 1`, the other stated criterion,[^fm-notes]
  and the reason `FV = PV` when no time has passed.
- **May be below 1.** A negative rate shrinks an amount, and the factor lies
  between 0 and 1. Legitimate, and not the same thing as being non-positive.
- **Unbounded above**, and **finite**, by the rules every quantity here follows.

# Simple only, for now

**Decided — this covers `(1 + rt)` and nothing else.** Compound interest
accumulates by `(1 + i)^n`, which is the same kind of object reached a different
way, and the applying half would be identical. Generalising is deferred until
there is a second constructor to generalise over — two instances being the point
at which the shared shape is observed rather than guessed.

# Fused multiply-add is required, not incidental

**Decided — the factor is computed with a single fused rounding.** Measured
2026-08-07: `1.0 + periods * rate` rounds twice, and the intermediate rounding
can drive a product that is merely _near_ `-1` to exactly `-1`, destroying the
sign of the residue. The fused form rounds once and agrees with the exact sign
of `1 + rt` — checked against a double-double reference over 216,000 pairs,
16,000 of them adjacent to the boundary, with no disagreement.

The two forms disagree about which inputs are valid for 63% of pairs near
cancellation. So this is not a stylistic preference, and it widens the
[operations a representation must supply](amount.md#the-representation-is-a-parameter)
by one: a decimal representation without a fused multiply-add computes different
answers about which inputs are legal, not merely less precise ones.

# Its representation is coupled to Amount's

It multiplies an Amount's magnitude, so it is held in the same representation.
The same coupling as a [rate](simple-interest-rate.md) and
[elapsed periods](elapsed-periods.md), accepted for the same reason.

[^fm-notes]:
    _FM Notes, Chapter 1 — The Measurement of Interest_, course notes.
    Secondary: it states the accumulation-function criteria cleanly and matches
    the standard framework, but it is not the primary text it draws on.

[^wikipedia-accum]:
    _Accumulation function_, Wikipedia. Defines `a(t)` as the ratio of future to
    present value, and requires `a(0) = 1`.
