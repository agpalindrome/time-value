---
type: Quantity
title: Simple interest rate
description: The rate per time period at which simple interest accrues.
tags: [simple-interest]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T16:20:15Z }
generated: { by: claude/opus-5, at: 2026-08-07T16:02:06Z }
sources:
  - id: wikipedia-fv
    resource: https://en.wikipedia.org/wiki/Future_value
    title: Future value — Wikipedia
    author: team:wikipedia-contributors
    last_modified: 2026-08-04
  - id: fm-notes
    resource: https://drbeane.github.io/_pages/courses/mth324/FM%20Notes.pdf
    title: FM Notes, Chapter 1 — The Measurement of Interest
    author: human:drbeane
---

# What it is

The `r` of [future value](future-value.md): the rate per time period at which
simple interest accrues.[^wikipedia-fv] Interest is earned on the original
amount alone, never on interest already accrued.

It is not an [Amount](amount.md). It carries no money, and its dimension is
`1/time` — a rate _per_ period, which is why `rt` cancels to a pure number.

# It is a fraction, and the formula forces that

**Derived — `r` is a decimal fraction, not a percentage.** `1 + rt` works only
with `0.05`; supplying `5` for "5%" gives `1 + 15 = 16`. This is not a
convention chosen here — the expression fixes it, and any other reading changes
the formula.

**No type can catch a percentage passed as a fraction.** Both are the same kind
of number, and `5` is a legal rate of 500%, so a validating constructor cannot
reject it and a distinct type cannot tell the two apart. It is the one hazard in
this Concept that
[illegal states are unrepresentable](../principles/illegal-states-unrepresentable.md)
cannot reach, and the error is a silent factor of one hundred.

**Decided — the mitigation is a separate, named way in.** A construction from a
percentage, distinct from construction from a fraction, so the call site states
which it means. That is the same shape as a rounding mode or a comparison
tolerance: supply the mechanism, require the intent to be stated, rather than
document a convention and hope it is read.

# Domain

- **Finite.** A rate is a definite quantity; a representation's non-values are
  not rates. The same rule as [Amount](amount.md), for the same reason.
- **May be negative.** Negative policy rates exist and the formula stays
  coherent. See the section below for why this does not conflict with the
  positivity requirement it appears to.
- **May be zero.** No interest accrues and `FV = PV`.
- **Unbounded.** `r = 10` is 1000% a period. Hyperinflation is real, and nothing
  in the mathematics caps a rate.

**Decided — the requirement that an accumulation factor stay positive is not a
constraint on `r`.** It constrains `r` and `t` jointly, and is checked where the
formula is evaluated rather than where a rate is built. See
[future value](future-value.md#the-accumulation-factor-must-stay-positive).

# The symbol and the period are source-specific

Two conventions differ between the sources consulted, and neither is universal:

|                                  | [Wikipedia][^wikipedia-fv] | [FM notes][^fm-notes] |
| -------------------------------- | -------------------------- | --------------------- |
| symbol for the **simple** rate   | `r`                        | `i`                   |
| symbol for the **compound** rate | `i`                        | —                     |
| the period                       | "per time period", unnamed | "annual"              |

This bundle follows Wikipedia on both counts: `r` for the simple rate, and a
period that is not named. The disagreement is recorded because a reader arriving
from an actuarial text will find `i` used for precisely the quantity this
Concept calls `r`, and because it bears on whether a simple rate and a compound
rate are one kind of thing or two — deferred until a compound rate exists.

# Its representation is coupled to Amount's

`(1 + rt)` is computed in some numeric type and its result multiplies an
Amount's magnitude, so `r` cannot be held in one representation while the
magnitude is held in another. Parameterizing [Amount](amount.md) over its
representation therefore carries `r` along with it. This is accepted rather than
worked around.

# Not decided

- Whether a simple rate and a compound rate are one quantity or two. They share
  the dimension `1/time` and are not interchangeable — using either where the
  other belongs computes a wrong answer in silence. Deferred until compound
  interest exists.

[^wikipedia-fv]: _Future value_, Wikipedia, revision last modified 2026-08-04.

[^fm-notes]:
    _FM Notes, Chapter 1 — The Measurement of Interest_, course notes.
    Secondary: it states the accumulation-function criteria cleanly and matches
    the standard framework, but it is not the primary text it draws on.
