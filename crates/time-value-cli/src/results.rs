//! Typed results for the CLI (ADR-0039).
//!
//! Each command builds one of these DTOs and renders it two ways from the **one**
//! shape: as a JSON object under `--json` (via [`Serialize`]), or as the plain
//! line / TSV table otherwise. Defining the shape once is what keeps the two
//! renderings in step — the amortization schedule in particular was previously
//! spelled out separately for JSON and for the table.
//!
//! The shapes mirror the MCP surface (ADR-0028 §4 as amended by ADR-0039): a
//! monetary result is `{ value, currency? }`, a rate or period count is
//! `{ value }`, and the schedule is `{ schedule: [...], currency? }`. The DTOs
//! live here in the binary crate; the core carries no wire contract.

use time_value::{amortization::Installment, Currency, Money};

use serde::Serialize;

/// A monetary result: a magnitude and, when not currency-agnostic (`XXX`), the
/// ISO 4217 code it is denominated in.
#[derive(Debug, Serialize)]
pub(crate) struct MoneyResult {
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<String>,
}

impl From<Money> for MoneyResult {
    fn from(money: Money) -> Self {
        let currency = money.currency();
        Self {
            value: money.value(),
            currency: (currency != Currency::Xxx).then(|| currency.code().to_owned()),
        }
    }
}

/// A non-monetary result — a rate or a period count.
#[derive(Debug, Serialize)]
pub(crate) struct ScalarResult {
    value: f64,
}

/// The output of a scalar (single-value) command: either monetary (may echo a
/// currency) or a bare number. The variant carries which, so rendering and the
/// `--json` shape follow from the type rather than a runtime flag.
pub(crate) enum ScalarOutput {
    Money(MoneyResult),
    Plain(ScalarResult),
}

impl ScalarOutput {
    /// A monetary result, taking its currency from the [`Money`] the operation
    /// produced (ADR-0037).
    pub(crate) fn money(amount: Money) -> Self {
        Self::Money(amount.into())
    }

    /// A bare numeric result (a rate or a period count).
    pub(crate) fn plain(value: f64) -> Self {
        Self::Plain(ScalarResult { value })
    }

    /// The `--json` rendering: `{ "value": … }`, plus `"currency"` for a
    /// non-agnostic monetary result.
    pub(crate) fn to_json(&self) -> String {
        match self {
            Self::Money(m) => serde_json::to_string(m),
            Self::Plain(s) => serde_json::to_string(s),
        }
        .expect("a result DTO always serializes")
    }

    /// The plain rendering: the bare number, with the currency code appended for a
    /// non-agnostic monetary result.
    pub(crate) fn print(&self) {
        match self {
            Self::Money(MoneyResult {
                value,
                currency: Some(code),
            }) => println!("{value} {code}"),
            Self::Money(MoneyResult { value, .. }) | Self::Plain(ScalarResult { value }) => {
                println!("{value}");
            }
        }
    }
}

/// One row of an amortization schedule (the CLI analogue of the MCP row).
#[derive(Debug, Serialize)]
pub(crate) struct ScheduleRow {
    period: u32,
    payment: f64,
    interest: f64,
    principal: f64,
    balance: f64,
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

/// A tabular result: the schedule rows and — when not currency-agnostic — the
/// code they are denominated in.
#[derive(Debug, Serialize)]
pub(crate) struct ScheduleResult {
    schedule: Vec<ScheduleRow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    currency: Option<String>,
}

impl ScheduleResult {
    /// Collect a schedule's rows, tagging them with `currency` unless agnostic.
    pub(crate) fn new(rows: impl IntoIterator<Item = Installment>, currency: Currency) -> Self {
        Self {
            schedule: rows.into_iter().map(ScheduleRow::from).collect(),
            currency: (currency != Currency::Xxx).then(|| currency.code().to_owned()),
        }
    }

    /// The `--json` rendering: `{ "schedule": [ … ], "currency"? }`.
    pub(crate) fn to_json(&self) -> String {
        serde_json::to_string(self).expect("a schedule DTO always serializes")
    }

    /// The plain rendering: an optional `# currency:` line, a header, then one
    /// tab-separated row per period.
    pub(crate) fn print(&self) {
        if let Some(code) = &self.currency {
            println!("# currency: {code}");
        }
        println!("period\tpayment\tinterest\tprincipal\tbalance");
        for r in &self.schedule {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                r.period, r.payment, r.interest, r.principal, r.balance
            );
        }
    }
}
