//! [`Period`] — a periodicity-tagged count of periods.

use core::fmt;
use core::marker::PhantomData;

use crate::{Periodicity, TvmError};

/// A number of periods, tagged with its [`Periodicity`].
///
/// `Period<Monthly>` is "how many *monthly* periods", so it shares the crate's one
/// compile-time axis with [`Rate<P>`](crate::Rate) and
/// [`Cashflows<P>`](crate::Cashflows): an operation that pairs a rate with a
/// duration requires the **same** `P` on both, so a `Period<Annual>` used with a
/// `Rate<Monthly>` is a compile error rather than a silent "annual meant, monthly
/// computed" bug (`docs/adr/0035-periodicity-tagged-time.md`).
///
/// Because operations take an accompanying `Rate<P>`, the periodicity is usually
/// **inferred** from the rate, so `Period::new(12.0)` reads the same as before at
/// most call sites; name it explicitly (`Period::<Monthly>::new(12.0)`) only where
/// there is nothing else to infer from.
///
/// A count may be fractional (e.g. `1.5` periods); it is always finite and
/// non-negative.
///
/// `Period` is available with the `std` or `libm` feature, alongside the
/// operations that consume it (`docs/adr/0014-transcendental-single-sum-operations.md`).
///
/// ```
/// use time_value::{Monthly, Period, Rate, single_sum, Money};
///
/// // The periodicity is inferred from the rate — both are `Monthly`.
/// let fv = single_sum::future_value(
///     Rate::<Monthly>::new(0.01)?,
///     Period::new(12.0)?,
///     Money::agnostic(1000.0)?,
/// )?;
/// assert!((fv.value() - 1126.825).abs() < 1e-3);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// A duration of the wrong periodicity does not type-check:
///
/// ```compile_fail
/// use time_value::{Annual, Monthly, Period, Rate, single_sum, Money};
///
/// let _ = single_sum::future_value(
///     Rate::<Monthly>::new(0.01).unwrap(),
///     Period::<Annual>::new(1.0).unwrap(), // annual duration, monthly rate — won't compile
///     Money::agnostic(1000.0).unwrap(),
/// );
/// ```
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Period<P: Periodicity> {
    count: f64,
    marker: PhantomData<P>,
}

impl<P: Periodicity> Period<P> {
    /// No periods, at any periodicity.
    pub const ZERO: Self = Self::from_valid(0.0);

    /// Wraps a period `count`.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::NegativePeriods`] if `count` is negative or not finite.
    pub fn new(count: f64) -> Result<Self, TvmError> {
        if count.is_finite() && count >= 0.0 {
            Ok(Self::from_valid(count))
        } else {
            Err(TvmError::NegativePeriods)
        }
    }

    /// The number of periods as a plain `f64`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.count
    }

    /// The number of periods of this count's periodicity in one year.
    #[must_use]
    pub const fn periods_per_year(self) -> u16 {
        P::PERIODS_PER_YEAR
    }

    /// Wraps a count already known to be valid (finite and non-negative), skipping
    /// the check — for `const` construction and internal callers that have
    /// validated the value.
    pub(crate) const fn from_valid(count: f64) -> Self {
        Self {
            count,
            marker: PhantomData,
        }
    }

    /// Constructs from the `f64` result of a solve (e.g. a solved NPER),
    /// distinguishing a non-finite result from a finite but negative one.
    ///
    /// A non-finite value reaching here is a solved count that overflowed the
    /// representable range, so it is [`TvmError::Overflow`] — the mirror of
    /// [`Money::from_operation`](crate::Money) and [`Rate::from_operation`] (per
    /// ADR-0021, ADR-0031); the mathematically undefined solves (a zero rate, a
    /// non-positive logarithm argument) are guarded at their call sites and return
    /// [`TvmError::NoRealSolution`] or [`TvmError::PaymentDoesNotAmortize`] before
    /// reaching here (ADR-0052). A finite negative count — a
    /// period count solved into the past — is [`TvmError::NegativePeriods`], the
    /// same variant [`new`](Self::new) uses.
    ///
    /// [`Rate::from_operation`]: crate::Rate
    pub(crate) fn from_operation(count: f64) -> Result<Self, TvmError> {
        if count.is_finite() {
            Self::new(count)
        } else {
            Err(TvmError::Overflow)
        }
    }
}

/// The default `Period` is [`ZERO`](Period::ZERO) (ADR-0032).
impl<P: Periodicity> Default for Period<P> {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Fallibly wraps an `f64` count, mirroring [`Period::new`] (ADR-0032). The
/// periodicity is inferred from context.
///
/// # Errors
///
/// Returns [`TvmError::NegativePeriods`] if the value is negative or not finite.
impl<P: Periodicity> TryFrom<f64> for Period<P> {
    type Error = TvmError;

    fn try_from(count: f64) -> Result<Self, Self::Error> {
        Self::new(count)
    }
}

/// Formats the bare count; the periodicity tag is a compile-time concern and is
/// not shown.
impl<P: Periodicity> fmt::Debug for Period<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Period")
            .field("count", &self.count)
            .field("periodicity", &P::NAME)
            .finish()
    }
}

impl<P: Periodicity> fmt::Display for Period<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.count, P::NAME)
    }
}

#[cfg(test)]
mod tests {
    // Exactly-representable round-trips, so exact `==` is correct here.
    #![allow(clippy::float_cmp)]

    use crate::{Annual, Monthly, Period, TvmError};

    #[test]
    fn accepts_non_negative_finite_counts() {
        assert_eq!(Period::<Monthly>::new(0.0).unwrap().value(), 0.0);
        assert_eq!(Period::<Monthly>::new(12.0).unwrap().value(), 12.0);
        assert_eq!(Period::<Monthly>::new(1.5).unwrap().value(), 1.5);
    }

    #[test]
    fn rejects_negative_or_non_finite_counts() {
        assert_eq!(Period::<Monthly>::new(-1.0), Err(TvmError::NegativePeriods));
        assert_eq!(
            Period::<Monthly>::new(f64::NAN),
            Err(TvmError::NegativePeriods)
        );
        assert_eq!(
            Period::<Monthly>::new(f64::INFINITY),
            Err(TvmError::NegativePeriods)
        );
    }

    #[test]
    fn periods_per_year_comes_from_the_tag() {
        assert_eq!(Period::<Monthly>::ZERO.periods_per_year(), 12);
        assert_eq!(Period::<Annual>::ZERO.periods_per_year(), 1);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Period::<Monthly>::default(), Period::<Monthly>::ZERO);
    }

    #[test]
    fn try_from_mirrors_new() {
        assert_eq!(Period::<Monthly>::try_from(12.0).unwrap().value(), 12.0);
        assert_eq!(
            Period::<Monthly>::try_from(-1.0),
            Err(TvmError::NegativePeriods)
        );
        let n: Period<Monthly> = 3.0.try_into().unwrap();
        assert_eq!(n.value(), 3.0);
    }
}
