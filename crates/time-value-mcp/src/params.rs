//! Typed inputs for the MCP tools.
//!
//! Each struct derives [`JsonSchema`] (so the server advertises an input schema)
//! and [`Deserialize`] (so `rmcp` can parse the call arguments). Field doc
//! comments become the schema descriptions. Keeping the parsing here leaves the
//! library's typed core untouched (ADR-0011).

use schemars::JsonSchema;
use serde::Deserialize;
use time_value::Currency;

// The tool inputs take the core [`Currency`] directly: the core's `serde`
// `Deserialize` resolves an ISO 4217 code via `from_code` (a friendly "unknown
// ISO 4217 currency code" error), and its `schemars` `JsonSchema` advertises the
// full code `enum` from `Currency::ALL` (ADR-0044). This replaces the former
// `CurrencyCode` string newtype, which hand-wrote both halves in this crate.

/// The compounding periodicity a `rate_*` tool operates at — the only place a
/// periodicity is a runtime input (ADR-0028 §3). A closed set, so it is a typed
/// enum rather than a free string (ADR-0039): an unknown value is refused by
/// deserialization at the boundary, and the schema advertises the six choices.
/// Serialized names are lower-kebab (`semi-annual`), matching the marker types
/// in the core.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Periodicity {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
}

/// A per-period rate and a cashflow series — inputs for `npv` and `nfv`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SeriesInput {
    /// Per-period rate (e.g. `0.01` for 1% per period).
    pub rate: f64,
    /// Cashflows at periods 0, 1, 2, … (signed: outflow negative, inflow
    /// positive). Period 0 is "now" and is not discounted.
    pub cashflows: Vec<f64>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// A cashflow series and an optional solver guess — input for `irr`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct IrrInput {
    /// Cashflows at periods 0, 1, 2, … (signed: outflow negative).
    pub cashflows: Vec<f64>,
    /// Initial guess for the Newton–Raphson solve (default `0.1`).
    #[serde(default = "default_guess")]
    pub guess: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

fn default_guess() -> f64 {
    0.1
}

/// A finance rate, a reinvestment rate, and a cashflow series — input for `mirr`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct MirrInput {
    /// Per-period finance rate: discounts the outflows to the present.
    pub finance: f64,
    /// Per-period reinvestment rate: compounds the inflows to the final period.
    pub reinvest: f64,
    /// Cashflows at periods 0, 1, 2, … (signed: outflow negative).
    pub cashflows: Vec<f64>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// A single dated cashflow — an ISO date and a signed amount.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DatedFlow {
    /// The cashflow date, ISO `YYYY-MM-DD`.
    pub date: String,
    /// The signed cashflow amount (outflow negative, inflow positive).
    pub amount: f64,
}

/// An annual rate and dated cashflows — input for `xnpv`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DatedSeriesInput {
    /// Annual discount rate (e.g. `0.1` for 10% per year).
    pub rate: f64,
    /// Dated cashflows; the first date is the valuation reference.
    pub flows: Vec<DatedFlow>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Dated cashflows and an optional solver guess — input for `xirr`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct DatedIrrInput {
    /// Dated cashflows; the first date is the valuation reference.
    pub flows: Vec<DatedFlow>,
    /// Initial guess for the Newton–Raphson solve, annual (default `0.1`).
    #[serde(default = "default_guess")]
    pub guess: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `single_sum_present_value` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PresentValueInput {
    /// Per-period discount rate.
    pub rate: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The future amount to discount to today.
    pub future: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `single_sum_future_value` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct FutureValueInput {
    /// Per-period rate.
    pub rate: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The present amount to compound forward.
    pub present: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `single_sum_periods` tool (solve for the number of periods).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SingleSumPeriodsInput {
    /// Per-period rate.
    pub rate: f64,
    /// The present amount.
    pub present: f64,
    /// The future amount.
    pub future: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `single_sum_rate` tool (solve for the per-period rate).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct SingleSumRateInput {
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The present amount.
    pub present: f64,
    /// The future amount.
    pub future: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_periods` and `annuity_due_periods` tools. Provide exactly
/// one of `present` or `future` (the value the payment stream is anchored to).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AnnuityPeriodsInput {
    /// Per-period rate.
    pub rate: f64,
    /// The payment made at the end of each period (at the *start*, for
    /// `annuity_due_periods`).
    pub payment: f64,
    /// Solve from this present value (mutually exclusive with `future`).
    #[serde(default)]
    pub present: Option<f64>,
    /// Solve from this future value (mutually exclusive with `present`).
    #[serde(default)]
    pub future: Option<f64>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_rate` and `annuity_due_rate` tools. Provide exactly one of
/// `present` or `future`.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AnnuityRateInput {
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The payment made at the end of each period (at the *start*, for
    /// `annuity_due_rate`).
    pub payment: f64,
    /// Solve from this present value (mutually exclusive with `future`).
    #[serde(default)]
    pub present: Option<f64>,
    /// Solve from this future value (mutually exclusive with `present`).
    #[serde(default)]
    pub future: Option<f64>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_perpetuity` and `annuity_due_perpetuity` tools.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct PerpetuityInput {
    /// Per-period rate (must exceed 0).
    pub rate: f64,
    /// The payment made at the end of each period, forever.
    pub payment: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_growing_perpetuity` and `annuity_growing_due_perpetuity`
/// tools.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GrowingPerpetuityInput {
    /// Per-period rate (must exceed the growth rate).
    pub rate: f64,
    /// The per-period growth rate of the payment.
    pub growth: f64,
    /// The first payment (at the end of period 1).
    pub payment: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `rate_effective_annual` and `rate_nominal` tools.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RateEffectiveAnnualInput {
    /// The per-period rate.
    pub rate: f64,
    /// The periodicity the rate is expressed in.
    pub periodicity: Periodicity,
}

/// Input for the `rate_convert` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RateConvertInput {
    /// The per-period rate expressed under `from`.
    pub rate: f64,
    /// The periodicity the rate is expressed in.
    pub from: Periodicity,
    /// The periodicity to express the rate in.
    pub to: Periodicity,
}

/// Input for the `rate_from_nominal` tool.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct RateFromNominalInput {
    /// The nominal annual rate (APR).
    pub nominal: f64,
    /// The compounding periodicity.
    pub periodicity: Periodicity,
}

/// Input for the `amortize` tool. Provide exactly one of `periods` (amortise over
/// a term) or `payment` (amortise with a level payment).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AmortizeInput {
    /// Per-period rate.
    pub rate: f64,
    /// The principal to amortise.
    pub principal: f64,
    /// Amortise over this many periods (mutually exclusive with `payment`).
    #[serde(default)]
    pub periods: Option<f64>,
    /// Amortise with this level payment (mutually exclusive with `periods`).
    #[serde(default)]
    pub payment: Option<f64>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_present_value` and `annuity_future_value` tools.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AnnuityValueInput {
    /// Per-period rate.
    pub rate: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The payment made at the end of each period.
    pub payment: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the four growing-annuity tools — `annuity_growing_present_value`,
/// `annuity_growing_future_value`, and their `annuity_due_*` counterparts
/// (ADR-0048).
///
/// Unlike [`GrowingPerpetuityInput`], `rate` need not exceed `growth`: a finite
/// growing annuity converges for every rate and growth pair.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GrowingAnnuityInput {
    /// Per-period rate.
    pub rate: f64,
    /// The per-period growth rate of the payment.
    pub growth: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The first payment — at the end of period 1 (ordinary) or the start of
    /// period 1 (annuity-due). Each later payment is `(1 + growth)` times the one
    /// before.
    pub payment: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_growing_payment` tool (ADR-0063).
///
/// Unlike the level [`AnnuityPaymentInput`] there is no `future` anchor: a growing
/// annuity's inverses exist only from the present value, because its future value
/// differences two exponentials and has no closed-form inverse in the term.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GrowingPaymentInput {
    /// Per-period rate.
    pub rate: f64,
    /// The per-period growth rate of the payment (may exceed `rate`).
    pub growth: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// Amortise this present value into a growing payment stream.
    pub present: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_growing_periods` tool (ADR-0063). Present-anchored only,
/// for the reason [`GrowingPaymentInput`] records.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GrowingPeriodsInput {
    /// Per-period rate.
    pub rate: f64,
    /// The per-period growth rate of the payment (may exceed `rate`).
    pub growth: f64,
    /// The first payment, at the end of period 1. Each later payment is
    /// `(1 + growth)` times the one before.
    pub payment: f64,
    /// Amortise this present value. When `rate` exceeds `growth` it must be below the
    /// growing-perpetuity value `payment / (rate − growth)`, which no finite number of
    /// payments reaches.
    pub present: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `annuity_growing_rate` tool (ADR-0063). Present-anchored only, for
/// the reason [`GrowingPaymentInput`] records; the rate is what is being solved for,
/// so only `growth` is supplied.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct GrowingRateInput {
    /// The per-period growth rate of the payment.
    pub growth: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// The first payment, at the end of period 1. Each later payment is
    /// `(1 + growth)` times the one before.
    pub payment: f64,
    /// Amortise this present value.
    pub present: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `continuous_future_value` and `continuous_present_value` tools.
/// `rate` is the force of interest δ; `years` is a continuous span (it may be
/// fractional or negative), not a period count (ADR-0036).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ContinuousValueInput {
    /// The force of interest δ (e.g. `0.05` for a 5% continuously compounded
    /// annual rate).
    pub rate: f64,
    /// The span in years (a continuous duration; may be fractional or negative).
    pub years: f64,
    /// The amount to grow (`continuous_future_value`) or discount
    /// (`continuous_present_value`).
    pub amount: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `continuous_rate` tool (solve for the force of interest). Both
/// amounts are required — unlike the annuity solves there is no present/future
/// anchor to choose, because the force of interest is read from the pair
/// (ADR-0064).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ContinuousRateSolveInput {
    /// The span in years (a continuous duration; may be fractional or negative).
    pub years: f64,
    /// The present amount.
    pub present: f64,
    /// The future amount. Must be non-zero and the same sign as `present`.
    pub future: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `continuous_years` tool (solve for the span in years).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ContinuousYearsSolveInput {
    /// The force of interest δ. Must be non-zero: at zero nothing grows, so every
    /// span satisfies `future = present` and none is the answer.
    pub rate: f64,
    /// The present amount.
    pub present: f64,
    /// The future amount. Must be non-zero and the same sign as `present`.
    pub future: f64,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}

/// Input for the `continuous_from_effective` and `continuous_effective` bridge
/// tools — a single rate, whose meaning depends on the tool (an effective annual
/// rate in, or a force of interest in). Rate-only, so no currency.
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ContinuousRateInput {
    /// For `continuous_from_effective`, an effective annual rate (e.g. `0.05`);
    /// for `continuous_effective`, a force of interest δ.
    pub rate: f64,
}

/// Input for the `convert` tool (foreign-exchange). The amount is denominated in
/// `from`; the result is in `to`. Unlike the amount-bearing tools, currency is
/// intrinsic here, so `from`/`to` are required (not the optional `currency`
/// field).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct ConvertInput {
    /// The amount to convert, denominated in `from`.
    pub amount: f64,
    /// The currency the amount is in (ISO 4217, e.g. `USD`).
    pub from: Currency,
    /// The currency to convert into (ISO 4217, e.g. `EUR`).
    pub to: Currency,
    /// Units of `to` per unit of `from` (must be finite and positive).
    pub rate: f64,
}

/// Input for the `annuity_payment` and `annuity_due_payment` tools. Provide exactly
/// one of `present` (amortise a balance) or `future` (accumulate to a target — the
/// sinking-fund payment), the same anchored shape [`AnnuityPeriodsInput`] and
/// [`AnnuityRateInput`] use (ADR-0062).
#[derive(Debug, Deserialize, JsonSchema)]
pub(crate) struct AnnuityPaymentInput {
    /// Per-period rate.
    pub rate: f64,
    /// Number of periods (may be fractional).
    pub periods: f64,
    /// Amortise this present value into level payments (mutually exclusive with
    /// `future`).
    #[serde(default)]
    pub present: Option<f64>,
    /// Accumulate to this future value — the sinking-fund payment, "how much must
    /// be set aside each period to reach this target" (mutually exclusive with
    /// `present`).
    #[serde(default)]
    pub future: Option<f64>,
    /// ISO 4217 currency to denominate the amounts in (e.g. `USD`, `JPY`).
    /// Omit for currency-agnostic (`XXX`) amounts. An unknown code is rejected.
    #[serde(default)]
    pub currency: Option<Currency>,
}
