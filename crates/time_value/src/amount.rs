//! A quantity of money located at a point in time.

use core::{cmp::Ordering, fmt, str::FromStr};

use crate::error::{Error, Result};

/// A quantity of money.
///
/// The location in time is *not* carried here — it is supplied by whatever
/// operates on the value. That is what lets one type serve as both the present
/// and the future value of a calculation.
///
/// No currency is carried either. Nothing in this library yet brings two
/// Amounts together, which is the only place a currency would be checked.
///
/// # Examples
///
/// ```
/// use time_value::Amount;
///
/// let amount = Amount::new(1_234.5)?;
/// assert_eq!(amount.to_string(), "1234.5");
///
/// // A liability is an ordinary Amount.
/// assert!(Amount::new(-40.0).is_ok());
///
/// // A non-value is not.
/// assert!(Amount::new(f64::NAN).is_err());
/// # Ok::<(), time_value::Error>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Amount(f64);

/// Amounts are never NaN, which is the only thing that would break reflexivity.
impl Eq for Amount {}

impl Ord for Amount {
    fn cmp(&self, other: &Self) -> Ordering {
        // Ordering must agree with equality, so this uses the IEEE comparison
        // that `PartialEq` derives rather than `total_cmp`, which would place
        // negative zero below zero while equality calls them the same.
        self.0
            .partial_cmp(&other.0)
            .expect("an Amount should be finite, so a comparison always answers")
    }
}

impl PartialOrd for Amount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Amount {
    /// Builds an Amount.
    ///
    /// # Errors
    ///
    /// [`Error::NotFinite`] if `magnitude` is a NaN or an infinity. Those are
    /// not quantities of money, and admitting one would let it propagate
    /// silently through every later operation.
    pub fn new(magnitude: f64) -> Result<Self> {
        if magnitude.is_finite() {
            Ok(Self(magnitude))
        } else {
            Err(Error::NotFinite)
        }
    }

    /// The magnitude, as a number.
    #[must_use]
    pub const fn magnitude(self) -> f64 {
        self.0
    }

    /// Whether two Amounts agree to within a tolerance.
    ///
    /// This is **not** equality and must not be used as one: it is not
    /// transitive, so it cannot underpin sorting, deduplication or lookup. It
    /// answers whether arithmetic drifted, which is what a test comparing a
    /// computed result against an expected one is actually asking.
    ///
    /// Both tolerances are required and neither is defaulted. `absolute`
    /// carries the comparison near zero, where a relative tolerance always
    /// rejects; `relative` carries it at scale, where an absolute tolerance
    /// falls below a single representable step and silently becomes equality.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::Amount;
    ///
    /// let computed = Amount::new(0.1 + 0.2)?;
    /// let expected = Amount::new(0.3)?;
    ///
    /// assert_ne!(computed, expected); // exact equality says no
    /// assert!(computed.is_close(expected, 1e-12, 1e-12)); // drift says yes
    ///
    /// # Ok::<(), time_value::Error>(())
    /// ```
    #[must_use]
    pub fn is_close(self, other: Self, absolute: f64, relative: f64) -> bool {
        let difference = (self.0 - other.0).abs();
        difference <= absolute.max(relative * self.0.abs().max(other.0.abs()))
    }
}

/// Renders exactly: the shortest text that reads back as the same value.
///
/// Deliberately not a presentation format. There is no currency symbol, no
/// digit grouping and no rounding, because this library has no rounding rule to
/// apply — `0.1 + 0.2` renders as `0.30000000000000004` rather than hiding the
/// representation error behind two decimal places.
impl fmt::Display for Amount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Amount {
    type Err = Error;

    /// Reads what [`Display`](fmt::Display) wrote.
    ///
    /// # Errors
    ///
    /// [`Error::Unparsable`] if the text is not a number, or
    /// [`Error::NotFinite`] if it is one this type does not admit.
    fn from_str(text: &str) -> Result<Self> {
        // `let else` rather than `map_err`: the underlying parse error says
        // only that the text was not a float, which this variant already
        // says, so there is no source worth threading through.
        let Ok(magnitude) = text.trim().parse::<f64>() else {
            return Err(Error::Unparsable);
        };
        Self::new(magnitude)
    }
}

#[cfg(test)]
mod tests {
    use super::Amount;
    use crate::error::Error;

    #[test]
    fn refuses_non_values() {
        for magnitude in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(Amount::new(magnitude), Err(Error::NotFinite)));
        }
    }

    #[test]
    fn admits_negative_and_zero() {
        Amount::new(-1_000.0).expect("a liability should be an Amount");
        Amount::new(0.0).expect("zero should be an Amount");
    }

    #[test]
    fn negative_zero_equals_zero() {
        // They differ in bits and are the same sum of money. Ordering has to
        // agree with that, which is why `total_cmp` is not used above.
        let negative = Amount::new(-0.0).expect("valid");
        let positive = Amount::new(0.0).expect("valid");
        assert_eq!(negative, positive);
        assert_eq!(negative.cmp(&positive), core::cmp::Ordering::Equal);
    }

    #[test]
    fn orders_totally() {
        let mut amounts = [
            Amount::new(3.0).expect("valid"),
            Amount::new(-1.0).expect("valid"),
            Amount::new(2.0).expect("valid"),
        ];
        amounts.sort_unstable();
        assert_eq!(amounts[0], Amount::new(-1.0).expect("valid"));
        assert_eq!(amounts[2], Amount::new(3.0).expect("valid"));
    }

    #[test]
    fn equality_is_exact_not_approximate() {
        // Two routes to the same real number land on different values, and
        // equality reports that honestly rather than papering over it.
        let one_way = Amount::new(0.1 + 0.2).expect("valid");
        let other_way = Amount::new(0.3).expect("valid");
        assert_ne!(one_way, other_way);
        assert!(one_way.is_close(other_way, 1e-12, 1e-12));
    }

    #[test]
    fn an_absolute_tolerance_is_what_carries_zero() {
        let computed = Amount::new(1e-300).expect("valid");
        let zero = Amount::new(0.0).expect("valid");
        // A relative tolerance alone always rejects here: the relative error
        // against zero is 1, whatever the magnitudes.
        assert!(!computed.is_close(zero, 0.0, 1e-9));
        assert!(computed.is_close(zero, 1e-12, 0.0));
    }

    #[test]
    fn a_relative_tolerance_is_what_carries_scale() {
        let big = Amount::new(1e20).expect("valid");
        let nudged = Amount::new(1e20 + 16_384.0).expect("valid");
        // One representable step apart near 1e20 is about 16384, so an
        // absolute tolerance of 0.01 sits below a single step and degenerates
        // into exact equality.
        assert!(!nudged.is_close(big, 0.01, 0.0));
        assert!(nudged.is_close(big, 0.0, 1e-9));
    }

    #[test]
    fn renders_exactly_rather_than_tidily() {
        let awkward = Amount::new(0.1 + 0.2).expect("valid");
        assert_eq!(awkward.to_string(), "0.30000000000000004");
    }

    #[test]
    fn rejects_text_that_is_not_a_number() {
        assert!(matches!("twelve".parse::<Amount>(), Err(Error::Unparsable)));
        assert!(matches!("".parse::<Amount>(), Err(Error::Unparsable)));
        assert!(matches!("NaN".parse::<Amount>(), Err(Error::NotFinite)));
        assert!(matches!("inf".parse::<Amount>(), Err(Error::NotFinite)));
    }
}
