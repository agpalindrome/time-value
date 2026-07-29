//! Typed results for the MCP tools (ADR-0039).
//!
//! Each tool returns one of these DTOs wrapped in [`rmcp::Json`], so the tool
//! macro derives and declares the `outputSchema` from the type and the value
//! lands in the response's `structuredContent`. The shapes are **uniform and
//! reused** across the tool families (ADR-0028 §4, amended by ADR-0039): a
//! monetary result is `{ value, currency? }`, a rate or period count is
//! `{ value }`. Building them *from* the library [`Money`] value is the single
//! place the [`Currency`] echo (ADR-0037) is applied, instead of at every tool
//! site.
//!
//! The DTOs live here, in the binary crate — the `no_std` core carries no wire
//! contract (ADR-0005, ADR-0011).

use schemars::JsonSchema;
use serde::Serialize;
use time_value::{amortization::Installment, Currency, Money};

/// A monetary tool result: a numeric magnitude and — when the amount is not
/// currency-agnostic (`XXX`) — the ISO 4217 code it is denominated in. The
/// `currency` field is omitted for agnostic amounts, so those keep the plain
/// `{ value }` shape.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct MoneyResult {
    /// The resulting amount.
    pub value: f64,
    /// The ISO 4217 currency code the amount is denominated in (e.g. `USD`);
    /// absent for currency-agnostic (`XXX`) amounts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

impl From<Money> for MoneyResult {
    /// Take the magnitude and currency straight from the library [`Money`] the
    /// operation produced — the operations preserve the inputs' currency, so this
    /// is the one place the ISO code is echoed (ADR-0037).
    fn from(money: Money) -> Self {
        let currency = money.currency();
        Self {
            value: money.value(),
            currency: (currency != Currency::Xxx).then(|| currency.code().to_owned()),
        }
    }
}

/// A non-monetary tool result — a rate or a period count, which carry no
/// currency.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ScalarResult {
    /// The resulting value (a per-period rate, an effective/nominal annual rate,
    /// or a number of periods).
    pub value: f64,
}

impl ScalarResult {
    /// A bare numeric result.
    pub(crate) fn new(value: f64) -> Self {
        Self { value }
    }
}

/// One row of an amortization schedule: the period index and the payment split
/// into interest and principal, with the balance remaining after it. The amounts
/// are plain magnitudes — the whole schedule shares one currency, echoed once on
/// [`ScheduleResult`] rather than repeated per row.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ScheduleRow {
    /// The 1-based period this installment falls in.
    pub period: u32,
    /// The total payment made this period.
    pub payment: f64,
    /// The portion of the payment that services interest.
    pub interest: f64,
    /// The portion of the payment that retires principal.
    pub principal: f64,
    /// The outstanding balance after this payment.
    pub balance: f64,
}

impl From<Installment> for ScheduleRow {
    fn from(installment: Installment) -> Self {
        Self {
            period: installment.period(),
            payment: installment.payment().value(),
            interest: installment.interest().value(),
            principal: installment.principal().value(),
            balance: installment.balance().value(),
        }
    }
}

/// A tabular tool result: the schedule rows and — when not currency-agnostic —
/// the ISO 4217 code they are denominated in (the tabular analogue of
/// [`MoneyResult`], ADR-0028 §4 as amended by ADR-0039).
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ScheduleResult {
    /// The amortization schedule, one row per period until the balance is
    /// retired.
    pub schedule: Vec<ScheduleRow>,
    /// The ISO 4217 currency code the amounts are denominated in (e.g. `USD`);
    /// absent for currency-agnostic (`XXX`) amounts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

impl ScheduleResult {
    /// Collect a schedule's rows, tagging them with `currency` unless it is the
    /// agnostic `XXX`.
    pub(crate) fn new(rows: impl IntoIterator<Item = Installment>, currency: Currency) -> Self {
        Self {
            schedule: rows.into_iter().map(ScheduleRow::from).collect(),
            currency: (currency != Currency::Xxx).then(|| currency.code().to_owned()),
        }
    }
}
