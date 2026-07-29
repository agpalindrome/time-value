//! [`Cashflows`] — a periodicity-tagged cashflow series and the discrete
//! operations over it.

use core::marker::PhantomData;

#[cfg(any(feature = "std", feature = "libm"))]
use crate::math::powf;
use crate::money::combine;
use crate::root::{self, abs};
use crate::{Currency, Money, Periodicity, Rate, TvmError};

/// A periodicity-tagged series of cashflows at consecutive periods `0, 1, 2, …`.
///
/// `flows[t]` is the cashflow at period `t`; period `0` is "now" and is not
/// discounted. Cashflows are signed (outflow negative, inflow positive).
///
/// `Cashflows` **borrows** its underlying slice rather than owning a `Vec`, so
/// it stays `no_std` and allocation-free (see
/// `docs/adr/0013-core-api-values-and-discrete-operations.md`). The periodicity
/// tag `P` ties it to a matching [`Rate<P>`] in every discounting operation.
///
/// # Examples
///
/// ```
/// use time_value::{Cashflows, Money, Monthly, Rate};
///
/// let flows = [Money::agnostic(-100.0)?, Money::agnostic(60.0)?, Money::agnostic(60.0)?];
/// let series = Cashflows::<Monthly>::new(&flows);
/// let rate = Rate::<Monthly>::new(0.01)?;
///
/// let npv = series.net_present_value(rate)?;
/// assert!((npv.value() - 18.2237).abs() < 1e-3);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// A rate of the wrong periodicity does not type-check:
///
/// ```compile_fail
/// use time_value::{Annual, Cashflows, Money, Monthly, Rate};
///
/// let flows = [Money::agnostic(-100.0).unwrap(), Money::agnostic(60.0).unwrap()];
/// let series = Cashflows::<Monthly>::new(&flows);
/// let annual = Rate::<Annual>::new(0.05).unwrap();
/// let _ = series.net_present_value(annual); // mismatched periodicity — won't compile
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cashflows<'a, P: Periodicity> {
    flows: &'a [Money],
    marker: PhantomData<P>,
}

impl<'a, P: Periodicity> Cashflows<'a, P> {
    /// Wraps a slice of cashflows; `flows[t]` occurs at period `t`.
    #[must_use]
    pub const fn new(flows: &'a [Money]) -> Self {
        Self {
            flows,
            marker: PhantomData,
        }
    }

    /// The underlying cashflows.
    #[must_use]
    pub const fn as_slice(self) -> &'a [Money] {
        self.flows
    }

    /// The number of cashflows in the series.
    #[must_use]
    pub const fn len(self) -> usize {
        self.flows.len()
    }

    /// Whether the series is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.flows.is_empty()
    }

    /// The single [`Currency`] the series is denominated in, by the
    /// [`Currency::Xxx`] identity rule — the denomination of any monetary result.
    /// An empty (or wholly agnostic) series is [`Currency::Xxx`].
    ///
    /// # Errors
    ///
    /// [`TvmError::CurrencyMismatch`] if the flows mix distinct non-`Xxx`
    /// currencies (ADR-0034).
    fn currency(self) -> Result<Currency, TvmError> {
        let mut acc = Currency::Xxx;
        for cf in self.flows {
            acc = combine(acc, cf.currency())?;
        }
        Ok(acc)
    }

    /// The net present value of the series discounted at `rate`.
    ///
    /// `NPV = Σₜ CFₜ / (1 + r)ᵗ`, evaluated with only elementary arithmetic (no
    /// transcendental functions), so it is available in the default `no_std`,
    /// dependency-free build. An **empty** series has value `0` (nothing to
    /// discount).
    ///
    /// Evaluated by Horner's method from the last cashflow — `CF₀ + d(CF₁ + d(CF₂ + …))`
    /// with `d = 1/(1+r)` — which keeps every partial bounded by the cashflow
    /// magnitudes.
    ///
    /// # Errors
    ///
    /// - [`TvmError::CurrencyMismatch`] if the flows mix distinct non-`Xxx`
    ///   currencies, so the result has no single denomination (ADR-0034).
    /// - [`TvmError::Overflow`] if the sum overflows to a non-finite value,
    ///   which needs cashflows near `f64::MAX` or a rate a hair above `−100%`
    ///   (ADR-0021).
    pub fn net_present_value(self, rate: Rate<P>) -> Result<Money, TvmError> {
        let currency = self.currency()?;
        let discount = 1.0 / (1.0 + rate.value());
        let mut acc = 0.0;
        for cf in self.flows.iter().rev() {
            acc = acc * discount + cf.value();
        }
        Money::from_operation(acc, currency)
    }

    /// The net future value of the series at its final period, compounded at
    /// `rate`.
    ///
    /// `NFV = Σₜ CFₜ (1 + r)ⁿ⁻¹⁻ᵗ` for a series of `n` cashflows, evaluated by
    /// Horner's method — again arithmetic-only. An **empty** series has value `0`.
    ///
    /// # Errors
    ///
    /// - [`TvmError::CurrencyMismatch`] if the flows mix distinct non-`Xxx`
    ///   currencies, so the result has no single denomination (ADR-0034).
    /// - [`TvmError::Overflow`] if the compounded sum overflows to a
    ///   non-finite value (ADR-0021).
    pub fn net_future_value(self, rate: Rate<P>) -> Result<Money, TvmError> {
        let currency = self.currency()?;
        let growth = 1.0 + rate.value();
        let mut acc = 0.0;
        for cf in self.flows {
            acc = acc * growth + cf.value();
        }
        Money::from_operation(acc, currency)
    }

    /// The internal rate of return: the [`Rate<P>`] at which the series' net
    /// present value is zero, from a default initial guess of 10% per period.
    ///
    /// # Errors
    ///
    /// See [`internal_rate_of_return_from`](Self::internal_rate_of_return_from).
    pub fn internal_rate_of_return(self) -> Result<Rate<P>, TvmError> {
        self.internal_rate_of_return_from(0.1)
    }

    /// The internal rate of return, seeding the solver with `guess` (a per-period
    /// rate).
    ///
    /// It first tries **Newton–Raphson** from `guess` — fast, and it converges to
    /// the root nearest the guess, which lets a caller steer toward the intended
    /// one when a non-conventional series has several. If Newton wanders off (a
    /// poor guess, a flat derivative, or an iterate that leaves the valid domain),
    /// it falls back to a **bracketing search**: it scans the rate domain for a
    /// sign change in the NPV and bisects it, so a root is found whenever one
    /// exists. The fallback returns the lowest bracketed root. Both methods are
    /// arithmetic-only (integer powers of the discount factor), so IRR stays in
    /// the default `no_std` build.
    ///
    /// # Errors
    ///
    /// - [`TvmError::EmptyCashflows`] if the series is empty.
    /// - [`TvmError::IrrDidNotConverge`] if neither method finds a root — in
    ///   particular when the NPV never changes sign over the valid rate domain,
    ///   so the series has no real IRR (e.g. cashflows that are all one sign).
    pub fn internal_rate_of_return_from(self, guess: f64) -> Result<Rate<P>, TvmError> {
        if self.flows.is_empty() {
            return Err(TvmError::EmptyCashflows);
        }
        // Try Newton from `guess`, then the robust bracketing fallback (ADR-0020) —
        // both shared with XIRR via the `root` module. Each NPV sample carries the
        // scale of its own discounted terms, so a rate is only a root when the NPV
        // is negligible against the cashflows still visible *at that rate*
        // (ADR-0021, ADR-0054).
        match root::newton(|r| self.npv_and_derivative(r), guess)
            .or_else(|| root::bracket_and_bisect(|r| self.npv_at(r)))
        {
            Some(rate) => Rate::new(rate),
            None => Err(TvmError::IrrDidNotConverge),
        }
    }

    /// The NPV at a candidate per-period `rate` (no derivative), accumulated in
    /// one pass: `NPV(r) = Σₜ CFₜ (1+r)⁻ᵗ`, alongside the scale it is judged
    /// against, `Σₜ |CFₜ| (1+r)⁻ᵗ`.
    fn npv_at(self, rate: f64) -> root::Residual {
        let discount = 1.0 / (1.0 + rate);
        let mut factor = 1.0; // discountᵗ
        let mut npv = 0.0;
        let mut scale = 0.0;
        for cf in self.flows {
            npv += cf.value() * factor;
            scale += abs(cf.value()) * abs(factor);
            factor *= discount;
        }
        root::Residual { value: npv, scale }
    }

    /// NPV and its derivative d(NPV)/dr at a candidate per-period `rate`.
    ///
    /// `NPV(r)  = Σₜ CFₜ (1+r)⁻ᵗ`, `NPV'(r) = Σₜ −t·CFₜ (1+r)⁻ᵗ⁻¹`. Both are
    /// accumulated in one pass over the series, with the residual's scale.
    fn npv_and_derivative(self, rate: f64) -> (root::Residual, f64) {
        let discount = 1.0 / (1.0 + rate);
        let mut factor = 1.0; // discountᵗ
        let mut npv = 0.0;
        let mut scale = 0.0;
        let mut derivative = 0.0;
        let mut t = 0.0;
        for cf in self.flows {
            let amount = cf.value();
            npv += amount * factor;
            scale += abs(amount) * abs(factor);
            derivative += -t * amount * factor * discount;
            factor *= discount;
            t += 1.0;
        }
        (root::Residual { value: npv, scale }, derivative)
    }
}

/// The modified internal rate of return — a transcendental cashflow operation, so
/// behind the `std` / `libm` features (ADR-0026), unlike the arithmetic-only
/// NPV/NFV/IRR above.
#[cfg(any(feature = "std", feature = "libm"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "libm"))))]
impl<P: Periodicity> Cashflows<'_, P> {
    /// The **modified** internal rate of return: the per-period rate at which the
    /// present value of the series' outflows grows to the future value of its
    /// inflows over the series' life.
    ///
    /// Unlike [`internal_rate_of_return`](Self::internal_rate_of_return), MIRR is
    /// unique. It discounts the **outflows** (negative cashflows) back to period
    /// `0` at `finance_rate`, compounds the **inflows** (positive cashflows)
    /// forward to the final period at `reinvestment_rate`, and returns the single
    /// rate equating the two: `MIRR = (TVᵢₙ / −PVₒᵤₜ)^(1/N) − 1` for a series
    /// spanning `N` periods (the index of its last cashflow). This resolves the
    /// multiple-root ambiguity a non-conventional series gives plain IRR
    /// (`docs/adr/0020-robust-irr-newton-with-bisection-fallback.md`), at the cost
    /// of two explicit rate assumptions instead of none.
    ///
    /// The two accumulations are arithmetic-only, but the terminal `N`-th root
    /// needs `powf`, which is why this operation is feature-gated (ADR-0026). All
    /// three rates share the periodicity `P`.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{Cashflows, Money, Monthly, Rate};
    ///
    /// // Pay 1000 now and 500 next month, then receive 800 and 900.
    /// let flows = [
    ///     Money::agnostic(-1000.0)?,
    ///     Money::agnostic(-500.0)?,
    ///     Money::agnostic(800.0)?,
    ///     Money::agnostic(900.0)?,
    /// ];
    /// let project = Cashflows::<Monthly>::new(&flows);
    ///
    /// let mirr = project.modified_internal_rate_of_return(
    ///     Rate::<Monthly>::new(0.10)?, // finance rate for the outflows
    ///     Rate::<Monthly>::new(0.12)?, // reinvestment rate for the inflows
    /// )?;
    /// assert!((mirr.value() - 0.072819).abs() < 1e-5);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// - [`TvmError::EmptyCashflows`] if the series is empty.
    /// - [`TvmError::ZeroPeriods`] if the series has fewer than two cashflows, so
    ///   `N = 0` and there is no span to annualise over (ADR-0052).
    /// - [`TvmError::NoOutflows`] if the series has no outflows to discount — no
    ///   present value to grow from (ADR-0052).
    /// - [`TvmError::Overflow`] if the terminal value overflows on extreme
    ///   magnitudes.
    /// - [`TvmError::RateOutOfRange`] if the series has no inflows — the terminal
    ///   value is zero, so the implied rate is `−100%`.
    pub fn modified_internal_rate_of_return(
        self,
        finance_rate: Rate<P>,
        reinvestment_rate: Rate<P>,
    ) -> Result<Rate<P>, TvmError> {
        if self.flows.is_empty() {
            return Err(TvmError::EmptyCashflows);
        }
        if self.flows.len() < 2 {
            // A single cashflow spans no periods, so there is nothing to annualise.
            return Err(TvmError::ZeroPeriods);
        }

        let finance_discount = 1.0 / (1.0 + finance_rate.value());
        let reinvest_growth = 1.0 + reinvestment_rate.value();
        let reinvest_discount = 1.0 / reinvest_growth;

        // Present value of the outflows at period 0 (finance rate), and the inflows
        // discounted to period 0 at the reinvestment rate — compounded up to the
        // final period below. Factors run alongside a float period counter, so no
        // `usize as f64` cast is needed.
        let mut present_outflows = 0.0; // ≤ 0
        let mut discounted_inflows = 0.0; // Σ inflow · reinvest_growth⁻ᵗ
        let mut finance_factor = 1.0; // finance_discount ᵗ
        let mut reinvest_factor = 1.0; // reinvest_discount ᵗ
        let mut periods = 0.0;
        for cf in self.flows {
            let amount = cf.value();
            if amount < 0.0 {
                present_outflows += amount * finance_factor;
            } else if amount > 0.0 {
                discounted_inflows += amount * reinvest_factor;
            }
            finance_factor *= finance_discount;
            reinvest_factor *= reinvest_discount;
            periods += 1.0;
        }
        let n = periods - 1.0; // index of the last cashflow, ≥ 1 here

        if present_outflows == 0.0 {
            // No outflows: there is no present value to grow from, so the
            // annualised return is undefined rather than merely too large.
            return Err(TvmError::NoOutflows);
        }

        // Compound the inflows forward to period n, then take the n-th root of the
        // growth from the outflows' present value to the inflows' terminal value.
        let terminal_inflows = discounted_inflows * powf(reinvest_growth, n);
        let growth = terminal_inflows / -present_outflows;
        Rate::from_operation(powf(growth, 1.0 / n) - 1.0)
    }
}

/// An **owned** cashflow series — the allocating complement to the borrowed
/// [`Cashflows`], behind the `alloc` feature (implied by `std`; ADR-0043).
///
/// [`Cashflows`] borrows a `&[Money]` and stays `no_std`/allocation-free
/// (ADR-0013). `OwnedCashflows` owns a `Vec<Money>`, so it can be **built from an
/// iterator** or handed around without keeping the source slice alive — at the
/// cost of an allocation. It carries the same periodicity tag `P`.
///
/// The operations are **not reimplemented** here: an owned series lends a borrowed
/// [`Cashflows`] view via [`as_cashflows`](Self::as_cashflows), and the methods
/// below forward to it, so there is a single source of truth for the math. (A new
/// operation added to [`Cashflows`] should gain a one-line forward here too.)
///
/// # Examples
///
/// ```
/// use time_value::{Money, Monthly, OwnedCashflows, Rate};
///
/// // Build straight from an iterator — no backing slice to keep alive.
/// let series: OwnedCashflows<Monthly> =
///     [-100.0, 60.0, 60.0].into_iter().map(Money::agnostic).collect::<Result<_, _>>()?;
/// let npv = series.net_present_value(Rate::<Monthly>::new(0.01)?)?;
/// assert!((npv.value() - 18.2237).abs() < 1e-3);
/// # Ok::<(), time_value::TvmError>(())
/// ```
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedCashflows<P: Periodicity> {
    flows: alloc::vec::Vec<Money>,
    marker: PhantomData<P>,
}

#[cfg(feature = "alloc")]
impl<P: Periodicity> OwnedCashflows<P> {
    /// Takes ownership of `flows`; `flows[t]` occurs at period `t`.
    #[must_use]
    pub const fn new(flows: alloc::vec::Vec<Money>) -> Self {
        Self {
            flows,
            marker: PhantomData,
        }
    }

    /// Borrows the series as a [`Cashflows`] view — the bridge the forwarding
    /// operations go through, and the way to reach any [`Cashflows`] method not
    /// forwarded here.
    #[must_use]
    pub fn as_cashflows(&self) -> Cashflows<'_, P> {
        Cashflows::new(&self.flows)
    }

    /// The underlying cashflows.
    #[must_use]
    pub fn as_slice(&self) -> &[Money] {
        &self.flows
    }

    /// Consumes the series, returning the owned `Vec`.
    #[must_use]
    pub fn into_vec(self) -> alloc::vec::Vec<Money> {
        self.flows
    }

    /// The number of cashflows in the series.
    #[must_use]
    pub fn len(&self) -> usize {
        self.flows.len()
    }

    /// Whether the series is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
    }

    /// The net present value of the series discounted at `rate`. See
    /// [`Cashflows::net_present_value`].
    ///
    /// # Errors
    ///
    /// As [`Cashflows::net_present_value`].
    pub fn net_present_value(&self, rate: Rate<P>) -> Result<Money, TvmError> {
        self.as_cashflows().net_present_value(rate)
    }

    /// The net future value of the series compounded at `rate`. See
    /// [`Cashflows::net_future_value`].
    ///
    /// # Errors
    ///
    /// As [`Cashflows::net_future_value`].
    pub fn net_future_value(&self, rate: Rate<P>) -> Result<Money, TvmError> {
        self.as_cashflows().net_future_value(rate)
    }

    /// The internal rate of return from a default 10% guess. See
    /// [`Cashflows::internal_rate_of_return`].
    ///
    /// # Errors
    ///
    /// As [`Cashflows::internal_rate_of_return`].
    pub fn internal_rate_of_return(&self) -> Result<Rate<P>, TvmError> {
        self.as_cashflows().internal_rate_of_return()
    }

    /// The internal rate of return seeded with `guess`. See
    /// [`Cashflows::internal_rate_of_return_from`].
    ///
    /// # Errors
    ///
    /// As [`Cashflows::internal_rate_of_return_from`].
    pub fn internal_rate_of_return_from(&self, guess: f64) -> Result<Rate<P>, TvmError> {
        self.as_cashflows().internal_rate_of_return_from(guess)
    }
}

/// The modified internal rate of return forwards like the others, but is gated on
/// the transcendental-math features that give [`Cashflows`] its `mirr` (ADR-0026).
#[cfg(all(feature = "alloc", any(feature = "std", feature = "libm")))]
#[cfg_attr(
    docsrs,
    doc(cfg(all(feature = "alloc", any(feature = "std", feature = "libm"))))
)]
impl<P: Periodicity> OwnedCashflows<P> {
    /// The modified internal rate of return. See
    /// [`Cashflows::modified_internal_rate_of_return`].
    ///
    /// # Errors
    ///
    /// As [`Cashflows::modified_internal_rate_of_return`].
    pub fn modified_internal_rate_of_return(
        &self,
        finance_rate: Rate<P>,
        reinvestment_rate: Rate<P>,
    ) -> Result<Rate<P>, TvmError> {
        self.as_cashflows()
            .modified_internal_rate_of_return(finance_rate, reinvestment_rate)
    }
}

#[cfg(feature = "alloc")]
impl<P: Periodicity> From<alloc::vec::Vec<Money>> for OwnedCashflows<P> {
    fn from(flows: alloc::vec::Vec<Money>) -> Self {
        Self::new(flows)
    }
}

#[cfg(feature = "alloc")]
impl<P: Periodicity> From<Cashflows<'_, P>> for OwnedCashflows<P> {
    /// Copies a borrowed series into an owned one.
    fn from(borrowed: Cashflows<'_, P>) -> Self {
        Self::new(borrowed.as_slice().to_vec())
    }
}

#[cfg(feature = "alloc")]
impl<P: Periodicity> FromIterator<Money> for OwnedCashflows<P> {
    /// Collects cashflows in period order (`0, 1, 2, …`) into an owned series.
    fn from_iter<I: IntoIterator<Item = Money>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::root::within;
    use crate::{Cashflows, Money, Monthly, Rate, TvmError};

    /// `no_std`-safe approximate equality for the tests (no `f64::abs`).
    fn approx(a: f64, b: f64) -> bool {
        within(a - b, 1e-6)
    }

    fn money(values: &[f64]) -> [Money; 3] {
        assert_eq!(values.len(), 3);
        [
            Money::agnostic(values[0]).unwrap(),
            Money::agnostic(values[1]).unwrap(),
            Money::agnostic(values[2]).unwrap(),
        ]
    }

    /// `money`, for a series of any length — the tests that need four or five
    /// flows. No allocation, so it stays available in the `no_std` build.
    fn agnostic<const N: usize>(values: [f64; N]) -> [Money; N] {
        values.map(|value| Money::agnostic(value).unwrap())
    }

    #[test]
    fn npv_matches_manual_sum() {
        let flows = money(&[-100.0, 60.0, 60.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let rate = Rate::<Monthly>::new(0.01).unwrap();
        let expected = -100.0 + 60.0 / 1.01 + 60.0 / (1.01 * 1.01);
        assert!(approx(
            series.net_present_value(rate).unwrap().value(),
            expected
        ));
    }

    #[test]
    fn npv_at_zero_rate_is_the_plain_sum() {
        let flows = money(&[-100.0, 60.0, 60.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let rate = Rate::<Monthly>::new(0.0).unwrap();
        assert!(approx(
            series.net_present_value(rate).unwrap().value(),
            20.0
        ));
    }

    #[test]
    fn nfv_is_npv_compounded_to_the_final_period() {
        let flows = money(&[-100.0, 60.0, 60.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let rate = Rate::<Monthly>::new(0.01).unwrap();
        let present = series.net_present_value(rate).unwrap().value();
        let future = series.net_future_value(rate).unwrap().value();
        // NFV = NPV * (1 + r)^(n - 1); here n = 3.
        assert!(approx(future, present * 1.01 * 1.01));
    }

    #[test]
    fn irr_zeroes_the_npv() {
        let flows = money(&[-100.0, 60.0, 60.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let irr = series.internal_rate_of_return().unwrap();
        // Discounting at the IRR gives (approximately) zero NPV.
        assert!(within(series.net_present_value(irr).unwrap().value(), 1e-6));
        // Closed form: 3x^2 + 3x - 5 = 0, x = 1/(1+r), x = (-3 + sqrt(69))/6,
        // giving r = 0.130662386…
        assert!(approx(irr.value(), 0.130_662_386));
    }

    /// The `# Errors` sections of `net_present_value` / `net_future_value` name
    /// `CurrencyMismatch`; pin it (ADR-0045 rule 2, ADR-0052).
    #[test]
    fn npv_and_nfv_reject_a_series_of_mixed_currencies() {
        use crate::Currency;
        let flows = [
            Money::new(-100.0, Currency::Usd).unwrap(),
            Money::new(60.0, Currency::Eur).unwrap(),
        ];
        let series = Cashflows::<Monthly>::new(&flows);
        let rate = Rate::<Monthly>::new(0.01).unwrap();
        // `left` is the currency accumulated so far, `right` the offending flow's.
        let expected = TvmError::CurrencyMismatch {
            left: Currency::Usd,
            right: Currency::Eur,
        };
        assert_eq!(series.net_present_value(rate), Err(expected.clone()));
        assert_eq!(series.net_future_value(rate), Err(expected));
    }

    #[test]
    fn irr_on_empty_series_errors() {
        let flows: [Money; 0] = [];
        let series = Cashflows::<Monthly>::new(&flows);
        assert_eq!(
            series.internal_rate_of_return(),
            Err(TvmError::EmptyCashflows)
        );
    }

    #[test]
    fn irr_without_a_root_does_not_converge() {
        // All inflows: NPV is positive for every rate > -100%, so there is no IRR.
        let flows = money(&[100.0, 60.0, 60.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        assert_eq!(
            series.internal_rate_of_return(),
            Err(TvmError::IrrDidNotConverge)
        );
    }

    #[test]
    fn irr_falls_back_to_bisection_from_a_bad_guess() {
        // A wildly off guess sends Newton out of the valid domain on its first
        // step; the bracketing fallback must still find the same root.
        let flows = money(&[-100.0, 60.0, 60.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let irr = series.internal_rate_of_return_from(1e6).unwrap();
        assert!(within(series.net_present_value(irr).unwrap().value(), 1e-6));
        assert!(approx(irr.value(), 0.130_662_386));
    }

    #[test]
    fn irr_recovers_a_large_rate() {
        // Root well above the 10% default guess: -1 now, +2 next period is a
        // 100% per-period return. Newton reaches it, but confirm the value.
        let flows = money(&[-1.0, 2.0, 0.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let irr = series.internal_rate_of_return().unwrap();
        assert!(within(series.net_present_value(irr).unwrap().value(), 1e-6));
        assert!(approx(irr.value(), 1.0));
    }

    #[test]
    fn npv_and_nfv_of_an_empty_series_are_zero() {
        // Convention (ADR-0021): nothing to discount or compound is `Ok(0)`.
        let empty: [Money; 0] = [];
        let series = Cashflows::<Monthly>::new(&empty);
        let rate = Rate::<Monthly>::new(0.05).unwrap();
        assert_eq!(series.net_present_value(rate).unwrap(), Money::ZERO);
        assert_eq!(series.net_future_value(rate).unwrap(), Money::ZERO);
    }

    #[test]
    fn npv_and_nfv_overflow_to_a_non_finite_result() {
        // Two near-max cashflows sum past `f64::MAX`, so both discounted and
        // compounded totals overflow — surfaced as an error, not a silent `inf`.
        let big = [
            Money::agnostic(f64::MAX).unwrap(),
            Money::agnostic(f64::MAX).unwrap(),
        ];
        let series = Cashflows::<Monthly>::new(&big);
        let rate = Rate::<Monthly>::new(0.0).unwrap();
        assert_eq!(series.net_present_value(rate), Err(TvmError::Overflow));
        assert_eq!(series.net_future_value(rate), Err(TvmError::Overflow));
    }

    /// The defect ADR-0054 fixes. `[0, 0, -100, 110]` is an ordinary
    /// construction-phase shape — nothing happens for two periods, then an outlay
    /// and a repayment — and its IRR is exactly `0.1`, uniquely:
    /// `−100(1+r)⁻² + 110(1+r)⁻³ = 0` gives `1 + r = 1.1`. But `NPV(r) → CF₀ = 0`
    /// as `r → ∞`, so against a tolerance fixed from `Σ|CFₜ|` every large enough
    /// rate passed: `internal_rate_of_return_from(0.9)` answered `28114.44`, a
    /// return of 2,811,444% per period.
    #[test]
    fn irr_rejects_a_root_that_is_only_zero_because_everything_discounted_away() {
        let flows = agnostic([0.0, 0.0, -100.0, 110.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        for guess in [0.1, 0.9, 5.0, 50.0, 0.0, -0.5] {
            let irr = series.internal_rate_of_return_from(guess).unwrap();
            assert!(
                approx(irr.value(), 0.1),
                "guess {guess} found {} instead of the unique IRR 0.1",
                irr.value(),
            );
        }
    }

    /// The mechanism, confirmed from the other side: give the first flow a
    /// magnitude above the old tolerance and the spurious roots disappear even
    /// under the old rule. Both series must now solve to (near) the same rate.
    #[test]
    fn a_leading_zero_flow_does_not_change_the_rate_it_solves_to() {
        let zero_led = agnostic([0.0, 0.0, -100.0, 110.0]);
        let tiny_led = agnostic([-1e-6, 0.0, -100.0, 110.0]);
        let a = Cashflows::<Monthly>::new(&zero_led)
            .internal_rate_of_return_from(0.9)
            .unwrap();
        let b = Cashflows::<Monthly>::new(&tiny_led)
            .internal_rate_of_return_from(0.9)
            .unwrap();
        assert!(within(a.value() - b.value(), 1e-6));
    }

    /// Whatever rate the solver returns, it must actually zero the NPV *relative to
    /// the cashflows still being discounted at that rate* — the property the
    /// acceptance rule now encodes. A rate 5 orders of magnitude off cannot satisfy
    /// it, however small the raw NPV happens to be there (ADR-0054).
    #[test]
    fn every_located_irr_zeroes_the_npv_relative_to_its_own_terms() {
        fn check<const N: usize>(values: [f64; N]) {
            let flows = agnostic(values);
            let cashflows = Cashflows::<Monthly>::new(&flows);
            let Ok(irr) = cashflows.internal_rate_of_return() else {
                return; // no root at all is a different (and honest) answer
            };
            let npv = cashflows.net_present_value(irr).unwrap().value();
            // `Σ|CFₜ|(1+r)⁻ᵗ` at the located rate — the same scale the solver used.
            let discount = 1.0 / (1.0 + irr.value());
            let mut factor = 1.0;
            let mut scale = 0.0;
            for value in values {
                scale += crate::root::abs(value) * factor;
                factor *= discount;
            }
            assert!(
                crate::root::abs(npv) <= 1e-6 * scale,
                "{values:?} solved to {} with NPV {npv} against a term scale of {scale}",
                irr.value(),
            );
        }

        check([-100.0, 60.0, 60.0]);
        check([0.0, 0.0, -100.0, 110.0]);
        check([0.0, 0.0, 0.0, -100.0, 110.0]);
        check([-1.0, 2.0, 0.0]);
        check([-1_000_000.0, 600_000.0, 600_000.0]);
        check([-100.0, 230.0, -132.0]);
    }

    #[test]
    fn irr_converges_for_large_magnitude_cashflows() {
        // Millions-scale: an absolute NPV tolerance would be unreachable, but the
        // magnitude-scaled tolerance (ADR-0021) converges to the same rate as the
        // unit-scale version of this series (irr_zeroes_the_npv).
        let flows = money(&[-1_000_000.0, 600_000.0, 600_000.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        let irr = series.internal_rate_of_return().unwrap();
        assert!(approx(irr.value(), 0.130_662_386));
    }

    #[test]
    fn len_and_is_empty() {
        let flows = money(&[-1.0, 2.0, 3.0]);
        let series = Cashflows::<Monthly>::new(&flows);
        assert_eq!(series.len(), 3);
        assert!(!series.is_empty());

        let empty: [Money; 0] = [];
        assert!(Cashflows::<Monthly>::new(&empty).is_empty());
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    mod mirr {
        use super::{approx, Cashflows, Money, Monthly, Rate, TvmError};

        fn rate(r: f64) -> Rate<Monthly> {
            Rate::<Monthly>::new(r).unwrap()
        }

        fn series(values: &[f64]) -> [Money; 4] {
            assert_eq!(values.len(), 4);
            [
                Money::agnostic(values[0]).unwrap(),
                Money::agnostic(values[1]).unwrap(),
                Money::agnostic(values[2]).unwrap(),
                Money::agnostic(values[3]).unwrap(),
            ]
        }

        #[test]
        fn matches_the_manual_formula() {
            // Outflows -1000 (t0) and -500 (t1) discounted at 10% -> PV -1454.5454;
            // inflows 800 (t2) and 900 (t3) compounded to t3 at 12% -> TV 1796;
            // MIRR = (1796 / 1454.5454)^(1/3) - 1 = 0.0728187…
            let flows = series(&[-1000.0, -500.0, 800.0, 900.0]);
            let mirr = Cashflows::<Monthly>::new(&flows)
                .modified_internal_rate_of_return(rate(0.10), rate(0.12))
                .unwrap();
            assert!(approx(mirr.value(), 0.072_818_724_6));
        }

        #[test]
        fn equal_rates_and_a_single_outflow() {
            // One outflow at t0, so the finance rate is inert; 500 each at t1..t3
            // compounded at 10% to t3 gives TV 1655, MIRR = 1.655^(1/3) - 1.
            let flows = series(&[-1000.0, 500.0, 500.0, 500.0]);
            let mirr = Cashflows::<Monthly>::new(&flows)
                .modified_internal_rate_of_return(rate(0.10), rate(0.10))
                .unwrap();
            assert!(approx(mirr.value(), 0.182_858_148_6));
        }

        #[test]
        fn empty_series_errors() {
            let empty: [Money; 0] = [];
            assert_eq!(
                Cashflows::<Monthly>::new(&empty)
                    .modified_internal_rate_of_return(rate(0.10), rate(0.10)),
                Err(TvmError::EmptyCashflows)
            );
        }

        #[test]
        fn single_cashflow_has_no_span() {
            // N = 0: the two degenerate MIRR cases are now told apart (ADR-0052).
            let flows = [Money::agnostic(-1000.0).unwrap()];
            assert_eq!(
                Cashflows::<Monthly>::new(&flows)
                    .modified_internal_rate_of_return(rate(0.10), rate(0.10)),
                Err(TvmError::ZeroPeriods)
            );
        }

        #[test]
        fn no_outflows_has_no_present_value_to_grow_from() {
            let flows = series(&[1000.0, 500.0, 500.0, 500.0]);
            assert_eq!(
                Cashflows::<Monthly>::new(&flows)
                    .modified_internal_rate_of_return(rate(0.10), rate(0.10)),
                Err(TvmError::NoOutflows)
            );
        }

        #[test]
        fn no_inflows_is_a_total_loss() {
            // No positive cashflow, so the terminal value is 0 and MIRR = -100%.
            let flows = series(&[-1000.0, -500.0, -500.0, -500.0]);
            assert_eq!(
                Cashflows::<Monthly>::new(&flows)
                    .modified_internal_rate_of_return(rate(0.10), rate(0.10)),
                Err(TvmError::RateOutOfRange)
            );
        }
    }

    #[cfg(feature = "alloc")]
    mod owned {
        use alloc::vec::Vec;

        use super::{approx, Cashflows, Money, Monthly, Rate};
        use crate::OwnedCashflows;

        fn flows() -> Vec<Money> {
            [-100.0, 60.0, 60.0]
                .into_iter()
                .map(|v| Money::agnostic(v).unwrap())
                .collect()
        }

        #[test]
        fn owned_operations_match_the_borrowed_view() {
            let v = flows();
            let rate = Rate::<Monthly>::new(0.01).unwrap();
            let borrowed = Cashflows::<Monthly>::new(&v);
            let owned = OwnedCashflows::<Monthly>::new(v.clone());

            assert_eq!(
                owned.net_present_value(rate).unwrap(),
                borrowed.net_present_value(rate).unwrap()
            );
            assert_eq!(
                owned.net_future_value(rate).unwrap(),
                borrowed.net_future_value(rate).unwrap()
            );
            assert_eq!(
                owned.internal_rate_of_return().unwrap(),
                borrowed.internal_rate_of_return().unwrap()
            );
        }

        #[test]
        fn builds_from_an_iterator() {
            let owned: OwnedCashflows<Monthly> = [-100.0, 60.0, 60.0]
                .into_iter()
                .map(Money::agnostic)
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(owned.len(), 3);
            assert!(approx(
                owned.internal_rate_of_return().unwrap().value(),
                0.130_662_386
            ));
        }

        #[test]
        fn from_vec_and_from_a_borrowed_view_agree() {
            let v = flows();
            let from_vec = OwnedCashflows::<Monthly>::from(v.clone());
            let from_borrowed = OwnedCashflows::<Monthly>::from(Cashflows::<Monthly>::new(&v));
            assert_eq!(from_vec, from_borrowed);
            assert_eq!(from_vec.as_slice(), &v[..]);
        }

        #[test]
        fn as_cashflows_bridges_and_into_vec_recovers() {
            let v = flows();
            let owned = OwnedCashflows::<Monthly>::new(v.clone());
            assert_eq!(owned.as_cashflows().len(), 3);
            assert!(!owned.is_empty());
            assert_eq!(owned.into_vec(), v);
        }

        #[test]
        fn an_empty_owned_series_is_zero() {
            let owned = OwnedCashflows::<Monthly>::new(Vec::new());
            assert!(owned.is_empty());
            let rate = Rate::<Monthly>::new(0.05).unwrap();
            assert_eq!(owned.net_present_value(rate).unwrap(), Money::ZERO);
        }

        #[cfg(any(feature = "std", feature = "libm"))]
        #[test]
        fn owned_mirr_matches_the_borrowed_view() {
            let v: Vec<Money> = [-1000.0, -500.0, 800.0, 900.0]
                .into_iter()
                .map(|x| Money::agnostic(x).unwrap())
                .collect();
            let finance = Rate::<Monthly>::new(0.10).unwrap();
            let reinvest = Rate::<Monthly>::new(0.12).unwrap();
            assert_eq!(
                OwnedCashflows::<Monthly>::new(v.clone())
                    .modified_internal_rate_of_return(finance, reinvest)
                    .unwrap(),
                Cashflows::<Monthly>::new(&v)
                    .modified_internal_rate_of_return(finance, reinvest)
                    .unwrap()
            );
        }
    }
}
