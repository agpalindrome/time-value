//! The rate per period at which simple interest accrues.

use crate::error::{Error, Quantity, Result};

/// A simple interest rate, per time period.
///
/// Held as a fraction, because the formula requires one: `1 + rt` works with
/// `0.05`, and `5` for "5%" gives `1 + 15 = 16`.
///
/// The period is not recorded. This formula never consults it — a rate of
/// `0.05` per period over `3` periods gives the same answer whether a period is
/// a year or a month — and naming it would be a claim the source does not make.
///
/// # Examples
///
/// ```
/// use time_value::SimpleInterestRate;
///
/// // The two constructors exist so a call site says which it means. No type
/// // can tell 5% from 500%, since both are legal rates.
/// let from_fraction = SimpleInterestRate::from_fraction(0.05)?;
/// let from_percent = SimpleInterestRate::from_percent(5.0)?;
/// assert_eq!(from_fraction, from_percent);
///
/// // Negative rates are real and permitted.
/// assert!(SimpleInterestRate::from_fraction(-0.004).is_ok());
/// # Ok::<(), time_value::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimpleInterestRate(f64);

/// Rates are never NaN.
impl Eq for SimpleInterestRate {}

impl SimpleInterestRate {
    /// Builds a rate from a fraction, where `0.05` is five percent.
    ///
    /// # Errors
    ///
    /// [`Error::NotFinite`] if `fraction` is a NaN or an infinity. Any finite
    /// value is admitted: negative rates exist, zero means no interest, and
    /// nothing in the mathematics caps a rate from above.
    pub fn from_fraction(fraction: f64) -> Result<Self> {
        if fraction.is_finite() {
            Ok(Self(fraction))
        } else {
            Err(Error::NotFinite {
                quantity: Quantity::Rate,
            })
        }
    }

    /// Builds a rate from a percentage, where `5.0` is five percent.
    ///
    /// This exists because passing `5` where `0.05` was meant is a silent
    /// factor of one hundred that no type can catch — both are legal rates, so
    /// a validating constructor cannot reject either. Naming the two ways in
    /// makes the call site say which it means, which is the only defence
    /// available.
    ///
    /// # Errors
    ///
    /// [`Error::NotFinite`] if `percent` is a NaN or an infinity. Dividing a
    /// finite value by one hundred cannot itself be non-finite, so that is not
    /// a second failure.
    pub fn from_percent(percent: f64) -> Result<Self> {
        Self::from_fraction(percent / 100.0)
    }

    /// The rate as a fraction, where five percent reads back as `0.05`.
    #[must_use]
    pub const fn as_fraction(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::SimpleInterestRate;
    use crate::error::{Error, Quantity};

    #[test]
    fn a_percentage_is_a_hundredth_of_itself() {
        let percent = SimpleInterestRate::from_percent(5.0).expect("valid");
        let fraction = SimpleInterestRate::from_fraction(0.05).expect("valid");
        assert_eq!(percent, fraction);
    }

    #[test]
    fn admits_negative_zero_and_large() {
        for fraction in [-0.004, 0.0, 10.0] {
            SimpleInterestRate::from_fraction(fraction)
                .expect("any finite rate should be admitted");
        }
    }

    #[test]
    fn refuses_non_values() {
        for fraction in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                SimpleInterestRate::from_fraction(fraction),
                Err(Error::NotFinite {
                    quantity: Quantity::Rate,
                })
            ));
        }
    }

    #[test]
    fn five_hundred_percent_is_a_legal_rate() {
        // Which is exactly why no constructor can catch `5` passed for `0.05`.
        let rate = SimpleInterestRate::from_fraction(5.0).expect("500% is a rate");
        assert!((rate.as_fraction() - 5.0).abs() < f64::EPSILON);
    }
}
