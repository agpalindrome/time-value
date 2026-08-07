//! A quantity of money located at a point in time.

use core::{cmp::Ordering, fmt, str::FromStr};

use crate::{
    error::{Error, Quantity, Result},
    tolerance::Tolerance,
};

/// A quantity of money.
///
/// The location in time is *not* carried here — it is supplied by whatever
/// operates on the value. That is what lets one type serve as both the present
/// and the future value of a calculation.
///
/// No currency is carried yet either. When one arrives, two Amounts recording
/// different currencies will not compare, which is why this type implements
/// [`PartialOrd`] and deliberately **not** [`Ord`]: a total order would have to
/// be withdrawn to keep that promise, and withdrawing a trait is a breaking
/// change. Equality is unaffected — "are these the same?" has an answer across
/// currencies, "which is larger?" does not.
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

/// Ordering, but not a total one.
///
/// Today every Amount compares against every other, so this never returns
/// `None`. It is `PartialOrd` rather than `Ord` because that stops being true
/// the moment an Amount records a currency, and a promise withdrawn later is a
/// breaking change while one never made is free.
///
/// The comparison is the IEEE one, matching [`PartialEq`], rather than
/// `f64::total_cmp` — which would place negative zero below zero while equality
/// calls them the same sum of money.
impl PartialOrd for Amount {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
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
            Err(Error::NotFinite {
                quantity: Quantity::Amount,
            })
        }
    }

    /// The magnitude, as a number.
    ///
    /// The escape hatch. A value type nobody can get a value out of does not
    /// get used, so this exists — but it is a deliberate hole and worth naming
    /// as one.
    ///
    /// Today it is harmless: no Amount records a currency, so the number is the
    /// whole value. **When one does, this returns a value stripped of its unit
    /// while every existing call site goes on compiling** — and adding two
    /// magnitudes drawn from different currencies is exactly the mistake the
    /// type exists to prevent. At that point this should be renamed to
    /// something that reads wrong at a call site doing arithmetic with it; a
    /// name is the only warning available once the number is out.
    ///
    /// Reach for an operation before reaching for this. [`Amount::ratio_to`]
    /// exists because this crate's own test was using the hatch to divide two
    /// Amounts, which was evidence the operation was needed rather than that
    /// the hatch was fine.
    #[must_use]
    pub const fn magnitude(self) -> f64 {
        self.0
    }

    /// How many times this Amount the other is — a dimensionless ratio.
    ///
    /// `FV/PV` is the accumulation factor reached from the other direction,
    /// which is what makes this the route to solving for a rate or a span.
    ///
    /// The result is a bare number, not a
    /// [`SimpleAccumulationFactor`](crate::SimpleAccumulationFactor): a ratio
    /// of two Amounts may be negative or zero, and a factor may not.
    ///
    /// # Errors
    ///
    /// [`Error::ZeroDivisor`] if `other` is zero — the ratio to nothing is
    /// undefined rather than large, so it is a domain failure and not a range
    /// one. [`Error::NotFinite`] if the ratio overflows, which is a range one.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::Amount;
    ///
    /// let future = Amount::new(115.0)?;
    /// let present = Amount::new(100.0)?;
    /// assert!((future.ratio_to(present)? - 1.15).abs() < 1e-12);
    ///
    /// assert!(future.ratio_to(Amount::new(0.0)?).is_err());
    /// # Ok::<(), time_value::Error>(())
    /// ```
    pub fn ratio_to(self, other: Self) -> Result<f64> {
        if other.0 == 0.0 {
            return Err(Error::ZeroDivisor);
        }
        let ratio = self.0 / other.0;
        if ratio.is_finite() {
            Ok(ratio)
        } else {
            Err(Error::NotFinite {
                quantity: Quantity::Product,
            })
        }
    }

    /// Whether two Amounts agree to within a [`Tolerance`].
    ///
    /// This is **not** equality and must not be used as one: it is not
    /// transitive, so it cannot underpin sorting, deduplication or lookup. It
    /// answers whether arithmetic drifted, which is what a test comparing a
    /// computed result against an expected one is actually asking.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{Amount, Tolerance};
    ///
    /// let computed = Amount::new(0.1 + 0.2)?;
    /// let expected = Amount::new(0.3)?;
    /// let tolerance = Tolerance::absolute(1e-12)?.and_relative(1e-12)?;
    ///
    /// assert_ne!(computed, expected); // exact equality says no
    /// assert!(computed.is_close(expected, tolerance)); // drift says yes
    ///
    /// # Ok::<(), time_value::Error>(())
    /// ```
    #[must_use]
    pub fn is_close(self, other: Self, tolerance: Tolerance) -> bool {
        let difference = (self.0 - other.0).abs();
        tolerance.permits(difference, self.0.abs().max(other.0.abs()))
    }
}

/// Renders the value exactly: the text reads back as the same Amount.
///
/// Deliberately not a presentation format. There is no currency symbol, no
/// digit grouping and no rounding, because this library has no rounding rule to
/// apply — `0.1 + 0.2` renders as `0.30000000000000004` rather than hiding the
/// representation error behind two decimal places.
///
/// Exact, not *short*: this never uses exponent notation, so a subnormal runs
/// to several hundred characters. Round-tripping is the guarantee; brevity is
/// not.
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
    /// [`Error::Unparsable`] if the text is not a number, carrying the
    /// underlying reason — which distinguishes empty text from malformed text.
    /// [`Error::NotFinite`] if it is a number this type does not admit.
    fn from_str(text: &str) -> Result<Self> {
        match text.trim().parse::<f64>() {
            Ok(magnitude) => Self::new(magnitude),
            Err(source) => Err(Error::Unparsable { source }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Amount;
    use crate::{
        error::{Error, Quantity},
        tolerance::Tolerance,
    };

    fn tolerance(absolute: f64, relative: f64) -> Tolerance {
        Tolerance::absolute(absolute)
            .and_then(|t| t.and_relative(relative))
            .expect("a valid tolerance")
    }

    #[test]
    fn refuses_non_values() {
        for magnitude in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                Amount::new(magnitude),
                Err(Error::NotFinite {
                    quantity: Quantity::Amount
                })
            ));
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
        // agree with that, which is why `total_cmp` is not used.
        let negative = Amount::new(-0.0).expect("valid");
        let positive = Amount::new(0.0).expect("valid");
        assert_eq!(negative, positive);
        assert_eq!(
            negative.partial_cmp(&positive),
            Some(core::cmp::Ordering::Equal)
        );
    }

    #[test]
    fn orders_within_a_unit() {
        // `sort_by` rather than `sort_unstable`: this is deliberately
        // `PartialOrd` and not `Ord`, so that a currency can be added without
        // withdrawing a promise.
        let mut amounts = [
            Amount::new(3.0).expect("valid"),
            Amount::new(-1.0).expect("valid"),
            Amount::new(2.0).expect("valid"),
        ];
        amounts.sort_by(|a, b| a.partial_cmp(b).expect("no Amount records a currency yet"));
        assert_eq!(amounts[0], Amount::new(-1.0).expect("valid"));
        assert_eq!(amounts[2], Amount::new(3.0).expect("valid"));
    }

    #[test]
    fn equality_is_exact_not_approximate() {
        let one_way = Amount::new(0.1 + 0.2).expect("valid");
        let other_way = Amount::new(0.3).expect("valid");
        assert_ne!(one_way, other_way);
        assert!(one_way.is_close(other_way, tolerance(1e-12, 1e-12)));
    }

    #[test]
    fn an_absolute_tolerance_is_what_carries_zero() {
        let computed = Amount::new(1e-300).expect("valid");
        let zero = Amount::new(0.0).expect("valid");
        // A relative tolerance alone always rejects here: the relative error
        // against zero is 1, whatever the magnitudes.
        assert!(!computed.is_close(zero, Tolerance::relative(1e-9).expect("valid")));
        assert!(computed.is_close(zero, Tolerance::absolute(1e-12).expect("valid")));
    }

    #[test]
    fn a_relative_tolerance_is_what_carries_scale() {
        let big = Amount::new(1e20).expect("valid");
        let nudged = Amount::new(1e20 + 16_384.0).expect("valid");
        // One representable step apart near 1e20 is about 16384, so an absolute
        // tolerance of 0.01 sits below a single step and degenerates into exact
        // equality.
        assert!(!nudged.is_close(big, Tolerance::absolute(0.01).expect("valid")));
        assert!(nudged.is_close(big, Tolerance::relative(1e-9).expect("valid")));
    }

    #[test]
    fn opposite_extremes_are_never_close() {
        // Their difference overflows. Without a guard both sides of the
        // comparison reach infinity and `inf <= inf` calls them close.
        let high = Amount::new(f64::MAX).expect("valid");
        let low = Amount::new(-f64::MAX).expect("valid");
        assert!(!high.is_close(low, Tolerance::relative(1.5).expect("valid")));
    }

    #[test]
    fn is_close_is_reflexive_and_symmetric() {
        let a = Amount::new(1e6).expect("valid");
        let b = Amount::new(1_000_001.0).expect("valid");
        let t = tolerance(1e-12, 1e-3);
        assert!(a.is_close(a, t));
        assert_eq!(a.is_close(b, t), b.is_close(a, t));
    }

    #[test]
    fn is_close_is_not_transitive() {
        // Which is exactly why it cannot serve as equality, and why the type
        // carries the warning it does.
        let (a, b, c) = (
            Amount::new(1.0).expect("valid"),
            Amount::new(1.9).expect("valid"),
            Amount::new(2.8).expect("valid"),
        );
        let t = Tolerance::absolute(1.0).expect("valid");
        assert!(a.is_close(b, t) && b.is_close(c, t));
        assert!(!a.is_close(c, t));
    }

    #[test]
    fn a_ratio_recovers_how_many_times_one_amount_the_other_is() {
        let future = Amount::new(115.0).expect("valid");
        let present = Amount::new(100.0).expect("valid");
        let ratio = future.ratio_to(present).expect("100 is not zero");
        assert!((ratio - 1.15).abs() < 1e-12, "got {ratio}");
    }

    #[test]
    fn a_ratio_to_zero_is_undefined_not_large() {
        // A domain failure: no wider representation answers it. Reported
        // differently from a ratio that merely overflows.
        let error = Amount::new(1.0)
            .expect("valid")
            .ratio_to(Amount::new(0.0).expect("valid"))
            .expect_err("the ratio to nothing is undefined");
        assert!(matches!(error, Error::ZeroDivisor), "{error:?}");
    }

    #[test]
    fn a_ratio_that_overflows_is_a_range_failure() {
        let error = Amount::new(f64::MAX)
            .expect("valid")
            .ratio_to(Amount::new(f64::MIN_POSITIVE).expect("valid"))
            .expect_err("MAX / MIN_POSITIVE does not fit");
        assert!(matches!(error, Error::NotFinite { .. }), "{error:?}");
    }

    #[test]
    fn renders_exactly_rather_than_tidily() {
        let awkward = Amount::new(0.1 + 0.2).expect("valid");
        assert_eq!(awkward.to_string(), "0.30000000000000004");
    }

    #[test]
    fn rendering_is_exact_but_not_short() {
        // The guarantee is that it reads back, not that it is brief: `Display`
        // for f64 never uses exponent notation.
        let subnormal = Amount::new(5e-324).expect("valid");
        let rendered = subnormal.to_string();
        assert!(rendered.len() > 300, "expected a long rendering");
        assert_eq!(rendered.parse::<Amount>().expect("reads back"), subnormal);
    }

    #[test]
    fn rejects_text_that_is_not_a_number() {
        assert!(matches!(
            "twelve".parse::<Amount>(),
            Err(Error::Unparsable { .. })
        ));
        assert!(matches!(
            "".parse::<Amount>(),
            Err(Error::Unparsable { .. })
        ));
        assert!(matches!(
            "NaN".parse::<Amount>(),
            Err(Error::NotFinite { .. })
        ));
        assert!(matches!(
            "inf".parse::<Amount>(),
            Err(Error::NotFinite { .. })
        ));
    }

    #[test]
    fn the_parse_failure_keeps_its_reason() {
        // Empty text and malformed text are different problems, and the source
        // is what tells them apart.
        use core::error::Error as _;
        let empty = "".parse::<Amount>().expect_err("empty is not a number");
        let malformed = "twelve".parse::<Amount>().expect_err("not a number");
        let empty_reason = empty.source().expect("a source").to_string();
        let malformed_reason = malformed.source().expect("a source").to_string();
        assert_ne!(empty_reason, malformed_reason);
    }
}
