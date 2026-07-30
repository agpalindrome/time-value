//! The MCP server: time-value-of-money operations exposed as tools.
//!
//! The server is stateless — every tool is a pure function of its arguments —
//! so it holds only its tool router. Tools build their result as structured
//! JSON; domain failures ([`TvmError`]) become MCP `invalid_params` errors.

// rmcp's `#[tool]` methods must take `&self`; the server is stateless, so they
// do not use it.
#![allow(clippy::unused_self)]

use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router, ErrorData, Json, ServerHandler,
};
use time_value::{
    amortization, annuity, continuous, single_sum, Annual, Cashflows, ContinuousRate, Currency,
    DatedCashflow, DatedCashflows, FutureValue, FxRate, Growth, Money, Monthly, Payment, Period,
    PresentValue, Principal, Rate, TvmError,
};
use time_value_daycount::{act365_year_fraction, iso_to_day};

use crate::params::{
    AmortizeInput, AnnuityPaymentInput, AnnuityPeriodsInput, AnnuityRateInput, AnnuityValueInput,
    ContinuousRateInput, ContinuousValueInput, ConvertInput, DatedFlow, DatedIrrInput,
    DatedSeriesInput, FutureValueInput, GrowingAnnuityInput, GrowingPaymentInput,
    GrowingPeriodsInput, GrowingPerpetuityInput, GrowingRateInput, IrrInput, MirrInput,
    Periodicity, PerpetuityInput, PresentValueInput, RateConvertInput, RateEffectiveAnnualInput,
    RateFromNominalInput, SeriesInput, SingleSumPeriodsInput, SingleSumRateInput,
};
use crate::results::{MoneyResult, ScalarResult, ScheduleResult};

/// Run `$body` with the type alias `$ty` bound to the core periodicity marker for
/// the [`Periodicity`] value `$value`. Used by the `rate_*` conversion tools,
/// where periodicity is intrinsic (ADR-0028/0029). The match is exhaustive: an
/// unknown periodicity is already refused by deserialization at the boundary
/// (ADR-0039), so there is no error arm.
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

/// Told to clients on initialise.
const INSTRUCTIONS: &str = "\
Time-value-of-money calculations, grouped by family. Series: `npv`, `nfv` (net \
present / future value at a per-period rate), `irr`, `mirr` (modified IRR, with \
finance and reinvestment rates), and `xnpv`/`xirr` (cashflows on irregular ISO \
dates, at an annual rate). Single sum: `single_sum_present_value`, \
`single_sum_future_value`, and the solves `single_sum_periods` (NPER) / \
`single_sum_rate` (RATE). Annuity: `annuity_present_value`, \
`annuity_future_value`, the solves `annuity_payment` / `annuity_periods` / \
`annuity_rate` (each from a present or future value — `annuity_payment` from a \
future value is the sinking-fund payment), `annuity_perpetuity`, \
`annuity_growing_perpetuity`, the finite growing forms \
`annuity_growing_present_value` / `annuity_growing_future_value` (a payment that \
grows each period; unlike a perpetuity, the rate need not exceed the growth), and \
the annuity-due forms `annuity_due_present_value`, `annuity_due_future_value`, \
`annuity_due_payment`, `annuity_due_periods`, `annuity_due_rate`, \
`annuity_due_perpetuity` — the due solves take the same present/future anchor the \
ordinary ones do. The growing forms carry a \
`growing` prefix throughout: `annuity_growing_present_value`, \
`annuity_growing_future_value`, `annuity_growing_due_present_value`, \
`annuity_growing_due_future_value`, `annuity_growing_due_perpetuity`, and the \
inverses `annuity_growing_payment` / `annuity_growing_periods` / \
`annuity_growing_rate`, which are present-anchored only (a growing annuity's \
future value has no closed-form inverse in the term). \
Rate conversions: `rate_effective_annual` (EAR), `rate_convert` (between \
periodicities), `rate_from_nominal` and `rate_nominal` (nominal/APR) — each takes \
a periodicity (daily, weekly, monthly, quarterly, semi-annual, annual). \
Continuous compounding: `continuous_future_value` / `continuous_present_value` \
grow/discount an amount at a force of interest δ (`rate`) over a real-number \
`years` span (not a period count), and `continuous_from_effective` / \
`continuous_effective` bridge δ ↔ an effective annual rate. \
`amortize` returns a schedule (an array of period/payment/interest/principal/\
balance rows) from a term or a level payment. `convert` restates an amount in \
another currency at a caller-supplied exchange rate (`amount`, `from`, `to`, \
`rate` — units of `to` per unit of `from`). Rates are per period (annual for \
`xnpv`/`xirr`); cashflows are signed (outflow negative). Every amount-bearing \
tool accepts an optional `currency` (an ISO 4217 code, e.g. `USD`); it \
denominates the amounts and is echoed on monetary results (omit for \
currency-agnostic). Source: https://github.com/ojhermann-org/time-value";

/// The MCP server. Stateless: the operations are pure functions of their inputs.
#[derive(Clone)]
pub(crate) struct TimeValueServer {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl TimeValueServer {
    /// Build the server.
    pub(crate) fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        name = "npv",
        description = "Net present value of a cashflow series discounted at a per-period rate: sum of CF_t / (1+r)^t."
    )]
    fn npv(
        &self,
        Parameters(input): Parameters<SeriesInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let flows = cashflows(&input.cashflows, currency)?;
        let series = Cashflows::<Monthly>::new(&flows);
        let money = series.net_present_value(rate(input.rate)?).map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "nfv",
        description = "Net future value of a cashflow series compounded to its final period at a per-period rate."
    )]
    fn nfv(
        &self,
        Parameters(input): Parameters<SeriesInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let flows = cashflows(&input.cashflows, currency)?;
        let series = Cashflows::<Monthly>::new(&flows);
        let money = series.net_future_value(rate(input.rate)?).map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "irr",
        description = "Internal rate of return (per period) of a cashflow series: the rate at which its net present value is zero."
    )]
    fn irr(
        &self,
        Parameters(input): Parameters<IrrInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let flows = cashflows(&input.cashflows, currency)?;
        let series = Cashflows::<Monthly>::new(&flows);
        let irr = series
            .internal_rate_of_return_from(input.guess)
            .map_err(tvm)?;
        Ok(Json(ScalarResult::new(irr.value())))
    }

    #[tool(
        name = "mirr",
        description = "Modified internal rate of return (per period): discounts outflows at a finance rate and compounds inflows at a reinvestment rate, then equates the two over the series' life."
    )]
    fn mirr(
        &self,
        Parameters(input): Parameters<MirrInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let flows = cashflows(&input.cashflows, currency)?;
        let series = Cashflows::<Monthly>::new(&flows);
        let mirr = series
            .modified_internal_rate_of_return(rate(input.finance)?, rate(input.reinvest)?)
            .map_err(tvm)?;
        Ok(Json(ScalarResult::new(mirr.value())))
    }

    #[tool(
        name = "xnpv",
        description = "Net present value of cashflows on irregular dates (XNPV), discounted at an annual rate by the year-fraction (ACT/365) from the first date."
    )]
    fn xnpv(
        &self,
        Parameters(input): Parameters<DatedSeriesInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let flows = dated_flows(&input.flows, currency)?;
        let series = DatedCashflows::new(&flows);
        let money = series
            .net_present_value(annual_rate(input.rate)?)
            .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "xirr",
        description = "Internal rate of return of cashflows on irregular dates (XIRR), as an annual rate: the rate at which their XNPV (ACT/365 from the first date) is zero."
    )]
    fn xirr(
        &self,
        Parameters(input): Parameters<DatedIrrInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let flows = dated_flows(&input.flows, currency)?;
        let series = DatedCashflows::new(&flows);
        let irr = series
            .internal_rate_of_return_from(input.guess)
            .map_err(tvm)?;
        Ok(Json(ScalarResult::new(irr.value())))
    }

    #[tool(
        name = "single_sum_present_value",
        description = "Present value of a single future amount, discounted at a per-period rate over a (possibly fractional) number of periods."
    )]
    fn single_sum_present_value(
        &self,
        Parameters(input): Parameters<PresentValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = single_sum::present_value(
            rate(input.rate)?,
            period(input.periods)?,
            money(input.future, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "single_sum_future_value",
        description = "Future value of a single present amount, compounded at a per-period rate over a (possibly fractional) number of periods."
    )]
    fn single_sum_future_value(
        &self,
        Parameters(input): Parameters<FutureValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = single_sum::future_value(
            rate(input.rate)?,
            period(input.periods)?,
            money(input.present, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "single_sum_periods",
        description = "Solve for the number of periods that grows a present amount to a future amount at a per-period rate (NPER)."
    )]
    fn single_sum_periods(
        &self,
        Parameters(input): Parameters<SingleSumPeriodsInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let periods = single_sum::periods(
            rate(input.rate)?,
            PresentValue(money(input.present, currency)?),
            FutureValue(money(input.future, currency)?),
        )
        .map_err(tvm)?;
        Ok(Json(ScalarResult::new(periods.value())))
    }

    #[tool(
        name = "single_sum_rate",
        description = "Solve for the per-period rate that grows a present amount to a future amount over a number of periods (RATE)."
    )]
    fn single_sum_rate(
        &self,
        Parameters(input): Parameters<SingleSumRateInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let solved = single_sum::rate::<Monthly>(
            period(input.periods)?,
            PresentValue(money(input.present, currency)?),
            FutureValue(money(input.future, currency)?),
        )
        .map_err(tvm)?;
        Ok(Json(ScalarResult::new(solved.value())))
    }

    #[tool(
        name = "annuity_present_value",
        description = "Present value of an ordinary annuity that pays a fixed amount at the end of each period."
    )]
    fn annuity_present_value(
        &self,
        Parameters(input): Parameters<AnnuityValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::present_value(
            rate(input.rate)?,
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_future_value",
        description = "Future value of an ordinary annuity that pays a fixed amount at the end of each period."
    )]
    fn annuity_future_value(
        &self,
        Parameters(input): Parameters<AnnuityValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::future_value(
            rate(input.rate)?,
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_payment",
        description = "The level end-of-period payment for a number of periods at a per-period rate, from a present value (amortising a balance) or a future value (the sinking-fund payment, i.e. how much to set aside each period to reach a target). Provide exactly one."
    )]
    fn annuity_payment(
        &self,
        Parameters(input): Parameters<AnnuityPaymentInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let payment = level_payment(
            &input,
            currency,
            (annuity::payment, annuity::payment_from_future),
        )?;
        Ok(Json(payment.into()))
    }

    #[tool(
        name = "annuity_periods",
        description = "Solve for the number of level end-of-period payments, from a present value or a future value (provide exactly one)."
    )]
    fn annuity_periods(
        &self,
        Parameters(input): Parameters<AnnuityPeriodsInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let periods = solved_periods(&input, (annuity::periods, annuity::periods_from_future))?;
        Ok(Json(ScalarResult::new(periods.value())))
    }

    #[tool(
        name = "annuity_rate",
        description = "Solve for the per-period rate of an annuity, from a present value or a future value (provide exactly one)."
    )]
    fn annuity_rate(
        &self,
        Parameters(input): Parameters<AnnuityRateInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let solved = solved_rate(
            &input,
            (
                annuity::rate::<Monthly>,
                annuity::rate_from_future::<Monthly>,
            ),
        )?;
        Ok(Json(ScalarResult::new(solved.value())))
    }

    #[tool(
        name = "annuity_perpetuity",
        description = "Present value of a level perpetuity — a fixed end-of-period payment forever — at a per-period rate (which must exceed 0)."
    )]
    fn annuity_perpetuity(
        &self,
        Parameters(input): Parameters<PerpetuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money =
            annuity::perpetuity(rate(input.rate)?, money(input.payment, currency)?).map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_perpetuity",
        description = "Present value of a perpetuity whose payment grows each period, at a per-period rate that must exceed the growth rate."
    )]
    fn annuity_growing_perpetuity(
        &self,
        Parameters(input): Parameters<GrowingPerpetuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::growing_perpetuity(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_present_value",
        description = "Present value of a growing annuity — an end-of-period payment that grows each period, over a finite number of periods. Unlike a growing perpetuity, the rate need not exceed the growth rate."
    )]
    fn annuity_growing_present_value(
        &self,
        Parameters(input): Parameters<GrowingAnnuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::growing_present_value(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_future_value",
        description = "Future value of a growing annuity — an end-of-period payment that grows each period, over a finite number of periods."
    )]
    fn annuity_growing_future_value(
        &self,
        Parameters(input): Parameters<GrowingAnnuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::growing_future_value(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_due_present_value",
        description = "Present value of an annuity-due that pays a fixed amount at the start of each period."
    )]
    fn annuity_due_present_value(
        &self,
        Parameters(input): Parameters<AnnuityValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::due::present_value(
            rate(input.rate)?,
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_due_future_value",
        description = "Future value of an annuity-due that pays a fixed amount at the start of each period."
    )]
    fn annuity_due_future_value(
        &self,
        Parameters(input): Parameters<AnnuityValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::due::future_value(
            rate(input.rate)?,
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_due_payment",
        description = "The level start-of-period payment for a number of periods at a per-period rate, from a present value (amortising a balance) or a future value (the sinking-fund payment). Provide exactly one."
    )]
    fn annuity_due_payment(
        &self,
        Parameters(input): Parameters<AnnuityPaymentInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let payment = level_payment(
            &input,
            currency,
            (annuity::due::payment, annuity::due::payment_from_future),
        )?;
        Ok(Json(payment.into()))
    }

    #[tool(
        name = "annuity_due_periods",
        description = "Solve for the number of level START-of-period payments, from a present value or a future value (provide exactly one)."
    )]
    fn annuity_due_periods(
        &self,
        Parameters(input): Parameters<AnnuityPeriodsInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let periods = solved_periods(
            &input,
            (annuity::due::periods, annuity::due::periods_from_future),
        )?;
        Ok(Json(ScalarResult::new(periods.value())))
    }

    #[tool(
        name = "annuity_due_rate",
        description = "Solve for the per-period rate of an annuity-due (START-of-period payments), from a present value or a future value (provide exactly one). From a present value the term must exceed one period: a single start-of-period payment is not discounted at all, so every rate satisfies it and none is the answer."
    )]
    fn annuity_due_rate(
        &self,
        Parameters(input): Parameters<AnnuityRateInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let solved = solved_rate(
            &input,
            (
                annuity::due::rate::<Monthly>,
                annuity::due::rate_from_future::<Monthly>,
            ),
        )?;
        Ok(Json(ScalarResult::new(solved.value())))
    }

    #[tool(
        name = "annuity_growing_payment",
        description = "Solve for the FIRST payment of a growing annuity that amortises a present value, each later payment being (1 + growth) times the one before. There is no future-value form: a growing annuity's future value has no closed-form inverse."
    )]
    fn annuity_growing_payment(
        &self,
        Parameters(input): Parameters<GrowingPaymentInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::growing_payment(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            period(input.periods)?,
            money(input.present, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_periods",
        description = "Solve for the number of growing payments that amortise a present value. When the rate exceeds the growth the present value is capped by the growing perpetuity (payment / (rate - growth)); a target at or above that cap is reached by no finite number of payments."
    )]
    fn annuity_growing_periods(
        &self,
        Parameters(input): Parameters<GrowingPeriodsInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let periods = annuity::growing_periods(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            Payment(money(input.payment, currency)?),
            PresentValue(money(input.present, currency)?),
        )
        .map_err(tvm)?;
        Ok(Json(ScalarResult::new(periods.value())))
    }

    #[tool(
        name = "annuity_growing_rate",
        description = "Solve for the per-period rate at which a growing payment stream amortises a present value. Only the growth rate is supplied; the discount rate is what is solved for."
    )]
    fn annuity_growing_rate(
        &self,
        Parameters(input): Parameters<GrowingRateInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let solved = annuity::growing_rate(
            Growth(rate(input.growth)?),
            period(input.periods)?,
            Payment(money(input.payment, currency)?),
            PresentValue(money(input.present, currency)?),
        )
        .map_err(tvm)?;
        Ok(Json(ScalarResult::new(solved.value())))
    }

    #[tool(
        name = "annuity_due_perpetuity",
        description = "Present value of a level perpetuity-due — a fixed payment at the START of every period forever, so the first payment is not discounted — at a per-period rate (which must exceed 0). The ordinary perpetuity scaled by (1 + rate)."
    )]
    fn annuity_due_perpetuity(
        &self,
        Parameters(input): Parameters<PerpetuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::due::perpetuity(rate(input.rate)?, money(input.payment, currency)?)
            .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_due_present_value",
        description = "Present value of a growing annuity-due — a start-of-period payment that grows each period, over a finite number of periods."
    )]
    fn annuity_growing_due_present_value(
        &self,
        Parameters(input): Parameters<GrowingAnnuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::due::growing_present_value(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_due_future_value",
        description = "Future value of a growing annuity-due — a start-of-period payment that grows each period, over a finite number of periods."
    )]
    fn annuity_growing_due_future_value(
        &self,
        Parameters(input): Parameters<GrowingAnnuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::due::growing_future_value(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            period(input.periods)?,
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "annuity_growing_due_perpetuity",
        description = "Present value of a growing perpetuity-due — a payment at the START of every period forever, growing each period, so the first payment is not discounted — at a per-period rate that must exceed the growth rate."
    )]
    fn annuity_growing_due_perpetuity(
        &self,
        Parameters(input): Parameters<GrowingPerpetuityInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = annuity::due::growing_perpetuity(
            rate(input.rate)?,
            Growth(rate(input.growth)?),
            money(input.payment, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "rate_effective_annual",
        description = "The effective annual rate (EAR) equivalent to a per-period rate at a given periodicity."
    )]
    fn rate_effective_annual(
        &self,
        Parameters(input): Parameters<RateEffectiveAnnualInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let value = dispatch_periodicity!(input.periodicity, P => {
            Rate::<P>::new(input.rate)
                .map_err(tvm)?
                .effective_annual()
                .map_err(tvm)?
                .value()
        });
        Ok(Json(ScalarResult::new(value)))
    }

    #[tool(
        name = "rate_convert",
        description = "Convert a per-period rate from one periodicity to another, preserving the effective annual rate."
    )]
    fn rate_convert(
        &self,
        Parameters(input): Parameters<RateConvertInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let value = dispatch_periodicity!(input.from, P => {
            let source = Rate::<P>::new(input.rate).map_err(tvm)?;
            dispatch_periodicity!(input.to, Q => {
                source.convert::<Q>().map_err(tvm)?.value()
            })
        });
        Ok(Json(ScalarResult::new(value)))
    }

    #[tool(
        name = "rate_from_nominal",
        description = "The per-period rate implied by a nominal annual rate (APR) compounded at a given periodicity."
    )]
    fn rate_from_nominal(
        &self,
        Parameters(input): Parameters<RateFromNominalInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let value = dispatch_periodicity!(input.periodicity, P => {
            Rate::<P>::from_nominal_annual(input.nominal)
                .map_err(tvm)?
                .value()
        });
        Ok(Json(ScalarResult::new(value)))
    }

    #[tool(
        name = "rate_nominal",
        description = "The nominal annual rate (APR) quoted from a per-period rate at a given periodicity."
    )]
    fn rate_nominal(
        &self,
        Parameters(input): Parameters<RateEffectiveAnnualInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let value = dispatch_periodicity!(input.periodicity, P => {
            Rate::<P>::new(input.rate)
                .map_err(tvm)?
                .nominal_annual()
                .map_err(tvm)?
        });
        Ok(Json(ScalarResult::new(value)))
    }

    #[tool(
        name = "amortize",
        description = "An amortization schedule: one row (period, payment, interest, principal, balance) per period until the balance is retired. Provide exactly one of `periods` (a term) or `payment` (a level payment)."
    )]
    fn amortize(
        &self,
        Parameters(input): Parameters<AmortizeInput>,
    ) -> Result<Json<ScheduleResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let r = rate(input.rate)?;
        let principal = money(input.principal, currency)?;
        let schedule = match (input.periods, input.payment) {
            (Some(n), None) => {
                amortization::Schedule::<Monthly>::for_term(r, period(n)?, principal)
            }
            (None, Some(p)) => amortization::Schedule::<Monthly>::with_payment(
                r,
                Payment(money(p, currency)?),
                Principal(principal),
            ),
            (None, None) => {
                return Err(ErrorData::invalid_params(
                    "provide either `periods` or `payment`".to_string(),
                    None,
                ))
            }
            (Some(_), Some(_)) => {
                return Err(ErrorData::invalid_params(
                    "`periods` and `payment` are mutually exclusive".to_string(),
                    None,
                ))
            }
        }
        .map_err(tvm)?;

        Ok(Json(ScheduleResult::new(schedule, currency)))
    }

    #[tool(
        name = "convert",
        description = "Convert an amount into another currency at a caller-supplied exchange rate (foreign exchange): the amount is denominated in `from`, the result in `to`. `rate` is units of `to` per unit of `from` and must be finite and positive."
    )]
    fn convert(
        &self,
        Parameters(input): Parameters<ConvertInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        // `from`/`to` are required `Currency` values (resolved at deserialize),
        // unlike the optional `currency` field; `XXX` stays valid (a caller may
        // convert to/from the agnostic unit).
        let from = input.from;
        let fx = FxRate::new(from, input.to, input.rate).map_err(tvm)?;
        let converted = money(input.amount, from)?.convert(fx).map_err(tvm)?;
        Ok(Json(converted.into()))
    }

    #[tool(
        name = "continuous_future_value",
        description = "Future value of a present amount grown continuously at a force of interest over a span of years: FV = PV·e^(δ·years). `years` is a continuous duration (may be fractional or negative), not a period count."
    )]
    fn continuous_future_value(
        &self,
        Parameters(input): Parameters<ContinuousValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = continuous::future_value(
            continuous_rate(input.rate)?,
            input.years,
            money(input.amount, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "continuous_present_value",
        description = "Present value of a future amount discounted continuously at a force of interest over a span of years: PV = FV·e^(−δ·years) — the inverse of continuous_future_value."
    )]
    fn continuous_present_value(
        &self,
        Parameters(input): Parameters<ContinuousValueInput>,
    ) -> Result<Json<MoneyResult>, ErrorData> {
        let currency = resolve_currency(input.currency);
        let money = continuous::present_value(
            continuous_rate(input.rate)?,
            input.years,
            money(input.amount, currency)?,
        )
        .map_err(tvm)?;
        Ok(Json(money.into()))
    }

    #[tool(
        name = "continuous_from_effective",
        description = "The force of interest δ equivalent to an effective annual rate: δ = ln(1 + r). The bridge from the discrete effective-rate machinery to continuous compounding."
    )]
    fn continuous_from_effective(
        &self,
        Parameters(input): Parameters<ContinuousRateInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let force = ContinuousRate::from_effective_annual(annual_rate(input.rate)?);
        Ok(Json(ScalarResult::new(force.value())))
    }

    #[tool(
        name = "continuous_effective",
        description = "The effective annual rate equivalent to a force of interest: r = e^δ − 1. The inverse bridge, letting a continuous rate be compared with discrete per-period rates."
    )]
    fn continuous_effective(
        &self,
        Parameters(input): Parameters<ContinuousRateInput>,
    ) -> Result<Json<ScalarResult>, ErrorData> {
        let r_eff = continuous_rate(input.rate)?
            .effective_annual()
            .map_err(tvm)?;
        Ok(Json(ScalarResult::new(r_eff.value())))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TimeValueServer {
    fn get_info(&self) -> ServerInfo {
        let mut info = ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(INSTRUCTIONS);
        // `ServerInfo::new` fills `server_info` from the rmcp crate's build env;
        // identify ourselves with this crate's own name/version instead. The
        // package name (`time-value-mcp`) is the product/binary name.
        info.server_info.name = env!("CARGO_PKG_NAME").to_string();
        info.server_info.version = env!("CARGO_PKG_VERSION").to_string();
        info
    }
}

// The periodicity tag does not affect any result (ADR-0010); the server fixes it
// to one marker to satisfy the type parameter.
fn rate(value: f64) -> Result<Rate<Monthly>, ErrorData> {
    Rate::new(value).map_err(tvm)
}

/// The dated `xnpv`/`xirr` discount is intrinsically annual (ADR-0029).
fn annual_rate(value: f64) -> Result<Rate<Annual>, ErrorData> {
    Rate::new(value).map_err(tvm)
}

/// A force of interest δ for the `continuous_*` tools. Every finite force is valid
/// — no `> −100%` floor as for a per-period [`Rate`] (ADR-0036).
fn continuous_rate(value: f64) -> Result<ContinuousRate, ErrorData> {
    ContinuousRate::new(value).map_err(tvm)
}

fn period(value: f64) -> Result<Period<Monthly>, ErrorData> {
    Period::new(value).map_err(tvm)
}

/// An omitted `currency` field (`None`) is [`Currency::Xxx`] (currency-agnostic),
/// preserving the pre-currency behaviour. A *present* code was already resolved to
/// a [`Currency`] (or rejected with the friendly "unknown ISO 4217 code" error) by
/// the core's `serde` `Deserialize` at the boundary (ADR-0044), so this is now
/// infallible.
fn resolve_currency(code: Option<Currency>) -> Currency {
    code.unwrap_or(Currency::Xxx)
}

fn money(value: f64, currency: Currency) -> Result<Money, ErrorData> {
    Money::new(value, currency).map_err(tvm)
}

fn cashflows(values: &[f64], currency: Currency) -> Result<Vec<Money>, ErrorData> {
    values.iter().copied().map(|v| money(v, currency)).collect()
}

/// The value a solve-for operation is anchored to — exactly one of a present or a
/// future amount. `present` and `future` are mutually exclusive and one is required.
enum Anchor {
    Present(f64),
    Future(f64),
}

/// A core solve for the level payment: from a present value (amortisation) or from
/// a future one (a sinking fund). Both signatures are identical, which is what lets
/// one helper serve the `annuity_payment` and `annuity_due_payment` tools alike.
type PaymentSolve = fn(Rate<Monthly>, Period<Monthly>, Money) -> Result<Money, TvmError>;

/// Solve the level payment from whichever value the caller anchored to — the same
/// mutually-exclusive `present` / `future` pair `annuity_periods` and `annuity_rate`
/// already take (ADR-0028 §2, ADR-0062).
fn level_payment(
    input: &AnnuityPaymentInput,
    currency: Currency,
    (from_present, from_future): (PaymentSolve, PaymentSolve),
) -> Result<Money, ErrorData> {
    let (rate, periods) = (rate(input.rate)?, period(input.periods)?);
    match anchor(input.present, input.future)? {
        Anchor::Present(p) => from_present(rate, periods, money(p, currency)?),
        Anchor::Future(f) => from_future(rate, periods, money(f, currency)?),
    }
    .map_err(tvm)
}

type PeriodsFromPresent =
    fn(Rate<Monthly>, Payment, PresentValue) -> Result<Period<Monthly>, TvmError>;
type PeriodsFromFuture =
    fn(Rate<Monthly>, Payment, FutureValue) -> Result<Period<Monthly>, TvmError>;
type RateFromPresent =
    fn(Period<Monthly>, Payment, PresentValue) -> Result<Rate<Monthly>, TvmError>;
type RateFromFuture = fn(Period<Monthly>, Payment, FutureValue) -> Result<Rate<Monthly>, TvmError>;

/// Solve for the number of payments from whichever value the caller anchored to,
/// shared by `annuity_periods` and `annuity_due_periods` (ADR-0063).
///
/// The two core functions differ in the *role* their amount carries
/// ([`PresentValue`] against [`FutureValue`]), so unlike [`level_payment`]'s pair they
/// cannot share a single `fn` type; taking them as a pair of function pointers serves
/// the same purpose, and keeps the ordinary and the due tool from drifting apart.
fn solved_periods(
    input: &AnnuityPeriodsInput,
    (from_present, from_future): (PeriodsFromPresent, PeriodsFromFuture),
) -> Result<Period<Monthly>, ErrorData> {
    let currency = resolve_currency(input.currency);
    let (rate, payment) = (rate(input.rate)?, Payment(money(input.payment, currency)?));
    match anchor(input.present, input.future)? {
        Anchor::Present(p) => from_present(rate, payment, PresentValue(money(p, currency)?)),
        Anchor::Future(f) => from_future(rate, payment, FutureValue(money(f, currency)?)),
    }
    .map_err(tvm)
}

/// Solve for the per-period rate from whichever value the caller anchored to, shared
/// by `annuity_rate` and `annuity_due_rate` (ADR-0063).
fn solved_rate(
    input: &AnnuityRateInput,
    (from_present, from_future): (RateFromPresent, RateFromFuture),
) -> Result<Rate<Monthly>, ErrorData> {
    let currency = resolve_currency(input.currency);
    let (periods, payment) = (
        period(input.periods)?,
        Payment(money(input.payment, currency)?),
    );
    match anchor(input.present, input.future)? {
        Anchor::Present(p) => from_present(periods, payment, PresentValue(money(p, currency)?)),
        Anchor::Future(f) => from_future(periods, payment, FutureValue(money(f, currency)?)),
    }
    .map_err(tvm)
}

fn anchor(present: Option<f64>, future: Option<f64>) -> Result<Anchor, ErrorData> {
    match (present, future) {
        (Some(p), None) => Ok(Anchor::Present(p)),
        (None, Some(f)) => Ok(Anchor::Future(f)),
        (None, None) => Err(ErrorData::invalid_params(
            "provide either `present` or `future`".to_string(),
            None,
        )),
        (Some(_), Some(_)) => Err(ErrorData::invalid_params(
            "`present` and `future` are mutually exclusive".to_string(),
            None,
        )),
    }
}

// ---- Dated flows (XNPV/XIRR): ISO dates → ACT/365 year-offsets ----
//
// The core takes year-offsets, not a date type (ADR-0029); the server accepts ISO
// `YYYY-MM-DD` dates and converts them with the shared `time-value-daycount`
// ACT/365 day-count (ADR-0030), so no date dependency reaches the binary.

/// Convert dated inputs to core [`DatedCashflow`]s, rebasing offsets to the first
/// flow (ACT/365). A malformed date becomes an MCP `invalid_params` error.
fn dated_flows(flows: &[DatedFlow], currency: Currency) -> Result<Vec<DatedCashflow>, ErrorData> {
    let mut out = Vec::with_capacity(flows.len());
    let mut reference: Option<i64> = None;
    for flow in flows {
        let day =
            iso_to_day(&flow.date).map_err(|e| ErrorData::invalid_params(e.to_string(), None))?;
        let reference = *reference.get_or_insert(day);
        let offset_years = act365_year_fraction(reference, day);
        out.push(DatedCashflow::new(offset_years, money(flow.amount, currency)?).map_err(tvm)?);
    }
    Ok(out)
}

/// Map a library error to an MCP `invalid_params` error — every `TvmError` here
/// is caused by the caller's arguments (an out-of-range rate, a non-convergent
/// IRR, a degenerate annuity). Takes the error by value so it can be used
/// directly as a `Result::map_err` argument.
#[allow(clippy::needless_pass_by_value)]
fn tvm(error: TvmError) -> ErrorData {
    ErrorData::invalid_params(error.to_string(), None)
}
