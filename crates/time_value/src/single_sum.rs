//! Present and future value of a single amount.
//!
//! Both functions compound with `powf`; on extreme rate/period magnitudes the
//! result can overflow to a non-finite [`Money`] (see its docs).

use crate::math::{ln, powf};
use crate::{FutureValue, Money, Period, Periodicity, PresentValue, Rate, TvmError};

/// The present value of a single `future` amount received `periods` periods from
/// now, discounted at `rate`: `PV = FV / (1 + r)ⁿ`.
///
/// Unlike the discrete [`Cashflows`](crate::Cashflows) operations, this admits a
/// **fractional** number of periods, so it needs `powf` and is available only
/// with the `std` or `libm` feature
/// (`docs/adr/0014-transcendental-single-sum-operations.md`).
///
/// # Examples
///
/// ```
/// use time_value::{single_sum, Money, Monthly, Period, Rate};
///
/// // 1000 a year out, at 1% per month, is worth ~887.45 today.
/// let pv = single_sum::present_value(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(1000.0)?,
/// )?;
/// assert!((pv.value() - 887.449).abs() < 1e-3);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the
/// discounting overflows to a non-finite value on extreme rate/period magnitudes
/// (ADR-0021).
pub fn present_value<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    future: Money,
) -> Result<Money, TvmError> {
    let growth = powf(1.0 + rate.value(), periods.value());
    Money::from_operation(future.value() / growth, future.currency())
}

/// The future value of a single `present` amount after `periods` periods,
/// compounded at `rate`: `FV = PV (1 + r)ⁿ`.
///
/// Admits a **fractional** number of periods; available with the `std` or `libm`
/// feature (`docs/adr/0014-transcendental-single-sum-operations.md`).
///
/// # Examples
///
/// ```
/// use time_value::{single_sum, Money, Monthly, Period, Rate};
///
/// // 1000 today, at 1% per month for a year, grows to ~1126.83.
/// let fv = single_sum::future_value(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(1000.0)?,
/// )?;
/// assert!((fv.value() - 1126.825).abs() < 1e-3);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// [`TvmError::Overflow`] if the
/// compounding overflows to a non-finite value on extreme rate/period magnitudes
/// (ADR-0021).
pub fn future_value<P: Periodicity>(
    rate: Rate<P>,
    periods: Period<P>,
    present: Money,
) -> Result<Money, TvmError> {
    let growth = powf(1.0 + rate.value(), periods.value());
    Money::from_operation(present.value() * growth, present.currency())
}

/// The number of periods for a single `present` amount to grow to `future` at
/// `rate` — [`future_value`] / [`present_value`] solved for `n` (the single-sum
/// NPER).
///
/// `n = ln(FV / PV) / ln(1 + r)`. A zero rate is rejected: with no growth, no
/// finite `n` maps `PV` to a different `FV`.
///
/// # Examples
///
/// ```
/// use time_value::{single_sum, FutureValue, Money, Monthly, Period, PresentValue, Rate};
///
/// // How long for 1000 to reach ~1126.83 at 1% per month? A year.
/// let n = single_sum::periods(
///     Rate::<Monthly>::new(0.01)?,
///     PresentValue(Money::agnostic(1000.0)?),
///     FutureValue(Money::agnostic(1126.825)?),
/// )?;
/// assert!((n.value() - 12.0).abs() < 1e-2);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// The two amounts are role-tagged, so a transposed call does not compile — and
/// note that this argument order is *not* the one [`annuity::periods`] takes,
/// which is exactly why the roles are spelled out (ADR-0050):
///
/// ```compile_fail
/// use time_value::{single_sum, FutureValue, Money, Monthly, PresentValue, Rate};
///
/// let _ = single_sum::periods(
///     Rate::<Monthly>::new(0.01).unwrap(),
///     FutureValue(Money::agnostic(1126.825).unwrap()), // future where present goes
///     PresentValue(Money::agnostic(1000.0).unwrap()),
/// );
/// ```
///
/// [`annuity::periods`]: crate::annuity::periods
///
/// # Errors
///
/// - [`TvmError::NoRealSolution`] if `rate` is zero (nothing grows, so no `n`
///   reaches `future`) or `future / present` is not a positive finite number
///   (including a zero `present`), so the logarithm has no real value (ADR-0052).
/// - [`TvmError::NegativePeriods`] if the solved `n` is negative — `future` lies
///   *before* `present` at this rate (e.g. `future < present` with a positive
///   rate).
pub fn periods<P: Periodicity>(
    rate: Rate<P>,
    present: PresentValue,
    future: FutureValue,
) -> Result<Period<P>, TvmError> {
    let ratio = future.money().value() / present.money().value();
    if rate.value() == 0.0 || !ratio.is_finite() || ratio <= 0.0 {
        // No growth (rate 0), or a ratio with no real logarithm: the solve has no
        // real answer. Both conditions share a variant because both mean exactly
        // that — there is no `n`, for different arithmetic reasons (ADR-0052).
        return Err(TvmError::NoRealSolution);
    }
    let n = ln(ratio) / ln(1.0 + rate.value());
    Period::from_operation(n)
}

/// The per-period rate at which a single `present` amount grows to `future` over
/// `periods` — [`future_value`] / [`present_value`] solved for `r` (the
/// single-sum RATE).
///
/// `r = (FV / PV)^(1/n) − 1`. The scalar inputs carry no periodicity, so the
/// caller names it: `single_sum::rate::<Monthly>(…)`.
///
/// # Examples
///
/// ```
/// use time_value::{single_sum, FutureValue, Money, Monthly, Period, PresentValue, Rate};
///
/// // What monthly rate grows 1000 to ~1126.83 over a year? About 1%.
/// let r = single_sum::rate::<Monthly>(
///     Period::new(12.0)?,
///     PresentValue(Money::agnostic(1000.0)?),
///     FutureValue(Money::agnostic(1126.825)?),
/// )?;
/// assert!((r.value() - 0.01).abs() < 1e-4);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Errors
///
/// - [`TvmError::ZeroPeriods`] if `periods` is zero (no elapsed time, so no rate
///   is implied) — ADR-0052.
/// - [`TvmError::Overflow`] if the power overflows on extreme magnitudes.
/// - [`TvmError::RateOutOfRange`] if the implied growth factor `(FV / PV)^(1/n)`
///   is non-positive — e.g. `future / present` is negative — so the rate would be
///   `≤ −100%`.
pub fn rate<P: Periodicity>(
    periods: Period<P>,
    present: PresentValue,
    future: FutureValue,
) -> Result<Rate<P>, TvmError> {
    if periods.value() <= 0.0 {
        return Err(TvmError::ZeroPeriods);
    }
    let growth = powf(
        future.money().value() / present.money().value(),
        1.0 / periods.value(),
    );
    Rate::from_operation(growth - 1.0)
}

#[cfg(test)]
mod tests {
    use super::{future_value, periods, present_value, rate as solve_rate};
    use crate::{FutureValue, Money, Monthly, Period, PresentValue, Rate, TvmError};

    /// `no_std`-safe approximate equality (no `f64::abs`).
    fn approx(a: f64, b: f64) -> bool {
        let d = a - b;
        d < 1e-6 && d > -1e-6
    }

    fn setup() -> (Rate<Monthly>, Period<Monthly>, Money) {
        (
            Rate::<Monthly>::new(0.01).unwrap(),
            Period::new(12.0).unwrap(),
            Money::agnostic(1000.0).unwrap(),
        )
    }

    #[test]
    fn present_and_future_value_are_inverses() {
        let (rate, periods, amount) = setup();
        let fv = future_value(rate, periods, amount).unwrap();
        let back = present_value(rate, periods, fv).unwrap();
        assert!(approx(back.value(), amount.value()));
    }

    #[test]
    fn future_value_matches_manual_compounding() {
        let (rate, periods, amount) = setup();
        let mut expected = amount.value();
        for _ in 0..12 {
            expected *= 1.01;
        }
        assert!(approx(
            future_value(rate, periods, amount).unwrap().value(),
            expected
        ));
    }

    #[test]
    fn zero_periods_is_the_amount_itself() {
        let (rate, _, amount) = setup();
        let now = Period::ZERO;
        assert!(approx(
            present_value(rate, now, amount).unwrap().value(),
            amount.value()
        ));
        assert!(approx(
            future_value(rate, now, amount).unwrap().value(),
            amount.value()
        ));
    }

    #[test]
    fn present_value_below_face_for_positive_rate() {
        let (rate, periods, amount) = setup();
        assert!(present_value(rate, periods, amount).unwrap().value() < amount.value());
    }

    #[test]
    fn future_value_overflow_is_reported() {
        // 2^2000 is well past f64::MAX, so compounding overflows — an error, not
        // a silent `inf` (ADR-0021).
        let rate = Rate::<Monthly>::new(1.0).unwrap(); // 100% per period
        let periods = Period::new(2000.0).unwrap();
        let amount = Money::agnostic(1e6).unwrap();
        assert_eq!(future_value(rate, periods, amount), Err(TvmError::Overflow));
    }

    #[test]
    fn periods_inverts_future_value() {
        let (rate, n, present) = setup();
        let future = future_value(rate, n, present).unwrap();
        let recovered = periods(rate, PresentValue(present), FutureValue(future)).unwrap();
        assert!(approx(recovered.value(), n.value()));
    }

    #[test]
    fn rate_inverts_future_value() {
        let (r, n, present) = setup();
        let future = future_value(r, n, present).unwrap();
        let recovered =
            solve_rate::<Monthly>(n, PresentValue(present), FutureValue(future)).unwrap();
        assert!(approx(recovered.value(), r.value()));
    }

    #[test]
    fn periods_with_zero_rate_has_no_real_solution() {
        // No growth, so no finite n maps 1000 to 2000.
        let rate = Rate::<Monthly>::new(0.0).unwrap();
        assert_eq!(
            periods(
                rate,
                PresentValue(Money::agnostic(1000.0).unwrap()),
                FutureValue(Money::agnostic(2000.0).unwrap())
            ),
            Err(TvmError::NoRealSolution)
        );
    }

    #[test]
    fn periods_with_a_non_positive_ratio_has_no_real_solution() {
        // ln(FV/PV) needs a positive ratio: a sign flip, or a zero `present`
        // (an infinite ratio), has no real answer (ADR-0052).
        let rate = Rate::<Monthly>::new(0.01).unwrap();
        assert_eq!(
            periods(
                rate,
                PresentValue(Money::agnostic(1000.0).unwrap()),
                FutureValue(Money::agnostic(-2000.0).unwrap())
            ),
            Err(TvmError::NoRealSolution)
        );
        assert_eq!(
            periods(
                rate,
                PresentValue(Money::ZERO),
                FutureValue(Money::agnostic(2000.0).unwrap())
            ),
            Err(TvmError::NoRealSolution)
        );
    }

    #[test]
    fn periods_for_a_future_below_present_is_negative() {
        // At a positive rate, reaching 500 from 1000 is in the past.
        let rate = Rate::<Monthly>::new(0.01).unwrap();
        assert_eq!(
            periods(
                rate,
                PresentValue(Money::agnostic(1000.0).unwrap()),
                FutureValue(Money::agnostic(500.0).unwrap())
            ),
            Err(TvmError::NegativePeriods)
        );
    }

    #[test]
    fn rate_over_zero_periods_is_a_zero_term() {
        assert_eq!(
            solve_rate::<Monthly>(
                Period::ZERO,
                PresentValue(Money::agnostic(1000.0).unwrap()),
                FutureValue(Money::agnostic(2000.0).unwrap()),
            ),
            Err(TvmError::ZeroPeriods)
        );
    }
}
