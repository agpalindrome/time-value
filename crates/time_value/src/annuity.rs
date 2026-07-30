//! Annuities: a fixed payment each period.
//!
//! The top-level functions model an **ordinary** annuity — payments fall at the
//! *end* of each period, the default in finance and the basis of loan
//! amortisation. The [`due`] submodule mirrors them for an **annuity-due**
//! (payments at the *start* of each period), whose factors are the ordinary
//! factors scaled by `(1 + r)`. [`perpetuity`] and [`growing_perpetuity`] give
//! the present value of a payment that continues forever, and
//! [`due::perpetuity`] / [`due::growing_perpetuity`] the start-of-period
//! variants (ADR-0062).
//!
//! The level payment is solved from either end of the horizon: [`payment`]
//! amortises a present value, and [`payment_from_future`] is the **sinking-fund**
//! payment that accumulates to a future one (Excel's `PMT(rate, nper, 0, fv)`).
//! Both have [`due`] counterparts.
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

use crate::math::{exp_m1, ln, ln_1p, powf};
use crate::root::{abs, bracket_and_bisect, Residual};
use crate::{
    FutureValue, Growth, Money, Payment, Period, Periodicity, PresentValue, Rate, TvmError,
};

/// Rate magnitude below which a **logarithmic** solve takes its `r → 0` limit
/// instead of the closed form (which divides by `ln(1 + r)`, ill-conditioned near
/// zero). The annuity *factors* need no such band — they are exact at every rate
/// but `0` itself (ADR-0054).
const RATE_NEAR_ZERO: f64 = 1e-9;

fn near_zero(x: f64) -> bool {
    x < RATE_NEAR_ZERO && x > -RATE_NEAR_ZERO
}

/// The present-value annuity factor `(1 - (1 + r)⁻ⁿ) / r`, taking the limit `n`
/// at `r = 0`.
///
/// Evaluated as `−expm1(−n·ln1p(r)) / r` rather than from the literal closed form
/// (ADR-0054). Written literally, `(1 + r)⁻ⁿ` is a number just below `1` for small
/// `r`, and subtracting it from `1` cancels away every significant digit of the
/// answer: at `r = 1e-9, n = 12` the literal form made the factor *rise* with the
/// rate — `12.000000882` against a true `11.999999922` — which is not merely
/// imprecise but the wrong sign of change. `expm1`/`ln1p` compute the same
/// quantity without ever forming the near-`1` intermediate, so the factor is
/// accurate to a few ULP everywhere and stays **monotone** in `r`, which is the
/// property [`solve_rate`] relies on to have a unique root.
fn present_value_factor(rate: f64, periods: f64) -> f64 {
    if rate == 0.0 {
        periods
    } else {
        -exp_m1(-periods * ln_1p(rate)) / rate
    }
}

/// The future-value annuity factor `((1 + r)ⁿ - 1) / r`, taking the limit `n` at
/// `r = 0`.
///
/// The mirror of [`present_value_factor`], with the same cancellation and the same
/// fix: `(1 + r)ⁿ` is just *above* `1` for small `r`, so subtracting `1` loses the
/// answer. Evaluated as `expm1(n·ln1p(r)) / r`.
fn future_value_factor(rate: f64, periods: f64) -> f64 {
    if rate == 0.0 {
        periods
    } else {
        exp_m1(periods * ln_1p(rate)) / rate
    }
}

/// The present-value factor for a **growing** annuity,
/// `(1 - ((1 + g)/(1 + r))ⁿ) / (r - g)`, taking the limit `n / (1 + r)` at
/// `g = r` (ADR-0048).
///
/// The limit is taken on the *spread* `r - g` rather than on `r` alone: the closed
/// form is `0/0` when the discount rate exactly matches the growth rate (every
/// payment then discounts to the same present amount).
///
/// Evaluated as `−expm1(n·ln1p(−spread / (1 + r))) / spread`, the same
/// cancellation-free shape as [`present_value_factor`] — which is its `g = 0` case,
/// since `ln1p(−r/(1 + r)) = −ln1p(r)`. Note the ratio `(1 + g)/(1 + r)` is never
/// formed: rewriting it as `1 − spread/(1 + r)` keeps the whole computation in
/// terms of the spread, so the factor is accurate right down to the limit instead
/// of needing a fuzzy band around it (ADR-0054).
fn growing_present_value_factor(rate: f64, growth: f64, periods: f64) -> f64 {
    let spread = rate - growth;
    if spread == 0.0 {
        periods / (1.0 + rate)
    } else {
        -exp_m1(periods * ln_1p(-spread / (1.0 + rate))) / spread
    }
}

/// The future-value factor for a **growing** annuity,
/// `((1 + r)ⁿ - (1 + g)ⁿ) / (r - g)`, taking the limit `n · (1 + r)ⁿ⁻¹` at `g = r`
/// (ADR-0048).
///
/// Computed as exactly what its docs have always said it is —
/// [`growing_present_value_factor`] compounded forward by `(1 + r)ⁿ` — rather than
/// as a second closed form differencing two powers, which cancels for a small
/// spread the way the present-value form did. The identity also carries the limit:
/// `(1 + r)ⁿ · n/(1 + r)` *is* `n · (1 + r)ⁿ⁻¹`, so there is one limit branch
/// between the two factors instead of two that must be kept in step. It reduces to
/// [`future_value_factor`] at `g = 0`.
fn growing_future_value_factor(rate: f64, growth: f64, periods: f64) -> f64 {
    powf(1.0 + rate, periods) * growing_present_value_factor(rate, growth, periods)
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
/// Returns [`TvmError::ZeroPeriods`] if `periods` is zero, so there is nothing to
/// amortise over and the payment has no answer (the factor is `0`), or
/// [`TvmError::Overflow`] if the division overflows on extreme magnitudes
/// (ADR-0021, ADR-0031, ADR-0052).
pub fn payment<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    present: Money,
) -> Result<Money, TvmError> {
    if periods.value() == 0.0 {
        // Nothing to amortise over: the annuity factor is 0, so the payment is
        // undefined rather than merely too large.
        return Err(TvmError::ZeroPeriods);
    }
    let factor = present_value_factor(rate.value(), periods.value());
    Money::from_operation(present.value() / factor, present.currency())
}

/// The level payment that accumulates to a `future` value over `periods` periods
/// at `rate` — the inverse of [`future_value`], and the **sinking-fund** payment:
/// how much must be set aside each period to reach a target. Excel's
/// `PMT(rate, nper, 0, fv)`.
///
/// `PMT = FV / s(r, n)`, where `s` is the future-value annuity factor
/// `((1 + r)ⁿ − 1) / r`; at `r = 0` that factor is `n`, so `PMT = FV / n`.
///
/// This completes the `_from_future` coinage the solves already use
/// ([`periods_from_future`], [`rate_from_future`]): the same relationship read
/// from the far end of the horizon rather than from today (ADR-0062).
///
/// # Examples
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// // Reach 1268.25 in a year at 1% per month -> set aside ~100 each month.
/// let pmt = annuity::payment_from_future(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(1268.250)?,
/// )?;
/// assert!((pmt.value() - 100.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// Over a single period the factor is `1` — the one payment falls at the end of
/// the term and never compounds — so the payment is the target, at every rate (to
/// within a couple of ULP: `1` is the factor's algebraic value, and `expm1 ∘ ln1p`
/// is not bit-exactly the identity). That is well defined, unlike
/// [`rate_from_future`], which is
/// [indeterminate](TvmError::IndeterminateRate) on the very same term because
/// there the rate is what is being solved for (ADR-0056):
///
/// ```
/// use time_value::{annuity, Money, Monthly, Period, Rate};
///
/// let pmt = annuity::payment_from_future(
///     Rate::<Monthly>::new(0.25)?,
///     Period::new(1.0)?,
///     Money::agnostic(500.0)?,
/// )?;
/// assert!((pmt.value() - 500.0).abs() < 1e-9);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// Returns [`TvmError::ZeroPeriods`] if `periods` is zero, so there is nothing to
/// contribute over and the payment has no answer (the factor is `0`) — the same
/// degeneracy, and the same variant, as [`payment`]. [`TvmError::Overflow`] if the
/// division overflows on extreme magnitudes (ADR-0021, ADR-0031, ADR-0052).
///
/// The factor is `0` only at `n = 0`: it is at least `1` for every `n ≥ 1` and
/// every rate above `−100%`, so no other term divides by zero (ADR-0056's table).
pub fn payment_from_future<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    future: Money,
) -> Result<Money, TvmError> {
    if periods.value() == 0.0 {
        // Nothing to contribute over: the annuity factor is 0, so the payment is
        // undefined rather than merely too large — as in `payment`.
        return Err(TvmError::ZeroPeriods);
    }
    let factor = future_value_factor(rate.value(), periods.value());
    Money::from_operation(future.value() / factor, future.currency())
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
    growing_perpetuity(rate, Growth(Rate::from_valid(0.0)), payment)
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
/// use time_value::{annuity, Growth, Money, Monthly, Rate};
///
/// // First payment 100 at month end, growing 2%/month, discounted at 5%/month.
/// let pv = annuity::growing_perpetuity(
///     Rate::<Monthly>::new(0.05)?,
///     Growth(Rate::new(0.02)?),
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
    growth: Growth<P>,
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
/// use time_value::{annuity, Growth, Money, Monthly, Period, Rate};
///
/// // First payment 100 at month end, growing 2%/month for a year, at 5%/month.
/// let pv = annuity::growing_present_value(
///     Rate::<Monthly>::new(0.05)?,
///     Growth(Rate::new(0.02)?),
///     Period::new(12.0)?,
///     Money::agnostic(100.0)?,
/// )?;
/// assert!((pv.value() - 979.318).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// Only the *second* rate is a [`Growth`], so discounting at the growth rate and
/// growing at the discount rate does not compile — the untagged version priced
/// that swap at `1386.73` without complaint (ADR-0050):
///
/// ```compile_fail
/// use time_value::{annuity, Growth, Money, Monthly, Period, Rate};
///
/// let _ = annuity::growing_present_value(
///     Growth(Rate::<Monthly>::new(0.02).unwrap()), // growth where the discount rate goes
///     Rate::<Monthly>::new(0.05).unwrap(),
///     Period::new(12.0).unwrap(),
///     Money::agnostic(100.0).unwrap(),
/// );
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the discounted sum overflows to a non-finite value
/// on extreme rate/growth/period magnitudes (ADR-0021).
pub fn growing_present_value<P: Periodicity>(
    rate: Rate<P>,
    growth: Growth<P>,
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
/// use time_value::{annuity, Growth, Money, Monthly, Period, Rate};
///
/// let fv = annuity::growing_future_value(
///     Rate::<Monthly>::new(0.05)?,
///     Growth(Rate::new(0.02)?),
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
    growth: Growth<P>,
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
/// use time_value::{annuity, Money, Monthly, Payment, PresentValue, Rate};
///
/// // How many 100/month payments retire a 1125.508 loan at 1%/month? A year.
/// let n = annuity::periods(
///     Rate::<Monthly>::new(0.01)?,
///     Payment(Money::agnostic(100.0)?),
///     PresentValue(Money::agnostic(1125.508)?),
/// )?;
/// assert!((n.value() - 12.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// The two amounts are role-tagged, so transposing them does not compile —
/// unlike the untagged version, which answered `0.089` payments as readily as
/// `12` (ADR-0050):
///
/// ```compile_fail
/// use time_value::{annuity, Money, Monthly, Payment, PresentValue, Rate};
///
/// let _ = annuity::periods(
///     Rate::<Monthly>::new(0.01).unwrap(),
///     PresentValue(Money::agnostic(1125.508).unwrap()), // the balance, where the payment goes
///     Payment(Money::agnostic(100.0).unwrap()),
/// );
/// ```
///
/// # Errors
///
/// [`TvmError::PaymentDoesNotAmortize`] if the payment never retires the balance —
/// when `PMT ≤ PV·r`, the payment does not even cover the period's interest, so the
/// logarithm's argument is non-positive and `n` has no answer (likewise a zero
/// payment, which retires nothing at any rate). This is the same condition
/// [`Schedule::with_payment`](crate::amortization::Schedule::with_payment) rejects
/// (ADR-0052). [`NegativePeriods`] if the solved `n` is negative.
///
/// [`NegativePeriods`]: TvmError::NegativePeriods
pub fn periods<P: Periodicity>(
    rate: Rate<P>,
    payment: Payment,
    present: PresentValue,
) -> Result<Period<P>, TvmError> {
    let (payment, present) = (payment.money(), present.money());
    let r = rate.value();
    let n = if near_zero(r) {
        if payment.value() == 0.0 {
            // No interest accrues, but a zero payment still never retires a
            // balance.
            return Err(TvmError::PaymentDoesNotAmortize);
        }
        present.value() / payment.value()
    } else {
        let arg = 1.0 - present.value() * r / payment.value();
        if arg <= 0.0 || arg.is_nan() {
            // PMT ≤ PV·r (or a zero payment): the logarithm's argument is
            // non-positive, so no finite number of payments retires the balance.
            return Err(TvmError::PaymentDoesNotAmortize);
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
/// use time_value::{annuity, FutureValue, Money, Monthly, Payment, Rate};
///
/// // How many 100/month contributions reach ~1268.25 at 1%/month? A year.
/// let n = annuity::periods_from_future(
///     Rate::<Monthly>::new(0.01)?,
///     Payment(Money::agnostic(100.0)?),
///     FutureValue(Money::agnostic(1268.250)?),
/// )?;
/// assert!((n.value() - 12.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// The payment and the target value cannot be transposed (ADR-0050):
///
/// ```compile_fail
/// use time_value::{annuity, FutureValue, Money, Monthly, Payment, Rate};
///
/// let _ = annuity::periods_from_future(
///     Rate::<Monthly>::new(0.01).unwrap(),
///     FutureValue(Money::agnostic(1268.250).unwrap()), // the target, where the payment goes
///     Payment(Money::agnostic(100.0).unwrap()),
/// );
/// ```
///
/// # Errors
///
/// [`TvmError::NoRealSolution`] if `1 + FV·r / PMT` is non-positive (no real
/// logarithm — the payment and the target are inconsistent in sign or magnitude)
/// or the payment is zero (nothing accumulates), or
/// [`TvmError::NegativePeriods`] if the solved `n` is negative. Unlike
/// [`periods`], nothing here is being *amortised*, so there is no interest
/// threshold to name (ADR-0052).
pub fn periods_from_future<P: Periodicity>(
    rate: Rate<P>,
    payment: Payment,
    future: FutureValue,
) -> Result<Period<P>, TvmError> {
    let (payment, future) = (payment.money(), future.money());
    let r = rate.value();
    let n = if near_zero(r) {
        if payment.value() == 0.0 {
            // No interest and no contribution: nothing ever accumulates.
            return Err(TvmError::NoRealSolution);
        }
        future.value() / payment.value()
    } else {
        let arg = 1.0 + future.value() * r / payment.value();
        if arg <= 0.0 || arg.is_nan() {
            // The logarithm's argument is non-positive: these contributions never
            // reach that target.
            return Err(TvmError::NoRealSolution);
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
/// use time_value::{annuity, Money, Monthly, Payment, Period, PresentValue, Rate};
///
/// // What monthly rate amortises 1125.508 with 12 payments of 100? About 1%.
/// let r = annuity::rate::<Monthly>(
///     Period::new(12.0)?,
///     Payment(Money::agnostic(100.0)?),
///     PresentValue(Money::agnostic(1125.508)?),
/// )?;
/// assert!((r.value() - 0.01).abs() < 1e-4);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// - [`TvmError::ZeroPeriods`] if `periods` is zero: the annuity factor is then
///   `0` whatever the rate, so the equation constrains nothing (ADR-0056).
/// - [`TvmError::SolveDidNotConverge`] if no rate prices the payment stream at
///   `present` (e.g. incompatible signs).
/// - [`TvmError::RateOutOfRange`] / [`TvmError::Overflow`] if the located root is
///   outside the valid rate domain or non-finite.
pub fn rate<P: Periodicity>(
    periods: Period<P>,
    payment: Payment,
    present: PresentValue,
) -> Result<Rate<P>, TvmError> {
    solve_rate(
        periods.value(),
        payment.money().value(),
        present.money().value(),
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
/// use time_value::{annuity, FutureValue, Money, Monthly, Payment, Period, Rate};
///
/// // What monthly rate accumulates 12 payments of 100 to ~1268.25? About 1%.
/// let r = annuity::rate_from_future::<Monthly>(
///     Period::new(12.0)?,
///     Payment(Money::agnostic(100.0)?),
///     FutureValue(Money::agnostic(1268.250)?),
/// )?;
/// assert!((r.value() - 0.01).abs() < 1e-4);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// As [`rate`], plus the single-period degeneracy: over one period the
/// future-value factor is exactly `1` for every rate, so the equation reduces to
/// `PMT = FV` with the rate absent. [`TvmError::IndeterminateRate`] if the two are
/// equal (every rate satisfies it), [`TvmError::NoRealSolution`] if they differ
/// (none does) — ADR-0056.
pub fn rate_from_future<P: Periodicity>(
    periods: Period<P>,
    payment: Payment,
    future: FutureValue,
) -> Result<Rate<P>, TvmError> {
    // Over a single period the future-value factor is exactly `1` for every rate —
    // the one payment falls at the end of the term and is never compounded — so the
    // equation reduces to `PMT = FV` with `r` absent. Either every rate satisfies it
    // or none does, and which it is turns on that comparison alone. Without this
    // guard the bracketing scan finds its very first probe to be a root and returns
    // that arbitrary sentinel, `−0.9999` (ADR-0056).
    //
    // `n = 1` is the only such case: `((1+r)ⁿ − 1)/r` is constant in `r` at `n = 0`
    // (handled in `solve_rate`) and at `n = 1`, and at no larger `n`.
    //
    // "Equal" here is the solver's own root test, not `==`. A target a hair away
    // from the payment still leaves a residual inside the accepted tolerance at
    // every rate, so exact equality would let those near-misses keep leaking the
    // sentinel. Reusing `Residual::is_root` means this guard and the solver cannot
    // disagree about what counts as satisfied.
    //
    // The term test is exact — `n = 1` is an identity, not a neighbourhood — and is
    // written against zero because that is the form the crate's `float_cmp` lint
    // permits; `n - 1.0 == 0.0` holds for exactly the finite `n` where `n == 1.0`.
    if periods.value() - 1.0 == 0.0 {
        let priced = payment.money().value(); // the factor is exactly 1
        let target = future.money().value();
        let residual = Residual {
            value: priced - target,
            scale: abs(priced) + abs(target),
        };
        return Err(if residual.is_root() {
            TvmError::IndeterminateRate
        } else {
            TvmError::NoRealSolution
        });
    }
    solve_rate(
        periods.value(),
        payment.money().value(),
        future.money().value(),
        future_value_factor,
    )
}

/// Solve `payment · factor(r, periods) = target` for the per-period rate `r`.
///
/// `factor` is [`present_value_factor`] or [`future_value_factor`]; both are
/// monotone in `r` — a property the `expm1`/`ln1p` formulation exists to make
/// true in floating point and not merely in algebra (ADR-0054) — so the residual
/// has a single root, located by the shared bracketing bisection
/// ([`root::bracket_and_bisect`](crate::root)).
///
/// The residual is judged against the scale of the two quantities differenced to
/// form it, `|PMT·factor(r,n)| + |target|`, evaluated at the same `r`. A tolerance
/// fixed in advance from `|target|` alone would, for a target near zero, accept
/// any rate large enough to drive the priced value to nothing — the same loophole
/// [`Residual::is_root`](crate::root) closes for the IRR (ADR-0021, ADR-0054).
fn solve_rate<P: Periodicity>(
    periods: f64,
    payment: f64,
    target: f64,
    factor: impl Fn(f64, f64) -> f64,
) -> Result<Rate<P>, TvmError> {
    // Over a zero term both factors are identically `0`, so the priced value is `0`
    // whatever the rate: the equation says nothing about `r`. `payment` already
    // rejects a zero term as `ZeroPeriods`; the solves said `SolveDidNotConverge`,
    // which blamed the iteration for a degenerate input (ADR-0056).
    if periods == 0.0 {
        return Err(TvmError::ZeroPeriods);
    }
    let residual = |r: f64| {
        let priced = payment * factor(r, periods);
        Residual {
            value: priced - target,
            scale: abs(priced) + abs(target),
        }
    };
    match bracket_and_bisect(residual) {
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
///
/// The mirroring is complete: [`payment_from_future`](due::payment_from_future) is
/// the start-of-period sinking fund, and [`perpetuity`](due::perpetuity) /
/// [`growing_perpetuity`](due::growing_perpetuity) are the start-of-period
/// perpetuities ADR-0015's amendment deferred as "again a `(1 + r)` scaling"
/// (ADR-0062).
pub mod due {
    use super::{
        future_value_factor, growing_future_value_factor, growing_present_value_factor,
        present_value_factor,
    };
    use crate::{Growth, Money, Period, Periodicity, Rate, TvmError};

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
    /// Returns [`TvmError::ZeroPeriods`] if `periods` is zero, so the factor is `0`
    /// and the payment has no answer, or [`TvmError::Overflow`] if the division
    /// overflows on extreme magnitudes (ADR-0021, ADR-0031, ADR-0052).
    pub fn payment<P: Periodicity>(
        rate: Rate<P>,
        periods: Period<P>,
        present: Money,
    ) -> Result<Money, TvmError> {
        if periods.value() == 0.0 {
            return Err(TvmError::ZeroPeriods);
        }
        let factor = present_value_factor(rate.value(), periods.value()) * (1.0 + rate.value());
        Money::from_operation(present.value() / factor, present.currency())
    }

    /// The level payment, made at the *start* of each period, that accumulates to a
    /// `future` value over `periods` periods at `rate` — the inverse of
    /// [`future_value`], and the annuity-due sinking fund.
    ///
    /// `PMT = FV / [(1 + r) · s(r, n)]`, where `s` is the ordinary future-value
    /// annuity factor; at `r = 0` that whole factor is `n`, so `PMT = FV / n`.
    /// Contributing at the start of each period earns one extra period of interest
    /// on every payment, so the required payment is the ordinary one divided by
    /// `(1 + r)`.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Period, Rate};
    ///
    /// // Reach 1280.93 in a year at 1%/month with start-of-month contributions
    /// // -> ~100 per month.
    /// let pmt = annuity::due::payment_from_future(
    ///     Rate::<Monthly>::new(0.01)?,
    ///     Period::new(12.0)?,
    ///     Money::agnostic(1280.933)?,
    /// )?;
    /// assert!((pmt.value() - 100.0).abs() < 1e-2);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::ZeroPeriods`] if `periods` is zero, so the factor is `0`
    /// and the payment has no answer, or [`TvmError::Overflow`] if the division
    /// overflows on extreme magnitudes (ADR-0021, ADR-0031, ADR-0052). The `(1 + r)`
    /// scaling never introduces a second zero: [`Rate`] rejects anything at or below
    /// `−100%`, so `1 + r` is strictly positive.
    pub fn payment_from_future<P: Periodicity>(
        rate: Rate<P>,
        periods: Period<P>,
        future: Money,
    ) -> Result<Money, TvmError> {
        if periods.value() == 0.0 {
            return Err(TvmError::ZeroPeriods);
        }
        let factor = future_value_factor(rate.value(), periods.value()) * (1.0 + rate.value());
        Money::from_operation(future.value() / factor, future.currency())
    }

    /// The present value of a **level perpetuity-due** — a `payment` at the *start*
    /// of every period, forever — discounted at `rate`.
    ///
    /// `PV = (PMT / r) · (1 + r)`. The first payment falls today and is not
    /// discounted, which is exactly the `(1 + r)` this module applies everywhere.
    /// Convergence is the ordinary perpetuity's condition unchanged — bringing every
    /// payment forward one period rescales the sum, it does not make a divergent one
    /// converge — so `r > 0` is still required. This is the `g = 0` case of
    /// [`growing_perpetuity`], as it is at the module top level.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Money, Monthly, Rate};
    ///
    /// // 100 at the start of every month, forever, discounted at 5% per month.
    /// let pv = annuity::due::perpetuity(Rate::<Monthly>::new(0.05)?, Money::agnostic(100.0)?)?;
    /// assert!((pv.value() - 2100.0).abs() < 1e-9); // ordinary 2000 × 1.05
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::DivergentPerpetuity`] if `rate` is not strictly positive
    /// (the present value diverges), or [`TvmError::Overflow`] if the division
    /// overflows on extreme magnitudes (ADR-0021).
    pub fn perpetuity<P: Periodicity>(rate: Rate<P>, payment: Money) -> Result<Money, TvmError> {
        growing_perpetuity(rate, Growth(Rate::from_valid(0.0)), payment)
    }

    /// The present value of a **growing perpetuity-due** — a payment at the *start*
    /// of every period, forever, growing at `growth` each period — discounted at
    /// `rate`.
    ///
    /// `PV = (PMT / (r − g)) · (1 + r)`, where `PMT` is the *first* payment, made
    /// today. Like [`super::growing_perpetuity`], the sum converges only when
    /// `r > g`, and the same [`DivergentPerpetuity`](TvmError::DivergentPerpetuity)
    /// rejection applies — this delegates to it and scales the result, so the two
    /// cannot disagree about which rate/growth pairs are admissible.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{annuity, Growth, Money, Monthly, Rate};
    ///
    /// // First payment 100 today, growing 2%/month, discounted at 5%/month.
    /// let pv = annuity::due::growing_perpetuity(
    ///     Rate::<Monthly>::new(0.05)?,
    ///     Growth(Rate::new(0.02)?),
    ///     Money::agnostic(100.0)?,
    /// )?;
    /// assert!((pv.value() - 3500.0).abs() < 1e-9); // ordinary 3333.33… × 1.05
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::DivergentPerpetuity`] if `rate <= growth` (the present
    /// value diverges), or [`TvmError::Overflow`] if the division or the `(1 + r)`
    /// scaling overflows on extreme magnitudes (ADR-0021).
    pub fn growing_perpetuity<P: Periodicity>(
        rate: Rate<P>,
        growth: Growth<P>,
        payment: Money,
    ) -> Result<Money, TvmError> {
        let ordinary = super::growing_perpetuity(rate, growth, payment)?;
        Money::from_operation(ordinary.value() * (1.0 + rate.value()), ordinary.currency())
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
    /// use time_value::{annuity, Growth, Money, Monthly, Period, Rate};
    ///
    /// // First payment 100 at month start, growing 2%/month for a year, at 5%.
    /// let pv = annuity::due::growing_present_value(
    ///     Rate::<Monthly>::new(0.05)?,
    ///     Growth(Rate::new(0.02)?),
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
        growth: Growth<P>,
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
    /// use time_value::{annuity, Growth, Money, Monthly, Period, Rate};
    ///
    /// let fv = annuity::due::growing_future_value(
    ///     Rate::<Monthly>::new(0.05)?,
    ///     Growth(Rate::new(0.02)?),
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
        growth: Growth<P>,
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
    use crate::{
        annuity, FutureValue, Growth, Money, Monthly, Payment, Period, PresentValue, Rate, TvmError,
    };

    /// `no_std`-safe approximate equality (no `f64::abs`).
    fn approx(a: f64, b: f64, tolerance: f64) -> bool {
        let d = a - b;
        d < tolerance && d > -tolerance
    }

    fn rate(r: f64) -> Rate<Monthly> {
        Rate::<Monthly>::new(r).unwrap()
    }

    fn growth(g: f64) -> Growth<Monthly> {
        Growth(rate(g))
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
        assert_eq!(result, Err(TvmError::ZeroPeriods));
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
        assert_eq!(result, Err(TvmError::ZeroPeriods));
    }

    /// The sinking-fund payment (ADR-0062). The closed form divides by the
    /// future-value annuity factor, so it is checked against that factor summed
    /// term by term — an independent reference, rather than against the crate's own
    /// `future_value`.
    mod sinking_fund {
        use super::{approx, rate};
        use crate::{annuity, Currency, Money, Period, Rate, TvmError};

        /// `Σ (1+r)^k` for `k = 0..n` — the future-value annuity factor built one
        /// payment at a time: the `k`-th of `n` end-of-period payments compounds for
        /// `n − k` periods.
        fn future_value_factor_by_summation(r: f64, n: u32) -> f64 {
            let mut total = 0.0;
            let mut compounded = 1.0;
            for _ in 0..n {
                total += compounded;
                compounded *= 1.0 + r;
            }
            total
        }

        /// `Σ (1+r)^k` for `k = 1..=n` — the same stream contributed at the *start*
        /// of each period, so every payment earns one period more.
        fn due_future_value_factor_by_summation(r: f64, n: u32) -> f64 {
            (1.0 + r) * future_value_factor_by_summation(r, n)
        }

        #[test]
        fn payment_from_future_matches_a_direct_summation() {
            let target = Money::agnostic(1_268.250).unwrap();
            let pmt = annuity::payment_from_future(rate(0.01), Period::new(12.0).unwrap(), target)
                .unwrap();
            let expected = target.value() / future_value_factor_by_summation(0.01, 12);
            assert!(approx(pmt.value(), expected, 1e-9));
        }

        #[test]
        fn due_payment_from_future_matches_a_direct_summation() {
            let target = Money::agnostic(1_280.933).unwrap();
            let pmt =
                annuity::due::payment_from_future(rate(0.01), Period::new(12.0).unwrap(), target)
                    .unwrap();
            let expected = target.value() / due_future_value_factor_by_summation(0.01, 12);
            assert!(approx(pmt.value(), expected, 1e-9));
        }

        /// At `r = 0` the factor is `n`, so the contribution is the target split
        /// evenly — and the due scaling `(1 + r)` is `1`, so both forms agree.
        #[test]
        fn a_zero_rate_uses_the_limit() {
            let target = Money::agnostic(1_200.0).unwrap();
            let periods = Period::new(12.0).unwrap();
            assert!(approx(
                annuity::payment_from_future(rate(0.0), periods, target)
                    .unwrap()
                    .value(),
                100.0,
                1e-9,
            ));
            assert!(approx(
                annuity::due::payment_from_future(rate(0.0), periods, target)
                    .unwrap()
                    .value(),
                100.0,
                1e-9,
            ));
        }

        /// Over one period the ordinary factor is exactly `1`, so the payment *is*
        /// the target whatever the rate. This is the `n = 1` case ADR-0056 records as
        /// degenerate for `rate_from_future` — where the rate is the unknown — and
        /// which is perfectly well posed here, where it is given.
        #[test]
        fn a_single_period_payment_is_the_target_at_any_rate() {
            let target = Money::agnostic(500.0).unwrap();
            let one = Period::new(1.0).unwrap();
            for r in [-0.5, 0.0, 0.01, 5.0] {
                let pmt = annuity::payment_from_future(rate(r), one, target).unwrap();
                assert!(approx(pmt.value(), 500.0, 1e-9), "rate {r}");
                // The due form still discounts the extra period of interest.
                let due = annuity::due::payment_from_future(rate(r), one, target).unwrap();
                assert!(approx(due.value(), 500.0 / (1.0 + r), 1e-9), "due rate {r}");
            }
        }

        /// A zero term makes the factor `0`, so the contribution has no answer — the
        /// same degeneracy, and the same variant, as `annuity::payment` (ADR-0056).
        #[test]
        fn a_zero_term_is_degenerate_in_both_forms() {
            let target = Money::agnostic(1_000.0).unwrap();
            assert_eq!(
                annuity::payment_from_future(rate(0.01), Period::ZERO, target),
                Err(TvmError::ZeroPeriods),
            );
            assert_eq!(
                annuity::due::payment_from_future(rate(0.01), Period::ZERO, target),
                Err(TvmError::ZeroPeriods),
            );
        }

        /// Every rate above `−100%` is admissible, and none of them makes the factor
        /// zero for `n ≥ 1`: the factor is at least `1` (ADR-0056's table), so a rate
        /// close to the `Rate` floor still yields a finite payment rather than a
        /// division by zero.
        #[test]
        fn a_rate_near_the_floor_still_resolves() {
            let target = Money::agnostic(1_000.0).unwrap();
            let periods = Period::new(24.0).unwrap();
            let almost_minus_one = Rate::<crate::Monthly>::new(-0.999_999).unwrap();
            let pmt = annuity::payment_from_future(almost_minus_one, periods, target).unwrap();
            // The factor tends to 1 as `1 + r → 0`, so the payment tends to the
            // target: only the first contribution survives the discounting.
            assert!(approx(pmt.value(), 1_000.0, 1e-3));
        }

        #[test]
        fn the_payment_keeps_the_target_currency() {
            let target = Money::new(1_268.250, Currency::Usd).unwrap();
            let periods = Period::new(12.0).unwrap();
            assert_eq!(
                annuity::payment_from_future(rate(0.01), periods, target)
                    .unwrap()
                    .currency(),
                Currency::Usd,
            );
            assert_eq!(
                annuity::due::payment_from_future(rate(0.01), periods, target)
                    .unwrap()
                    .currency(),
                Currency::Usd,
            );
        }
    }

    /// The perpetuity-due (ADR-0062). Its closed form is checked against the
    /// geometric series summed term by term, truncated where the remaining tail is
    /// provably far below the tolerance.
    mod perpetuity_due {
        use super::{approx, growth, rate};
        use crate::{annuity, Currency, Money, TvmError};

        /// `Σ PMT·((1+g)/(1+r))^k` for `k = 0..TERMS` — the growing perpetuity-due
        /// summed directly, the first payment falling today and so undiscounted.
        ///
        /// Truncation is safe by a wide margin: the tail beyond `TERMS` terms is the
        /// whole sum times `((1+g)/(1+r))^TERMS`. At the `r = 5%, g = 2%` used below
        /// that ratio is `0.9714`, so `1500` terms leave a *relative* remainder of
        /// about `1e-19` — eleven orders below the `1e-9` the assertions allow, and
        /// the level (`g = 0`) case converges faster still.
        fn by_summation(r: f64, g: f64, pmt: f64) -> f64 {
            const TERMS: u32 = 1_500;
            let ratio = (1.0 + g) / (1.0 + r);
            let mut total = 0.0;
            let mut term = pmt;
            for _ in 0..TERMS {
                total += term;
                term *= ratio;
            }
            total
        }

        #[test]
        fn a_level_perpetuity_due_matches_a_direct_summation() {
            let pv = annuity::due::perpetuity(rate(0.05), Money::agnostic(100.0).unwrap())
                .unwrap()
                .value();
            assert!(approx(pv, by_summation(0.05, 0.0, 100.0), 1e-9));
            // And the algebraic form the rustdoc states: (PMT / r) · (1 + r).
            assert!(approx(pv, 2_000.0 * 1.05, 1e-9));
        }

        #[test]
        fn a_growing_perpetuity_due_matches_a_direct_summation() {
            let pv = annuity::due::growing_perpetuity(
                rate(0.05),
                growth(0.02),
                Money::agnostic(100.0).unwrap(),
            )
            .unwrap()
            .value();
            assert!(approx(pv, by_summation(0.05, 0.02, 100.0), 1e-9));
            assert!(approx(pv, 3_500.0, 1e-9)); // 100/(0.05−0.02) × 1.05
        }

        /// The module-wide relation: a due form is its ordinary counterpart scaled by
        /// `(1 + r)`.
        #[test]
        fn both_forms_are_the_ordinary_ones_scaled_by_one_plus_r() {
            let payment = Money::agnostic(100.0).unwrap();
            let level = annuity::perpetuity(rate(0.05), payment).unwrap().value();
            let level_due = annuity::due::perpetuity(rate(0.05), payment)
                .unwrap()
                .value();
            assert!(approx(level_due, level * 1.05, 1e-9));

            let grown = annuity::growing_perpetuity(rate(0.05), growth(0.02), payment)
                .unwrap()
                .value();
            let grown_due = annuity::due::growing_perpetuity(rate(0.05), growth(0.02), payment)
                .unwrap()
                .value();
            assert!(approx(grown_due, grown * 1.05, 1e-9));
        }

        /// `perpetuity` is the `g = 0` case of `growing_perpetuity` here exactly as it
        /// is at the module top level — it delegates.
        #[test]
        fn the_level_form_is_the_zero_growth_growing_form() {
            let payment = Money::agnostic(100.0).unwrap();
            let level = annuity::due::perpetuity(rate(0.05), payment).unwrap();
            let grown = annuity::due::growing_perpetuity(rate(0.05), growth(0.0), payment).unwrap();
            assert!(approx(level.value(), grown.value(), 1e-9));
        }

        /// Bringing every payment forward one period rescales a convergent sum; it
        /// cannot rescue a divergent one. So the due forms reject exactly what the
        /// ordinary ones reject, with the same variant.
        #[test]
        fn divergence_is_rejected_on_the_same_condition() {
            let payment = Money::agnostic(100.0).unwrap();
            for result in [
                annuity::due::perpetuity(rate(0.0), payment),
                annuity::due::perpetuity(rate(-0.01), payment),
                // r = g: an infinity from division by zero.
                annuity::due::growing_perpetuity(rate(0.03), growth(0.03), payment),
                // r < g: a finite but meaningless value, still rejected.
                annuity::due::growing_perpetuity(rate(0.02), growth(0.05), payment),
            ] {
                assert_eq!(result, Err(TvmError::DivergentPerpetuity));
            }
        }

        #[test]
        fn the_present_value_keeps_the_payment_currency() {
            let payment = Money::new(100.0, Currency::Jpy).unwrap();
            assert_eq!(
                annuity::due::perpetuity(rate(0.05), payment)
                    .unwrap()
                    .currency(),
                Currency::Jpy,
            );
            assert_eq!(
                annuity::due::growing_perpetuity(rate(0.05), growth(0.02), payment)
                    .unwrap()
                    .currency(),
                Currency::Jpy,
            );
        }
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
            annuity::growing_perpetuity(rate(0.05), growth(0.0), Money::agnostic(100.0).unwrap())
                .unwrap();
        assert!(approx(pv.value(), grown.value(), 1e-9));
    }

    #[test]
    fn growing_perpetuity_discounts_by_the_spread() {
        // 100 / (0.05 - 0.02) = 3333.333...
        let pv =
            annuity::growing_perpetuity(rate(0.05), growth(0.02), Money::agnostic(100.0).unwrap())
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
            annuity::growing_perpetuity(rate(0.03), growth(0.03), payment),
            Err(TvmError::DivergentPerpetuity),
        );
        // r < g: a finite but meaningless value, still rejected.
        assert_eq!(
            annuity::growing_perpetuity(rate(0.02), growth(0.05), payment),
            Err(TvmError::DivergentPerpetuity),
        );
    }

    #[test]
    fn periods_inverts_present_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let present = annuity::present_value(rate(0.01), periods, payment).unwrap();
        let recovered =
            annuity::periods(rate(0.01), Payment(payment), PresentValue(present)).unwrap();
        assert!(approx(recovered.value(), periods.value(), 1e-6));
    }

    #[test]
    fn periods_from_future_inverts_future_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let future = annuity::future_value(rate(0.01), periods, payment).unwrap();
        let recovered =
            annuity::periods_from_future(rate(0.01), Payment(payment), FutureValue(future))
                .unwrap();
        assert!(approx(recovered.value(), periods.value(), 1e-6));
    }

    #[test]
    fn periods_zero_rate_uses_the_limit() {
        // At r = 0, PV = PMT·n, so n = PV / PMT.
        let n = annuity::periods(
            rate(0.0),
            Payment(Money::agnostic(100.0).unwrap()),
            PresentValue(Money::agnostic(1200.0).unwrap()),
        )
        .unwrap();
        assert!(approx(n.value(), 12.0, 1e-9));
    }

    #[test]
    fn periods_when_payment_cannot_cover_interest_does_not_amortise() {
        // 5% on a 10000 balance is 500/period, but the payment is only 100, so the
        // balance never falls: there is no n.
        assert_eq!(
            annuity::periods(
                rate(0.05),
                Payment(Money::agnostic(100.0).unwrap()),
                PresentValue(Money::agnostic(10_000.0).unwrap()),
            ),
            Err(TvmError::PaymentDoesNotAmortize)
        );
    }

    #[test]
    fn periods_with_a_zero_payment_does_not_amortise() {
        // The `r → 0` branch: no interest accrues, but paying nothing still never
        // retires a balance (ADR-0052).
        assert_eq!(
            annuity::periods(
                rate(0.0),
                Payment(Money::ZERO),
                PresentValue(Money::agnostic(1000.0).unwrap()),
            ),
            Err(TvmError::PaymentDoesNotAmortize)
        );
    }

    #[test]
    fn periods_from_future_with_no_real_logarithm_has_no_solution() {
        // Contributing -100/period cannot reach +1268 at 1%: `1 + FV·r/PMT` is
        // negative, so the logarithm has no real value (ADR-0052).
        assert_eq!(
            annuity::periods_from_future(
                rate(0.01),
                Payment(Money::agnostic(-100.0).unwrap()),
                FutureValue(Money::agnostic(1_268_250.0).unwrap()),
            ),
            Err(TvmError::NoRealSolution)
        );
    }

    #[test]
    fn periods_from_future_with_a_zero_payment_has_no_solution() {
        // The `r → 0` branch: nothing ever accumulates (ADR-0052).
        assert_eq!(
            annuity::periods_from_future(
                rate(0.0),
                Payment(Money::ZERO),
                FutureValue(Money::agnostic(1000.0).unwrap()),
            ),
            Err(TvmError::NoRealSolution)
        );
    }

    #[test]
    fn rate_inverts_present_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let present = annuity::present_value(rate(0.01), periods, payment).unwrap();
        let recovered =
            annuity::rate::<Monthly>(periods, Payment(payment), PresentValue(present)).unwrap();
        assert!(approx(recovered.value(), 0.01, 1e-6));
    }

    #[test]
    fn rate_from_future_inverts_future_value() {
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let future = annuity::future_value(rate(0.01), periods, payment).unwrap();
        let recovered =
            annuity::rate_from_future::<Monthly>(periods, Payment(payment), FutureValue(future))
                .unwrap();
        assert!(approx(recovered.value(), 0.01, 1e-6));
    }

    #[test]
    fn rate_recovers_a_negative_rate() {
        // A payment stream can price above PMT·n only at a negative rate.
        let periods = Period::new(12.0).unwrap();
        let payment = Money::agnostic(100.0).unwrap();
        let present = annuity::present_value(rate(-0.02), periods, payment).unwrap();
        let recovered =
            annuity::rate::<Monthly>(periods, Payment(payment), PresentValue(present)).unwrap();
        assert!(approx(recovered.value(), -0.02, 1e-6));
    }

    /// The cancellation defect ADR-0054 fixes, at the exact point it was
    /// reproduced: `(1 - (1+r)⁻ⁿ)/r` at `r = 1e-9, n = 12` priced 12 payments of
    /// 100 at `12000.000881862` — *above* the `r = 0` value of `12000`, which no
    /// positive rate can produce. The true value is `11999.999922000000364`.
    #[test]
    fn present_value_near_a_zero_rate_is_accurate() {
        let payment = Money::agnostic(1000.0).unwrap();
        let periods = Period::new(12.0).unwrap();
        let pv = |r: f64| {
            annuity::present_value(rate(r), periods, payment)
                .unwrap()
                .value()
        };
        // Correct to well within a cent on a 12000 stream; the defect was 0.0009
        // out and on the wrong side.
        assert!(approx(pv(1e-9), 11_999.999_922, 1e-6));
        assert!(approx(pv(5e-10), 11_999.999_961, 1e-6));
        assert!(approx(pv(0.0), 12_000.0, 1e-9));
    }

    /// The present-value factor is **non-increasing** in the rate — money later is
    /// worth less when discounting harder. `solve_rate`'s rustdoc cites exactly
    /// this monotonicity as the reason its residual has a single root, so it has to
    /// hold in floating point and not only in algebra (ADR-0045 rule 2, ADR-0054).
    /// The literal closed form broke it in a band around zero.
    #[test]
    fn present_value_is_non_increasing_in_the_rate() {
        let payment = Money::agnostic(1000.0).unwrap();
        for n in [1.0, 12.0, 30.0, 360.0] {
            let periods = Period::new(n).unwrap();
            let pv = |r: f64| {
                annuity::present_value(rate(r), periods, payment)
                    .unwrap()
                    .value()
            };
            // A grid straddling zero at the scale where the cancellation bit,
            // plus ordinary rates either side.
            let mut previous = f64::INFINITY;
            let mut r = -2e-8;
            while r <= 2e-8 {
                let current = pv(r);
                assert!(
                    current <= previous,
                    "PV rose from {previous} to {current} between rates either side of {r} (n = {n})",
                );
                previous = current;
                r += 1e-10;
            }
            // And the far field, where the factor saturates.
            let mut previous = f64::INFINITY;
            for r in [-0.5, -0.02, -1e-4, 0.0, 1e-4, 0.02, 0.5, 5.0, 100.0] {
                let current = pv(r);
                assert!(current <= previous, "PV rose at rate {r} (n = {n})");
                previous = current;
            }
        }
    }

    /// The future-value factor has the mirror-image cancellation — `(1+r)ⁿ` is just
    /// *above* one for a small rate — and the mirror-image monotonicity: it is
    /// non-**de**creasing in the rate.
    #[test]
    fn future_value_is_non_decreasing_in_the_rate() {
        let payment = Money::agnostic(1000.0).unwrap();
        let periods = Period::new(12.0).unwrap();
        let fv = |r: f64| {
            annuity::future_value(rate(r), periods, payment)
                .unwrap()
                .value()
        };
        let mut previous = f64::NEG_INFINITY;
        let mut r = -2e-8;
        while r <= 2e-8 {
            let current = fv(r);
            assert!(
                current >= previous,
                "FV fell from {previous} to {current} around rate {r}",
            );
            previous = current;
            r += 1e-10;
        }
        // 12 payments of 1000 at 1e-9/period: each earns interest for a whole
        // number of periods, so FV = 1000·(12 + 66e-9) to first order.
        assert!(approx(fv(1e-9), 12_000.000_066, 1e-6));
    }

    /// `rate` inverts `present_value`, so round-tripping the library's own PV must
    /// return the rate that produced it — *including its sign*. The cancellation
    /// defect made a true rate of `+1e-9` solve to `−1.12e-8` (ADR-0054).
    #[test]
    fn rate_round_trips_a_near_zero_rate_with_the_right_sign() {
        let payment = Money::agnostic(1000.0).unwrap();
        let periods = Period::new(12.0).unwrap();
        for r in [1e-9, 5e-10, 1e-8, -1e-9, -1e-8] {
            let present = annuity::present_value(rate(r), periods, payment).unwrap();
            let recovered =
                annuity::rate::<Monthly>(periods, Payment(payment), PresentValue(present))
                    .unwrap()
                    .value();
            assert!(
                (recovered < 0.0) == (r < 0.0),
                "rate {r} round-tripped to {recovered}, on the wrong side of zero",
            );
            // The residual tolerance is `1e-9 · (|priced| + |target|) ≈ 2.4e-5` on
            // a 12000 present value, and `dPV/dr ≈ −PMT·n(n+1)/2 = −78000`, so the
            // solver can resolve the rate to about `3e-10` and no finer. That is
            // the floor this bound sits just above — the defect missed by `1.2e-8`
            // at `r = 1e-9` and `2.6e-9` at `r = 5e-10`, both well outside it.
            assert!(
                approx(recovered, r, 1e-9),
                "rate {r} round-tripped to {recovered}",
            );
        }
    }

    #[test]
    fn rate_without_a_solution_does_not_converge() {
        // A positive payment can never price to a negative present value, so no
        // rate solves it.
        assert_eq!(
            annuity::rate::<Monthly>(
                Period::new(12.0).unwrap(),
                Payment(Money::agnostic(100.0).unwrap()),
                PresentValue(Money::agnostic(-1000.0).unwrap()),
            ),
            Err(TvmError::SolveDidNotConverge)
        );
    }

    /// A degenerate rate solve must never hand back the bracketing scan's starting
    /// sentinel (`−0.9999`) as if it were the answer (ADR-0056).
    mod degenerate_rate_solves {
        use super::*;

        /// Over one period the future-value factor is `1` for every rate, so when
        /// the target equals the payment every rate satisfies the equation. This is
        /// the case that used to return `Ok(-0.9999)`.
        #[test]
        fn a_single_period_future_solve_is_indeterminate_when_the_target_matches() {
            assert_eq!(
                annuity::rate_from_future::<Monthly>(
                    Period::new(1.0).unwrap(),
                    Payment(Money::agnostic(100.0).unwrap()),
                    FutureValue(Money::agnostic(100.0).unwrap()),
                ),
                Err(TvmError::IndeterminateRate)
            );
        }

        /// The target need not match the payment *exactly* to be satisfied by every
        /// rate — a difference inside the solver's tolerance leaves the residual a
        /// root at every rate just the same. An exact `==` guard would have let this
        /// one keep returning the sentinel, so the check reuses the solver's own
        /// root test.
        #[test]
        fn a_single_period_future_solve_is_indeterminate_within_the_root_tolerance() {
            assert_eq!(
                annuity::rate_from_future::<Monthly>(
                    Period::new(1.0).unwrap(),
                    Payment(Money::agnostic(100.0).unwrap()),
                    // 1e-13 adrift: far inside 1e-9 × (100 + 100).
                    FutureValue(Money::agnostic(100.000_000_000_000_1).unwrap()),
                ),
                Err(TvmError::IndeterminateRate)
            );
        }

        /// The same factor with a target the payment cannot reach: no rate works,
        /// which is the opposite failure and a different variant.
        #[test]
        fn a_single_period_future_solve_has_no_solution_when_the_target_differs() {
            assert_eq!(
                annuity::rate_from_future::<Monthly>(
                    Period::new(1.0).unwrap(),
                    Payment(Money::agnostic(100.0).unwrap()),
                    FutureValue(Money::agnostic(150.0).unwrap()),
                ),
                Err(TvmError::NoRealSolution)
            );
        }

        /// A zero term makes both factors identically zero, so the equation
        /// constrains nothing. `annuity::payment` already reports `ZeroPeriods`;
        /// the solves now agree instead of blaming the iteration.
        #[test]
        fn a_zero_term_is_zero_periods_in_both_solves() {
            for result in [
                annuity::rate::<Monthly>(
                    Period::ZERO,
                    Payment(Money::agnostic(500.0).unwrap()),
                    PresentValue(Money::ZERO),
                ),
                annuity::rate_from_future::<Monthly>(
                    Period::ZERO,
                    Payment(Money::agnostic(500.0).unwrap()),
                    FutureValue(Money::ZERO),
                ),
            ] {
                assert_eq!(result, Err(TvmError::ZeroPeriods));
            }
        }

        /// The guards must not swallow well-posed solves: two periods is the first
        /// term at which the future-value factor actually varies with the rate.
        ///
        /// The tolerance is derived, not chosen. A root is accepted when the
        /// residual is within `1e-9` of the scale `|priced| + |target| ≈ 410`, i.e.
        /// `4.1e-7`; the factor `(2 + r)` moves the priced value by `100` per unit
        /// of rate, so the rate itself is pinned only to about `4.1e-9`. The
        /// observed error is `1.9e-9`, comfortably inside that.
        #[test]
        fn a_two_period_future_solve_still_resolves() {
            let periods = Period::new(2.0).unwrap();
            let payment = Money::agnostic(100.0).unwrap();
            let future = annuity::future_value(rate(0.05), periods, payment).unwrap();
            let recovered = annuity::rate_from_future::<Monthly>(
                periods,
                Payment(payment),
                FutureValue(future),
            )
            .unwrap();
            assert!(approx(recovered.value(), 0.05, 1e-8));
        }
    }

    /// The growing annuity (ADR-0048). The closed forms are checked against a
    /// term-by-term sum of the payments themselves — an independent reference, so
    /// a mistranscribed factor cannot agree with it by construction.
    mod growing {
        use super::{approx, growth, rate};
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
                growth(0.02),
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
                growth(0.02),
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

            let pv = annuity::growing_present_value(rate(0.05), growth(0.05), periods, payment)
                .unwrap()
                .value();
            assert!(approx(pv, 10.0 * 100.0 / 1.05, 1e-9));
            assert!(approx(
                pv,
                present_value_by_summation(0.05, 0.05, 10, 100.0),
                1e-9,
            ));

            let fv = annuity::growing_future_value(rate(0.05), growth(0.05), periods, payment)
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
                growth(0.05),
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
                    growth(0.05),
                    Money::agnostic(100.0).unwrap()
                ),
                Err(TvmError::DivergentPerpetuity),
            );
        }

        /// The growing factors carry the same cancellation as the level ones, and
        /// take the same `expm1`/`ln1p` fix (ADR-0054). Checked against the
        /// term-by-term summation at spreads far too small for the old
        /// `|r − g| < 1e-9` limit band, which simply returned `n/(1+r)` and was
        /// wrong in the eighth digit there.
        #[test]
        fn a_vanishing_spread_is_accurate_not_merely_bounded() {
            for (r, g, n) in [
                (0.05, 0.05 - 1e-9, 10u32),
                (0.05, 0.05 - 1e-10, 10),
                (0.05, 0.05 + 1e-9, 10),
                (0.1, 0.1 - 1e-9, 30),
            ] {
                let pv = annuity::growing_present_value(
                    rate(r),
                    growth(g),
                    Period::new(f64::from(n)).unwrap(),
                    Money::agnostic(100.0).unwrap(),
                )
                .unwrap()
                .value();
                let expected = 100.0 * present_value_by_summation(r, g, n, 1.0);
                assert!(
                    approx(pv, expected, 1e-9),
                    "r = {r}, g = {g}, n = {n}: got {pv}, summation says {expected}",
                );
            }
        }

        /// The `g = 0` case of the growing factor *is* the level factor, and the
        /// future-value factor *is* the present-value one compounded forward — both
        /// relations the rustdoc asserts, and the second is now how
        /// `growing_future_value_factor` is computed, so the identity is exact
        /// rather than approximate (ADR-0045 rule 2).
        #[test]
        fn zero_growth_recovers_the_level_annuity() {
            let payment = Money::agnostic(100.0).unwrap();
            for (r, n) in [(0.05, 12.0), (0.0, 12.0), (1e-9, 12.0), (-0.02, 30.0)] {
                let periods = Period::new(n).unwrap();
                let level = annuity::present_value(rate(r), periods, payment)
                    .unwrap()
                    .value();
                let grown = annuity::growing_present_value(rate(r), growth(0.0), periods, payment)
                    .unwrap()
                    .value();
                assert!(approx(level, grown, 1e-9), "r = {r}, n = {n}");

                let level_future = annuity::future_value(rate(r), periods, payment)
                    .unwrap()
                    .value();
                let grown_future =
                    annuity::growing_future_value(rate(r), growth(0.0), periods, payment)
                        .unwrap()
                        .value();
                assert!(
                    approx(level_future, grown_future, 1e-8),
                    "future: r = {r}, n = {n}",
                );
            }
        }

        #[test]
        fn due_variants_are_the_ordinary_ones_scaled_by_one_plus_r() {
            let periods = Period::new(12.0).unwrap();
            let payment = Money::agnostic(100.0).unwrap();

            let ordinary_present =
                annuity::growing_present_value(rate(0.05), growth(0.02), periods, payment)
                    .unwrap()
                    .value();
            let due_present =
                annuity::due::growing_present_value(rate(0.05), growth(0.02), periods, payment)
                    .unwrap()
                    .value();
            assert!(approx(due_present, ordinary_present * 1.05, 1e-9));

            let ordinary_future =
                annuity::growing_future_value(rate(0.05), growth(0.02), periods, payment)
                    .unwrap()
                    .value();
            let due_future =
                annuity::due::growing_future_value(rate(0.05), growth(0.02), periods, payment)
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
                annuity::growing_present_value(rate(0.05), growth(0.02), periods, payment)
                    .unwrap()
                    .currency(),
                Currency::Usd,
            );
            assert_eq!(
                annuity::due::growing_future_value(rate(0.05), growth(0.02), periods, payment)
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
                annuity::growing_present_value(rate(0.05), growth(0.02), Period::ZERO, payment)
                    .unwrap()
                    .value(),
                0.0,
                1e-12,
            ));
            assert!(approx(
                annuity::growing_future_value(rate(0.05), growth(0.02), Period::ZERO, payment)
                    .unwrap()
                    .value(),
                0.0,
                1e-12,
            ));
        }
    }
}
