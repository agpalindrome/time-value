//! `time-value` — the command-line interface for the [`time_value`] library.
//!
//! A thin calculator over the library's operations (see
//! `docs/adr/0010-cli-surface.md`, extended by `docs/adr/0028-binary-surface-conventions.md`).
//! Commands are grouped by relationship family: `series` (net present/future
//! value, IRR, MIRR, and the dated XNPV/XIRR), `single-sum` (present/future value
//! and the solve-for `nper`/`rate` inverses), `annuity` (ordinary, annuity-`due`,
//! perpetuity, and growing forms, plus the `payment`/`nper`/`rate` solves — each
//! from a present or a future value), `rate` (conversions
//! between periodicities and nominal/effective quotes — the only family that
//! takes a periodicity), `continuous` (continuous compounding at a force of
//! interest — `fv`/`pv` over a real-number `years` span, plus the
//! effective-annual bridge; ADR-0036/0041, #68), `amortize` (a schedule), and the
//! standalone `convert` (foreign-exchange: restate an amount in another currency
//! at a caller-supplied rate — ADR-0034/0037, #67). `--rate` is a per-period
//! rate (annual for the dated `series xnpv`/`xirr`, a force of interest for
//! `continuous`); cashflows are positional.
//! Most results print as a plain number, or as a `{ "value": … }` JSON object
//! with `--json` (ADR-0039); `amortize` prints a table, or a
//! `{ "schedule": [ … ] }` JSON object under `--json`. A global
//! `--currency <CODE>` flag (default `XXX`, currency-agnostic) denominates every
//! amount in an ISO 4217 currency; a non-`XXX` code is echoed alongside monetary
//! results (`docs/adr/0034-money-and-currency.md`, ADR-0037).

mod results;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use time_value::{
    amortization, annuity, continuous, single_sum, Annual, Cashflows, ContinuousRate, Currency,
    DatedCashflow, DatedCashflows, FutureValue, FxRate, Growth, Money, Monthly, Payment, Period,
    PresentValue, Principal, Rate, TvmError,
};
use time_value_daycount::{act365_year_fraction, iso_to_day};

use crate::results::{ScalarOutput, ScheduleResult};

/// Type-safe time-value-of-money calculations.
#[derive(Parser)]
#[command(name = "time-value", version, about)]
struct Cli {
    /// Print the result as a JSON object instead of a plain number: `{ "value":
    /// … }`, plus a `"currency"` field for a monetary result in a non-`XXX`
    /// currency, or `{ "schedule": [ … ] }` for `amortize`.
    #[arg(long, global = true)]
    json: bool,

    /// Denominate all monetary amounts in this ISO 4217 currency (e.g. `USD`,
    /// `JPY`). Defaults to `XXX` (currency-agnostic), preserving plain-number
    /// output; a non-`XXX` currency is echoed alongside monetary results.
    #[arg(long, global = true, default_value = "XXX", value_parser = parse_currency)]
    currency: Currency,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Cashflow-series operations: net present/future value, IRR, MIRR, and the
    /// dated XNPV/XIRR.
    Series {
        #[command(subcommand)]
        command: SeriesCommand,
    },
    /// Single-sum operations: present/future value and the solve-for inverses.
    SingleSum {
        #[command(subcommand)]
        command: SingleSumCommand,
    },
    /// Annuity operations: ordinary, annuity-due, perpetuity, growing, and the solves.
    Annuity {
        #[command(subcommand)]
        command: AnnuityCommand,
    },
    /// Rate conversions: effective-annual, between periodicities, and nominal.
    Rate {
        #[command(subcommand)]
        command: RateCommand,
    },
    /// Continuous compounding: future/present value at a force of interest over a
    /// real-number span of years, plus the effective-annual rate bridge.
    Continuous {
        #[command(subcommand)]
        command: ContinuousCommand,
    },
    /// Amortization schedule: one row per period until the balance is retired.
    Amortize {
        /// Per-period rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The principal to amortise.
        #[arg(long, allow_hyphen_values = true)]
        principal: f64,
        /// Amortise over this many periods (mutually exclusive with --payment).
        #[arg(long, allow_hyphen_values = true)]
        periods: Option<f64>,
        /// Amortise with this level payment (mutually exclusive with --periods).
        #[arg(long, allow_hyphen_values = true)]
        payment: Option<f64>,
    },
    /// Convert an amount into another currency at a caller-supplied exchange
    /// rate (foreign exchange). The amount is denominated in `--from`; the result
    /// is in `--to`. The global `--currency` flag does not apply here.
    Convert {
        /// The currency the amount is in (ISO 4217, e.g. `USD`).
        #[arg(long, value_parser = parse_currency)]
        from: Currency,
        /// The currency to convert into (ISO 4217, e.g. `EUR`).
        #[arg(long, value_parser = parse_currency)]
        to: Currency,
        /// Units of `--to` per unit of `--from` (must be finite and positive).
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The amount to convert, denominated in `--from`.
        #[arg(value_name = "AMOUNT", allow_hyphen_values = true)]
        amount: f64,
    },
}

#[derive(Subcommand)]
enum SeriesCommand {
    /// Net present value of a cashflow series discounted at a per-period rate.
    Npv {
        /// Per-period discount rate (e.g. 0.01 for 1% per period).
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// Cashflows at periods 0, 1, 2, … (signed: outflow negative).
        #[arg(value_name = "CASHFLOW", allow_hyphen_values = true, num_args = 1.., required = true)]
        cashflows: Vec<f64>,
    },
    /// Net future value of a cashflow series compounded at a per-period rate.
    Nfv {
        /// Per-period rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// Cashflows at periods 0, 1, 2, … (signed: outflow negative).
        #[arg(value_name = "CASHFLOW", allow_hyphen_values = true, num_args = 1.., required = true)]
        cashflows: Vec<f64>,
    },
    /// Internal rate of return (per period) of a cashflow series.
    Irr {
        /// Initial guess for the Newton–Raphson solve.
        #[arg(long, default_value_t = 0.1, allow_hyphen_values = true)]
        guess: f64,
        /// Cashflows at periods 0, 1, 2, … (signed: outflow negative).
        #[arg(value_name = "CASHFLOW", allow_hyphen_values = true, num_args = 1.., required = true)]
        cashflows: Vec<f64>,
    },
    /// Modified internal rate of return of a cashflow series.
    Mirr {
        /// Per-period finance rate for discounting the outflows.
        #[arg(long, allow_hyphen_values = true)]
        finance: f64,
        /// Per-period reinvestment rate for compounding the inflows.
        #[arg(long, allow_hyphen_values = true)]
        reinvest: f64,
        /// Cashflows at periods 0, 1, 2, … (signed: outflow negative).
        #[arg(value_name = "CASHFLOW", allow_hyphen_values = true, num_args = 1.., required = true)]
        cashflows: Vec<f64>,
    },
    /// Net present value of cashflows on irregular dates, discounted at an annual
    /// rate (XNPV). Flows are `DATE:AMOUNT` pairs, e.g. `2020-01-01:-1000`.
    Xnpv {
        /// Annual discount rate (e.g. 0.1 for 10% per year).
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// Dated cashflows as `YYYY-MM-DD:AMOUNT` (signed: outflow negative). The
        /// first date is the valuation reference.
        #[arg(value_name = "DATE:AMOUNT", num_args = 1.., required = true)]
        flows: Vec<String>,
    },
    /// Internal rate of return of cashflows on irregular dates (XIRR), as an
    /// annual rate. Flows are `DATE:AMOUNT` pairs.
    Xirr {
        /// Initial guess for the Newton–Raphson solve (annual).
        #[arg(long, default_value_t = 0.1, allow_hyphen_values = true)]
        guess: f64,
        /// Dated cashflows as `YYYY-MM-DD:AMOUNT` (signed: outflow negative). The
        /// first date is the valuation reference.
        #[arg(value_name = "DATE:AMOUNT", num_args = 1.., required = true)]
        flows: Vec<String>,
    },
}

#[derive(Subcommand)]
enum SingleSumCommand {
    /// Present value of a single future amount.
    Pv {
        /// Per-period discount rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// Number of periods (may be fractional).
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The future amount to discount.
        #[arg(long, allow_hyphen_values = true)]
        future: f64,
    },
    /// Future value of a single present amount.
    Fv {
        /// Per-period rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// Number of periods (may be fractional).
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The present amount to compound.
        #[arg(long, allow_hyphen_values = true)]
        present: f64,
    },
    /// Solve for the number of periods that grows a present to a future amount.
    Nper {
        /// Per-period rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The present amount.
        #[arg(long, allow_hyphen_values = true)]
        present: f64,
        /// The future amount.
        #[arg(long, allow_hyphen_values = true)]
        future: f64,
    },
    /// Solve for the per-period rate that grows a present to a future amount.
    Rate {
        /// Number of periods (may be fractional).
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The present amount.
        #[arg(long, allow_hyphen_values = true)]
        present: f64,
        /// The future amount.
        #[arg(long, allow_hyphen_values = true)]
        future: f64,
    },
}

#[derive(Subcommand)]
enum AnnuityCommand {
    /// Present value of an annuity paying a fixed amount each period.
    Pv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The payment made at the end of each period.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Future value of an annuity paying a fixed amount each period.
    Fv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The payment made at the end of each period.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Level payment, from a present value (amortisation) or a future value (a
    /// sinking fund).
    Payment {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// Amortise this present value (mutually exclusive with --future).
        #[arg(long, allow_hyphen_values = true)]
        present: Option<f64>,
        /// Accumulate to this future value — the sinking-fund payment (mutually
        /// exclusive with --present).
        #[arg(long, allow_hyphen_values = true)]
        future: Option<f64>,
    },
    /// Solve for the number of level payments, from a present or future value.
    Nper {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The payment made at the end of each period.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
        /// Solve from this present value (mutually exclusive with --future).
        #[arg(long, allow_hyphen_values = true)]
        present: Option<f64>,
        /// Solve from this future value (mutually exclusive with --present).
        #[arg(long, allow_hyphen_values = true)]
        future: Option<f64>,
    },
    /// Solve for the per-period rate of an annuity, from a present or future value.
    Rate {
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The payment made at the end of each period.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
        /// Solve from this present value (mutually exclusive with --future).
        #[arg(long, allow_hyphen_values = true)]
        present: Option<f64>,
        /// Solve from this future value (mutually exclusive with --present).
        #[arg(long, allow_hyphen_values = true)]
        future: Option<f64>,
    },
    /// Present value of a level perpetuity (a payment forever).
    Perpetuity {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The payment made at the end of each period, forever.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Present value of a perpetuity whose payment grows each period.
    GrowingPerpetuity {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The per-period growth rate of the payment (must be below --rate).
        #[arg(long, allow_hyphen_values = true)]
        growth: f64,
        /// The first payment (at the end of period 1).
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Growing-annuity calculations: the payment grows each period.
    Growing {
        #[command(subcommand)]
        command: AnnuityGrowingCommand,
    },
    /// Annuity-due (start-of-period payment) calculations.
    Due {
        #[command(subcommand)]
        command: AnnuityDueCommand,
    },
}

#[derive(Subcommand)]
enum AnnuityDueCommand {
    /// Present value of an annuity-due paying a fixed amount each period.
    Pv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The payment made at the start of each period.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Future value of an annuity-due paying a fixed amount each period.
    Fv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The payment made at the start of each period.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Level start-of-period payment, from a present value (amortisation) or a
    /// future value (a sinking fund).
    Payment {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// Amortise this present value (mutually exclusive with --future).
        #[arg(long, allow_hyphen_values = true)]
        present: Option<f64>,
        /// Accumulate to this future value — the sinking-fund payment (mutually
        /// exclusive with --present).
        #[arg(long, allow_hyphen_values = true)]
        future: Option<f64>,
    },
    /// Present value of a level perpetuity-due (a payment at the start of every
    /// period, forever).
    Perpetuity {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The payment made at the start of each period, forever.
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
}

/// The growing-annuity subcommands (ADR-0048/0049). Grouped under `growing`
/// rather than spread across the ordinary and `due` groups: growth is one
/// variation applied to all four values, and keeping them together also keeps the
/// `annuity` dispatcher short.
#[derive(Subcommand)]
enum AnnuityGrowingCommand {
    /// Present value of a growing annuity (payments at the end of each period).
    Pv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The per-period growth rate of the payment (may exceed --rate).
        #[arg(long, allow_hyphen_values = true)]
        growth: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The first payment (at the end of period 1).
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Future value of a growing annuity (payments at the end of each period).
    Fv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The per-period growth rate of the payment (may exceed --rate).
        #[arg(long, allow_hyphen_values = true)]
        growth: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The first payment (at the end of period 1).
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Present value of a growing annuity-due (payments at the start of each period).
    DuePv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The per-period growth rate of the payment (may exceed --rate).
        #[arg(long, allow_hyphen_values = true)]
        growth: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The first payment (at the start of period 1).
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Future value of a growing annuity-due (payments at the start of each period).
    DueFv {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The per-period growth rate of the payment (may exceed --rate).
        #[arg(long, allow_hyphen_values = true)]
        growth: f64,
        #[arg(long, allow_hyphen_values = true)]
        periods: f64,
        /// The first payment (at the start of period 1).
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
    /// Present value of a growing perpetuity-due (payments at the start of every
    /// period, forever).
    DuePerpetuity {
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The per-period growth rate of the payment (must be below --rate).
        #[arg(long, allow_hyphen_values = true)]
        growth: f64,
        /// The first payment (at the start of period 1, i.e. today).
        #[arg(long, allow_hyphen_values = true)]
        payment: f64,
    },
}

/// The compounding periodicity a `rate` command operates at — the only place a
/// periodicity is a runtime input (ADR-0028 §3). A closed set, so it is a clap
/// `ValueEnum` rather than a free string (ADR-0039): an unknown value is rejected
/// by parsing, and `--help` lists the choices. The names are lower-kebab
/// (`semi-annual`), matching the core marker types.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum Periodicity {
    Daily,
    Weekly,
    Monthly,
    Quarterly,
    SemiAnnual,
    Annual,
}

#[derive(Subcommand)]
enum RateCommand {
    /// Effective annual rate (EAR) equivalent to a per-period rate.
    Ear {
        /// The per-period rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The periodicity the rate is expressed in.
        #[arg(long, value_enum)]
        periodicity: Periodicity,
    },
    /// Convert a per-period rate from one periodicity to another (same EAR).
    Convert {
        /// The per-period rate under `--from`.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The periodicity the rate is expressed in.
        #[arg(long, value_enum)]
        from: Periodicity,
        /// The periodicity to express the rate in.
        #[arg(long, value_enum)]
        to: Periodicity,
    },
    /// Per-period rate from a nominal annual rate (APR) at a periodicity.
    FromNominal {
        /// The nominal annual rate (APR).
        #[arg(long, allow_hyphen_values = true)]
        nominal: f64,
        /// The compounding periodicity.
        #[arg(long, value_enum)]
        periodicity: Periodicity,
    },
    /// Nominal annual rate (APR) quoted from a per-period rate.
    Nominal {
        /// The per-period rate.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The compounding periodicity.
        #[arg(long, value_enum)]
        periodicity: Periodicity,
    },
}

/// Continuous-compounding operations (ADR-0036). `--rate` is the force of interest
/// δ; `--years` is a real-number span (it may be fractional or negative), not a
/// period count. The `fv`/`pv` amounts honour the global `--currency`; the rate
/// bridges (`from-effective`/`effective`) are pure rates and carry none.
#[derive(Subcommand)]
enum ContinuousCommand {
    /// Future value grown continuously: `FV = PV · e^(δ · years)`.
    Fv {
        /// The force of interest δ (e.g. 0.05 for a 5% continuously compounded
        /// annual rate).
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The span in years (continuous; may be fractional or negative).
        #[arg(long, allow_hyphen_values = true)]
        years: f64,
        /// The present amount to grow.
        #[arg(long, allow_hyphen_values = true)]
        present: f64,
    },
    /// Present value discounted continuously: `PV = FV · e^(−δ · years)`.
    Pv {
        /// The force of interest δ.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
        /// The span in years (continuous; may be fractional or negative).
        #[arg(long, allow_hyphen_values = true)]
        years: f64,
        /// The future amount to discount.
        #[arg(long, allow_hyphen_values = true)]
        future: f64,
    },
    /// The force of interest δ equivalent to an effective annual rate:
    /// `δ = ln(1 + r)`.
    FromEffective {
        /// The effective annual rate (e.g. 0.05 for 5% EAR).
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
    },
    /// The effective annual rate equivalent to a force of interest:
    /// `r = e^δ − 1`.
    Effective {
        /// The force of interest δ.
        #[arg(long, allow_hyphen_values = true)]
        rate: f64,
    },
}

// The periodicity tag does not affect any result for the period-indexed
// operations (ADR-0010); the CLI fixes it to one marker to satisfy the type
// parameter. The dated `series xnpv`/`xirr` are intrinsically annual (ADR-0029).
// The `rate` conversion group is the exception: periodicity is intrinsic there,
// so it dispatches the runtime name to a marker type (`dispatch_periodicity!`).
type Per = Monthly;

/// Run `$body` with the type alias `$ty` bound to the core periodicity marker for
/// the [`Periodicity`] value `$value`. The match is exhaustive: an unknown
/// periodicity is already rejected by clap parsing (ADR-0039), so there is no
/// error arm.
macro_rules! dispatch_periodicity {
    ($value:expr, $ty:ident => $body:expr) => {{
        match $value {
            Periodicity::Daily => {
                type $ty = time_value::Daily;
                $body
            }
            Periodicity::Weekly => {
                type $ty = time_value::Weekly;
                $body
            }
            Periodicity::Monthly => {
                type $ty = time_value::Monthly;
                $body
            }
            Periodicity::Quarterly => {
                type $ty = time_value::Quarterly;
                $body
            }
            Periodicity::SemiAnnual => {
                type $ty = time_value::SemiAnnual;
                $body
            }
            Periodicity::Annual => {
                type $ty = time_value::Annual;
                $body
            }
        }
    }};
}

/// clap `value_parser` for `--currency`: resolves an ISO 4217 code to a
/// [`Currency`], rejecting anything not in the canonical set.
fn parse_currency(code: &str) -> Result<Currency, String> {
    Currency::from_code(code).ok_or_else(|| format!("unknown ISO 4217 currency code `{code}`"))
}

/// A monetary result — carries its currency so a non-`XXX` code is echoed.
fn money_out(amount: Money) -> ScalarOutput {
    ScalarOutput::money(amount)
}

/// A non-monetary result (a rate or a period count) — never carries a currency.
fn plain_out(value: f64) -> ScalarOutput {
    ScalarOutput::plain(value)
}

fn rate(value: f64) -> Result<Rate<Per>> {
    Rate::new(value).context("invalid rate (must be finite and greater than -100%)")
}

fn annual_rate(value: f64) -> Result<Rate<Annual>> {
    Rate::new(value).context("invalid rate (must be finite and greater than -100%)")
}

/// A force of interest δ for the `continuous` family. Unlike a per-period [`Rate`],
/// every finite force is valid — there is no `> −100%` floor (ADR-0036).
fn continuous_rate(value: f64) -> Result<ContinuousRate> {
    ContinuousRate::new(value).context("invalid force of interest (must be finite)")
}

fn period(value: f64) -> Result<Period<Per>> {
    Period::new(value).context("invalid period count (must be finite and non-negative)")
}

fn money(value: f64, currency: Currency) -> Result<Money> {
    Money::new(value, currency).context("invalid amount (must be finite)")
}

fn cashflows(values: &[f64], currency: Currency) -> Result<Vec<Money>> {
    values.iter().copied().map(|v| money(v, currency)).collect()
}

/// The value a solve-for operation is anchored to — exactly one of a present or a
/// future amount. The two `--present`/`--future` flags are mutually exclusive and
/// one is required.
///
/// `Copy`, so a dispatcher can resolve the anchor once and hand it to a helper
/// ([`level_payment`]) without ceremony.
#[derive(Clone, Copy)]
enum Anchor {
    Present(f64),
    Future(f64),
}

fn anchor(present: Option<f64>, future: Option<f64>) -> Result<Anchor> {
    match (present, future) {
        (Some(p), None) => Ok(Anchor::Present(p)),
        (None, Some(f)) => Ok(Anchor::Future(f)),
        (None, None) => bail!("provide either --present or --future"),
        (Some(_), Some(_)) => bail!("--present and --future are mutually exclusive"),
    }
}

// ---- Dated flows (XNPV/XIRR): ISO dates → ACT/365 year-offsets ----
//
// The core takes year-offsets, not a date type (ADR-0029); the CLI accepts real
// `YYYY-MM-DD` dates and converts them with the shared `time-value-daycount`
// ACT/365 day-count (ADR-0030), so no date dependency reaches the binary.

/// Parse `DATE:AMOUNT` pairs into dated cashflows, converting each ISO date to a
/// year-offset from the first flow (ACT/365).
fn dated_flows(pairs: &[String], currency: Currency) -> Result<Vec<DatedCashflow>> {
    let mut flows = Vec::with_capacity(pairs.len());
    let mut reference: Option<i64> = None;
    for pair in pairs {
        let (date, amount) = pair.split_once(':').with_context(|| {
            format!("invalid flow `{pair}` (expected DATE:AMOUNT, e.g. 2020-01-01:-1000)")
        })?;
        let day = iso_to_day(date)?;
        let reference = *reference.get_or_insert(day);
        let amount: f64 = amount
            .parse()
            .with_context(|| format!("invalid amount in flow `{pair}`"))?;
        let offset_years = act365_year_fraction(reference, day);
        flows.push(DatedCashflow::new(offset_years, money(amount, currency)?)?);
    }
    Ok(flows)
}

/// Dispatch the `series` subcommands, returning the JSON label and result value.
fn run_series(command: SeriesCommand, currency: Currency) -> Result<ScalarOutput> {
    Ok(match command {
        SeriesCommand::Npv {
            rate: r,
            cashflows: cf,
        } => {
            let flows = cashflows(&cf, currency)?;
            let series = Cashflows::<Per>::new(&flows);
            money_out(series.net_present_value(rate(r)?)?)
        }
        SeriesCommand::Nfv {
            rate: r,
            cashflows: cf,
        } => {
            let flows = cashflows(&cf, currency)?;
            let series = Cashflows::<Per>::new(&flows);
            money_out(series.net_future_value(rate(r)?)?)
        }
        SeriesCommand::Irr {
            guess,
            cashflows: cf,
        } => {
            let flows = cashflows(&cf, currency)?;
            let series = Cashflows::<Per>::new(&flows);
            let irr = series
                .internal_rate_of_return_from(guess)
                .context("no internal rate of return found")?;
            plain_out(irr.value())
        }
        SeriesCommand::Mirr {
            finance,
            reinvest,
            cashflows: cf,
        } => {
            let flows = cashflows(&cf, currency)?;
            let series = Cashflows::<Per>::new(&flows);
            let mirr = series
                .modified_internal_rate_of_return(rate(finance)?, rate(reinvest)?)
                .map_err(|e| anyhow!("modified internal rate of return: {e}"))?;
            plain_out(mirr.value())
        }
        SeriesCommand::Xnpv { rate: r, flows } => {
            let dated = dated_flows(&flows, currency)?;
            let series = DatedCashflows::new(&dated);
            money_out(series.net_present_value(annual_rate(r)?)?)
        }
        SeriesCommand::Xirr { guess, flows } => {
            let dated = dated_flows(&flows, currency)?;
            let series = DatedCashflows::new(&dated);
            let irr = series
                .internal_rate_of_return_from(guess)
                .context("no internal rate of return found")?;
            plain_out(irr.value())
        }
    })
}

/// Dispatch the `single-sum` subcommands.
#[allow(clippy::needless_pass_by_value)]
fn run_single_sum(command: SingleSumCommand, currency: Currency) -> Result<ScalarOutput> {
    Ok(match command {
        SingleSumCommand::Pv {
            rate: r,
            periods: n,
            future,
        } => money_out(single_sum::present_value(
            rate(r)?,
            period(n)?,
            money(future, currency)?,
        )?),
        SingleSumCommand::Fv {
            rate: r,
            periods: n,
            present,
        } => money_out(single_sum::future_value(
            rate(r)?,
            period(n)?,
            money(present, currency)?,
        )?),
        SingleSumCommand::Nper {
            rate: r,
            present,
            future,
        } => {
            let n = single_sum::periods(
                rate(r)?,
                PresentValue(money(present, currency)?),
                FutureValue(money(future, currency)?),
            )
            .map_err(|e| anyhow!("number of periods: {e}"))?;
            plain_out(n.value())
        }
        SingleSumCommand::Rate {
            periods: n,
            present,
            future,
        } => {
            let r = single_sum::rate::<Per>(
                period(n)?,
                PresentValue(money(present, currency)?),
                FutureValue(money(future, currency)?),
            )
            .context("no rate solves these inputs")?;
            plain_out(r.value())
        }
    })
}

/// Dispatch the `annuity` subcommands (ordinary, solves, perpetuities, and due).
// By-value dispatch mirrors the other `run_*` helpers; the arms are all-`Copy`, so
// clippy would rather borrow — but owning the parsed command here is the clearer shape.
#[allow(clippy::needless_pass_by_value)]
fn run_annuity(command: AnnuityCommand, currency: Currency) -> Result<ScalarOutput> {
    Ok(match command {
        AnnuityCommand::Pv {
            rate: r,
            periods: n,
            payment,
        } => money_out(annuity::present_value(
            rate(r)?,
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityCommand::Fv {
            rate: r,
            periods: n,
            payment,
        } => money_out(annuity::future_value(
            rate(r)?,
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityCommand::Payment {
            rate: r,
            periods: n,
            present,
            future,
        } => level_payment(
            r,
            n,
            anchor(present, future)?,
            currency,
            (annuity::payment, annuity::payment_from_future),
            "annuity payment",
        )?,
        AnnuityCommand::Nper {
            rate: r,
            payment,
            present,
            future,
        } => {
            let n = match anchor(present, future)? {
                Anchor::Present(p) => annuity::periods(
                    rate(r)?,
                    Payment(money(payment, currency)?),
                    PresentValue(money(p, currency)?),
                ),
                Anchor::Future(f) => annuity::periods_from_future(
                    rate(r)?,
                    Payment(money(payment, currency)?),
                    FutureValue(money(f, currency)?),
                ),
            }
            .map_err(|e| anyhow!("number of periods: {e}"))?;
            plain_out(n.value())
        }
        AnnuityCommand::Rate {
            periods: n,
            payment,
            present,
            future,
        } => {
            let r = match anchor(present, future)? {
                Anchor::Present(p) => annuity::rate::<Per>(
                    period(n)?,
                    Payment(money(payment, currency)?),
                    PresentValue(money(p, currency)?),
                ),
                Anchor::Future(f) => annuity::rate_from_future::<Per>(
                    period(n)?,
                    Payment(money(payment, currency)?),
                    FutureValue(money(f, currency)?),
                ),
            }
            .context("no rate solves these inputs")?;
            plain_out(r.value())
        }
        AnnuityCommand::Perpetuity { rate: r, payment } => {
            let pv = annuity::perpetuity(rate(r)?, money(payment, currency)?)
                .context("perpetuity diverges (rate must exceed 0)")?;
            money_out(pv)
        }
        AnnuityCommand::GrowingPerpetuity {
            rate: r,
            growth,
            payment,
        } => {
            let pv = annuity::growing_perpetuity(
                rate(r)?,
                Growth(rate(growth)?),
                money(payment, currency)?,
            )
            .context("growing perpetuity diverges (rate must exceed growth)")?;
            money_out(pv)
        }
        AnnuityCommand::Growing { command } => run_annuity_growing(command, currency)?,
        AnnuityCommand::Due { command } => run_annuity_due(command, currency)?,
    })
}

/// A core solve for the level payment: from a present value (amortisation) or from
/// a future one (a sinking fund). Both signatures are identical, which is what lets
/// one helper serve the ordinary and the annuity-due groups alike.
type PaymentSolve = fn(Rate<Per>, Period<Per>, Money) -> Result<Money, TvmError>;

/// Solve the level payment from whichever value the caller anchored to — ADR-0028
/// §2's mutually-exclusive `--present` / `--future` pair, as `nper` and `rate`
/// already use it (ADR-0062).
///
/// Shared by `annuity payment` and `annuity due payment`, which differ only in the
/// pair of core functions and the error label: keeping the branch here instead of
/// inlining it in both arms is what stops `run_annuity` growing past the
/// function-length lint, the same structural answer ADR-0049 reached for the
/// growing subgroup.
fn level_payment(
    r: f64,
    n: f64,
    anchor: Anchor,
    currency: Currency,
    (from_present, from_future): (PaymentSolve, PaymentSolve),
    label: &str,
) -> Result<ScalarOutput> {
    let (rate, periods) = (rate(r)?, period(n)?);
    let payment = match anchor {
        Anchor::Present(p) => from_present(rate, periods, money(p, currency)?),
        Anchor::Future(f) => from_future(rate, periods, money(f, currency)?),
    }
    .map_err(|e| anyhow!("{label}: {e}"))?;
    Ok(money_out(payment))
}

/// Dispatch the `annuity growing` subcommands — the finite growing annuity in all
/// four forms (present/future × ordinary/due), ADR-0048/0049.
#[allow(clippy::needless_pass_by_value)]
fn run_annuity_growing(command: AnnuityGrowingCommand, currency: Currency) -> Result<ScalarOutput> {
    Ok(match command {
        AnnuityGrowingCommand::Pv {
            rate: r,
            growth,
            periods: n,
            payment,
        } => money_out(annuity::growing_present_value(
            rate(r)?,
            Growth(rate(growth)?),
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityGrowingCommand::Fv {
            rate: r,
            growth,
            periods: n,
            payment,
        } => money_out(annuity::growing_future_value(
            rate(r)?,
            Growth(rate(growth)?),
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityGrowingCommand::DuePv {
            rate: r,
            growth,
            periods: n,
            payment,
        } => money_out(annuity::due::growing_present_value(
            rate(r)?,
            Growth(rate(growth)?),
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityGrowingCommand::DueFv {
            rate: r,
            growth,
            periods: n,
            payment,
        } => money_out(annuity::due::growing_future_value(
            rate(r)?,
            Growth(rate(growth)?),
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityGrowingCommand::DuePerpetuity {
            rate: r,
            growth,
            payment,
        } => {
            let pv = annuity::due::growing_perpetuity(
                rate(r)?,
                Growth(rate(growth)?),
                money(payment, currency)?,
            )
            .context("growing perpetuity-due diverges (rate must exceed growth)")?;
            money_out(pv)
        }
    })
}

/// Dispatch the `annuity due` subcommands.
#[allow(clippy::needless_pass_by_value)]
fn run_annuity_due(command: AnnuityDueCommand, currency: Currency) -> Result<ScalarOutput> {
    Ok(match command {
        AnnuityDueCommand::Pv {
            rate: r,
            periods: n,
            payment,
        } => money_out(annuity::due::present_value(
            rate(r)?,
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityDueCommand::Fv {
            rate: r,
            periods: n,
            payment,
        } => money_out(annuity::due::future_value(
            rate(r)?,
            period(n)?,
            money(payment, currency)?,
        )?),
        AnnuityDueCommand::Payment {
            rate: r,
            periods: n,
            present,
            future,
        } => level_payment(
            r,
            n,
            anchor(present, future)?,
            currency,
            (annuity::due::payment, annuity::due::payment_from_future),
            "annuity-due payment",
        )?,
        AnnuityDueCommand::Perpetuity { rate: r, payment } => {
            let pv = annuity::due::perpetuity(rate(r)?, money(payment, currency)?)
                .context("perpetuity-due diverges (rate must exceed 0)")?;
            money_out(pv)
        }
    })
}

/// Dispatch the `rate` subcommands. Each names a periodicity at runtime, resolved
/// to a marker type via `dispatch_periodicity!`.
fn run_rate(command: &RateCommand) -> Result<ScalarOutput> {
    Ok(match *command {
        RateCommand::Ear {
            rate: r,
            periodicity,
        } => {
            let ear = dispatch_periodicity!(periodicity, P => {
                Rate::<P>::new(r)?
                    .effective_annual()
                    .context("effective annual rate is undefined for this input")?
                    .value()
            });
            plain_out(ear)
        }
        RateCommand::Convert { rate: r, from, to } => {
            let converted = dispatch_periodicity!(from, P => {
                let source = Rate::<P>::new(r)?;
                dispatch_periodicity!(to, Q => {
                    source
                        .convert::<Q>()
                        .context("rate conversion is undefined for this input")?
                        .value()
                })
            });
            plain_out(converted)
        }
        RateCommand::FromNominal {
            nominal,
            periodicity,
        } => {
            let periodic = dispatch_periodicity!(periodicity, P => {
                Rate::<P>::from_nominal_annual(nominal)
                    .context("invalid nominal rate")?
                    .value()
            });
            plain_out(periodic)
        }
        RateCommand::Nominal {
            rate: r,
            periodicity,
        } => {
            let nominal = dispatch_periodicity!(periodicity, P => {
                Rate::<P>::new(r)?
                    .nominal_annual()
                    .context("nominal annual rate is undefined for this input")?
            });
            plain_out(nominal)
        }
    })
}

/// The scalar (single-value) operations. Tabular `amortize` is handled separately.
fn run_scalar(command: Command, currency: Currency) -> Result<ScalarOutput> {
    match command {
        Command::Series { command } => run_series(command, currency),
        Command::SingleSum { command } => run_single_sum(command, currency),
        Command::Annuity { command } => run_annuity(command, currency),
        // The `rate` family takes no monetary amounts, so currency does not apply.
        Command::Rate { command } => run_rate(&command),
        Command::Continuous { command } => run_continuous(command, currency),
        // `convert` names its own currencies (`--from`/`--to`); the global
        // `--currency` does not apply.
        Command::Convert {
            from,
            to,
            rate: r,
            amount,
        } => run_convert(from, to, r, amount),
        Command::Amortize { .. } => unreachable!("amortize is handled by run_amortize"),
    }
}

/// Dispatch the `continuous` subcommands (ADR-0036/0041, #68). `fv`/`pv` are
/// monetary and honour the global `--currency`; the rate bridges are pure rates.
#[allow(clippy::needless_pass_by_value)]
fn run_continuous(command: ContinuousCommand, currency: Currency) -> Result<ScalarOutput> {
    Ok(match command {
        ContinuousCommand::Fv {
            rate: r,
            years,
            present,
        } => money_out(
            continuous::future_value(continuous_rate(r)?, years, money(present, currency)?)
                .context("continuous future value is undefined for these inputs")?,
        ),
        ContinuousCommand::Pv {
            rate: r,
            years,
            future,
        } => money_out(
            continuous::present_value(continuous_rate(r)?, years, money(future, currency)?)
                .context("continuous present value is undefined for these inputs")?,
        ),
        ContinuousCommand::FromEffective { rate: r } => {
            plain_out(ContinuousRate::from_effective_annual(annual_rate(r)?).value())
        }
        ContinuousCommand::Effective { rate: r } => {
            let r_eff = continuous_rate(r)?
                .effective_annual()
                .context("effective annual rate is undefined for this force of interest")?;
            plain_out(r_eff.value())
        }
    })
}

/// Convert an amount from one currency into another at a caller-supplied FX rate
/// (ADR-0034/0037). The result is a monetary value tagged the `to` currency, so it
/// echoes that code like any other monetary result. `FxRate::new` rejects a
/// non-positive or non-finite rate, and the extreme band whose reciprocal would
/// overflow (ADR-0053); the multiply can overflow on an extreme input.
///
/// The rate error interpolates the library's message rather than restating it
/// statically, per ADR-0052 — the library names the accepted band, which no static
/// string here could keep in step with.
fn run_convert(from: Currency, to: Currency, rate: f64, amount: f64) -> Result<ScalarOutput> {
    let fx = FxRate::new(from, to, rate).map_err(|e| anyhow!("invalid exchange rate: {e}"))?;
    let converted = money(amount, from)?
        .convert(fx)
        .context("converted amount is out of range")?;
    Ok(money_out(converted))
}

/// Print an amortization schedule: aligned rows, or a `{ "schedule": [ … ] }`
/// JSON object under `--json` (ADR-0028's tabular convention, typed per ADR-0039).
fn run_amortize(
    json: bool,
    currency: Currency,
    r: f64,
    principal: f64,
    periods: Option<f64>,
    payment: Option<f64>,
) -> Result<()> {
    let rate = rate(r)?;
    let principal = money(principal, currency)?;
    let schedule = match (periods, payment) {
        (Some(n), None) => amortization::Schedule::<Per>::for_term(rate, period(n)?, principal),
        (None, Some(p)) => amortization::Schedule::<Per>::with_payment(
            rate,
            Payment(money(p, currency)?),
            Principal(principal),
        ),
        (None, None) => bail!("provide either --periods or --payment"),
        (Some(_), Some(_)) => bail!("--periods and --payment are mutually exclusive"),
    }
    .map_err(|e| anyhow!("amortization schedule: {e}"))?;

    // One typed shape backs both renderings (ADR-0039), so the JSON object and the
    // TSV table cannot drift.
    let result = ScheduleResult::new(schedule, currency);
    if json {
        println!("{}", result.to_json());
    } else {
        result.print();
    }
    Ok(())
}

fn run(cli: Cli) -> Result<()> {
    let json = cli.json;
    let currency = cli.currency;
    match cli.command {
        Command::Amortize {
            rate: r,
            principal,
            periods,
            payment,
        } => return run_amortize(json, currency, r, principal, periods, payment),
        command => {
            let output = run_scalar(command, currency)?;
            if json {
                println!("{}", output.to_json());
            } else {
                output.print();
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        // Print only the outermost message, not anyhow's full `{:#}` chain: our
        // context strings already restate the library error in user terms, so the
        // chain just doubled the text (ADR-0028 / #30). An uncontexted `TvmError`
        // still surfaces its own Display here. Where the library error is now more
        // specific than any static context could be — the degenerate cases split
        // out of `Undefined` (ADR-0052) — the site builds one message that names
        // the operation *and* interpolates the error, so nothing is lost and
        // nothing is doubled.
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
