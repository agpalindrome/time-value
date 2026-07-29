//! Annuities: a fixed payment each period.
//!
//! The top-level functions model an **ordinary** annuity — payments fall at the
//! *end* of each period, the default in finance and the basis of loan
//! amortisation. The [`due`] submodule mirrors them for an **annuity-due**
//! (payments at the *start* of each period), whose factors are the ordinary
//! factors scaled by `(1 + r)`. [`perpetuity`] and [`growing_perpetuity`] give
//! the present value of a payment that continues forever.
//!
//! A payment that *grows* each period is priced by
//! [`growing_present_value`] / [`growing_future_value`] and their [`due`]
//! counterparts (ADR-0048). Unlike a growing perpetuity, a finite growing annuity
//! converges for every rate and growth pair, so `r ≤ g` is priced rather than
//! rejected; at `r = g` the factors take their limit, as they do at `r = 0`.
//!
//! Every function takes an interest `rate`; the dated ones also take a number of
//! `periods`. They are available with the `std` or `libm` feature, like the
//! single-sum operations (`docs/adr/0014-transcendental-single-sum-operations.md`),
//! and handle the `r → 0` limit, where the annuity factors collapse to `n`
//! (`docs/adr/0015-annuities.md`). The factors compound with `powf`, so on
//! extreme rate/period magnitudes a value can overflow to a non-finite
//! [`Money`] (see its docs). A perpetuity instead diverges when its rate does not
//! exceed its growth rate, which its constructors reject.
//!
//! Like every rate-and-period operation, the dated annuity functions require the
//! `rate` and `periods` to share a periodicity: a `Rate<Monthly>` applied over a
//! `Period<Annual>` is a compile error, not a silent unit mismatch (ADR-0005,
//! ADR-0045):
//!
//! ```compile_fail
//! use time_value::{annuity, Annual, Money, Monthly, Period, Rate};
//!
//! let _ = annuity::present_value(
//!     Rate::<Monthly>::new(0.01).unwrap(),
//!     Period::<Annual>::new(12.0).unwrap(), // annual periods, monthly rate — won't compile
//!     Money::agnostic(100.0).unwrap(),
//! );
//! ```

use crate::math::{ln, powf};
use crate::root::{abs, bracket_and_bisect, relative_tolerance};
use crate::{Money, Period, Periodicity, Rate, TvmError};

/// Rate magnitude below which the `r → 0` limit is used instead of the closed
/// form (which is `0/0` at exactly `r = 0` and ill-conditioned near it).
const RATE_NEAR_ZERO: f64 = 1e-9;

fn near_zero(x: f64) -> bool {
    x < RATE_NEAR_ZERO && x > -RATE_NEAR_ZERO
}

/// The present-value annuity factor `(1 - (1 + r)⁻ⁿ) / r`, taking the limit `n`
/// as `r → 0`.
fn present_value_factor(rate: f64, periods: f64) -> f64 {
    if near_zero(rate) {
        periods
    } else {
        (1.0 - powf(1.0 + rate, -periods)) / rate
    }
}

/// The future-value annuity factor `((1 + r)ⁿ - 1) / r`, taking the limit `n` as
/// `r → 0`.
fn future_value_factor(rate: f64, periods: f64) -> f64 {
    if near_zero(rate) {
        periods
    } else {
        (powf(1.0 + rate, periods) - 1.0) / rate
    }
}

/// The present-value factor for a **growing** annuity,
/// `(1 - ((1 + g)/(1 + r))ⁿ) / (r - g)`, taking the limit `n / (1 + r)` as
/// `g → r` (ADR-0048).
///
/// The limit is taken on the *spread* `r - g` rather than on `r` alone: the
/// closed form is `0/0` when the discount rate exactly matches the growth rate
/// (every payment then discounts to the same present amount), and
/// ill-conditioned near it — the same bargain [`present_value_factor`] makes at
/// `r ≈ 0`, which remains the `g = 0` case of this one.
fn growing_present_value_factor(rate: f64, growth: f64, periods: f64) -> f64 {
    let spread = rate - growth;
    if near_zero(spread) {
        periods / (1.0 + rate)
    } else {
        (1.0 - powf((1.0 + growth) / (1.0 + rate), periods)) / spread
    }
}

/// The future-value factor for a **growing** annuity,
/// `((1 + r)ⁿ - (1 + g)ⁿ) / (r - g)`, taking the limit `n · (1 + r)ⁿ⁻¹` as
/// `g → r` (ADR-0048).
///
/// This is [`growing_present_value_factor`] compounded forward by `(1 + r)ⁿ`,
/// and reduces to [`future_value_factor`] at `g = 0`.
fn growing_future_value_factor(rate: f64, growth: f64, periods: f64) -> f64 {
    let spread = rate - growth;
    if near_zero(spread) {
        periods * powf(1.0 + rate, periods - 1.0)
    } else {
        (powf(1.0 + rate, periods) - powf(1.0 + growth, periods)) / spread
    }
}

/// The present value of an ordinary annuity that pays `payment` at the end of
/// each of `periods` periods, discounted at `rate`.
///
/// `PV = PMT · (1 - (1 + r)⁻ⁿ) / r`, or `PV = PMT · n` when `r = 0`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// // 100 at the end of each month for a year, at 1% per month.
/// let pv = annuity::present_value(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((pv.value() - 1125.508).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the discounted sum overflows to a non-finite
/// value on extreme rate/period magnitudes (ADR-0021).
pub fn present_value<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    payment: Money,
) -> Result<Money, TvmError> {
    Money::from_operation(
        payment.value() * present_value_factor(rate.value(), periods.value()),
        payment.currency(),
    )
}

/// The future value of an ordinary annuity that pays `payment` at the end of
/// each of `periods` periods, compounded at `rate`.
///
/// `FV = PMT · ((1 + r)ⁿ - 1) / r`, or `FV = PMT · n` when `r = 0`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// let fv = annuity::future_value(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((fv.value() - 1268.250).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the compounded sum overflows to a non-finite
/// value on extreme rate/period magnitudes (ADR-0021).
pub fn future_value<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    payment: Money,
) -> Result<Money, TvmError> {
    Money::from_operation(
        payment.value() * future_value_factor(rate.value(), periods.value()),
        payment.currency(),
    )
}

/// The level payment that amortises a `present` value over `periods` periods at
/// `rate` — the inverse of [`present_value`].
///
/// `PMT = PV · r / (1 - (1 + r)⁻ⁿ)`, or `PMT = PV / n` when `r = 0`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// // Amortise a 1125.508 loan over a year at 1% per month -> ~100 per month.
/// let pmt = annuity::payment(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(1125.508)?,
/// )?;
/// assert!((pmt.value() - 100.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// Returns [`TvmError::Undefined`] if the amortisation is degenerate — in
/// particular when `periods` is zero, so there is nothing to amortise over and
/// the payment has no answer (the factor is `0`) — or [`TvmError::Overflow`] if
/// the division overflows on extreme magnitudes (ADR-0021, ADR-0031).
pub fn payment<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    present: Money,
) -> Result<Money, TvmError> {
    if periods.value() == 0.0 {
        // Nothing to amortise over: the annuity factor is 0, so the payment is
        // undefined rather than merely too large.
        return Err(TvmError::Undefined);
    }
    let factor = present_value_factor(rate.value(), periods.value());
    Money::from_operation(present.value() / factor, present.currency())
}

/// The present value of a **level perpetuity** — a `payment` at the end of every
/// period, forever — discounted at `rate`.
///
/// `PV = PMT / r`. The sum converges only when `r > 0`; a non-positive rate makes
/// the series diverge, so it is rejected rather than returning the finite-looking
/// `PMT / r`. This is the `g = 0` case of [`growing_perpetuity`].
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Rate};
///
/// // 100 at the end of every month, forever, discounted at 5% per month.
/// let pv = annuity::perpetuity(Rate::<Monthly>::new(0.05)?, Money::agnostic(100.0)?)?;
/// assert!((pv.value() - 2000.0).abs() < 1e-9);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// Returns [`TvmError::DivergentPerpetuity`] if `rate` is not strictly positive
/// (the present value diverges), or [`TvmError::Overflow`] if the division
/// overflows on extreme magnitudes (ADR-0021).
pub fn perpetuity<P: Periodicity>(rate: Rate<P>, payment: Money) -> Result<Money, TvmError> {
    growing_perpetuity(rate, Rate::from_valid(0.0), payment)
}

/// The present value of a **growing perpetuity** — a payment at the end of every
/// period, forever, growing at `growth` each period — discounted at `rate`.
///
/// `PV = PMT / (r - g)`, where `PMT` is the *first* payment (one period from now)
/// and `g` is the per-period growth rate. The sum converges only when `r > g`; if
/// `r <= g` the series diverges (`r = g` gives an infinity, `r < g` a finite but
/// meaningless value), so it is rejected. `rate` and `growth` share the
/// periodicity `P`, so mixing a monthly rate with an annual growth is a compile
/// error.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Rate};
///
/// // First payment 100 at month end, growing 2%/month, discounted at 5%/month.
/// let pv = annuity::growing_perpetuity(
///     Rate::<Monthly>::new(0.05)?,
///     Rate::<Monthly>::new(0.02)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((pv.value() - 3333.333).abs() < 1e-3); // 100 / (0.05 - 0.02)
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// Returns [`TvmError::DivergentPerpetuity`] if `rate <= growth` (the present
/// value diverges), or [`TvmError::Overflow`] if the division overflows on
/// extreme magnitudes (ADR-0021).
pub fn growing_perpetuity<P: Periodicity>(
    rate: Rate<P>,
    growth: Rate<P>,
    payment: Money,
) -> Result<Money, TvmError> {
    if rate.value() <= growth.value() {
        return Err(TvmError::DivergentPerpetuity);
    }
    Money::from_operation(
        payment.value() / (rate.value() - growth.value()),
        payment.currency(),
    )
}

/// The present value of a **growing annuity** — `payment` at the end of the first
/// period, growing at `growth` each period, for `periods` periods — discounted at
/// `rate` (ADR-0048).
///
/// `PV = PMT · (1 - ((1 + g)/(1 + r))ⁿ) / (r - g)`, or `PV = PMT · n / (1 + r)`
/// when `r = g`. `PMT` is the *first* payment, so the `k`-th is `PMT · (1 + g)^(k-1)`.
///
/// Unlike [`growing_perpetuity`], this converges for **every** rate and growth
/// pair — a finite sum of finite terms — so `r ≤ g` is priced, not rejected. At
/// `r = g` every payment discounts to the same amount and the present value is
/// simply `n` of them. It is the `g = 0` case that recovers [`present_value`], and
/// [`growing_perpetuity`] that is its `n → ∞` limit (when `r > g`).
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// // First payment 100 at month end, growing 2%/month for a year, at 5%/month.
/// let pv = annuity::growing_present_value(
///     Rate::<Monthly>::new(0.05)?,
///     Rate::<Monthly>::new(0.02)?,
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((pv.value() - 979.318).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the discounted sum overflows to a non-finite value
/// on extreme rate/growth/period magnitudes (ADR-0021).
pub fn growing_present_value<P: Periodicity>(
    rate: Rate<P>,
    growth: Rate<P>,
    periods: Period<P>,
    payment: Money,
) -> Result<Money, TvmError> {
    Money::from_operation(
        payment.value()
            * growing_present_value_factor(rate.value(), growth.value(), periods.value()),
        payment.currency(),
    )
}

/// The future value of a **growing annuity** — `payment` at the end of the first
/// period, growing at `growth` each period, for `periods` periods — compounded at
/// `rate` (ADR-0048).
///
/// `FV = PMT · ((1 + r)ⁿ - (1 + g)ⁿ) / (r - g)`, or `FV = PMT · n · (1 + r)ⁿ⁻¹`
/// when `r = g`. This is [`growing_present_value`] compounded forward by
/// `(1 + r)ⁿ`, and recovers [`future_value`] at `g = 0`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// let fv = annuity::growing_future_value(
///     Rate::<Monthly>::new(0.05)?,
///     Rate::<Monthly>::new(0.02)?,
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((fv.value() - 1758.715).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the compounded sum overflows to a non-finite value
/// on extreme rate/growth/period magnitudes (ADR-0021).
pub fn growing_future_value<P: Periodicity>(
    rate: Rate<P>,
    growth: Rate<P>,
    periods: Period<P>,
    payment: Money,
) -> Result<Money, TvmError> {
    Money::from_operation(
        payment.value()
            * growing_future_value_factor(rate.value(), growth.value(), periods.value()),
        payment.currency(),
    )
}

/// The number of level `payment`s that amortise a `present` value at `rate` —
/// [`present_value`] solved for `n` (the annuity NPER).
///
/// `n = −ln(1 − PV·r / PMT) / ln(1 + r)`, or `n = PV / PMT` when `r = 0`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Rate};
///
/// // How many 100/month payments retire a 1125.508 loan at 1%/month? A year.
/// let n = annuity::periods(
///     Rate::<Monthly>::new(0.01)?,
///     Money::agnostic(100.0)?,
///     Money::agnostic(1125.508)?,
/// )?;
/// assert!((n.value() - 12.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Undefined`] if the payment never retires the balance — when
/// `PMT ≤ PV·r`, the payment does not even cover the period's interest, so the
/// logarithm's argument is non-positive and `n` has no answer (likewise a zero
/// payment). [`NegativePeriods`] if the solved `n` is negative.
///
/// [`NegativePeriods`]: TvmError::NegativePeriods
pub fn periods<P: Periodicity>(
    rate: Rate<P>,
    payment: Money,
    present: Money,
) -> Result<Period<P>, TvmError> {
    let r = rate.value();
    let n = if near_zero(r) {
        if payment.value() == 0.0 {
            return Err(TvmError::Undefined);
        }
        present.value() / payment.value()
    } else {
        let arg = 1.0 - present.value() * r / payment.value();
        if arg <= 0.0 || arg.is_nan() {
            // PMT ≤ PV·r (or a zero payment): the logarithm's argument is
            // non-positive, so no finite number of payments retires the balance.
            return Err(TvmError::Undefined);
        }
        -ln(arg) / ln(1.0 + r)
    };
    Period::from_operation(n)
}

/// The number of level `payment`s that accumulate to a `future` value at `rate` —
/// [`future_value`] solved for `n` (the annuity NPER, future-value form).
///
/// `n = ln(1 + FV·r / PMT) / ln(1 + r)`, or `n = FV / PMT` when `r = 0`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Rate};
///
/// // How many 100/month contributions reach ~1268.25 at 1%/month? A year.
/// let n = annuity::periods_from_future(
///     Rate::<Monthly>::new(0.01)?,
///     Money::agnostic(100.0)?,
///     Money::agnostic(1268.250)?,
/// )?;
/// assert!((n.value() - 12.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Undefined`] if `1 + FV·r / PMT` is non-positive (no real
/// logarithm) or the payment is zero, or [`TvmError::NegativePeriods`] if the
/// solved `n` is negative.
pub fn periods_from_future<P: Periodicity>(
    rate: Rate<P>,
    payment: Money,
    future: Money,
) -> Result<Period<P>, TvmError> {
    let r = rate.value();
    let n = if near_zero(r) {
        if payment.value() == 0.0 {
            return Err(TvmError::Undefined);
        }
        future.value() / payment.value()
    } else {
        let arg = 1.0 + future.value() * r / payment.value();
        if arg <= 0.0 || arg.is_nan() {
            return Err(TvmError::Undefined);
        }
        ln(arg) / ln(1.0 + r)
    };
    Period::from_operation(n)
}

/// The per-period rate at which `periods` level `payment`s amortise a `present`
/// value — [`present_value`] solved for `r` (the annuity RATE).
///
/// There is no closed form, so this solves iteratively, reusing the robust
/// bracketing search behind the internal rate of return (ADR-0020): the rate is
/// the root of `PMT · a(r, n) − PV`, where `a` is the present-value annuity
/// factor. The scalar inputs carry no periodicity, so the caller names it:
/// `annuity::rate::<Monthly>(…)`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// // What monthly rate amortises 1125.508 with 12 payments of 100? About 1%.
/// let r = annuity::rate::<Monthly>(
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
///     Money::agnostic(1125.508)?,
/// )?;
/// assert!((r.value() - 0.01).abs() < 1e-4);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::SolveDidNotConverge`] if no rate prices the payment stream at
/// `present` (e.g. incompatible signs), or [`TvmError::RateOutOfRange`] /
/// [`TvmError::Overflow`] if the located root is outside the valid rate
/// domain or non-finite.
pub fn rate<P: Periodicity>(
    periods: Period<P>,
    payment: Money,
    present: Money,
) -> Result<Rate<P>, TvmError> {
    solve_rate(
        periods.value(),
        payment.value(),
        present.value(),
        present_value_factor,
    )
}

/// The per-period rate at which `periods` level `payment`s accumulate to a
/// `future` value — [`future_value`] solved for `r` (the annuity RATE,
/// future-value form).
///
/// Solves iteratively like [`rate`], but for the root of `PMT · s(r, n) − FV`,
/// where `s` is the future-value annuity factor. Names its periodicity the same
/// way: `annuity::rate_from_future::<Monthly>(…)`.
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// // What monthly rate accumulates 12 payments of 100 to ~1268.25? About 1%.
/// let r = annuity::rate_from_future::<Monthly>(
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
///     Money::agnostic(1268.250)?,
/// )?;
/// assert!((r.value() - 0.01).abs() < 1e-4);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// As [`rate`].
pub fn rate_from_future<P: Periodicity>(
    periods: Period<P>,
    payment: Money,
    future: Money,
) -> Result<Rate<P>, TvmError> {
    solve_rate(
        periods.value(),
        payment.value(),
        future.value(),
        future_value_factor,
    )
}

/// Solve `payment · factor(r, periods) = target` for the per-period rate `r`.
///
/// `factor` is [`present_value_factor`] or [`future_value_factor`]; both are
/// monotone in `r`, so the residual has a single root, located by the shared
/// bracketing bisection ([`root::bracket_and_bisect`](crate::root)). The
/// tolerance is relative to the target magnitude (floored at `1`), mirroring the
/// IRR convergence check (ADR-0021).
fn solve_rate<P: Periodicity>(
    periods: f64,
    payment: f64,
    target: f64,
    factor: impl Fn(f64, f64) -> f64,
) -> Result<Rate<P>, TvmError> {
    let tolerance = relative_tolerance(abs(target));
    match bracket_and_bisect(|r| payment * factor(r, periods) - target, tolerance) {
        Some(r) => Rate::from_operation(r),
        None => Err(TvmError::SolveDidNotConverge),
    }
}

/// Annuity-due variants: a fixed payment at the *start* of each period.
///
/// These mirror the ordinary (end-of-period) functions in the parent module —
/// same signatures, same `r → 0` and degenerate-`n` handling — but each factor is
/// scaled by `(1 + r)`, because every payment is brought forward one period.
/// `PV_due = PV · (1 + r)`, `FV_due = FV · (1 + r)`, and [`payment`](due::payment)
/// inverts `present_value` here just as the ordinary `payment` inverts the
/// ordinary `present_value` (`docs/adr/0015-annuities.md`).
pub mod due {
    use super::{
        future_value_factor, growing_future_value_factor, growing_present_value_factor,
        present_value_factor,
    };
    use crate::{Money, Period, Periodicity, Rate, TvmError};

    /// The present value of an annuity-due that pays `payment` at the *start* of
    /// each of `periods` periods, discounted at `rate`.
    ///
    /// `PV = PMT · (1 + r) · (1 - (1 + r)⁻ⁿ) / r`, or `PV = PMT · n` when `r = 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Period, Rate};
    ///
    /// // 100 at the start of each month for a year, at 1% per month.
    /// let pv = annuity::due::present_value(
    ///     Rate::<Monthly>::new(0.01)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(100.0)?,
    /// )?;
    /// assert!((pv.value() - 1136.763).abs() < 1e-2); // ordinary 1125.508 × 1.01
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::Overflow`] if the discounted sum overflows to a
    /// non-finite value on extreme rate/period magnitudes (ADR-0021).
    pub fn present_value<P: Periodicity>(
        rate: Rate<P>,
        periods: Period<P>,
        payment: Money,
    ) -> Result<Money, TvmError> {
        let factor = present_value_factor(rate.value(), periods.value()) * (1.0 + rate.value());
        Money::from_operation(payment.value() * factor, payment.currency())
    }

    /// The future value of an annuity-due that pays `payment` at the *start* of
    /// each of `periods` periods, compounded at `rate`.
    ///
    /// `FV = PMT · (1 + r) · ((1 + r)ⁿ - 1) / r`, or `FV = PMT · n` when `r = 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Period, Rate};
    ///
    /// let fv = annuity::due::future_value(
    ///     Rate::<Monthly>::new(0.01)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(100.0)?,
    /// )?;
    /// assert!((fv.value() - 1280.933).abs() < 1e-2); // ordinary 1268.250 × 1.01
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::Overflow`] if the compounded sum overflows to a
    /// non-finite value on extreme rate/period magnitudes (ADR-0021).
    pub fn future_value<P: Periodicity>(
        rate: Rate<P>,
        periods: Period<P>,
        payment: Money,
    ) -> Result<Money, TvmError> {
        let factor = future_value_factor(rate.value(), periods.value()) * (1.0 + rate.value());
        Money::from_operation(payment.value() * factor, payment.currency())
    }

    /// The level payment, made at the *start* of each period, that amortises a
    /// `present` value over `periods` periods at `rate` — the inverse of
    /// [`present_value`].
    ///
    /// `PMT = PV / [(1 + r) · (1 - (1 + r)⁻ⁿ) / r]`, or `PMT = PV / n` when
    /// `r = 0`.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Period, Rate};
    ///
    /// // Amortise a 1136.763 loan over a year at 1%/month with start-of-month
    /// // payments -> ~100 per month.
    /// let pmt = annuity::due::payment(
    ///     Rate::<Monthly>::new(0.01)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(1136.763)?,
    /// )?;
    /// assert!((pmt.value() - 100.0).abs() < 1e-2);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::Undefined`] if the amortisation is degenerate — in
    /// particular when `periods` is zero, so the factor is `0` and the payment has
    /// no answer — or [`TvmError::Overflow`] if the division overflows on extreme
    /// magnitudes (ADR-0021, ADR-0031).
    pub fn payment<P: Periodicity>(
        rate: Rate<P>,
        periods: Period<P>,
        present: Money,
    ) -> Result<Money, TvmError> {
        if periods.value() == 0.0 {
            return Err(TvmError::Undefined);
        }
        let factor = present_value_factor(rate.value(), periods.value()) * (1.0 + rate.value());
        Money::from_operation(present.value() / factor, present.currency())
    }

    /// The present value of a **growing annuity-due** — `payment` at the *start*
    /// of the first period, growing at `growth` each period, for `periods`
    /// periods — discounted at `rate` (ADR-0048).
    ///
    /// `PV = PMT · (1 + r) · (1 - ((1 + g)/(1 + r))ⁿ) / (r - g)`, or
    /// `PV = PMT · n` when `r = g`. As everywhere in this module, the due factor
    /// is the ordinary one scaled by `(1 + r)`; here that scaling cancels the
    /// `1 / (1 + r)` in the `r = g` limit, leaving `n` undiscounted payments.
    ///
    /// Like [`growing_present_value`](super::growing_present_value), this prices
    /// every rate and growth pair — `r ≤ g` is not rejected.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Period, Rate};
    ///
    /// // First payment 100 at month start, growing 2%/month for a year, at 5%.
    /// let pv = annuity::due::growing_present_value(
    ///     Rate::<Monthly>::new(0.05)?,
    ///     Rate::<Monthly>::new(0.02)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(100.0)?,
    /// )?;
    /// assert!((pv.value() - 1028.284).abs() < 1e-2); // ordinary 979.318 × 1.05
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::Overflow`] if the discounted sum overflows to a non-finite
    /// value on extreme rate/growth/period magnitudes (ADR-0021).
    pub fn growing_present_value<P: Periodicity>(
        rate: Rate<P>,
        growth: Rate<P>,
        periods: Period<P>,
        payment: Money,
    ) -> Result<Money, TvmError> {
        let factor = growing_present_value_factor(rate.value(), growth.value(), periods.value())
            * (1.0 + rate.value());
        Money::from_operation(payment.value() * factor, payment.currency())
    }

    /// The future value of a **growing annuity-due** — `payment` at the *start*
    /// of the first period, growing at `growth` each period, for `periods`
    /// periods — compounded at `rate` (ADR-0048).
    ///
    /// `FV = PMT · (1 + r) · ((1 + r)ⁿ - (1 + g)ⁿ) / (r - g)`, or
    /// `FV = PMT · n · (1 + r)ⁿ` when `r = g`.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Period, Rate};
    ///
    /// let fv = annuity::due::growing_future_value(
    ///     Rate::<Monthly>::new(0.05)?,
    ///     Rate::<Monthly>::new(0.02)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(100.0)?,
    /// )?;
    /// assert!((fv.value() - 1846.651).abs() < 1e-2); // ordinary 1758.715 × 1.05
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::Overflow`] if the compounded sum overflows to a non-finite
    /// value on extreme rate/growth/period magnitudes (ADR-0021).
    pub fn growing_future_value<P: Periodicity>(
        rate: Rate<P>,
        growth: Rate<P>,
        periods: Period<P>,
        payment: Money,
    ) -> Result<Money, TvmError> {
        let factor = growing_future_value_factor(rate.value(), growth.value(), periods.value())
            * (1.0 + rate.value());
        Money::from_operation(payment.value() * factor, payment.currency())
    }
}

#[cfg(test)]
mod tests {
    use crate::{annuity, Money, Monthly, Period, Rate, TvmError};

    /// `no_std`-safe approximate equality (no `f64::abs`).
    fn approx(a: f64, b: f64, tolerance: f64) -> bool {
        let d = a - b;
        d < tolerance && d > -tolerance
    }

    fn rate(r: f64) -> Rate<Monthly> {
        Rate::<Monthly>::new(r).unwrap()
    }

    #[test]
    fn present_value_matches_closed_form() {
        let pv = annuity::present_value(
            rate(0.01),
            Period::new(12.0).unwrap(),
            Money::agnostic(100.0).unwrap(),
        )
        .unwrap();
        assert!(approx(pv.value(), 1125.508, 1e-2));
    }

    #[test]
    fn payment_inverts_present_value() {
        let payment = Money::agnostic(100.0).unwrap();
        let periods = Period::new(24.0).unwrap();
        let pv = annuity::present_value(rate(0.015), periods, payment).unwrap();
        let recovered = annuity::payment(rate(0.015), periods, pv).unwrap();
        assert!(approx(recovered.value(), payment.value(), 1e-9));
    }

    #[test]
    fn future_value_is_present_value_compounded() {
        let periods = Period::new(12.0).unwrap();
        let pv =
            annuity::present_value(rate(0.01), periods, Money::agnostic(100.0).unwrap()).unwrap();
        let fv =
            annuity::future_value(rate(0.01), periods, Money::agnostic(100.0).unwrap()).unwrap();
        // FV = PV * (1 + r)^n; compound manually to avoid needing powf here.
        let mut growth = 1.0;
        for _ in 0..12 {
            growth *= 1.01;
        }
        assert!(approx(fv.value(), pv.value() * growth, 1e-6));
    }

    #[test]
    fn zero_rate_uses_the_limit() {
        let periods = Period::new(10.0).unwrap();
        let payment = Money::agnostic(50.0).unwrap();
        // At r = 0 both factors are n, so PV = FV = payment * n.
        assert!(approx(
            annuity::present_value(rate(0.0), periods, payment)
                .unwrap()
                .value(),
            500.0,
            1e-9,
        ));
        assert!(approx(
            annuity::future_value(rate(0.0), periods, payment)
                .unwrap()
                .value(),
            500.0,
            1e-9,
        ));
    }

    #[test]
    fn payment_over_zero_periods_is_degenerate() {
        let result = annuity::payment(rate(0.01), Period::ZERO, Money::agnostic(1000.0).unwrap());
        assert_eq!(result, Err(TvmError::Undefined));
    }

    #[test]
    fn due_present_value_is_ordinary_scaled_by_one_plus_r() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let ordinary = annuity::present_value(rate(0.01), periods, payment).unwrap();
        let due = annuity::due::present_value(rate(0.01), periods, payment).unwrap();
        assert!(approx(due.value(), ordinary.value() * 1.01, 1e-9));
    }

    #[test]
    fn due_future_value_is_ordinary_scaled_by_one_plus_r() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let ordinary = annuity::future_value(rate(0.01), periods, payment).unwrap();
        let due = annuity::due::future_value(rate(0.01), periods, payment).unwrap();
        assert!(approx(due.value(), ordinary.value() * 1.01, 1e-9));
    }

    #[test]
    fn due_payment_inverts_due_present_value() {
        let payment = Money::agnostic(100.0).unwrap();
        let periods = Period::new(24.0).unwrap();
        let pv = annuity::due::present_value(rate(0.015), periods, payment).unwrap();
        let recovered = annuity::due::payment(rate(0.015), periods, pv).unwrap();
        assert!(approx(recovered.value(), payment.value(), 1e-9));
    }

    #[test]
    fn due_zero_rate_matches_ordinary_limit() {
        // At r = 0 the (1 + r) scaling is 1, so due == ordinary == payment * n.
        let periods = Period::new(10.0).unwrap();
        let payment = Money::agnostic(50.0).unwrap();
        assert!(approx(
            annuity::due::present_value(rate(0.0), periods, payment)
                .unwrap()
                .value(),
            500.0,
            1e-9,
        ));
        assert!(approx(
            annuity::due::future_value(rate(0.0), periods, payment)
                .unwrap()
                .value(),
            500.0,
            1e-9,
        ));
    }

    #[test]
    fn due_payment_over_zero_periods_is_degenerate() {
        let result =
            annuity::due::payment(rate(0.01), Period::ZERO, Money::agnostic(1000.0).unwrap());
        assert_eq!(result, Err(TvmError::Undefined));
    }

    #[test]
    fn perpetuity_is_payment_over_rate() {
        let pv = annuity::perpetuity(rate(0.05), Money::agnostic(100.0).unwrap()).unwrap();
        assert!(approx(pv.value(), 2000.0, 1e-9));
    }

    #[test]
    fn perpetuity_is_the_zero_growth_growing_perpetuity() {
        let pv = annuity::perpetuity(rate(0.05), Money::agnostic(100.0).unwrap()).unwrap();
        let grown =
            annuity::growing_perpetuity(rate(0.05), rate(0.0), Money::agnostic(100.0).unwrap())
                .unwrap();
        assert!(approx(pv.value(), grown.value(), 1e-9));
    }

    #[test]
    fn growing_perpetuity_discounts_by_the_spread() {
        // 100 / (0.05 - 0.02) = 3333.333...
        let pv =
            annuity::growing_perpetuity(rate(0.05), rate(0.02), Money::agnostic(100.0).unwrap())
                .unwrap();
        assert!(approx(pv.value(), 3_333.333_333_333_333, 1e-6));
    }

    #[test]
    fn perpetuity_with_non_positive_rate_diverges() {
        let payment = Money::agnostic(100.0).unwrap();
        assert_eq!(
            annuity::perpetuity(rate(0.0), payment),
            Err(TvmError::DivergentPerpetuity),
        );
        assert_eq!(
            annuity::perpetuity(rate(-0.01), payment),
            Err(TvmError::DivergentPerpetuity),
        );
    }

    #[test]
    fn growing_perpetuity_diverges_when_rate_does_not_exceed_growth() {
        let payment = Money::agnostic(100.0).unwrap();
        // r = g: an infinity from division by zero.
        assert_eq!(
            annuity::growing_perpetuity(rate(0.03), rate(0.03), payment),
            Err(TvmError::DivergentPerpetuity),
        );
        // r < g: a finite but meaningless value, still rejected.
        assert_eq!(
            annuity::growing_perpetuity(rate(0.02), rate(0.05), payment),
            Err(TvmError::DivergentPerpetuity),
        );
    }

    #[test]
    fn periods_inverts_present_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let present = annuity::present_value(rate(0.01), periods, payment).unwrap();
        let recovered = annuity::periods(rate(0.01), payment, present).unwrap();
        assert!(approx(recovered.value(), periods.value(), 1e-6));
    }

    #[test]
    fn periods_from_future_inverts_future_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let future = annuity::future_value(rate(0.01), periods, payment).unwrap();
        let recovered = annuity::periods_from_future(rate(0.01), payment, future).unwrap();
        assert!(approx(recovered.value(), periods.value(), 1e-6));
    }

    #[test]
    fn periods_zero_rate_uses_the_limit() {
        // At r = 0, PV = PMT·n, so n = PV / PMT.
        let n = annuity::periods(
            rate(0.0),
            Money::agnostic(100.0).unwrap(),
            Money::agnostic(1200.0).unwrap(),
        )
        .unwrap();
        assert!(approx(n.value(), 12.0, 1e-9));
    }

    #[test]
    fn periods_when_payment_cannot_cover_interest_is_undefined() {
        // 5% on a 10000 balance is 500/period, but the payment is only 100, so the
        // balance never amortises: n is undefined.
        assert_eq!(
            annuity::periods(
                rate(0.05),
                Money::agnostic(100.0).unwrap(),
                Money::agnostic(10_000.0).unwrap(),
            ),
            Err(TvmError::Undefined)
        );
    }

    #[test]
    fn rate_inverts_present_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let present = annuity::present_value(rate(0.01), periods, payment).unwrap();
        let recovered = annuity::rate::<Monthly>(periods, payment, present).unwrap();
        assert!(approx(recovered.value(), 0.01, 1e-6));
    }

    #[test]
    fn rate_from_future_inverts_future_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let future = annuity::future_value(rate(0.01), periods, payment).unwrap();
        let recovered = annuity::rate_from_future::<Monthly>(periods, payment, future).unwrap();
        assert!(approx(recovered.value(), 0.01, 1e-6));
    }

    #[test]
    fn rate_recovers_a_negative_rate() {
        // A payment stream can price above PMT·n only at a negative rate.
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let present = annuity::present_value(rate(-0.02), periods, payment).unwrap();
        let recovered = annuity::rate::<Monthly>(periods, payment, present).unwrap();
        assert!(approx(recovered.value(), -0.02, 1e-6));
    }

    #[test]
    fn rate_without_a_solution_does_not_converge() {
        // A positive payment can never price to a negative present value, so no
        // rate solves it.
        assert_eq!(
            annuity::rate::<Monthly>(
                Period::new(12.0).unwrap(),
                Money::agnostic(100.0).unwrap(),
                Money::agnostic(-1000.0).unwrap(),
            ),
            Err(TvmError::SolveDidNotConverge)
        );
    }

    /// The growing annuity (ADR-0048). The closed forms are checked against a
    /// term-by-term sum of the payments themselves — an independent reference, so
    /// a mistranscribed factor cannot agree with it by construction.
    mod growing {
        use super::{approx, rate};
        use crate::{annuity, Currency, Money, Period, TvmError};

        /// `Σ PMT·(1+g)^(k−1) / (1+r)^k` for `k = 1..=n` — the ordinary growing
        /// annuity summed directly, one payment at a time.
        fn present_value_by_summation(r: f64, g: f64, n: u32, pmt: f64) -> f64 {
            let mut total = 0.0;
            let mut payment = pmt;
            let mut discount = 1.0 + r;
            for _ in 0..n {
                total += payment / discount;
                payment *= 1.0 + g;
                discount *= 1.0 + r;
            }
            total
        }

        /// `Σ PMT·(1+g)^(k−1) · (1+r)^(n−k)` for `k = 1..=n` — the same stream
        /// carried forward to period `n` instead of discounted back.
        fn future_value_by_summation(r: f64, g: f64, n: u32, pmt: f64) -> f64 {
            let mut total = 0.0;
            let mut payment = pmt;
            for k in 0..n {
                let mut compounded = payment;
                for _ in 0..(n - 1 - k) {
                    compounded *= 1.0 + r;
                }
                total += compounded;
                payment *= 1.0 + g;
            }
            total
        }

        #[test]
        fn present_value_matches_a_direct_summation() {
            let pv = annuity::growing_present_value(
                rate(0.05),
                rate(0.02),
                Period::new(12.0).unwrap(),
                Money::agnostic(100.0).unwrap(),
            )
            .unwrap();
            assert!(approx(
                pv.value(),
                present_value_by_summation(0.05, 0.02, 12, 100.0),
                1e-9,
            ));
        }

        #[test]
        fn future_value_matches_a_direct_summation() {
            let fv = annuity::growing_future_value(
                rate(0.05),
                rate(0.02),
                Period::new(12.0).unwrap(),
                Money::agnostic(100.0).unwrap(),
            )
            .unwrap();
            assert!(approx(
                fv.value(),
                future_value_by_summation(0.05, 0.02, 12, 100.0),
                1e-9,
            ));
        }

        /// At `r = g` the closed form is `0/0`, so the factors switch to their
        /// limits: every payment discounts to the same amount, giving `n·PMT/(1+r)`
        /// present and `n·PMT·(1+r)ⁿ⁻¹` future. Both are checked against the
        /// summation as well, so the limit is pinned to the thing it is a limit of.
        #[test]
        fn growth_equal_to_the_rate_uses_the_limit() {
            let periods = Period::new(10.0).unwrap();
            let payment = Money::agnostic(100.0).unwrap();

            let pv = annuity::growing_present_value(rate(0.05), rate(0.05), periods, payment)
                .unwrap()
                .value();
            assert!(approx(pv, 10.0 * 100.0 / 1.05, 1e-9));
            assert!(approx(
                pv,
                present_value_by_summation(0.05, 0.05, 10, 100.0),
                1e-9,
            ));

            let fv = annuity::growing_future_value(rate(0.05), rate(0.05), periods, payment)
                .unwrap()
                .value();
            assert!(approx(
                fv,
                future_value_by_summation(0.05, 0.05, 10, 100.0),
                1e-9,
            ));
        }

        /// Growth *above* the discount rate is priced, not rejected: a finite sum
        /// of finite terms always converges. This is the deliberate difference from
        /// [`annuity::growing_perpetuity`], which returns `DivergentPerpetuity`
        /// for the very same rate/growth pair (ADR-0048).
        #[test]
        fn growth_above_the_rate_is_priced_not_rejected() {
            let pv = annuity::growing_present_value(
                rate(0.02),
                rate(0.05),
                Period::new(12.0).unwrap(),
                Money::agnostic(100.0).unwrap(),
            )
            .unwrap();
            assert!(approx(
                pv.value(),
                present_value_by_summation(0.02, 0.05, 12, 100.0),
                1e-9,
            ));

            assert_eq!(
                annuity::growing_perpetuity(
                    rate(0.02),
                    rate(0.05),
                    Money::agnostic(100.0).unwrap()
                ),
                Err(TvmError::DivergentPerpetuity),
            );
        }

        #[test]
        fn due_variants_are_the_ordinary_ones_scaled_by_one_plus_r() {
            let periods = Period::new(12.0).unwrap();
            let payment = Money::agnostic(100.0).unwrap();

            let ordinary_present =
                annuity::growing_present_value(rate(0.05), rate(0.02), periods, payment)
                    .unwrap()
                    .value();
            let due_present =
                annuity::due::growing_present_value(rate(0.05), rate(0.02), periods, payment)
                    .unwrap()
                    .value();
            assert!(approx(due_present, ordinary_present * 1.05, 1e-9));

            let ordinary_future =
                annuity::growing_future_value(rate(0.05), rate(0.02), periods, payment)
                    .unwrap()
                    .value();
            let due_future =
                annuity::due::growing_future_value(rate(0.05), rate(0.02), periods, payment)
                    .unwrap()
                    .value();
            assert!(approx(due_future, ordinary_future * 1.05, 1e-9));
        }

        /// The growing functions carry the payment's currency, like every other
        /// monetary operation (ADR-0034).
        #[test]
        fn growing_values_keep_the_payment_currency() {
            let payment = Money::new(100.0, Currency::Usd).unwrap();
            let periods = Period::new(12.0).unwrap();
            assert_eq!(
                annuity::growing_present_value(rate(0.05), rate(0.02), periods, payment)
                    .unwrap()
                    .currency(),
                Currency::Usd,
            );
            assert_eq!(
                annuity::due::growing_future_value(rate(0.05), rate(0.02), periods, payment)
                    .unwrap()
                    .currency(),
                Currency::Usd,
            );
        }

        /// A zero-period growing annuity has nothing to pay, so it is worth zero —
        /// not an error. (`payment` is the degenerate case that *is* rejected,
        /// because dividing by a zero factor has no answer.)
        #[test]
        fn a_zero_term_is_worth_nothing() {
            let payment = Money::agnostic(100.0).unwrap();
            assert!(approx(
                annuity::growing_present_value(rate(0.05), rate(0.02), Period::ZERO, payment)
                    .unwrap()
                    .value(),
                0.0,
                1e-12,
            ));
            assert!(approx(
                annuity::growing_future_value(rate(0.05), rate(0.02), Period::ZERO, payment)
                    .unwrap()
                    .value(),
                0.0,
                1e-12,
            ));
        }
    }
}
