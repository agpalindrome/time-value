# ADR-0036: Continuous compounding — a periodicity-free force of interest

- **Status:** Accepted
- **Date:** 2026-07-14
- **Deciders:** Project owner
- **Relates to:** [ADR-0015](0015-annuities.md),
  [ADR-0024](0024-rate-conversions-effective-and-nominal.md); implements issue #22
- **Follows:** [ADR-0033](0033-core-domain-model-two-axes-and-an-f64-engine.md),
  [ADR-0035](0035-periodicity-tagged-time.md)

> **Amended (2026-07-30) by [ADR-0064](0064-continuous-solves.md).** The `continuous`
> module below defines only the two *value* operations; ADR-0064 adds the two solves
> that complete `FV = PV·e^(δ·Y)` — `rate` for the force of interest and `years` for
> the span — both closed forms. Two decisions here are load-bearing there and are
> honoured rather than revisited: the span is a plain `f64` that may be negative, so
> the span solve *returns* a negative answer where `single_sum::periods` reports
> `NegativePeriods`; and a force of interest has no `−1` floor, so it needs a
> `from_operation` funnel of its own rather than `Rate`'s.

## Context

Continuous compounding (issue #22) is the limit of discrete compounding as the
frequency goes to infinity: growth over time `t` is `e^(δt)`, where `δ` is the
**force of interest** (the continuously-compounded rate). It is a normal thing to
ask of a TVM engine (option pricing, actuarial work, and physics-style growth all use
it), so it must fit the model designed in ADR-0033/0035 rather than be bolted on.

The difficulty: a force of interest has **no discrete periodicity**. The
[`Periodicity`](../..) trait promises a *finite* `PERIODS_PER_YEAR: u16`, which drives
`Rate<P>`, `Period<P>`, and `Cashflows<P>`. "Continuous" is `∞` per year, so it
cannot honestly be a `Periodicity` marker, and its time is a continuous duration, not
a count of `Period<P>`s.

## Decision

Model continuous compounding as a **sibling of `Rate<P>`, not a case of it** — a
distinct, periodicity-free type.

- **`ContinuousRate(f64)`** — the annualized force of interest `δ`. It carries no
  periodicity tag, because it has none.
- **Time is a continuous duration in years** (`f64`), like `DatedCashflows`
  (ADR-0035's calendar exception), *not* `Period<P>`.
- **Operations** live in a small `continuous` module:
  `future_value(rate: ContinuousRate, years: f64, present: Money) = present · e^(δ·years)`,
  and the present-value inverse with `e^(−δ·years)`. They need `exp`, so they sit
  behind the `std` / `libm` feature like the other transcendental operations
  (ADR-0014).
- **Conversions bridge to the periodic world** through the effective annual rate:
  `δ = ln(1 + r_eff)` and `r_eff = e^δ − 1`. So `ContinuousRate ⇄ Rate<Annual>`
  (effective) conversions let a continuous rate be compared with, and derived from,
  the discrete rates — reusing the effective-rate machinery of ADR-0024.

Currency threads through exactly as everywhere else: the operations take and return
`Money` (a runtime currency; ADR-0034), and are non-generic in currency.

## Consequences

- Continuous compounding fits without weakening the `Periodicity` abstraction: the
  trait keeps its honest "finite frequency" contract, and `Period<P>`/`Cashflows<P>`
  stay clean, because the continuous world is a separate, small surface.
- There are now two accepted "time in years, not periods" corners — `DatedCashflows`
  (ADR-0035) and continuous compounding — both justified by the same reason: the
  quantity genuinely is a continuous/real duration, not a count of a fixed period.
- The bridge conversions make the discrete and continuous views interoperable
  (`Rate<Annual>` ⇄ `ContinuousRate`) rather than two disconnected islands.

## Alternatives considered

- **A `Continuous` periodicity marker** (`Rate<Continuous>`) — would require
  `PERIODS_PER_YEAR` to be `∞`, breaking the trait's finite-frequency contract and
  poisoning `Period<Continuous>` / `Cashflows<Continuous>`, which are meaningless.
  Rejected: it pollutes a sound abstraction to avoid one new type.
- **Overloading `Rate<Annual>` to mean "continuous when you say so"** — an untyped
  mode flag on a rate; exactly the kind of silent-semantics footgun the crate's type
  discipline exists to avoid.
- **Deferring continuous compounding entirely** — leaves a foreseeable operation to
  retrofit against a frozen model; cheaper to reserve its shape now.
