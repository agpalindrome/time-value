# ADR-0026: Modified internal rate of return (MIRR)

- **Status:** Accepted (amended by [0065](0065-dated-counterparts.md) — a dated
  counterpart on `DatedCashflows`, annualised over a real number of years instead of
  a count of periods, whose degenerate zero span reports the outcome rather than
  `ZeroPeriods`)
- **Date:** 2026-07-10
- **Deciders:** Project owner

## Context

The plain internal rate of return
([ADR-0020](0020-robust-irr-newton-with-bisection-fallback.md)) has two known
weaknesses: a non-conventional series (more than one sign change) can have
several real roots — our solver returns the lowest bracketed one — and it
implicitly assumes every interim cashflow is reinvested at the IRR itself. The
**modified** IRR removes both: it discounts outflows at an explicit finance rate,
compounds inflows at an explicit reinvestment rate, and takes the single rate
equating the two. #32 adds it.

## Decision

Add a method on the cashflow series:

```rust
Cashflows::<P>::modified_internal_rate_of_return(
    finance_rate: Rate<P>,
    reinvestment_rate: Rate<P>,
) -> Result<Rate<P>, TvmError>
```

For a series whose last cashflow is at period `N`:

- discount the **outflows** (negative cashflows) to period `0` at `finance_rate`
  → `PVₒᵤₜ` (a non-positive magnitude);
- compound the **inflows** (positive cashflows) to period `N` at
  `reinvestment_rate` → `TVᵢₙ`;
- `MIRR = (TVᵢₙ / −PVₒᵤₜ)^(1/N) − 1`.

All three rates share the periodicity `P`. The two accumulations run in one pass
with running discount/compound factors and a float period counter (no
`usize as f64` cast, which the pedantic lint set forbids).

### Behind `std` / `libm`, unlike NPV/NFV/IRR

The two accumulations are arithmetic-only, but the terminal `N`-th root needs
`powf`. So — correcting the #32 issue text, which called MIRR "arithmetic-only
(stays in the default `no_std` build)" — the operation is **feature-gated**,
placed in a `#[cfg(any(feature = "std", feature = "libm"))]` impl block alongside
the other transcendental surface (`single_sum`, `annuity`, rate conversions). The
default `no_std` build keeps NPV, NFV, and IRR, which genuinely need only
elementary arithmetic.

### Degenerate cases (ADR-0021)

- Empty series → `EmptyCashflows`.
- Fewer than two cashflows (`N = 0`, no span to annualise over) → `NonFiniteResult`.
- No outflows (`PVₒᵤₜ = 0`, nothing to grow from) → `NonFiniteResult` (the division
  is non-finite).
- No inflows (`TVᵢₙ = 0`, so the root is `0` and the rate `−100%`) →
  `RateOutOfRange`, via `Rate::from_operation`.

## Consequences

- The series gains a unique, assumption-explicit alternative to IRR for
  non-conventional cashflows.
- The transcendental surface grows by one operation; the `no_std` default build is
  unchanged (the method is compiled out without a feature).
- No new error variant is needed — the degenerate cases map onto the existing
  ones.

## Alternatives considered

- **Keep MIRR in the `no_std` default build** — impossible for an arbitrary `N`-th
  root without `powf`; keeping it `no_std` would mean returning the terminal growth
  factor and making the caller take the root, which is no longer MIRR.
- **A single-rate convenience** (`finance_rate == reinvestment_rate`) — easily
  expressed by passing the same rate twice; a separate method earns nothing.
- **Reusing `single_sum::rate`** for the final root — the same computation, but
  routing through `Money`/`Period` newtypes to solve a value we already hold as an
  `f64` adds ceremony without safety; the direct `powf` is clearer.
