//! [`Rate`] — a periodicity-tagged interest rate.

use core::cmp::Ordering;
use core::fmt;
use core::marker::PhantomData;

#[cfg(any(feature = "std", feature = "libm"))]
use crate::{math, Annual};
use crate::{Periodicity, TvmError};

/// A per-period interest rate, tagged with its [`Periodicity`].
///
/// `Rate<Monthly>` and `Rate<Annual>` are **distinct types**, so an operation
/// that discounts monthly cashflows will not accept an annual rate: the
/// periodicity mismatch is a compile error, not a silent arithmetic bug (see
/// `docs/adr/0005-domain-modelling-and-strong-typing.md`).
///
/// A `Rate` is always finite and strictly greater than `-1.0` (−100%); rates at
/// or below that are economically meaningless for discounting and compounding.
///
/// The value is the plain per-period rate: `0.01` is 1% per period.
///
/// Rates of the same periodicity are **totally ordered** by their per-period
/// value, so a solved rate can be compared with a threshold directly — see the
/// [`Ord`] impl (`docs/adr/0059-the-finite-scalars-are-totally-ordered.md`).
#[derive(Clone, Copy)]
pub struct Rate<P: Periodicity> {
    per_period: f64,
    marker: PhantomData<P>,
}

impl<P: Periodicity> Rate<P> {
    /// A rate of zero — no growth and no discounting — at any periodicity.
    pub const ZERO: Self = Self::from_valid(0.0);

    /// Wraps a per-period `rate` (e.g. `0.01` for 1% per period).
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::RateOutOfRange`] if `rate` is not finite or is
    /// `<= -1.0` (≤ −100%).
    pub fn new(rate: f64) -> Result<Self, TvmError> {
        if rate.is_finite() && rate > -1.0 {
            Ok(Self::from_valid(rate))
        } else {
            Err(TvmError::RateOutOfRange)
        }
    }

    /// The per-period rate as a plain `f64`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.per_period
    }

    /// The number of periods of this rate's periodicity in one year.
    #[must_use]
    pub const fn periods_per_year(self) -> u16 {
        P::PERIODS_PER_YEAR
    }

    /// Builds a `Rate<P>` from a *nominal annual* rate (an APR) quoted at this
    /// periodicity's compounding frequency.
    ///
    /// A nominal rate is a quoting convention, not a per-period rate: an APR of
    /// 12% "compounded monthly" means a `Rate<Monthly>` of `0.12 / 12 = 0.01`,
    /// *not* an effective annual 12%. This divides the nominal rate by the
    /// periods per year to recover that per-period rate.
    ///
    /// To move a per-period rate to a different *periodicity* while preserving
    /// its economic value, use [`convert`][Self::convert] instead — that is a
    /// different operation (effective, compounding-aware).
    ///
    /// ```
    /// use time_value::{Monthly, Rate};
    ///
    /// // 12% APR compounded monthly is 1% per month.
    /// let monthly = Rate::<Monthly>::from_nominal_annual(0.12)?;
    /// assert!((monthly.value() - 0.01).abs() < 1e-12);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::RateOutOfRange`] if the resulting per-period rate is
    /// not finite or is `<= -1.0` (the same domain [`new`](Self::new) enforces).
    #[cfg_attr(
        not(any(feature = "std", feature = "libm")),
        doc = "

[Self::convert]: https://docs.rs/time_value/latest/time_value/struct.Rate.html#method.convert
"
    )]
    pub fn from_nominal_annual(nominal: f64) -> Result<Self, TvmError> {
        Self::new(nominal / f64::from(P::PERIODS_PER_YEAR))
    }

    /// The *nominal annual* rate (APR) equivalent to this per-period rate at this
    /// periodicity's compounding frequency — the inverse of
    /// [`from_nominal_annual`](Self::from_nominal_annual).
    ///
    /// This simply scales by the periods per year (`0.01` monthly → `0.12`); it
    /// is deliberately **not** an effective annual rate and does **not** return a
    /// `Rate<Annual>`, because a nominal quote compounded monthly is not an
    /// annual per-period rate. For the effective annual rate — one that a
    /// `Rate<Annual>` may legitimately hold — use
    /// [`effective_annual`][Self::effective_annual].
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::Overflow`] if scaling leaves the finite range (only for
    /// an absurdly large rate); see `docs/adr/0021-fallible-operations-on-non-finite-results.md`.
    #[cfg_attr(
        not(any(feature = "std", feature = "libm")),
        doc = "

[Self::effective_annual]: https://docs.rs/time_value/latest/time_value/struct.Rate.html#method.effective_annual
"
    )]
    pub fn nominal_annual(self) -> Result<f64, TvmError> {
        let nominal = self.per_period * f64::from(P::PERIODS_PER_YEAR);
        if nominal.is_finite() {
            Ok(nominal)
        } else {
            Err(TvmError::Overflow)
        }
    }

    /// Constructs from an `f64` already known to satisfy the domain (internal use
    /// — e.g. a solved [`Cashflows::internal_rate_of_return`] result that is
    /// guarded to stay above −100%).
    ///
    /// [`Cashflows::internal_rate_of_return`]: crate::Cashflows::internal_rate_of_return
    pub(crate) const fn from_valid(rate: f64) -> Self {
        Self {
            per_period: rate,
            marker: PhantomData,
        }
    }

    /// Constructs from the `f64` result of an operation, validating the rate
    /// domain (finite and `> -1.0`).
    ///
    /// The non-finite case is [`TvmError::Overflow`], the mirror of
    /// [`Money::from_operation`](crate::Money) (ADR-0021, ADR-0031). A finite but
    /// out-of-domain result — an exponentiation that underflows `1 + r` to `0`,
    /// yielding exactly `-1.0` — is [`TvmError::RateOutOfRange`], the same variant
    /// [`new`](Self::new) uses, because such a rate is meaningless, not overflowed.
    #[cfg(any(feature = "std", feature = "libm"))]
    pub(crate) fn from_operation(rate: f64) -> Result<Self, TvmError> {
        if !rate.is_finite() {
            Err(TvmError::Overflow)
        } else if rate > -1.0 {
            Ok(Self::from_valid(rate))
        } else {
            Err(TvmError::RateOutOfRange)
        }
    }
}

/// Conversions between periodicities — effective, compounding-aware, and so
/// dependent on `powf` (behind `std` / `libm`; ADR-0024).
#[cfg(any(feature = "std", feature = "libm"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "libm"))))]
impl<P: Periodicity> Rate<P> {
    /// Converts to the equivalent rate at a different periodicity `Q`, preserving
    /// economic value: the two rates compound to the same amount over any horizon.
    ///
    /// This is the **effective** conversion `(1 + r)^(m / k) − 1`, where `m` and
    /// `k` are the periods per year of `P` and `Q`. Converting a rate to a coarser
    /// periodicity and back recovers it (up to floating-point rounding).
    ///
    /// ```
    /// use time_value::{Annual, Monthly, Rate};
    ///
    /// // 1% per month compounds to ~12.68% per year, not 12%.
    /// let monthly = Rate::<Monthly>::new(0.01)?;
    /// let annual = monthly.convert::<Annual>()?;
    /// assert!((annual.value() - 0.126825).abs() < 1e-6);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::Overflow`] if the conversion overflows `f64`
    /// (compounding a large rate to a much coarser periodicity), or
    /// [`TvmError::RateOutOfRange`] in the degenerate case where compounding a
    /// near-total-loss rate underflows `1 + r` to zero (yielding `-1.0`).
    pub fn convert<Q: Periodicity>(self) -> Result<Rate<Q>, TvmError> {
        let exponent = f64::from(P::PERIODS_PER_YEAR) / f64::from(Q::PERIODS_PER_YEAR);
        Rate::from_operation(math::powf(1.0 + self.per_period, exponent) - 1.0)
    }

    /// The **effective annual rate** (EAR) equivalent to this per-period rate —
    /// the annual rate that compounds to the same amount over a year. A shorthand
    /// for [`convert::<Annual>()`](Self::convert).
    ///
    /// Contrast [`nominal_annual`](Self::nominal_annual), which merely scales by
    /// the periods per year and is a quote rather than an economically equivalent
    /// annual rate.
    ///
    /// # Errors
    ///
    /// As [`convert`](Self::convert).
    pub fn effective_annual(self) -> Result<Rate<Annual>, TvmError> {
        self.convert::<Annual>()
    }
}

/// The default `Rate` is [`ZERO`](Rate::ZERO) at the inferred periodicity
/// (ADR-0032).
impl<P: Periodicity> Default for Rate<P> {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Fallibly wraps an `f64` per-period rate, mirroring [`Rate::new`]; the
/// periodicity is inferred from context (ADR-0032).
///
/// # Errors
///
/// Returns [`TvmError::RateOutOfRange`] if the value is not finite or `<= -1.0`.
impl<P: Periodicity> TryFrom<f64> for Rate<P> {
    type Error = TvmError;

    fn try_from(rate: f64) -> Result<Self, Self::Error> {
        Self::new(rate)
    }
}

/// Orders rates of the same periodicity by their per-period value.
///
/// The order is **total**, not partial: a `Rate` is finite by construction, so
/// `NaN` — the one `f64` that has no place in an ordering — is unrepresentable.
/// Comparing rates of *different* periodicities does not compile, so `Ord` never
/// crosses clocks (ADR-0059).
///
/// ```
/// use time_value::{Monthly, Rate};
///
/// let irr = Rate::<Monthly>::new(0.012)?;
/// let hurdle = Rate::<Monthly>::new(0.010)?;
///
/// assert!(irr > hurdle);
/// assert_eq!(irr.max(hurdle), irr);
/// assert_eq!(hurdle.clamp(Rate::ZERO, irr), hurdle);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// A monthly rate cannot be compared with an annual one — the periodicity tag
/// rejects it at compile time, exactly as it does for the arithmetic operations:
///
/// ```compile_fail
/// use time_value::{Annual, Monthly, Rate};
///
/// let monthly = Rate::<Monthly>::new(0.01).unwrap();
/// let annual = Rate::<Annual>::new(0.12).unwrap();
/// let _ = monthly > annual; // different periodicities — won't compile
/// ```
impl<P: Periodicity> Ord for Rate<P> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Two finite `f64`s always compare, so the fallback is unreachable: every
        // constructor rejects the non-finite values. `unwrap_or` rather than an
        // `expect` keeps the impl panic-free by construction.
        self.per_period
            .partial_cmp(&other.per_period)
            .unwrap_or(Ordering::Equal)
    }
}

/// Delegates to the total order on [`Ord`], so it never returns `None`.
impl<P: Periodicity> PartialOrd for Rate<P> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Two rates of the same periodicity are equal exactly when their per-period
/// values are — `+0.0` and `-0.0` included, as [`f64`] has it.
///
/// Hand-written rather than derived: a derived `PartialEq` would carry a
/// `P: PartialEq` bound, which the periodicity markers happen to satisfy but which
/// is not part of the [`Periodicity`] contract and blocks [`Ord`] (whose `Eq`
/// supertrait would inherit it). Routing equality through [`cmp`](Ord::cmp) also
/// keeps `clippy::float_cmp` satisfied without an allow: there is no `==` on
/// floats here.
impl<P: Periodicity> PartialEq for Rate<P> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

/// Equality on rates is a full equivalence relation: reflexivity is the only law a
/// float newtype can fail, and it cannot fail here because `NaN` is unrepresentable
/// (ADR-0059).
///
/// Deliberately **not** accompanied by [`Hash`](core::hash::Hash): `+0.0 == -0.0`
/// while their bit patterns differ, so a correct `Hash` would have to normalise the
/// zero, and nothing needs a hashed rate.
impl<P: Periodicity> Eq for Rate<P> {}

// `Debug`/`Display` are hand-written so the periodicity shows as its name rather
// than a `PhantomData`.
impl<P: Periodicity> fmt::Debug for Rate<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Rate")
            .field("per_period", &self.per_period)
            .field("periodicity", &P::NAME)
            .finish()
    }
}

impl<P: Periodicity> fmt::Display for Rate<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.per_period, P::NAME)
    }
}

#[cfg(test)]
mod tests {
    // These tests round-trip exactly-representable values, so exact `==` is
    // correct here.
    #![allow(clippy::float_cmp)]

    use crate::{Annual, Monthly, Rate, TvmError};

    #[test]
    fn accepts_rates_above_minus_one() {
        assert_eq!(Rate::<Monthly>::new(0.05).unwrap().value(), 0.05);
        assert_eq!(Rate::<Monthly>::new(-0.5).unwrap().value(), -0.5);
    }

    #[test]
    fn rejects_meaningless_rates() {
        assert_eq!(Rate::<Monthly>::new(-1.0), Err(TvmError::RateOutOfRange));
        assert_eq!(Rate::<Monthly>::new(-1.5), Err(TvmError::RateOutOfRange));
        assert_eq!(
            Rate::<Monthly>::new(f64::NAN),
            Err(TvmError::RateOutOfRange)
        );
    }

    #[test]
    fn periods_per_year_comes_from_the_tag() {
        assert_eq!(Rate::<Monthly>::new(0.01).unwrap().periods_per_year(), 12);
        assert_eq!(Rate::<Annual>::new(0.01).unwrap().periods_per_year(), 1);
    }

    #[test]
    fn nominal_annual_scales_by_periods_per_year() {
        // 12% APR compounded monthly is 1% per month, and back.
        let monthly = Rate::<Monthly>::from_nominal_annual(0.12).unwrap();
        assert!((monthly.value() - 0.01).abs() < 1e-12);
        assert!((monthly.nominal_annual().unwrap() - 0.12).abs() < 1e-12);
        // Annual periodicity is a no-op: one period per year.
        let annual = Rate::<Annual>::from_nominal_annual(0.08).unwrap();
        assert_eq!(annual.value(), 0.08);
        assert_eq!(annual.nominal_annual().unwrap(), 0.08);
    }

    #[test]
    fn from_nominal_annual_rejects_an_out_of_domain_result() {
        // A nominal of -12.0 over 12 monthly periods is a per-period rate of
        // exactly -1.0 (≤ -100%), which is out of the rate domain.
        assert_eq!(
            Rate::<Monthly>::from_nominal_annual(-12.0),
            Err(TvmError::RateOutOfRange)
        );
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn zero_and_default_are_a_zero_rate() {
        assert_eq!(Rate::<Monthly>::ZERO.value(), 0.0);
        assert_eq!(Rate::<Annual>::default(), Rate::<Annual>::ZERO);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn try_from_mirrors_new() {
        assert_eq!(Rate::<Monthly>::try_from(0.01).unwrap().value(), 0.01);
        assert_eq!(
            Rate::<Monthly>::try_from(-1.0),
            Err(TvmError::RateOutOfRange)
        );
        let r: Rate<Monthly> = 0.02.try_into().unwrap();
        assert_eq!(r.value(), 0.02);
    }

    /// The headline ergonomic the ordering exists for (issue #103): a solved rate
    /// compared with a threshold, without unwrapping to `f64` and losing the tag.
    #[test]
    fn a_rate_compares_with_a_threshold() {
        let irr = Rate::<Monthly>::new(0.012).unwrap();
        let hurdle = Rate::<Monthly>::new(0.010).unwrap();

        let same_irr = Rate::<Monthly>::new(0.012).unwrap();
        assert!(irr > hurdle);
        assert!(hurdle < irr);
        assert!(irr >= same_irr);
        assert!(irr <= same_irr);
        assert_eq!(irr.max(hurdle), irr);
        assert_eq!(irr.min(hurdle), hurdle);
        assert_eq!(irr.clamp(Rate::ZERO, hurdle), hurdle);
        assert!(Rate::<Monthly>::new(-0.5).unwrap() < Rate::ZERO);
    }

    /// `Ord` and `PartialOrd` are two views of one comparison, so they must agree —
    /// and `PartialOrd` never declines to answer, because the order is total.
    #[test]
    fn cmp_agrees_with_partial_cmp() {
        let values = [-0.9, -0.5, 0.0, 0.01, 1.0, 1e9];
        for &a in &values {
            for &b in &values {
                let (x, y) = (
                    Rate::<Monthly>::new(a).unwrap(),
                    Rate::<Monthly>::new(b).unwrap(),
                );
                assert_eq!(x.partial_cmp(&y), Some(x.cmp(&y)));
                assert_eq!(x.cmp(&y), a.partial_cmp(&b).unwrap());
            }
        }
    }

    /// `Ord` unlocks the slice sorts; the ordering must be the numeric one.
    #[test]
    fn rates_sort_by_value() {
        let mut rates = [
            Rate::<Monthly>::new(0.02).unwrap(),
            Rate::<Monthly>::new(-0.5).unwrap(),
            Rate::<Monthly>::new(0.01).unwrap(),
        ];
        rates.sort_unstable();
        assert_eq!(rates[0].value(), -0.5);
        assert_eq!(rates[1].value(), 0.01);
        assert_eq!(rates[2].value(), 0.02);
    }

    /// `Eq` promises equality is reflexive, and `Ord` promises `cmp` returns `Equal`
    /// exactly when `eq` is true. The one `f64` pair that could break the pairing is
    /// the signed zeros — equal as numbers, different in their bits — so pin it.
    #[test]
    fn the_signed_zeros_are_one_rate() {
        let plus = Rate::<Monthly>::new(0.0).unwrap();
        let minus = Rate::<Monthly>::new(-0.0).unwrap();

        let minus_again = Rate::<Monthly>::new(-0.0).unwrap();
        assert_eq!(plus, minus);
        assert_eq!(plus.cmp(&minus), core::cmp::Ordering::Equal);
        assert_eq!(minus, minus_again); // reflexivity, the law `Eq` adds
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    mod conversions {
        #![allow(clippy::float_cmp)]

        use crate::{Annual, Monthly, Quarterly, Rate, TvmError};

        #[test]
        fn monthly_to_annual_is_effective_not_nominal() {
            let monthly = Rate::<Monthly>::new(0.01).unwrap();
            let annual = monthly.convert::<Annual>().unwrap();
            // (1.01)^12 - 1 ≈ 0.126825, not 0.12.
            assert!((annual.value() - 0.126_825_030_131_969_7).abs() < 1e-12);
            assert_eq!(annual.value(), monthly.effective_annual().unwrap().value());
        }

        #[test]
        fn converting_there_and_back_recovers_the_rate() {
            let quarterly = Rate::<Quarterly>::new(0.03).unwrap();
            let round_trip = quarterly
                .convert::<Monthly>()
                .unwrap()
                .convert::<Quarterly>()
                .unwrap();
            assert!((round_trip.value() - quarterly.value()).abs() < 1e-12);
        }

        #[test]
        fn converting_to_the_same_periodicity_is_the_identity() {
            let monthly = Rate::<Monthly>::new(0.02).unwrap();
            assert!((monthly.convert::<Monthly>().unwrap().value() - 0.02).abs() < 1e-12);
        }

        #[test]
        fn compounding_a_near_total_loss_to_a_coarser_periodicity_is_out_of_range() {
            // 1 + r = 0.01; (0.01)^12 = 1e-24, far below the ulp of 1.0, so the
            // annual `(1+r)^12 - 1` rounds to exactly -1.0 — a meaningless rate,
            // not an overflow.
            let ruinous = Rate::<Monthly>::new(-0.99).unwrap();
            assert_eq!(ruinous.convert::<Annual>(), Err(TvmError::RateOutOfRange));
        }

        #[test]
        fn compounding_a_huge_rate_to_a_coarser_periodicity_overflows() {
            // (1 + 1e290)^12 is well beyond f64::MAX → a non-finite result.
            let enormous = Rate::<Monthly>::new(1e290).unwrap();
            assert_eq!(enormous.convert::<Annual>(), Err(TvmError::Overflow));
        }
    }
}
