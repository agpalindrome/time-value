---
type: Formula
title: Future value
description:
  The value at a future date of a single present amount earning simple interest.
tags: [simple-interest]
status: draft
generated: { by: claude/opus-5, at: 2026-08-07T01:24:01Z }
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

# To be modelled

`FV` is a function of `PV`, `r`, and `t`.

`PV` and `FV` are both [Amounts](amount.md).

How `r` and `t` are represented is not decided.

[^wikipedia-fv]: _Future value_, Wikipedia, revision last modified 2026-08-04.
