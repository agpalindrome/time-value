//! What a tool accepts and returns.
//!
//! **Local, not derived from the library's types.** The alternative was `serde`
//! and `schemars` features on `time_value` itself, and this repo chose to keep
//! the published crate free of both: a surface owns its wire shape, and the
//! library owns what a valid value is. So every field here is a plain number
//! that the library's constructors then accept or refuse — the same arrangement
//! the CLI uses, for the same reason.
//!
//! The cost is honest: these types are a second description of quantities the
//! bundle already models. It is thin — three numbers and a rate convention —
//! and it buys `schemars` never entering the published crate's dependency
//! graph.
//!
//! **Doc comments here are paid for on every connection.** `schemars` copies
//! them into the schema and every client downloads the lot at `tools/list`, so
//! they say what a caller needs and nothing else. Measured 2026-08-08: moving
//! the rationale out of `///` and into `//` here and in `server.rs` took the
//! two-tool listing from 4,211 bytes to 3,274 — a fifth of it, for two tools,
//! and the ratio only gets worse as tools are added. Reasoning belongs in
//! comments like this one; instructions belong in the doc comment.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A per-period rate, said one of two ways: `{"fraction": 0.05}` or
/// `{"percent": 5}`. Those are the same rate. Never a bare number.
//
// Not a bare `f64`, deliberately. A field documented only as "per-period rate" is
// how an earlier version of this repo accepted `5` for five percent and computed
// at 500%, silently, across seventeen tool inputs (#130). The library draws the
// distinction in two constructors; this makes the caller pick one, and `schemars`
// renders the choice as a `oneOf` rather than as prose nobody reads.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Rate {
    /// A decimal fraction: `0.05` is five percent.
    Fraction(f64),
    /// A percentage: `5` is five percent.
    Percent(f64),
}

/// Arguments to `FV = PV(1 + rt)`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FutureValueRequest {
    /// The present value. Negative is a liability, and legal.
    pub(crate) amount: f64,
    /// The rate, per period.
    pub(crate) rate: Rate,
    /// Elapsed periods, in the same period the rate is quoted per.
    pub(crate) periods: f64,
}

/// Arguments to `1 + rt`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AccumulationFactorRequest {
    /// The rate, per period.
    pub(crate) rate: Rate,
    /// Elapsed periods, in the same period the rate is quoted per.
    pub(crate) periods: f64,
}

/// The value of an amount after a span.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct FutureValueResult {
    /// The future value, in the present value's unit.
    pub(crate) future_value: f64,
}

/// The multiplier simple interest applies over a span.
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub(crate) struct AccumulationFactorResult {
    /// The factor `1 + rt`. Strictly positive, and exactly 1 at zero periods.
    pub(crate) accumulation_factor: f64,
}
