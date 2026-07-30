# time_value

Type-safe time-value-of-money (TVM) calculations in Rust.

A deliberately type-heavy redesign, rebuilt from scratch for the `1.0` line and
sharing only its name with the [`0.x`
series](https://crates.io/crates/time_value/0.8.0). The goal: make TVM mistakes —
applying an annual rate to monthly cashflows, discounting with an economically
meaningless rate — into *compile errors*, while keeping the common path
ergonomic. `#![no_std]` and dependency-free by default.

## The idea

Values are validated newtypes, and **periodicity is part of the type**:

- `Money` — a monetary amount: an always-finite `f64` magnitude plus the
  `Currency` it is denominated in. Currency is a runtime *value*, not a type tag
  — `Currency::Xxx` is the currency-agnostic identity, and combining two distinct
  real currencies is a runtime `TvmError::CurrencyMismatch`. Cashflows are signed
  (outflow negative, inflow positive). Negate it with `-money`, and take its
  `abs` / `signum` infallibly; add, subtract, scale, total and compare it with the
  fallible `try_add` / `try_sub` / `try_mul` / `try_div` / `try_sum` / `try_min` /
  `try_max`, which return an error rather than an infinity (or, where the
  currencies clash, rather than an invented answer).
- `Rate<P>` — a per-period rate (finite, greater than −100%) tagged with a
  `Periodicity` marker `P` (`Annual`, `SemiAnnual`, `Quarterly`, `Monthly`,
  `Weekly`, `Daily`).
- `Cashflows<P>` — a periodicity-tagged series.
- `Payment`, `PresentValue`, `FutureValue`, `Principal`, `Growth<P>` — zero-cost
  *role* markers, used where an operation takes two same-typed arguments a caller
  could transpose.

Because `Rate<Monthly>` and `Rate<Annual>` are distinct types, discounting
monthly cashflows with an annual rate **does not compile** — the classic TVM bug
is caught before it can run.

The role markers catch its twin. `annuity::periods(rate, Payment(pmt),
PresentValue(pv))` and `single_sum::periods(rate, PresentValue(pv),
FutureValue(fv))` take their two amounts in *different* orders; wrapping each in
its role makes swapping them a compile error rather than a plausible wrong answer
(`docs/adr/0050-role-newtypes-for-ambiguous-arguments.md`).

## What it computes

| Available on | Operations |
|--------------|------------|
| **any build** (`no_std`, zero dependencies) | `Cashflows::net_present_value` / `net_future_value` / `internal_rate_of_return`; nominal-rate conversion (`Rate::from_nominal_annual` / `nominal_annual`); and the allocation-free `amortization::Schedule` from an explicit payment (`with_payment`) — they need only elementary arithmetic |
| **with `std` or `libm`** | single-sum `present_value` / `future_value` and their solve-for inverses `periods` (NPER) / `rate` (RATE); the `annuity` module — ordinary, annuity-`due`, `perpetuity` / `growing_perpetuity` (each with a `due` counterpart), and the finite growing forms (`growing_present_value` / `growing_future_value`, with `due` counterparts), plus the `payment`, `periods`, and `rate` solves, each from a present *or* a future value (`payment_from_future` is the sinking-fund payment) and each mirrored in `due`, and the growing annuity's own present-anchored inverses (`growing_payment` / `growing_periods` / `growing_rate`); the modified internal rate of return (`Cashflows::modified_internal_rate_of_return`); the term-based `amortization::Schedule::for_term`; effective rate conversion between periodicities (`Rate::convert` / `effective_annual`); the `continuous` module (compounding, discounting, and growth at a periodicity-free `ContinuousRate` — the force of interest — its own closed-form solves `rate` / `years`, and conversions to and from a `Rate<Annual>`); and `DatedCashflows` (XNPV / XIRR over irregularly dated flows, discounted by year-fraction) — they need `powf` / `ln` / `exp`, so they also admit a fractional number of periods |

## Example

```rust
use time_value::{Cashflows, Money, Monthly, Rate, TvmError};

fn main() -> Result<(), TvmError> {
    // Pure-number TVM is currency-agnostic: pay 100 now, receive 60 next month
    // and 60 the month after.
    let flows = [
        Money::agnostic(-100.0)?,
        Money::agnostic(60.0)?,
        Money::agnostic(60.0)?,
    ];
    let project = Cashflows::<Monthly>::new(&flows);

    // Worth doing at 1% a month.
    let npv = project.net_present_value(Rate::<Monthly>::new(0.01)?)?;
    assert!(npv.value() > 0.0);

    // The rate at which it breaks even: about 13.07% a month.
    let irr = project.internal_rate_of_return()?;
    assert!((irr.value() - 0.1307).abs() < 1e-4);

    Ok(())
}
```

Denominate an amount when the currency matters — it travels with the value and
is checked at every combination:

```rust
use time_value::{Currency, Money, TvmError};

fn main() -> Result<(), TvmError> {
    let fee = Money::new(25.0, Currency::Usd)?;
    let rent = Money::new(1_200.0, Currency::Usd)?;
    assert_eq!(fee.try_add(rent)?.value(), 1_225.0);
    assert_eq!(Money::try_sum([fee, rent])?.value(), 1_225.0);

    // A code parses case-insensitively; `Currency::from_code` is the strict form.
    assert_eq!("usd".parse::<Currency>()?, Currency::Usd);

    // Two distinct real currencies do not combine.
    let eur = Money::new(10.0, Currency::Eur)?;
    assert!(fee.try_add(eur).is_err());
    assert!(fee.try_max(eur).is_err()); // …and so have no larger one

    Ok(())
}
```

The constructors and the operations that can fail return a [`TvmError`], so `?`
carries them.

[`TvmError`]: https://docs.rs/time_value/latest/time_value/enum.TvmError.html

## Features

| Feature | Default | Effect |
|---------|:-------:|--------|
| `std`   |    no   | Use `std` for the transcendental math (`f64::powf`). Implies `alloc`. |
| `libm`  |    no   | Provide that math via [`libm`] instead, so the single-sum and annuity operations work in a `no_std` build. |
| `alloc` |    no   | The owned `OwnedCashflows` series (build from a `Vec` or an iterator), complementing the borrowed, allocation-free `Cashflows`. `no_std`-compatible; implied by `std`. |
| `serde` |    no   | Derive `Serialize`/`Deserialize` for the public value types (`Rate`/`Period`/`ContinuousRate` as bare numbers, `Money` as `{ amount, currency }`, `Currency` as its ISO 4217 code, plus `FxRate`/`DatedCashflow`/`Installment`, and — with `alloc` — `OwnedCashflows` as a bare array of `Money`). `no_std`-compatible; deserialization validates through the fallible constructors. |
| `schemars` | no | Implement `JsonSchema` for those same value types — the JSON-Schema companion to `serde`, describing the identical shapes. `no_std`-compatible; implies `alloc`. |

[`libm`]: https://crates.io/crates/libm

This crate is the library at the core of the [`time-value`] workspace, which also
provides a CLI (`time-value`) and an MCP server (`time-value-mcp`). See the
workspace README for development setup.

[`time-value`]: https://github.com/ojhermann-org/time-value

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
