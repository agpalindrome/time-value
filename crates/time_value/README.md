# time_value

Type-safe time-value-of-money (TVM) calculations in Rust.

A deliberately type-heavy redesign of [`time_value`](https://crates.io/crates/time_value),
rebuilt from scratch for the `1.0` line. The goal: make TVM mistakes — applying
an annual rate to monthly cashflows, discounting with an economically
meaningless rate — into *compile errors*, while keeping the common path
ergonomic. `#![no_std]` and dependency-free by default.

## The idea

Values are validated newtypes, and **periodicity is part of the type**:

- `Money` — an always-finite monetary amount (cashflows are signed: outflow
  negative, inflow positive). Negate it with `-money`; add, subtract and scale it
  with the fallible `try_add` / `try_sub` / `try_mul` / `try_div`, which return
  an error rather than an infinity.
- `Rate<P>` — a per-period rate (finite, greater than −100%) tagged with a
  `Periodicity` marker `P` (`Annual`, `SemiAnnual`, `Quarterly`, `Monthly`,
  `Weekly`, `Daily`).
- `Cashflows<P>` — a periodicity-tagged series.

Because `Rate<Monthly>` and `Rate<Annual>` are distinct types, discounting
monthly cashflows with an annual rate **does not compile** — the classic TVM bug
is caught before it can run.

## What it computes

| Available on | Operations |
|--------------|------------|
| **any build** (`no_std`, zero dependencies) | `Cashflows::net_present_value` / `net_future_value` / `internal_rate_of_return`; nominal-rate conversion (`Rate::from_nominal_annual` / `nominal_annual`); and the allocation-free `amortization::Schedule` from an explicit payment (`with_payment`) — they need only elementary arithmetic |
| **with `std` or `libm`** | single-sum `present_value` / `future_value` and their solve-for inverses `periods` (NPER) / `rate` (RATE); the `annuity` module — ordinary, annuity-`due`, `perpetuity` / `growing_perpetuity`, and the finite growing forms (`growing_present_value` / `growing_future_value`, with `due` counterparts), plus the `payment`, `periods`, and `rate` solves; the modified internal rate of return (`Cashflows::modified_internal_rate_of_return`); the term-based `amortization::Schedule::for_term`; effective rate conversion between periodicities (`Rate::convert` / `effective_annual`); and `DatedCashflows` (XNPV / XIRR over irregularly dated flows, discounted by year-fraction) — they need `powf` / `ln`, so they also admit a fractional number of periods |

## Example

```rust
use time_value::{Cashflows, Money, Monthly, Rate};

// Pay 100 now, receive 60 next month and 60 the month after.
let flows = [Money::new(-100.0)?, Money::new(60.0)?, Money::new(60.0)?];
let project = Cashflows::<Monthly>::new(&flows);

let npv = project.net_present_value(Rate::<Monthly>::new(0.01)?);
assert!(npv.value() > 0.0);                    // worth doing at 1%/month

let irr = project.internal_rate_of_return()?;  // ≈ 0.1307 per month
```

(The constructors and `internal_rate_of_return` are fallible — `?` propagates a
[`TvmError`].)

[`TvmError`]: https://docs.rs/time_value/latest/time_value/enum.TvmError.html

## Features

| Feature | Default | Effect |
|---------|:-------:|--------|
| `std`   |    no   | Use `std` for the transcendental math (`f64::powf`). Implies `alloc`. |
| `libm`  |    no   | Provide that math via [`libm`] instead, so the single-sum and annuity operations work in a `no_std` build. |
| `alloc` |    no   | The owned `OwnedCashflows` series (build from a `Vec` or an iterator), complementing the borrowed, allocation-free `Cashflows`. `no_std`-compatible; implied by `std`. |
| `serde` |    no   | Derive `Serialize`/`Deserialize` for the public value types (`Rate`/`Period`/`ContinuousRate` as bare numbers, `Money` as `{ amount, currency }`, `Currency` as its ISO 4217 code, plus `FxRate`/`DatedCashflow`/`Installment`). `no_std`-compatible; deserialization validates through the fallible constructors. |
| `schemars` | no | Implement `JsonSchema` for those same value types — the JSON-Schema companion to `serde`, describing the identical shapes. `no_std`-compatible; implies `alloc`. |

[`libm`]: https://crates.io/crates/libm

This crate is the library at the core of the [`time-value`] workspace, which also
provides a CLI (`time-value`) and an MCP server (`time-value-mcp`). See the
workspace README for development setup.

[`time-value`]: https://github.com/ojhermann-org/time-value

## License

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or
[MIT license](../../LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
