---
type: Formula
title: Future value
description:
  The value at a future date of a single present amount earning simple interest.
tags: [simple-interest]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T02:37:35Z }
generated: { by: claude/opus-5, at: 2026-08-07T16:15:56Z }
sources:
  - id: wikipedia-fv
    resource: https://en.wikipedia.org/wiki/Future_value
    title: Future value — Wikipedia
    author: team:wikipedia-contributors
    last_modified: 2026-08-04
  - id: wikipedia-accum
    resource: https://en.wikipedia.org/wiki/Accumulation_function
    title: Accumulation function — Wikipedia
    author: team:wikipedia-contributors
  - id: fm-notes
    resource: https://drbeane.github.io/_pages/courses/mth324/FM%20Notes.pdf
    title: FM Notes, Chapter 1 — The Measurement of Interest
    author: human:drbeane
---

# Statement

$$FV = PV(1 + rt)$$

| symbol | meaning                                      |
| ------ | -------------------------------------------- |
| `FV`   | future value                                 |
| `PV`   | present value                                |
| `r`    | the simple interest rate **per time period** |
| `t`    | the number of **time periods**               |

The source states it as: "where `r` is the simple interest rate per time period
and `t` is the number of time periods."[^wikipedia-fv]

Interest is earned on `PV` alone. It is not earned on interest already accrued,
which is what makes this the _simple_ case.

# What the source fixes, and what it leaves open

`r` and `t` are both stated **per time period**, and the source names no
particular period — it never says the period is a year. The formula therefore
constrains the two together rather than separately: `r` and `t` must be counted
in the _same_ period, whatever it is.

The source does not say what values `r` and `t` may take, what `PV` and `FV`
denominate, or what happens at the edges of any of them.

# Dimensions

`r` and `t` are **not** individually dimensionless. `r` is a rate _per_ time
period, so it carries dimension `1/time`; `t` is a count _of_ time periods, so
it carries dimension `time`. Their product cancels, which is what makes `1 + rt`
well-formed — a dimensionless number added to a dimensionless number.

This matters because the same-period rule is a statement about the pair.
Treating `r` and `t` as two independent dimensionless numbers makes that rule
unstateable: nothing would then connect them. They share only that neither is an
[Amount](amount.md).

`(1 + rt)` is the dimensionless quantity, and it is what multiplies the Amount.

# There is one period, and this formula does not need to know it

The source names the period once and states both quantities against it: `r` is
the rate "per time period" and `t` is "the number of time
periods."[^wikipedia-fv] Not two periods that must be made to agree — one
period, referred to twice.

**Derived — a disagreement between `r`'s period and `t`'s period is not
something this formula can express.** There is a single period, and both
quantities are defined relative to it.

**Derived — the formula is agnostic to what that period is.** A rate of `0.05`
per period over `3` periods gives `FV = PV(1.15)` whether a period is a year, a
month, or a fortnight. The arithmetic never consults it. A period is needed only
where someone says "5% annual" and "36 months" — at the boundary where human
units enter, not in the computation.

**Decided — no representation of periodicity is introduced here.** Take a rate
per period and a count of periods. Where periods are actually named, they are
named at the edge.

## This narrows a stated principle, deliberately

The project's headline design principle is to make time-value mistakes compile
errors, and it names this one: applying a rate of one periodicity to cashflows
of another. A period-agnostic core cannot catch that. The narrowing is recorded
rather than left implicit, for three reasons:

- **Typing catches a mismatched label, never a wrong one.** A monthly rate
  tagged as annual compiles cleanly and computes the wrong answer. The guarantee
  is narrower than it first appears.
- **A period is not always known when a model is written.** A command line takes
  it as an argument; a wire protocol takes it as a field. That is the same
  reason a currency cannot be a type-level tag, and it applies here too.
- **There is a point where the question becomes unavoidable, and it is not
  here.** Two genuinely different periods coexist the moment compounding
  frequency arrives: a nominal annual rate `j` compounded `m` times a year gives
  a per-period rate of `j/m` over `n = mt` periods.[^wikipedia-fv] That formula
  holds an annual quantity and a per-period quantity in one expression. It is
  where periodicity must be answered.

# The accumulation factor must stay positive

`(1 + rt)` is an accumulation function, and a valid one carries two criteria.
The FM notes state them directly: "There are two criteria that a function must
satisfy to be a valid accumulation function. These are that a(0) = 1 and a(t) >
0 for all t ≥ 0."[^fm-notes] Wikipedia's own treatment of accumulation functions
requires `a(0) = 1` and states no positivity condition at all,[^wikipedia-accum]
so this is one source supplying what another omits rather than two agreeing.

**Taken literally, that criterion forbids every negative rate.** For
`a(t) = 1 + rt` with `r < 0`, positivity holds only while `t < −1/r`: at
`r = −0.05` it fails from `t = 20` onward, and as `t` grows `1 + rt → −∞` for
any negative `r` whatever. No negative simple rate satisfies "for all `t ≥ 0`".

That cannot be the rule adopted here. Negative rates exist, and the criterion is
written for a general `a(t)` describing an account that persists indefinitely —
not for a formula evaluated at one horizon.

**Decided — positivity is required at the horizon being computed, not for all
horizons.** `1 + rt > 0` for the specific `t` in hand. So the constraint belongs
to the pair and is checked where the formula is applied, not where a
[rate](simple-interest-rate.md) or a duration is built.

**This constrains the factor, not the result. `FV` may be negative.** `PV` may
be negative — an outflow, or a liability — and `FV` is then negative too. The
sign of `FV` is inherited entirely from `PV`; a positive factor never changes
it. What a positive factor rules out is a multiplier that _flips_ the sign,
turning money owed into money held by nothing but the passage of time. Requiring
`FV > 0` would be a different and wrong rule, since it would outlaw every
liability.

This is the second property here that looks like it belongs to `r` or `t`
separately and turns out to belong to both together — the first being the period
they share.

**Derived — the formula can fail for a reason that is not overflow.** Inputs
each perfectly valid can be jointly meaningless, so evaluating `FV` is fallible
on domain grounds as well as on representation grounds.

# The shape of the operation

**Decided — two steps, because the two failures separate onto them.** Build a
[simple accumulation factor](simple-accumulation-factor.md) from `r` and `t`,
which fails only when `1 + rt ≤ 0`; then apply it to a `PV`, which fails only
when the result overflows. One step, one cause. As a single expression the
operation would carry two unrelated failures for a caller to tell apart.

**Decided — a one-call form exists as well.** The common path is a one-liner and
should read like one; it reports whichever failure occurred.

**No role tags are needed.** [Amount](amount.md),
[simple interest rate](simple-interest-rate.md) and
[elapsed periods](elapsed-periods.md) are three distinct kinds, so no two
arguments can be transposed. That protection is already paid for by the
distinctions drawn between them, and adding tags on top would be ceremony.

**The result is an Amount**, in `PV`'s unit, produced fallibly.

# What is modelled

`FV` is a function of `PV`, `r`, and `t`.

`PV` and `FV` are both [Amounts](amount.md), `r` is a
[simple interest rate](simple-interest-rate.md), `t` is a count of
[elapsed periods](elapsed-periods.md), and `(1 + rt)` is a
[simple accumulation factor](simple-accumulation-factor.md).

Every quantity in the formula has a concept, and the operation has a shape. What
remains is to write it.

# Not yet modelled

- **Discounting** — `PV = FV / (1 + rt)`, the inverse. Its own operation, taking
  a non-negative `t`, rather than an overload of this one.
- **Solving for `r` or for `t`** — both recovered from `FV/PV`, which is the
  accumulation factor arrived at from the other direction.

[^wikipedia-fv]: _Future value_, Wikipedia, revision last modified 2026-08-04.

[^wikipedia-accum]:
    _Accumulation function_, Wikipedia. Requires `a(0) = 1` and states no
    positivity, monotonicity or continuity condition.

[^fm-notes]:
    _FM Notes, Chapter 1 — The Measurement of Interest_, course notes.
    Secondary: it states the criteria cleanly and matches the standard
    framework, but it is not the primary text it draws on.
