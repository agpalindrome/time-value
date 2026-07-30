# ADR-0029: Dated cashflows — XNPV / XIRR

- **Status:** Accepted (amended by [0030](0030-shared-day-count-support-crate.md) — the
  binaries' ACT/365 day-count now lives in a shared crate, not copied into each; and by
  [0065](0065-dated-counterparts.md), which **adds** the dated net future value rejected
  under *Alternatives considered* below, plus a dated MIRR and an owned
  `OwnedDatedCashflows`)
- **Date:** 2026-07-13
- **Deciders:** Project owner

## Context

[`Cashflows`](0013-core-api-values-and-discrete-operations.md) discounts a series
at *evenly spaced* periods — flow `t` sits at index `t`, one period apart. Real
cashflows arrive on irregular calendar dates (drawdowns, invoices, dividends).
The spreadsheet answer is `XNPV`/`XIRR`: net present value and internal rate of
return over dated, unevenly spaced flows, discounting each by the **year-fraction**
from a reference date. This ADR adds that to the core (roadmap #23).

The open question the issue posed: how are dates represented without pulling a
date/day-count library into a `no_std`, zero-dependency core
([ADR-0009](0009-no_std-and-optional-libm.md))?

## Decision

### Year-offsets in the core; dates are a binary-surface concern

The core takes **`f64` year-offsets**, not a date type. A date type would only
compute these offsets (via some day-count); keeping it out of the library leaves
the core dependency-free and lets the day-count convention live where the raw
dates do — at the CLI/MCP boundary (see "Surface" below). This is the issue's
"take `f64` year-offsets to stay `no_std`" path.

- **`DatedCashflow`** — a `Copy` newtype bundling an `offset_years: f64` and a
  `Money`. Its constructor is fallible, validating the offset **finite**
  ([`TvmError::NonFiniteOffset`], a new variant); the offset may be negative or
  zero (a flow before or at the reference).
- **`DatedCashflows<'a>`** — borrows `&'a [DatedCashflow]` (allocation-free, like
  `Cashflows`; ADR-0013). The **first** flow is the valuation reference: each flow
  `i` is discounted by `(1 + r)^(tᵢ − t₀)`, so the first flow is undiscounted.
  Rebasing to the first entry (not the minimum) matches Excel's XNPV/XIRR.

### The rate is annual — `Rate<Annual>`

Offsets are in years and the discount is annual-effective, so the rate is an
**annual** rate. `net_present_value` takes `Rate<Annual>` and
`internal_rate_of_return` returns `Rate<Annual>`. This keeps the crate's headline
guarantee: a `Rate<Monthly>` on dated flows is a **compile error**, not a silent
mistake — you cannot mix a per-period rate into a year-fraction discount.

### Behind `std` / `libm`, reusing the existing machinery

`(1 + r)^t` for fractional `t` needs `powf`, so the whole `dated` module is gated
behind `std`/`libm` and goes through `math::powf` (ADR-0014), like `single_sum`,
`Period`, and MIRR. XIRR reuses the robust IRR solver unchanged
([ADR-0020](0020-robust-irr-newton-with-bisection-fallback.md)): Newton–Raphson
from a guess (default `0.1`), falling back to the shared
`root::bracket_and_bisect`, with the magnitude-scaled NPV tolerance
([ADR-0021](0021-fallible-operations-on-non-finite-results.md)). The residual and
its derivative are the only new code — `Σ CFᵢ (1+r)^(−tᵢ)` and its `r`-derivative,
now with `powf` instead of a running integer-power factor.

### Fallibility

- `net_present_value(rate) -> Result<Money, TvmError>` — empty series is `Ok(0)`;
  `NonFiniteResult` on overflow (ADR-0021).
- `internal_rate_of_return[_from] -> Result<Rate<Annual>, TvmError>` —
  `EmptyCashflows` on empty, `IrrDidNotConverge` when the residual never changes
  sign (reusing IRR's variant: XIRR *is* an IRR).

### Surface (per [ADR-0028](0028-binary-surface-conventions.md))

XNPV/XIRR join the `series` family: CLI `series xnpv` / `series xirr`, MCP tools
`xnpv` / `xirr` (bare acronyms). The binaries — which already carry non-core
dependencies — accept **ISO `YYYY-MM-DD` dates** and convert to year-offsets with
a small, self-contained **ACT/365** day-count (a proleptic-Gregorian
days-from-civil calculation; no new dependency), so a user supplies real dates
rather than pre-computing year-fractions. ACT/365 (calendar days over 365) matches
Excel's XNPV/XIRR. Callers who genuinely hold year-offsets use the core
`DatedCashflows` API directly; the binaries expose only the date form.

## Consequences

- XNPV/XIRR are available `no_std` (via `libm`) with zero core dependencies.
- Dated discounting is type-checked to an annual rate — the periodicity guarantee
  extends to irregular flows.
- The day-count convention is a binary concern; a future alternative (ACT/360,
  30/360) or a feature-gated date type is additive and does not touch the core.
- One new `TvmError` variant (`NonFiniteOffset`); `TvmError` is `#[non_exhaustive]`,
  so this is not breaking.

## Alternatives considered

- **A feature-gated date type in the core** — pulls a calendar/day-count into the
  published crate (or reinvents one); the year-offset seam keeps the core minimal
  and defers the day-count to where the dates are. Rejected for 1.0.
- **A general `Rate<P>` instead of `Rate<Annual>`** — the discount is intrinsically
  annual (offsets are years); a per-period tag would be meaningless here and lose
  the mismatch check. Rejected.
- **Raw year-offsets on the CLI/MCP** — trivial but useless for the actual use
  case (irregular calendar dates); rejected in favour of ISO dates + ACT/365.
- **A dated net *future* value** — XNFV is not a standard function and has no clear
  reference date to compound to; omitted. **Reversed by
  [ADR-0065](0065-dated-counterparts.md):** the first half stands and is not decisive
  (`Cashflows` carries an NFV Excel also lacks), and the second is answered — the
  horizon is the **latest** offset, which needs no reference date at all, since
  `Σ CFᵢ (1+r)^(T − tᵢ)` never mentions `t₀`.

[`TvmError::NonFiniteOffset`]: ../../crates/time_value/src/lib.rs
