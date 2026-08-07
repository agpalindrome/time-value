//! The multiplier by which simple interest grows an amount over a span.

use crate::{
    amount::Amount, error::Result, periods::ElapsedPeriods, rate::SimpleInterestRate, simple,
};

/// The accumulation factor `1 + rt`, for simple interest.
///
/// Strictly positive by construction, which is the whole reason it is a type
/// rather than an expression: the domain rule is established once, where the
/// value is built, instead of being re-checked wherever the formula appears.
///
/// It is also what inversion produces — `FV/PV` is this same factor reached
/// from the other direction.
///
/// # Examples
///
/// ```
/// use time_value::{
///     Amount, ElapsedPeriods, SimpleAccumulationFactor, SimpleInterestRate,
/// };
///
/// let rate = SimpleInterestRate::from_percent(5.0)?;
/// let periods = ElapsedPeriods::new(3.0)?;
///
/// let factor = SimpleAccumulationFactor::new(rate, periods)?;
/// let future = factor.apply(Amount::new(100.0)?)?;
/// assert!(future.is_close(Amount::new(115.0)?, 1e-9, 1e-9));
/// # Ok::<(), time_value::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimpleAccumulationFactor(f64);

/// Factors are never NaN.
impl Eq for SimpleAccumulationFactor {}

impl SimpleAccumulationFactor {
    /// Builds the factor `1 + rt`.
    ///
    /// This is the step that can fail on **domain** grounds, and the only one.
    ///
    /// # Errors
    ///
    /// [`Error::NonPositiveFactor`](crate::error::Error::NonPositiveFactor)
    /// when `1 + rt` is not strictly positive. Both arguments are
    /// individually valid — the failure is in the pair. Such a factor would
    /// flip the sign of whatever it multiplied, turning money
    /// held into money owed by nothing but elapsed time.
    ///
    /// [`Error::NotFinite`](crate::error::Error::NotFinite) if the factor
    /// itself leaves the representable range.
    pub fn new(rate: SimpleInterestRate, periods: ElapsedPeriods) -> Result<Self> {
        simple::accumulation_factor(rate.as_fraction(), periods.count()).map(Self)
    }

    /// Applies the factor to an amount, giving its value after the span.
    ///
    /// This is the step that can fail on **representation** grounds, and the
    /// only one. A factor is already known to be valid; what remains is whether
    /// the product fits.
    ///
    /// The result carries the sign of `present_value`: a liability accumulates
    /// into a larger liability, and a positive factor never turns one into an
    /// asset.
    ///
    /// # Errors
    ///
    /// [`Error::NotFinite`](crate::error::Error::NotFinite) if the product
    /// leaves the representable range.
    pub fn apply(self, present_value: Amount) -> Result<Amount> {
        Amount::new(present_value.magnitude() * self.0)
    }

    /// The factor, as a number.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }
}

/// `FV = PV(1 + rt)`, in one call.
///
/// Builds the factor and applies it. The two-step form exists because the two
/// failures have different causes and different remedies; this exists because
/// the common path is a one-liner and should read like one.
///
/// # Errors
///
/// Whatever [`SimpleAccumulationFactor::new`] or
/// [`SimpleAccumulationFactor::apply`] reports.
///
/// # Examples
///
/// ```
/// use time_value::{
///     Amount, ElapsedPeriods, SimpleInterestRate, future_value,
/// };
///
/// let future = future_value(
///     Amount::new(100.0)?,
///     SimpleInterestRate::from_percent(5.0)?,
///     ElapsedPeriods::new(3.0)?,
/// )?;
/// assert!(future.is_close(Amount::new(115.0)?, 1e-9, 1e-9));
/// # Ok::<(), time_value::Error>(())
/// ```
///
/// The three arguments are three distinct kinds, so no two can be transposed.
/// No role tags are needed to buy that — the distinctions already drawn between
/// an amount, a rate and a span pay for it:
///
/// ```compile_fail
/// use time_value::{Amount, ElapsedPeriods, SimpleInterestRate, future_value};
///
/// let future = future_value(
///     SimpleInterestRate::from_percent(5.0).unwrap(), // rate where an amount belongs
///     Amount::new(100.0).unwrap(),
///     ElapsedPeriods::new(3.0).unwrap(),
/// );
/// ```
pub fn future_value(
    present_value: Amount,
    rate: SimpleInterestRate,
    periods: ElapsedPeriods,
) -> Result<Amount> {
    SimpleAccumulationFactor::new(rate, periods)?.apply(present_value)
}

#[cfg(test)]
mod tests {
    use super::{SimpleAccumulationFactor, future_value};
    use crate::{amount::Amount, error::Error, periods::ElapsedPeriods, rate::SimpleInterestRate};

    fn rate(fraction: f64) -> SimpleInterestRate {
        SimpleInterestRate::from_fraction(fraction).expect("a finite rate")
    }

    fn periods(count: f64) -> ElapsedPeriods {
        ElapsedPeriods::new(count).expect("a non-negative span")
    }

    fn amount(magnitude: f64) -> Amount {
        Amount::new(magnitude).expect("a finite amount")
    }

    #[test]
    fn is_exactly_one_at_zero_periods() {
        let factor = SimpleAccumulationFactor::new(rate(0.05), periods(0.0)).expect("valid");
        assert!((factor.value() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn refuses_a_factor_that_would_flip_the_sign() {
        // Each argument is valid on its own; the pair is not. This is the
        // domain failure, and it is reported as one.
        let error = SimpleAccumulationFactor::new(rate(-0.5), periods(3.0))
            .expect_err("1 + rt would be -0.5");
        assert!(
            matches!(error, Error::NonPositiveFactor { .. }),
            "{error:?}"
        );
    }

    #[test]
    fn the_two_failures_are_distinguishable() {
        // The whole argument for splitting the operation. A valid factor whose
        // application overflows reports a representation failure, which is a
        // different thing from the domain failure above and says so.
        let factor = SimpleAccumulationFactor::new(rate(1.0), periods(1.0)).expect("2.0 is valid");
        let error = factor
            .apply(amount(f64::MAX))
            .expect_err("2 * MAX overflows");
        assert!(matches!(error, Error::NotFinite), "{error:?}");
    }

    #[test]
    fn a_zero_rate_leaves_the_amount_alone() {
        let future = future_value(amount(100.0), rate(0.0), periods(7.0)).expect("valid");
        assert!(future.is_close(amount(100.0), 1e-9, 1e-9), "{future}");
    }

    #[test]
    fn a_zero_amount_is_the_fixed_point() {
        let future = future_value(amount(0.0), rate(0.05), periods(3.0)).expect("valid");
        assert_eq!(future, amount(0.0));
    }

    #[test]
    fn a_liability_stays_a_liability() {
        let future = future_value(amount(-100.0), rate(0.05), periods(3.0)).expect("valid");
        assert!(future.is_close(amount(-115.0), 1e-9, 1e-9), "{future}");
        assert!(future < amount(0.0));
    }

    #[test]
    fn the_one_call_form_agrees_with_the_two_step_form() {
        let (present, r, t) = (amount(250.0), rate(0.04), periods(2.5));
        let stepwise = SimpleAccumulationFactor::new(r, t)
            .expect("valid")
            .apply(present)
            .expect("valid");
        let direct = future_value(present, r, t).expect("valid");
        assert_eq!(stepwise, direct);
    }

    #[test]
    fn dividing_recovers_the_factor() {
        let (present, r, t) = (amount(250.0), rate(0.04), periods(2.5));
        let future = future_value(present, r, t).expect("valid");
        let recovered = future.magnitude() / present.magnitude();
        let direct = SimpleAccumulationFactor::new(r, t).expect("valid");
        assert!((recovered - direct.value()).abs() < 1e-12);
    }
}
