---
type: Formula
title: Future value
description:
  The value at a future date of a single present amount earning simple interest.
tags: [simple-interest]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T02:37:35Z }
generated: { by: claude/opus-5, at: 2026-08-07T15:50:27Z }
sources:
  - id: wikipedia-fv
    resource: https://en.wikipedia.org/wiki/Future_value
    title: Future value — Wikipedia
    author: team:wikipedia-contributors
    last_modified: 2026-08-04
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

# To be modelled

`FV` is a function of `PV`, `r`, and `t`.

`PV` and `FV` are both [Amounts](amount.md).

How `r` and `t` are each represented is not decided.

[^wikipedia-fv]: _Future value_, Wikipedia, revision last modified 2026-08-04.
