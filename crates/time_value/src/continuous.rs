//! Continuous compounding — a periodicity-free [`ContinuousRate`] (force of
//! interest) and the operations over a continuous duration in years.
//!
//! Continuous compounding is the limit of discrete compounding as the frequency
//! goes to infinity: growth over a time `t` (in years) is `e^(δ·t)`, where `δ` is
//! the **force of interest** — the continuously-compounded annual rate. A force of
//! interest has *no discrete periodicity* (it is `∞` compoundings per year), so —
//! unlike [`Rate<P>`](crate::Rate) — it is **not** tagged with a
//! [`Periodicity`](crate::Periodicity),
//! and its time is a continuous `f64` duration in years rather than a
//! [`Period<P>`](crate::Period) count (`docs/adr/0036-continuous-compounding-force-of-interest.md`).
//!
//! The discrete and continuous worlds bridge through the *effective annual* rate:
//! `δ = ln(1 + r_eff)` and `r_eff = e^δ − 1`
//! ([`ContinuousRate::from_effective_annual`] / [`ContinuousRate::effective_annual`]).
//!
//! This module needs `exp`, so it lives behind the `std` / `libm` feature, like
//! the other transcendental operations (`docs/adr/0014-transcendental-single-sum-operations.md`).
//!
//! ```
//! use time_value::{continuous, ContinuousRate, Money};
//!
//! // 1000 growing at a 5% force of interest for 3 years: 1000·e^(0.05·3).
//! let rate = ContinuousRate::new(0.05)?;
//! let fv = continuous::future_value(rate, 3.0, Money::agnostic(1000.0)?)?;
//! assert!((fv.value() - 1161.834).abs() < 1e-3);
//!
//! // The present-value inverse recovers the original amount.
//! let pv = continuous::present_value(rate, 3.0, fv)?;
//! assert!((pv.value() - 1000.0).abs() < 1e-9);
//! # Ok::<(), time_value::TvmError>(())
//! ```

use crate::math::{exp, ln};
use crate::{Annual, Money, Rate, TvmError};

/// An annualized **force of interest** `δ` — a continuously-compounded rate.
///
/// A sibling of [`Rate<P>`](crate::Rate), *not* a case of it: a force of interest
/// has no discrete periodicity, so it carries no periodicity tag (ADR-0036).
/// Every *finite* force of interest is valid — its growth factor `e^δ` is always
/// positive, so there is no `> −1` floor as there is for a per-period
/// [`Rate`]; only a non-finite value is rejected.
///
/// The value is the plain force of interest: `0.05` is a 5% continuously
/// compounded annual rate.
///
/// Forces of interest are **totally ordered** by that value — see the [`Ord`] impl
/// (`docs/adr/0059-the-finite-scalars-are-totally-ordered.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContinuousRate(f64);

impl ContinuousRate {
    /// A force of interest of zero — no growth and no discounting.
    pub const ZERO: Self = Self(0.0);

    /// Wraps a force of interest `δ` (e.g. `0.05` for a 5% continuously compounded
    /// annual rate).
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::NonFiniteRate`] if `force` is not finite. Any finite
    /// value — including a negative one (continuous decay) or one at or below
    /// `−1` — is valid.
    pub fn new(force: f64) -> Result<Self, TvmError> {
        if force.is_finite() {
            Ok(Self(force))
        } else {
            Err(TvmError::NonFiniteRate)
        }
    }

    /// The force of interest as a plain `f64`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.0
    }

    /// The force of interest equivalent to an *effective annual* [`Rate<Annual>`]:
    /// `δ = ln(1 + r_eff)`.
    ///
    /// Infallible: a [`Rate`] is always finite and strictly greater
    /// than `−1`, so `1 + r_eff` is strictly positive and its logarithm is finite.
    ///
    /// ```
    /// use time_value::{Annual, ContinuousRate, Rate};
    ///
    /// // A 5% effective annual rate is a force of interest of ln(1.05) ≈ 0.04879.
    /// let delta = ContinuousRate::from_effective_annual(Rate::<Annual>::new(0.05)?);
    /// assert!((delta.value() - 0.048790).abs() < 1e-5);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    #[must_use]
    pub fn from_effective_annual(rate: Rate<Annual>) -> Self {
        Self(ln(1.0 + rate.value()))
    }

    /// The *effective annual* [`Rate<Annual>`] equivalent to this force of interest:
    /// `r_eff = e^δ − 1`.
    ///
    /// This is the inverse of [`from_effective_annual`](Self::from_effective_annual),
    /// letting a continuous rate be compared with the discrete per-period rates via
    /// the effective-rate machinery (ADR-0024).
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::Overflow`] if `e^δ` overflows the finite range (a very
    /// large `δ`), or [`TvmError::RateOutOfRange`] if a very negative `δ` drives
    /// `e^δ` to zero, so `r_eff` reaches the `−1` (−100%) floor a
    /// [`Rate`] cannot represent.
    ///
    /// ```
    /// use time_value::ContinuousRate;
    ///
    /// // A 5% force of interest is an effective annual rate of e^0.05 − 1 ≈ 0.05127.
    /// let r_eff = ContinuousRate::new(0.05)?.effective_annual()?;
    /// assert!((r_eff.value() - 0.051271).abs() < 1e-5);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    pub fn effective_annual(self) -> Result<Rate<Annual>, TvmError> {
        Rate::from_operation(exp(self.0) - 1.0)
    }
}

/// The default [`ContinuousRate`] is [`ZERO`](ContinuousRate::ZERO).
impl Default for ContinuousRate {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Fallibly wraps an `f64` force of interest, mirroring [`ContinuousRate::new`].
///
/// # Errors
///
/// Returns [`TvmError::NonFiniteRate`] if the value is not finite.
impl TryFrom<f64> for ContinuousRate {
    type Error = TvmError;

    fn try_from(force: f64) -> Result<Self, Self::Error> {
        Self::new(force)
    }
}

/// Orders forces of interest by their value.
///
/// The order is **total**, not partial: a `ContinuousRate` is finite by
/// construction, so `NaN` is unrepresentable. There is no periodicity tag to keep
/// apart here — a force of interest is periodicity-free (ADR-0036) — so any two are
/// comparable (ADR-0059).
///
/// ```
/// use time_value::ContinuousRate;
///
/// let quoted = ContinuousRate::new(0.05)?;
/// let floor = ContinuousRate::new(0.02)?;
///
/// assert!(quoted > floor);
/// assert_eq!(quoted.max(floor), quoted);
/// # Ok::<(), time_value::TvmError>(())
/// ```
impl Ord for ContinuousRate {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        // Unreachable fallback: `new` rejects the non-finite values, and two finite
        // `f64`s always compare.
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(core::cmp::Ordering::Equal)
    }
}

/// Delegates to the total order on [`Ord`], so it never returns `None`.
impl PartialOrd for ContinuousRate {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Equality on a force of interest is a full equivalence relation — `NaN`, the only
/// value that could break reflexivity, is unrepresentable. `PartialEq` stays derived
/// (there is no type parameter to drag a bound onto, unlike [`Rate`]), and there is
/// deliberately no [`Hash`](core::hash::Hash), for the reason given on [`Rate`]
/// (ADR-0059).
impl Eq for ContinuousRate {}

impl core::fmt::Display for ContinuousRate {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} continuous", self.0)
    }
}

/// The future value of a `present` amount grown at a continuous `rate` over
/// `years`: `FV = PV · e^(δ · years)`.
///
/// `years` is a continuous duration (it may be fractional or negative); the
/// currency of `present` is preserved.
///
/// # Errors
///
/// Returns [`TvmError::NonFiniteOffset`] if `years` is not finite, or
/// [`TvmError::Overflow`] if the growth overflows the finite range.
pub fn future_value(rate: ContinuousRate, years: f64, present: Money) -> Result<Money, TvmError> {
    if !years.is_finite() {
        return Err(TvmError::NonFiniteOffset);
    }
    Money::from_operation(
        present.value() * exp(rate.value() * years),
        present.currency(),
    )
}

/// The present value of a `future` amount discounted at a continuous `rate` over
/// `years`: `PV = FV · e^(−δ · years)` — the inverse of [`future_value`].
///
/// `years` is a continuous duration (it may be fractional or negative); the
/// currency of `future` is preserved.
///
/// # Errors
///
/// Returns [`TvmError::NonFiniteOffset`] if `years` is not finite, or
/// [`TvmError::Overflow`] if the discounting overflows the finite range.
pub fn present_value(rate: ContinuousRate, years: f64, future: Money) -> Result<Money, TvmError> {
    if !years.is_finite() {
        return Err(TvmError::NonFiniteOffset);
    }
    Money::from_operation(
        future.value() * exp(-rate.value() * years),
        future.currency(),
    )
}

#[cfg(test)]
mod tests {
    // Exactly-representable round-trips use exact `==`; approximate transcendental
    // results use a tolerance.
    #![allow(clippy::float_cmp)]

    use crate::{Annual, ContinuousRate, Currency, Money, Rate, TvmError};

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn accepts_any_finite_force_including_below_minus_one() {
        assert_eq!(ContinuousRate::new(0.05).unwrap().value(), 0.05);
        assert_eq!(ContinuousRate::new(-2.0).unwrap().value(), -2.0); // valid, unlike Rate
        assert_eq!(ContinuousRate::ZERO.value(), 0.0);
    }

    #[test]
    fn rejects_non_finite_force() {
        assert_eq!(ContinuousRate::new(f64::NAN), Err(TvmError::NonFiniteRate));
        assert_eq!(
            ContinuousRate::new(f64::INFINITY),
            Err(TvmError::NonFiniteRate)
        );
    }

    #[test]
    fn future_value_grows_by_the_exponential() {
        let rate = ContinuousRate::new(0.05).unwrap();
        let fv = super::future_value(rate, 3.0, Money::agnostic(1000.0).unwrap()).unwrap();
        assert!(approx(fv.value(), 1000.0 * (0.05_f64 * 3.0).exp()));
    }

    #[test]
    fn present_value_inverts_future_value_and_keeps_currency() {
        let rate = ContinuousRate::new(0.07).unwrap();
        let pv = Money::new(2500.0, Currency::Usd).unwrap();
        let fv = super::future_value(rate, 4.5, pv).unwrap();
        let back = super::present_value(rate, 4.5, fv).unwrap();
        assert!(approx(back.value(), 2500.0));
        assert_eq!(fv.currency(), Currency::Usd);
        assert_eq!(back.currency(), Currency::Usd);
    }

    #[test]
    fn zero_rate_and_zero_years_do_not_change_the_amount() {
        let m = Money::agnostic(100.0).unwrap();
        assert!(approx(
            super::future_value(ContinuousRate::ZERO, 5.0, m)
                .unwrap()
                .value(),
            100.0
        ));
        assert!(approx(
            super::future_value(ContinuousRate::new(0.1).unwrap(), 0.0, m)
                .unwrap()
                .value(),
            100.0
        ));
    }

    #[test]
    fn non_finite_years_is_an_error() {
        let m = Money::agnostic(100.0).unwrap();
        let rate = ContinuousRate::new(0.05).unwrap();
        assert_eq!(
            super::future_value(rate, f64::NAN, m),
            Err(TvmError::NonFiniteOffset)
        );
        assert_eq!(
            super::present_value(rate, f64::INFINITY, m),
            Err(TvmError::NonFiniteOffset)
        );
    }

    #[test]
    fn overflow_is_reported() {
        // An enormous force over a long horizon overflows `f64`.
        let rate = ContinuousRate::new(700.0).unwrap();
        assert_eq!(
            super::future_value(rate, 10.0, Money::agnostic(1.0).unwrap()),
            Err(TvmError::Overflow)
        );
    }

    #[test]
    fn bridges_to_and_from_the_effective_annual_rate() {
        // δ = ln(1 + r_eff); r_eff = e^δ − 1 — round-trips.
        let annual = Rate::<Annual>::new(0.05).unwrap();
        let delta = ContinuousRate::from_effective_annual(annual);
        assert!(approx(delta.value(), (1.05_f64).ln()));
        let back = delta.effective_annual().unwrap();
        assert!(approx(back.value(), 0.05));
    }

    #[test]
    fn effective_annual_overflows_for_a_huge_force() {
        assert_eq!(
            ContinuousRate::new(1000.0).unwrap().effective_annual(),
            Err(TvmError::Overflow)
        );
    }

    #[test]
    fn effective_annual_hits_the_floor_for_a_very_negative_force() {
        // e^δ → 0 as δ → −∞, so r_eff → −1, which a `Rate` cannot represent.
        assert_eq!(
            ContinuousRate::new(-1000.0).unwrap().effective_annual(),
            Err(TvmError::RateOutOfRange)
        );
    }

    #[test]
    fn default_and_try_from() {
        assert_eq!(ContinuousRate::default(), ContinuousRate::ZERO);
        assert_eq!(ContinuousRate::try_from(0.03).unwrap().value(), 0.03);
        assert_eq!(
            ContinuousRate::try_from(f64::NAN),
            Err(TvmError::NonFiniteRate)
        );
    }

    /// A force of interest carries no periodicity tag, so *every* pair is comparable
    /// — including one below `−1`, which `new` accepts (ADR-0059).
    #[test]
    fn forces_of_interest_are_totally_ordered() {
        let quoted = ContinuousRate::new(0.05).unwrap();
        let floor = ContinuousRate::new(0.02).unwrap();
        let decaying = ContinuousRate::new(-2.0).unwrap();
        let same_floor = ContinuousRate::new(0.02).unwrap();

        assert!(quoted > floor);
        assert!(decaying < floor);
        assert!(floor >= same_floor);
        assert_eq!(quoted.max(floor), quoted);
        assert_eq!(quoted.min(decaying), decaying);
        assert_eq!(quoted.partial_cmp(&floor), Some(quoted.cmp(&floor)));

        let mut forces = [quoted, decaying, floor];
        forces.sort_unstable();
        assert!(approx(forces[0].value(), -2.0));
        assert!(approx(forces[2].value(), 0.05));
    }

    /// The derived `PartialEq` and the hand-written `cmp` must agree, and the signed
    /// zeros are the pair that could split them.
    #[test]
    fn the_signed_zeros_are_one_force() {
        let plus = ContinuousRate::new(0.0).unwrap();
        let minus = ContinuousRate::new(-0.0).unwrap();

        assert_eq!(plus, minus);
        assert_eq!(plus.cmp(&minus), core::cmp::Ordering::Equal);
    }
}
