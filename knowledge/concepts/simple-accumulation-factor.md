---
type: Quantity
title: Simple accumulation factor
description:
  The dimensionless multiplier by which simple interest grows an amount over a
  span.
tags: [simple-interest, accumulation]
status: draft
generated: { by: claude/opus-5, at: 2026-08-07T16:15:56Z }
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

# Why it exists as a thing rather than an expression

**Decided — the factor is built, then applied, and each step can fail exactly
one way.**

| step                      | fails when           | kind of failure |
| ------------------------- | -------------------- | --------------- |
| build it from `r` and `t` | `1 + rt ≤ 0`         | domain          |
| apply it to an Amount     | the result overflows | representation  |

Written as a single expression, one operation carries two unrelated failures and
a caller has to disentangle which occurred. Split, each has one cause and one
meaning. The failures separating this cleanly is the evidence the decomposition
is real rather than merely tidy.

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
