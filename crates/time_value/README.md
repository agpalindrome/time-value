# `time_value`

Type-safe time-value-of-money calculations.

```rust
use time_value::{Amount, ElapsedPeriods, SimpleInterestRate, Tolerance, future_value};

let future = future_value(
    Amount::new(100.0)?,
    SimpleInterestRate::from_percent(5.0)?,
    ElapsedPeriods::new(3.0)?,
)?;

assert!(future.is_close(Amount::new(115.0)?, Tolerance::relative(1e-9)?));
# Ok::<(), time_value::Error>(())
```

Each quantity is its own kind, so no two arguments can be transposed, and each
is validated on the way in: an `Amount` is never a NaN, a span of periods is
never negative, and an accumulation factor is always strictly positive.

## What it refuses, and why

Failures are separated by cause, because the remedies differ. A **domain**
failure means the inputs were each valid and jointly meaningless — no wider
representation rescues it. A **representation** failure means the arithmetic
left what a `f64` can hold.

The subtler refusal is a factor too near zero for its sign to be trusted.
`1 + rt` is a difference, so when `rt` is near `-1` the leading digits cancel
and what survives is the rounding error already in the inputs rather than
anything the caller meant. Such a factor is positive as often as not, so it
would be accepted and applied in silence.

## Reasoning

Every decision here is recorded in the project's
[knowledge bundle](https://github.com/ojhermann-org/time-value/tree/main/knowledge),
along with what the source said, what follows from it, and what has been left
open. The code implements the bundle.

## Relationship to the published `0.x` series

Versions `0.1.0`–`0.8.0` on crates.io are a separate, immutable history. This
line does not continue them, and nothing here is published.

## License

Licensed under either of [Apache-2.0](../../LICENSE-APACHE) or
[MIT](../../LICENSE-MIT) at your option.
