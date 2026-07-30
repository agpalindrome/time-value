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
//! The relation `FV = PV·e^(δ·Y)` is solved for each of its four unknowns:
//! [`future_value`] and [`present_value`] for the two amounts, and [`rate`] and
//! [`years`] for the force of interest and the span
//! (`docs/adr/0064-continuous-solves.md`). Both solves are **closed forms** — the
//! same logarithm `ln(FV/PV)` divided by the other given — so neither needs the
//! bracketing solver the annuity rate solves use. The solve set therefore matches
//! [`single_sum`](crate::single_sum)'s, with `years` in place of `periods` and a
//! force of interest in place of a per-period rate.
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
//!
//! // …and the two solves read the same relation back the other way.
//! use time_value::{FutureValue, PresentValue};
//! let delta = continuous::rate(3.0, PresentValue(pv), FutureValue(fv))?;
//! let span = continuous::years(rate, PresentValue(pv), FutureValue(fv))?;
//! assert!((delta.value() - 0.05).abs() < 1e-12);
//! assert!((span - 3.0).abs() < 1e-12);
//! # Ok::<(), time_value::TvmError>(())
//! ```

use crate::math::{exp, ln, ln_1p};
use crate::root::{abs, unit_factor_outcome};
use crate::{Annual, FutureValue, Money, PresentValue, Rate, TvmError};

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

    /// Constructs from the `f64` result of an operation, validating finiteness.
    ///
    /// The mirror of [`Rate::from_operation`](crate::Rate) and
    /// [`Money::from_operation`](crate::Money), minus the domain floor those have:
    /// every *finite* force of interest is valid (ADR-0036), so the only way an
    /// operation can fail to produce one is [`TvmError::Overflow`] — the ADR-0021 /
    /// ADR-0031 rule that a non-finite value *produced* by arithmetic is an
    /// overflow, where a non-finite value passed *in* is
    /// [`TvmError::NonFiniteRate`].
    pub(crate) fn from_operation(force: f64) -> Result<Self, TvmError> {
        if force.is_finite() {
            Ok(Self(force))
        } else {
            Err(TvmError::Overflow)
        }
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

/// `ln(future / present)` — the quantity both solves divide, and the one place
/// their shared domain is enforced (ADR-0064).
///
/// # The domain
///
/// `e^(δ·Y)` is strictly positive for every finite `δ` and `Y`, so `FV = PV·e^(δ·Y)`
/// forces `FV / PV` to be strictly positive: the two amounts must be **non-zero and
/// of the same sign**. Two *negative* amounts are admissible and answer correctly —
/// the relation is homogeneous, so a liability growing at `δ` is the same solve as an
/// asset growing at `δ`. The ratio and its reciprocal must both be finite as well,
/// which is what makes the arithmetic below provably finite; that costs nothing a
/// caller would notice, since it excludes only amounts more than ~308 decades apart.
/// Everything outside is [`TvmError::NoRealSolution`], the same variant (and the same
/// reason — no real logarithm) that [`single_sum::periods`](crate::single_sum::periods)
/// reports for its own ratio.
///
/// # Why `ln1p` twice rather than `ln` once
///
/// The literal `ln(FV / PV)` is the cancellation ADR-0054 removed from the annuity
/// factors, in a new place: forming the ratio rounds it to within 1 ulp of `1`, so
/// for a small `δ·Y` every significant digit of the answer is already gone before
/// `ln` is called. At `δ·Y = 1e-12` it is wrong in the **fifth** digit (`1.4e-5`
/// relative).
///
/// `ln1p((FV − PV) / PV)` fixes that — the subtraction of two nearby amounts is
/// exact (Sterbenz) — but only from one side: as `FV / PV → 0` the argument
/// approaches `−1`, which cannot carry the information that `1 + x` is tiny, and at
/// `ln(FV/PV) = −30` that form is wrong by `5.5e-6` relative where plain `ln` is
/// exact to a few ULP.
///
/// So take the identity `ln(FV/PV) = −ln(PV/FV)` and evaluate whichever side keeps
/// the `ln1p` argument **non-negative**. There is no threshold to justify: the switch
/// is at the natural symmetry point `FV = PV`, where both forms give exactly `0`, and
/// each side is accurate to ~2 ULP over its whole half. The error bound is
/// `|Δ| ≲ 2u·(1 − e^(−|L|)) + u·|L|` — at most a couple of ULP of the answer,
/// everywhere.
fn log_ratio(present: f64, future: f64) -> Result<f64, TvmError> {
    let ratio = future / present;
    let inverse = present / future;
    if !(ratio.is_finite() && inverse.is_finite() && ratio > 0.0) {
        return Err(TvmError::NoRealSolution);
    }
    // The two amounts share a sign (`ratio > 0`), so neither difference can overflow,
    // and the divisor is always the smaller-magnitude one — which makes each quotient
    // `ratio − 1` or `inverse − 1`, both finite by the guard above. So this cannot
    // produce a non-finite value and needs no second check.
    Ok(if abs(future) >= abs(present) {
        ln_1p((future - present) / present)
    } else {
        -ln_1p((present - future) / future)
    })
}

/// The force of interest `δ` at which `present` grows to `future` over a span of
/// `years` — [`future_value`] / [`present_value`] solved for `δ` (ADR-0064).
///
/// `δ = ln(FV / PV) / Y`. A **closed form**: unlike the annuity rate solves this
/// needs no bracketing, no acceptance tolerance and no monotonicity argument, because
/// the equation is linear in `δ` once the logarithm is taken.
///
/// `years` is a continuous span, so it may be fractional or **negative** (ADR-0036),
/// and a negative span simply flips the sign of the answer: growing to `future` over
/// `−Y` is decaying to it over `Y`.
///
/// # Examples
///
/// ```
/// use time_value::{continuous, FutureValue, Money, PresentValue};
///
/// // What force of interest turns 1000 into 1161.83 over three years? 5%.
/// let delta = continuous::rate(
///     3.0,
///     PresentValue(Money::agnostic(1000.0)?),
///     FutureValue(Money::agnostic(1161.834242728283)?),
/// )?;
/// assert!((delta.value() - 0.05).abs() < 1e-12);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// The two amounts are role-tagged, so a transposed call does not compile
/// (ADR-0050):
///
/// ```compile_fail
/// use time_value::{continuous, FutureValue, Money, PresentValue};
///
/// let _ = continuous::rate(
///     3.0,
///     FutureValue(Money::agnostic(1161.834242728283).unwrap()), // future where present goes
///     PresentValue(Money::agnostic(1000.0).unwrap()),
/// );
/// ```
///
/// # Currency
///
/// **The two amounts' currencies are not folded, and never a `CurrencyMismatch`**
/// (ADR-0057). The result is a [`ContinuousRate`], which carries no denomination, so
/// there is no currency to derive; the answer is the force of interest relating the
/// bare magnitudes. Combine the amounts yourself first if a mixed pair should be an
/// error at your call site.
///
/// # Errors
///
/// - [`TvmError::NonFiniteOffset`] if `years` is not finite.
/// - [`TvmError::IndeterminateRate`] if `years` is zero **and** `present` equals
///   `future`: no time passes, so `FV = PV` holds at *every* force of interest.
/// - [`TvmError::NoRealSolution`] if `years` is zero and the two amounts differ (no
///   force of interest reconciles them), or if `future / present` — or its
///   reciprocal — is not a positive finite number, so the logarithm has no real
///   value. That covers a zero `present`, a zero `future`, and amounts of opposite
///   sign; two *negative* amounts are fine.
/// - [`TvmError::Overflow`] if the quotient overflows the finite range — a non-zero
///   but subnormal `years` against a large logarithm.
pub fn rate(
    years: f64,
    present: PresentValue,
    future: FutureValue,
) -> Result<ContinuousRate, TvmError> {
    if !years.is_finite() {
        return Err(TvmError::NonFiniteOffset);
    }
    let (pv, fv) = (present.money().value(), future.money().value());
    let log = log_ratio(pv, fv)?;
    // A zero span makes the growth factor `e^(δ·0) = 1` for every force of interest,
    // so the equation collapses to `PV = FV` with δ absent. That is the unit-factor
    // shape ADR-0056 named and ADR-0063 extracted: either every force satisfies it or
    // none does, judged by the solver's own root test rather than `==`, so a target a
    // hair away cannot be reported as though it pinned a rate. The comparison is
    // exact and written against zero for the crate's `float_cmp` lint.
    if years == 0.0 {
        return Err(unit_factor_outcome(pv, fv, TvmError::IndeterminateRate));
    }
    ContinuousRate::from_operation(log / years)
}

/// The span in years over which `present` grows to `future` at a continuous `rate` —
/// [`future_value`] / [`present_value`] solved for `Y` (ADR-0064).
///
/// `Y = ln(FV / PV) / δ`. A **closed form**, like [`rate`]: the two solves are the
/// same logarithm divided by the other given.
///
/// The answer is a plain `f64` span, not a [`Period<P>`](crate::Period) — continuous
/// time has no periodicity (ADR-0036) — and it is **signed**: discounting at a
/// positive force of interest (`FV < PV`) puts `future` in the past, and the honest
/// answer there is a negative span, not an error. This is where the continuous solve
/// parts company with [`single_sum::periods`](crate::single_sum::periods), whose
/// [`Period<P>`](crate::Period) cannot be negative and which reports
/// [`TvmError::NegativePeriods`] on the same shape of input.
///
/// # Examples
///
/// ```
/// use time_value::{continuous, ContinuousRate, FutureValue, Money, PresentValue};
///
/// // How long does 1000 take to reach 1161.83 at a 5% force of interest? Three years.
/// let years = continuous::years(
///     ContinuousRate::new(0.05)?,
///     PresentValue(Money::agnostic(1000.0)?),
///     FutureValue(Money::agnostic(1161.834242728283)?),
/// )?;
/// assert!((years - 3.0).abs() < 1e-12);
///
/// // Reaching a *smaller* amount at a positive force is in the past.
/// let back = continuous::years(
///     ContinuousRate::new(0.05)?,
///     PresentValue(Money::agnostic(1161.834242728283)?),
///     FutureValue(Money::agnostic(1000.0)?),
/// )?;
/// assert!((back + 3.0).abs() < 1e-12);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Currency
///
/// **The two amounts' currencies are not folded, and never a `CurrencyMismatch`**
/// (ADR-0057) — the result is a bare span, which carries no denomination. See
/// [`rate`] for the same note.
///
/// # Errors
///
/// - [`TvmError::IndeterminateSpan`] if `rate` is zero **and** `present` equals
///   `future`: nothing grows, so `FV = PV` holds after *every* span.
/// - [`TvmError::NoRealSolution`] if `rate` is zero and the two amounts differ (no
///   span reconciles them), or if `future / present` — or its reciprocal — is not a
///   positive finite number, exactly as for [`rate`].
/// - [`TvmError::Overflow`] if the quotient overflows the finite range — a non-zero
///   but subnormal `rate` against a large logarithm.
pub fn years(
    rate: ContinuousRate,
    present: PresentValue,
    future: FutureValue,
) -> Result<f64, TvmError> {
    let (pv, fv) = (present.money().value(), future.money().value());
    let log = log_ratio(pv, fv)?;
    // The mirror of `rate`'s zero-span guard: a zero force of interest makes the
    // growth factor `e^(0·Y) = 1` for every span, so the equation collapses to
    // `PV = FV` with `Y` absent. Same shared helper, same root test — only the
    // "satisfied" variant differs, because the unknown does.
    if rate.value() == 0.0 {
        return Err(unit_factor_outcome(pv, fv, TvmError::IndeterminateSpan));
    }
    let span = log / rate.value();
    if span.is_finite() {
        Ok(span)
    } else {
        // There is no `Span` newtype to funnel this through, so the finiteness rule
        // ADR-0021 gives every other operation is applied here by hand.
        Err(TvmError::Overflow)
    }
}

#[cfg(test)]
mod tests {
    // Exactly-representable round-trips use exact `==`; approximate transcendental
    // results use a tolerance.
    #![allow(clippy::float_cmp)]

    use crate::{
        Annual, ContinuousRate, Currency, FutureValue, Money, PresentValue, Rate, TvmError,
    };

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

    // ---- The solves (ADR-0064) ------------------------------------------------

    /// `1000·e^(0.05·3)` as an `f64` is `1161.834242728283`, and the force of
    /// interest those two amounts imply over three years is, to 60 significant
    /// digits computed independently in Python's `decimal`,
    /// `0.0499999999999999693603234449163444043610704502330799942405…`.
    ///
    /// That is not exactly `0.05`, and the difference is the point: the reference is
    /// the logarithm of the two *representable* amounts, not the `δ` they were built
    /// from, so the assertion below pins the arithmetic rather than the round trip.
    /// Each constant is that reference rounded to the nearest `f64`, which is well
    /// inside the tolerances asserted against it.
    const REFERENCE_PRESENT: f64 = 1000.0;
    const REFERENCE_FUTURE: f64 = 1_161.834_242_728_283;
    const REFERENCE_FORCE: f64 = 0.049_999_999_999_999_97;
    /// The same pair read as a span at `δ = 0.05`:
    /// `2.99999999999999799508595300120729449320634542350631904951…`.
    const REFERENCE_YEARS: f64 = 2.999_999_999_999_998;

    fn present(amount: f64) -> PresentValue {
        PresentValue(Money::agnostic(amount).unwrap())
    }

    fn future(amount: f64) -> FutureValue {
        FutureValue(Money::agnostic(amount).unwrap())
    }

    /// The closed form against an independent high-precision reference, not against
    /// the crate's own `future_value` (ADR-0045).
    #[test]
    fn the_solves_match_an_independent_high_precision_reference() {
        let force = super::rate(3.0, present(REFERENCE_PRESENT), future(REFERENCE_FUTURE)).unwrap();
        let span = super::years(
            ContinuousRate::new(0.05).unwrap(),
            present(REFERENCE_PRESENT),
            future(REFERENCE_FUTURE),
        )
        .unwrap();

        // Both are accurate to a couple of ULP of the reference, so the assertion is
        // relative and three orders tighter than "≈ 0.05" / "≈ 3" would be.
        assert!(approx_rel(force.value(), REFERENCE_FORCE, 1e-15));
        assert!(approx_rel(span, REFERENCE_YEARS, 1e-15));
    }

    /// `no_std`-safe relative comparison (no `f64::abs`).
    fn approx_rel(actual: f64, expected: f64, tolerance: f64) -> bool {
        let d = (actual - expected) / expected;
        d < tolerance && d > -tolerance
    }

    /// A negative span flips the sign of the force, and a force solved over a
    /// negative span is the negative of the same solve over the positive one — the
    /// span may be negative (ADR-0036), so this is an answer, not an error.
    #[test]
    fn a_negative_span_flips_the_sign_of_the_force() {
        let forward =
            super::rate(3.0, present(REFERENCE_PRESENT), future(REFERENCE_FUTURE)).unwrap();
        let backward =
            super::rate(-3.0, present(REFERENCE_PRESENT), future(REFERENCE_FUTURE)).unwrap();
        assert!(approx(backward.value(), -forward.value()));
    }

    /// The span solve is *signed*, where `single_sum::periods` cannot be: reaching a
    /// smaller amount at a positive force of interest lies in the past, and the
    /// honest answer is a negative span rather than `NegativePeriods`.
    #[test]
    fn a_span_into_the_past_is_negative_rather_than_an_error() {
        let span = super::years(
            ContinuousRate::new(0.05).unwrap(),
            present(REFERENCE_FUTURE),
            future(REFERENCE_PRESENT),
        )
        .unwrap();
        assert!(approx(span, -3.0));
    }

    /// Two *negative* amounts have a positive ratio, and the relation is homogeneous
    /// — a liability growing at 5% is the same solve as an asset growing at 5%.
    #[test]
    fn two_negative_amounts_solve_the_same_as_two_positive_ones() {
        let positive =
            super::rate(3.0, present(REFERENCE_PRESENT), future(REFERENCE_FUTURE)).unwrap();
        let negative =
            super::rate(3.0, present(-REFERENCE_PRESENT), future(-REFERENCE_FUTURE)).unwrap();
        assert!(approx(negative.value(), positive.value()));

        let span = super::years(
            ContinuousRate::new(0.05).unwrap(),
            present(-REFERENCE_PRESENT),
            future(-REFERENCE_FUTURE),
        )
        .unwrap();
        assert!(approx(span, 3.0));
    }

    /// The shared domain, enumerated: a zero present, a zero future, and amounts of
    /// opposite sign all leave `ln(FV/PV)` with no real value. Both solves report the
    /// same variant on the same inputs, because both ask the same helper.
    #[test]
    fn the_domain_rejections_are_the_same_for_both_solves() {
        let force = ContinuousRate::new(0.05).unwrap();
        for (pv, fv) in [
            (0.0, 1000.0),    // zero present: the ratio is infinite
            (1000.0, 0.0),    // zero future: e^(δ·Y) is never zero
            (0.0, 0.0),       // both zero: the ratio is `NaN`, and nothing is pinned
            (1000.0, -500.0), // opposite signs: a negative ratio has no logarithm
            (-1000.0, 500.0),
        ] {
            assert_eq!(
                super::rate(3.0, present(pv), future(fv)),
                Err(TvmError::NoRealSolution),
                "rate({pv}, {fv})"
            );
            assert_eq!(
                super::years(force, present(pv), future(fv)),
                Err(TvmError::NoRealSolution),
                "years({pv}, {fv})"
            );
        }
    }

    /// The reciprocal half of the domain: amounts more than ~308 decades apart make
    /// one of the two quotients overflow even though the other is finite, and the
    /// guard rejects the pair from either side.
    #[test]
    fn amounts_beyond_the_representable_ratio_have_no_real_solution() {
        assert_eq!(
            super::rate(1.0, present(1e-300), future(1e300)),
            Err(TvmError::NoRealSolution)
        );
        assert_eq!(
            super::rate(1.0, present(1e300), future(1e-300)),
            Err(TvmError::NoRealSolution)
        );
    }

    /// A zero span makes the growth factor `1`, so the equation is `PV = FV` with δ
    /// absent: every force satisfies it when the amounts agree, none when they do
    /// not. This is ADR-0056's distinction, reached through the shared helper.
    #[test]
    fn a_zero_span_is_indeterminate_or_unsolvable_in_the_force() {
        assert_eq!(
            super::rate(0.0, present(1000.0), future(1000.0)),
            Err(TvmError::IndeterminateRate)
        );
        assert_eq!(
            super::rate(0.0, present(1000.0), future(2000.0)),
            Err(TvmError::NoRealSolution)
        );
    }

    /// The mirror case, and the reason `IndeterminateSpan` exists: a zero force
    /// leaves the *span* under-determined, not the rate.
    #[test]
    fn a_zero_force_is_indeterminate_or_unsolvable_in_the_span() {
        assert_eq!(
            super::years(ContinuousRate::ZERO, present(1000.0), future(1000.0)),
            Err(TvmError::IndeterminateSpan)
        );
        assert_eq!(
            super::years(ContinuousRate::ZERO, present(1000.0), future(2000.0)),
            Err(TvmError::NoRealSolution)
        );
    }

    /// "Satisfied" is the solver's root test, not `==` (ADR-0056): a target a hair
    /// from the present amount is still satisfied at every force and every span, so
    /// an exact-equality guard would let the near-miss through as though it had been
    /// solved. `1e-9` relative is `Residual::is_root`'s tolerance, so `1000` against
    /// `1000 + 1e-7` is inside it and `1000` against `1000.1` is not.
    #[test]
    fn the_degeneracy_guards_use_the_root_test_rather_than_equality() {
        assert_eq!(
            super::rate(0.0, present(1000.0), future(1_000.000_000_1)),
            Err(TvmError::IndeterminateRate)
        );
        assert_eq!(
            super::years(
                ContinuousRate::ZERO,
                present(1000.0),
                future(1_000.000_000_1)
            ),
            Err(TvmError::IndeterminateSpan)
        );
        assert_eq!(
            super::rate(0.0, present(1000.0), future(1000.1)),
            Err(TvmError::NoRealSolution)
        );
    }

    /// A non-finite span is the same `NonFiniteOffset` the value operations report.
    /// The *span* solve has no equivalent guard: its `rate` is a `ContinuousRate`,
    /// which is finite by construction.
    #[test]
    fn a_non_finite_span_is_rejected_by_the_force_solve() {
        for years in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert_eq!(
                super::rate(years, present(1000.0), future(2000.0)),
                Err(TvmError::NonFiniteOffset)
            );
        }
    }

    /// Dividing a large logarithm by a subnormal span (or force) overflows, which is
    /// `Overflow` — a real answer too large to represent — not a degeneracy.
    #[test]
    fn a_subnormal_divisor_overflows() {
        assert_eq!(
            super::rate(f64::MIN_POSITIVE * 1e-10, present(1.0), future(1e300)),
            Err(TvmError::Overflow)
        );
        assert_eq!(
            super::years(
                ContinuousRate::new(f64::MIN_POSITIVE * 1e-10).unwrap(),
                present(1.0),
                future(1e300),
            ),
            Err(TvmError::Overflow)
        );
    }

    /// `ln1p` earns its place, measured rather than asserted (ADR-0054's class of
    /// defect). At `δ·Y = 1e-12` the ratio `FV/PV` rounds to within an ULP of `1`, so
    /// the literal `ln(FV/PV)` is wrong in the fifth digit; the `ln1p` form is exact
    /// to a couple of ULP. The reference is Python `decimal` at 60 digits.
    #[test]
    fn the_log1p_form_beats_the_literal_ratio_near_a_unit_ratio() {
        let pv = 1000.0_f64;
        let fv = 1000.0 * (1.0 + 1e-12);
        // ln(fv/pv) to 60 digits, for exactly these two `f64` amounts.
        let reference = 1.000_103_111_436_556e-12;

        let ours = super::rate(1.0, present(pv), future(fv)).unwrap().value();
        assert!(approx_rel(ours, reference, 1e-14), "ours = {ours}");

        // The form this deliberately does not use.
        let literal = crate::math::ln(fv / pv);
        assert!(
            !approx_rel(literal, reference, 1e-6),
            "the literal ratio was expected to be wrong by more than 1e-6 relative, got {literal}",
        );
    }

    /// …and the other half of the branch: as `FV/PV → 0` it is the one-sided `ln1p`
    /// form that fails, which is why the solve switches sides at `FV = PV` rather
    /// than using `ln1p((FV − PV)/PV)` throughout. At `ln(FV/PV) = −30` the one-sided
    /// form is wrong by ~`5.5e-6` relative.
    #[test]
    fn the_branch_keeps_a_deep_discount_accurate_too() {
        let pv = 1000.0_f64;
        let fv = 1000.0 * crate::math::exp(-30.0);
        // ln(fv/pv) to 60 digits: −30.0000000000000000129854968557204261860…
        let reference = -30.000_000_000_000_000_013;

        let ours = super::rate(1.0, present(pv), future(fv)).unwrap().value();
        assert!(approx_rel(ours, reference, 1e-14), "ours = {ours}");

        // The one-sided form, evaluated here to show the gap is real.
        let one_sided = crate::math::ln_1p((fv - pv) / pv);
        assert!(
            !approx_rel(one_sided, reference, 1e-8),
            "the one-sided ln1p was expected to be wrong by more than 1e-8 relative, got {one_sided}",
        );
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
