---
type: Quantity
title: Elapsed periods
description: A length of time counted in the periods a rate is stated against.
tags: [simple-interest, time]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T16:20:15Z }
generated: { by: claude/opus-5, at: 2026-08-07T16:09:41Z }
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

The `t` of [future value](future-value.md): how much time has passed, counted in
the periods the [rate](simple-interest-rate.md) is stated
against.[^wikipedia-fv]

It is a **length** of time, not a point in one. Nothing here identifies a date
or an instant; it says only how far the horizon sits from the start. That
distinction is worth holding onto, because dated cashflows will need both and
they are not the same kind of thing.

It is not an [Amount](amount.md). Its dimension is `time`, which is what cancels
against a rate's `1/time`.

# Why not "duration"

The obvious name is taken. In finance, **duration** already means a bond's
sensitivity to a change in interest rates — Macaulay duration, modified duration
— which is a quantity this library may well want later and which is not a length
of time at all.

Naming this "duration" would spend the word on the wrong thing and guarantee a
collision. "Elapsed periods" says what it is: a count of periods, elapsed.

# It is continuous, not a count

**Decided — `t` admits fractional values.** The source says "the number of time
periods",[^wikipedia-fv] which reads as though whole periods were meant. They
are not, and for simple interest fractional values are the ordinary case rather
than an edge one: money-market instruments accrue over spans like 91/360 of a
year, and day-count conventions produce fractions as their normal output.

Simple interest accrues linearly, so half a period earns exactly half the
interest and `1 + r(0.5)` needs no interpretation. Restricting `t` to whole
numbers would exclude most of what simple interest is actually used for.

# Domain

- **Zero is required, not merely permitted.** `a(0) = 1` is the criterion both
  sources state,[^fm-notes] and `FV = PV(1 + 0) = PV` is exactly it. Zero is the
  point the definition is anchored at.
- **Negative is excluded.** See below — the reason is protective.
- **Finite**, by the same rule as every other quantity here: a representation's
  non-values are not lengths of time.
- **No upper bound** beyond what a representation can hold.

# Negative values are excluded because they mislead

**Decided — `t ≥ 0`.**

Someone reaching for a negative `t` wants to run the formula backwards and
discount. Evaluating an accumulation function at a negative argument does not do
that. The true inverse of simple accumulation is `PV = FV / (1 + rt)`, not
`FV(1 − rt)`, and the two are equal only to first order:

| `r`  | `t` | true discount factor `1/(1 + rt)` | negative-`t` evaluation `1 + r(−t)` |
| ---- | --- | --------------------------------- | ----------------------------------- |
| 0.10 | 1   | 0.909090…                         | 0.900000                            |

About one percent apart — near enough to look right, far enough to matter, and
silent either way. Permitting negative `t` would offer a plausible-looking
answer to a question it does not answer.

Two things support the exclusion. The accumulation criterion is stated "for all
`t ≥ 0`",[^fm-notes] so the function's domain is non-negative by definition. And
discounting deserves to be its own operation, taking a non-negative `t` and
dividing, rather than an overload of this one that quietly computes a
first-order approximation of itself.

# Its representation is coupled to Amount's

`(1 + rt)` is computed in one numeric type and multiplies an Amount's magnitude,
so `t` cannot be held in a different representation from the magnitude. The same
coupling as a [rate](simple-interest-rate.md), accepted for the same reason.

[^wikipedia-fv]: _Future value_, Wikipedia, revision last modified 2026-08-04.

[^fm-notes]:
    _FM Notes, Chapter 1 — The Measurement of Interest_, course notes.
    Secondary: it states the accumulation-function criteria cleanly and matches
    the standard framework, but it is not the primary text it draws on.
