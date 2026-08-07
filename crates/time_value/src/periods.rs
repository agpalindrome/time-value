//! A length of time, counted in the periods a rate is stated against.

use crate::error::{Error, Quantity, Result};

/// How much time has elapsed, counted in the periods the rate is stated
/// against.
///
/// A *length* of time, not a point in one. Fractional values are ordinary
/// rather than exceptional: a money-market span of 91/360 of a year is the
/// shape of the domain, not an edge case.
///
/// # Examples
///
/// ```
/// use time_value::ElapsedPeriods;
///
/// let quarter = ElapsedPeriods::new(91.0 / 360.0)?;
/// assert!(ElapsedPeriods::new(0.0).is_ok());
///
/// // Negative spans are refused: running the formula backwards does not
/// // discount, it computes a convincing approximation of discounting.
/// assert!(ElapsedPeriods::new(-1.0).is_err());
/// # Ok::<(), time_value::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElapsedPeriods(f64);

/// Spans are never NaN.
impl Eq for ElapsedPeriods {}

impl ElapsedPeriods {
    /// Builds a span of elapsed periods.
    ///
    /// # Errors
    ///
    /// [`Error::NotFinite`] if `count` is a NaN or an infinity.
    /// [`Error::NegativePeriods`] if it is below zero — the inverse of simple
    /// accumulation is division, not evaluation at a negative argument, so a
    /// negative span would answer a different question than the one asked.
    pub fn new(count: f64) -> Result<Self> {
        // Sign before range: negative infinity is negative, which is the domain
        // failure this variant exists to name. Reporting it as out-of-range
        // would send a caller to a remedy that cannot help. NaN is not less
        // than zero, so it falls through to the range check.
        if count < 0.0 {
            return Err(Error::NegativePeriods { periods: count });
        }
        if !count.is_finite() {
            return Err(Error::NotFinite {
                quantity: Quantity::Periods,
            });
        }
        Ok(Self(count))
    }

    /// The span, as a number of periods.
    #[must_use]
    pub const fn count(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::ElapsedPeriods;
    use crate::error::{Error, Quantity};

    #[test]
    fn admits_fractional_spans() {
        // The ordinary case for simple interest, not an edge one.
        let span = ElapsedPeriods::new(91.0 / 360.0).expect("a partial period is a span");
        assert!(span.count() > 0.0 && span.count() < 1.0);
    }

    #[test]
    fn admits_zero() {
        // Required rather than merely permitted: a(0) = 1 is the criterion an
        // accumulation function is anchored on.
        ElapsedPeriods::new(0.0).expect("zero periods is in the domain");
    }

    #[test]
    fn refuses_negative_spans() {
        let error = ElapsedPeriods::new(-1.0).expect_err("a negative span is refused");
        assert!(matches!(error, Error::NegativePeriods { periods } if periods < 0.0));
    }

    #[test]
    fn refuses_non_values() {
        for count in [f64::NAN, f64::INFINITY] {
            assert!(matches!(
                ElapsedPeriods::new(count),
                Err(Error::NotFinite {
                    quantity: Quantity::Periods
                })
            ));
        }
    }

    #[test]
    fn negative_infinity_is_a_domain_failure_not_a_range_one() {
        // It is negative, which is knowable, and no wider representation makes
        // it non-negative. The two rules race here and the sign must win.
        let error = ElapsedPeriods::new(f64::NEG_INFINITY).expect_err("negative is refused");
        assert!(matches!(error, Error::NegativePeriods { .. }), "{error:?}");
    }
}
