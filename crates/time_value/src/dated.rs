//! [`DatedCashflows`] — cashflows on irregular calendar dates, discounted by the
//! year-fraction from a reference (XNPV / XIRR / XNFV / dated MIRR).
//!
//! Unlike [`Cashflows`](crate::Cashflows), whose flows sit at evenly spaced
//! periods, these flows carry an explicit **year-offset** and are discounted by
//! `(1 + r)^t` for a fractional `t` — so this module is behind `std` / `libm`
//! (it needs [`powf`](crate::math::powf)). The rate is annual: offsets are years,
//! so a [`Rate<Annual>`] is required and a per-period rate is a compile error
//! (`docs/adr/0029-dated-cashflows-xnpv-xirr.md`).
//!
//! **Two anchors, not one.** The offsets are arbitrary and need not be sorted, so the
//! operations here name the date they work at explicitly — the present value at the
//! first-listed flow (Excel's XNPV, ADR-0029), the future value at the *latest* offset,
//! and the dated MIRR over the whole life from earliest to latest, while the rate
//! solves have no date at all. This module is private and its documentation is not
//! rendered, so the table lives on [`DatedCashflows`] itself
//! (`docs/adr/0065-dated-counterparts.md`).

use crate::math::powf;
use crate::money::combine;
use crate::root::{self, abs};
use crate::{Annual, Money, Rate, TvmError};

/// A single cashflow at an offset, in **years**, from a reference point.
///
/// The offset may be negative (a flow before the reference) or zero, but must be
/// finite. Amounts are signed (outflow negative, inflow positive). The reference
/// is supplied by the enclosing [`DatedCashflows`] — its first flow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DatedCashflow {
    offset_years: f64,
    amount: Money,
}

impl DatedCashflow {
    /// A cashflow of `amount` at `offset_years` from the reference.
    ///
    /// # Errors
    ///
    /// [`TvmError::NonFiniteOffset`] if `offset_years` is not finite.
    pub fn new(offset_years: f64, amount: Money) -> Result<Self, TvmError> {
        if !offset_years.is_finite() {
            return Err(TvmError::NonFiniteOffset);
        }
        Ok(Self {
            offset_years,
            amount,
        })
    }

    /// The offset, in years, from the reference.
    #[must_use]
    pub const fn offset_years(self) -> f64 {
        self.offset_years
    }

    /// The signed cashflow amount.
    #[must_use]
    pub const fn amount(self) -> Money {
        self.amount
    }
}

/// A series of cashflows on irregular dates, discounted by year-fraction.
///
/// `DatedCashflows` **borrows** its slice (allocation-free, like
/// [`Cashflows`](crate::Cashflows); ADR-0013). The **first** flow is the
/// valuation reference: every flow is discounted by `(1 + r)^(tᵢ − t₀)`, so the
/// first flow is undiscounted. Rebasing to the first entry (rather than the
/// earliest) matches Excel's XNPV/XIRR.
///
/// # Which date each operation is anchored at
///
/// That first-entry reference belongs to the *present* value alone. The offsets are
/// arbitrary and **need not be sorted**, so where the periodic
/// [`Cashflows`](crate::Cashflows) has one anchor this type has three — a value is
/// quoted at a date, a rate is not (ADR-0065). Writing `t₀` for the first-listed
/// offset, `t₋` for the earliest and `T` for the latest:
///
/// | operation | anchored at | depends on the slice order? |
/// | --- | --- | --- |
/// | [`net_present_value`](Self::net_present_value) | `t₀`, the first entry | **yes** |
/// | [`net_future_value`](Self::net_future_value) | `T`, the horizon | no |
/// | [`internal_rate_of_return`](Self::internal_rate_of_return) | — a rate has no date | no |
/// | [`modified_internal_rate_of_return`](Self::modified_internal_rate_of_return) | `t₋` → `T`, the whole life | no |
///
/// For a series listed in date order all three coincide with the obvious reading, so
/// the distinction only bites on unsorted input — which this type accepts.
///
/// [`OwnedDatedCashflows`] is the allocating counterpart.
///
/// # Examples
///
/// ```
/// use time_value::{Annual, DatedCashflow, DatedCashflows, Money, Rate};
///
/// // Pay 100 now, receive 110 exactly one year later: a 10% annual return.
/// let flows = [
///     DatedCashflow::new(0.0, Money::agnostic(-100.0)?)?,
///     DatedCashflow::new(1.0, Money::agnostic(110.0)?)?,
/// ];
/// let dated = DatedCashflows::new(&flows);
///
/// let irr = dated.internal_rate_of_return()?;
/// assert!((irr.value() - 0.10).abs() < 1e-9);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// A per-period rate does not type-check — the discount is annual:
///
/// ```compile_fail
/// use time_value::{DatedCashflow, DatedCashflows, Money, Monthly, Rate};
///
/// let flows = [DatedCashflow::new(0.0, Money::agnostic(-100.0).unwrap()).unwrap()];
/// let dated = DatedCashflows::new(&flows);
/// let monthly = Rate::<Monthly>::new(0.01).unwrap();
/// let _ = dated.net_present_value(monthly); // wrong periodicity — won't compile
/// ```
// `OwnedDatedCashflows` lives behind `alloc`, so in a build without it the
// intra-doc link above has no target and rustdoc would warn (ADR-0055). A markdown
// link *reference definition* beats intra-doc resolution, so define it as the
// docs.rs URL when the feature is off — the same mechanism the crate root uses.
#[cfg_attr(
    not(feature = "alloc"),
    doc = "

[`OwnedDatedCashflows`]: https://docs.rs/time_value/latest/time_value/struct.OwnedDatedCashflows.html
"
)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DatedCashflows<'a> {
    flows: &'a [DatedCashflow],
}

impl<'a> DatedCashflows<'a> {
    /// Wraps a slice of dated cashflows; the first flow is the valuation reference.
    #[must_use]
    pub const fn new(flows: &'a [DatedCashflow]) -> Self {
        Self { flows }
    }

    /// The underlying dated cashflows.
    #[must_use]
    pub const fn as_slice(self) -> &'a [DatedCashflow] {
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

    /// The single [`Currency`](crate::Currency) the series is denominated in, by
    /// the [`Currency::Xxx`](crate::Currency::Xxx) identity rule. An empty (or
    /// wholly agnostic) series is `Xxx`.
    ///
    /// The dated counterpart of [`Cashflows::currency`](crate::Cashflows::currency),
    /// with the same purpose: it is the fold
    /// [`net_present_value`](Self::net_present_value) runs for itself, exposed so a
    /// caller can learn the denomination — or reject a malformed series — without
    /// paying for the XNPV. It is likewise the strict reading ADR-0057 points a
    /// caller of [`internal_rate_of_return`](Self::internal_rate_of_return) at,
    /// since the rate solves do not fold the currencies.
    ///
    /// ```
    /// use time_value::{Currency, DatedCashflow, DatedCashflows, Money};
    ///
    /// let flows = [
    ///     DatedCashflow::new(0.0, Money::agnostic(-100.0)?)?,
    ///     DatedCashflow::new(1.0, Money::new(110.0, Currency::Jpy)?)?,
    /// ];
    /// assert_eq!(DatedCashflows::new(&flows).currency()?, Currency::Jpy);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::CurrencyMismatch`] if the flows mix distinct non-`Xxx`
    /// currencies (ADR-0034). The fold stops at the first clash, so `left` is what
    /// had accumulated and `right` is the flow that broke it.
    pub fn currency(self) -> Result<crate::Currency, TvmError> {
        let mut acc = crate::Currency::Xxx;
        for cf in self.flows {
            acc = combine(acc, cf.amount.currency())?;
        }
        Ok(acc)
    }

    /// The net present value of the dated series discounted at an annual `rate`
    /// (XNPV): `Σᵢ CFᵢ / (1 + r)^(tᵢ − t₀)`, with `tᵢ` the offset in years and
    /// `t₀` the first flow's offset. An **empty** series has value `0`.
    ///
    /// # Errors
    ///
    /// [`TvmError::CurrencyMismatch`] if the flows mix distinct currencies, or
    /// [`TvmError::Overflow`] if the sum overflows to a non-finite value
    /// (ADR-0021).
    pub fn net_present_value(self, rate: Rate<Annual>) -> Result<Money, TvmError> {
        let currency = self.currency()?;
        Money::from_operation(self.xnpv_at(rate.value()).value, currency)
    }

    /// The net **future** value of the dated series at its horizon, compounded at an
    /// annual `rate`: `Σᵢ CFᵢ (1 + r)^(T − tᵢ)`, where `T = max tᵢ` is the **latest**
    /// offset in the series. An **empty** series has value `0`.
    ///
    /// # The horizon is the latest offset, not the last entry
    ///
    /// [`Cashflows::net_future_value`](crate::Cashflows::net_future_value) compounds
    /// to "the final period", which for evenly spaced flows is at once the last
    /// index *and* the latest point in time. Dated flows need not be sorted
    /// (ADR-0029 handles arbitrary order and negative offsets), so those two readings
    /// come apart and this one takes the **latest date** (ADR-0065):
    ///
    /// - Every exponent `T − tᵢ` is then `≥ 0`, so every flow is *compounded* — which
    ///   is what makes the answer a future value. Compounding to the last *entry* of
    ///   an unsorted slice would discount the flows dated after it, giving a value at
    ///   an arbitrary interior date.
    /// - The result does not depend on the order of the slice, unlike
    ///   [`net_present_value`](Self::net_present_value), whose reference *is* the
    ///   first entry. Where the latest flow happens to be listed first the two agree
    ///   exactly, since both then value the series at the same date.
    ///
    /// It is therefore the XNPV compounded over the series' life:
    /// `XNFV = XNPV · (1 + r)^(T − t₀)`, a span that is never negative.
    ///
    /// ```
    /// use time_value::{Annual, DatedCashflow, DatedCashflows, Money, Rate};
    ///
    /// // Pay 100 now, receive 110 exactly one year later, at 10% a year: the
    /// // present value is 0, so the value at the horizon is 0 too.
    /// let flows = [
    ///     DatedCashflow::new(0.0, Money::agnostic(-100.0)?)?,
    ///     DatedCashflow::new(1.0, Money::agnostic(110.0)?)?,
    /// ];
    /// let nfv = DatedCashflows::new(&flows).net_future_value(Rate::<Annual>::new(0.10)?)?;
    /// assert!(nfv.value().abs() < 1e-9);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// [`TvmError::CurrencyMismatch`] if the flows mix distinct currencies, so the
    /// result has no single denomination (ADR-0034, ADR-0057), or
    /// [`TvmError::Overflow`] if the compounded sum overflows to a non-finite value
    /// (ADR-0021).
    pub fn net_future_value(self, rate: Rate<Annual>) -> Result<Money, TvmError> {
        let currency = self.currency()?;
        let mut total = 0.0;
        // An empty series has no horizon and nothing to compound: `0`, as the
        // periodic `net_future_value` does (ADR-0021).
        if let Some((_, horizon)) = self.span_ends() {
            let base = 1.0 + rate.value();
            for cf in self.flows {
                total += cf.amount.value() * powf(base, horizon - cf.offset_years);
            }
        }
        Money::from_operation(total, currency)
    }

    /// The internal rate of return of the dated series (XIRR): the annual
    /// [`Rate<Annual>`] at which its XNPV is zero, from a default guess of 10%.
    ///
    /// # Errors
    ///
    /// See [`internal_rate_of_return_from`](Self::internal_rate_of_return_from).
    pub fn internal_rate_of_return(self) -> Result<Rate<Annual>, TvmError> {
        self.internal_rate_of_return_from(0.1)
    }

    /// The XIRR, seeding the solver with `guess` (an annual rate).
    ///
    /// Like [`Cashflows::internal_rate_of_return_from`](crate::Cashflows::internal_rate_of_return_from),
    /// it tries **Newton–Raphson** from `guess` and falls back to a **bracketing
    /// bisection** over the valid rate domain (ADR-0020), so a root is found
    /// whenever the XNPV changes sign. The convergence tolerance scales with the
    /// cashflow magnitudes (ADR-0021).
    ///
    /// # Currency
    ///
    /// **The flows' currencies are not folded, and never a `CurrencyMismatch`**
    /// (ADR-0057), for the same reason as the periodic
    /// [`Cashflows::internal_rate_of_return_from`](crate::Cashflows::internal_rate_of_return_from):
    /// the result is a rate, not a [`Money`], so it has no denomination to derive.
    /// A series [`net_present_value`](Self::net_present_value) rejects still has an
    /// XIRR. Call [`currency`](Self::currency) first if a mixed series should be an
    /// error at your call site.
    ///
    /// # Errors
    ///
    /// - [`TvmError::EmptyCashflows`] if the series is empty.
    /// - [`TvmError::IrrDidNotConverge`] if neither method finds a root — in
    ///   particular when the XNPV never changes sign over the valid rate domain
    ///   (e.g. cashflows that are all one sign).
    pub fn internal_rate_of_return_from(self, guess: f64) -> Result<Rate<Annual>, TvmError> {
        if self.flows.is_empty() {
            return Err(TvmError::EmptyCashflows);
        }
        // Newton from `guess`, then the robust bracketing fallback (ADR-0020) — all
        // shared with IRR via `root`, including its scale-carrying residual: XIRR
        // has the same `XNPV(r) → CF₀` limit, so a near-zero first flow would
        // otherwise make every large enough rate a root (ADR-0021, ADR-0054).
        match root::newton(|r| self.xnpv_and_derivative(r), guess)
            .or_else(|| root::bracket_and_bisect(|r| self.xnpv_at(r)))
        {
            Some(rate) => Rate::new(rate),
            None => Err(TvmError::IrrDidNotConverge),
        }
    }

    /// The **modified** internal rate of return of the dated series: the annual rate
    /// at which the present value of its outflows grows to the terminal value of its
    /// inflows over the series' life.
    ///
    /// The dated counterpart of
    /// [`Cashflows::modified_internal_rate_of_return`](crate::Cashflows::modified_internal_rate_of_return),
    /// with the same three steps and the same two explicit rate assumptions — it
    /// discounts the **outflows** (negative cashflows) at `finance_rate`, compounds
    /// the **inflows** (positive cashflows) at `reinvestment_rate`, and equates the
    /// two — but over a real-number span of years rather than a count of periods
    /// (ADR-0026, ADR-0065). Writing `t₋` for the **earliest** offset and `T` for the
    /// **latest**:
    ///
    /// ```text
    /// PVₒᵤₜ = Σ_{CFᵢ<0} CFᵢ (1 + f)^(t₋ − tᵢ)     (≤ 0, every exponent ≤ 0)
    /// TVᵢₙ  = Σ_{CFᵢ>0} CFᵢ (1 + i)^(T − tᵢ)      (≥ 0, every exponent ≥ 0)
    /// MIRR  = (TVᵢₙ / −PVₒᵤₜ)^(1 / (T − t₋)) − 1
    /// ```
    ///
    /// The annualising exponent is therefore `T − t₋` **years**, where the periodic
    /// MIRR uses `N = len − 1` *periods*: the span between the two dates the two
    /// amounts are quoted at. On whole-year offsets `0, 1, 2, …` the two operations
    /// agree exactly, which is how they are tested against each other.
    ///
    /// **The span is the series' whole life, earliest to latest — not the
    /// first-listed entry to the last.** MIRR returns a rate, and a rate has no date,
    /// so — like [`internal_rate_of_return`](Self::internal_rate_of_return), whose
    /// root is unchanged by which flow is listed first — the answer does not depend
    /// on the order of the slice (ADR-0065). Taking the *first* entry as the
    /// reference would make it order-dependent, and would collapse the span to zero
    /// for a series merely listed newest-first. All three rates are annual.
    ///
    /// # Currency
    ///
    /// **The flows' currencies are not folded, and never a `CurrencyMismatch`**
    /// (ADR-0057), exactly as for the periodic MIRR and for
    /// [`internal_rate_of_return_from`](Self::internal_rate_of_return_from): the
    /// result is a rate, not a [`Money`], so it has no denomination to derive. Call
    /// [`currency`](Self::currency) first if a mixed series should be an error at
    /// your call site.
    ///
    /// # Examples
    ///
    /// ```
    /// use time_value::{Annual, DatedCashflow, DatedCashflows, Money, Rate};
    ///
    /// // Pay 1000 now and 500 six months in, then receive 800 after 15 months and
    /// // 900 after two years.
    /// let flows = [
    ///     DatedCashflow::new(0.0, Money::agnostic(-1000.0)?)?,
    ///     DatedCashflow::new(0.5, Money::agnostic(-500.0)?)?,
    ///     DatedCashflow::new(1.25, Money::agnostic(800.0)?)?,
    ///     DatedCashflow::new(2.0, Money::agnostic(900.0)?)?,
    /// ];
    /// let mirr = DatedCashflows::new(&flows).modified_internal_rate_of_return(
    ///     Rate::<Annual>::new(0.10)?, // finance rate for the outflows
    ///     Rate::<Annual>::new(0.12)?, // reinvestment rate for the inflows
    /// )?;
    /// assert!((mirr.value() - 0.095_102_924).abs() < 1e-9);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    ///
    /// # Errors
    ///
    /// - [`TvmError::EmptyCashflows`] if the series is empty.
    /// - [`TvmError::NoOutflows`] if the series has no outflows to discount, so there
    ///   is no present value to grow from and the ratio does not exist (ADR-0052).
    /// - [`TvmError::IndeterminateRate`] if the series spans **no time** — every flow
    ///   on one date — and its outflows and inflows already match there, since every
    ///   rate then satisfies it (ADR-0056). Where they do *not* match,
    ///   [`TvmError::NoRealSolution`]: no rate does. This replaces the periodic
    ///   operation's [`ZeroPeriods`](TvmError::ZeroPeriods), which names an input that
    ///   does not exist here — the span is a computed number of years, not a
    ///   `Period<P>` (ADR-0064, ADR-0065).
    /// - [`TvmError::Overflow`] if the terminal value overflows on extreme
    ///   magnitudes.
    /// - [`TvmError::RateOutOfRange`] if the series has no inflows — the terminal
    ///   value is zero, so the implied rate is `−100%`.
    pub fn modified_internal_rate_of_return(
        self,
        finance_rate: Rate<Annual>,
        reinvestment_rate: Rate<Annual>,
    ) -> Result<Rate<Annual>, TvmError> {
        let Some((reference, horizon)) = self.span_ends() else {
            return Err(TvmError::EmptyCashflows);
        };

        let finance_base = 1.0 + finance_rate.value();
        let reinvest_base = 1.0 + reinvestment_rate.value();
        let mut present_outflows = 0.0; // ≤ 0
        let mut terminal_inflows = 0.0; // ≥ 0
        for cf in self.flows {
            let amount = cf.amount.value();
            if amount < 0.0 {
                present_outflows += amount * powf(finance_base, reference - cf.offset_years);
            } else if amount > 0.0 {
                terminal_inflows += amount * powf(reinvest_base, horizon - cf.offset_years);
            }
        }

        if present_outflows == 0.0 {
            // No outflows: no present value to grow from, so the ratio the root is
            // taken of does not exist. Checked before the span, because the ratio has
            // to exist before there is anything to annualise (ADR-0065).
            return Err(TvmError::NoOutflows);
        }

        let years = horizon - reference;
        if years == 0.0 {
            // Every flow on one date: the growth factor is `1` for every rate, so the
            // equation collapses to `−PVₒᵤₜ = TVᵢₙ` with the rate absent. Either every
            // rate satisfies it or none does (ADR-0056), decided by the solver's own
            // root test rather than by `==` — the shared helper's whole point.
            return Err(root::unit_factor_outcome(
                -present_outflows,
                terminal_inflows,
                TvmError::IndeterminateRate,
            ));
        }

        let growth = terminal_inflows / -present_outflows;
        Rate::from_operation(powf(growth, 1.0 / years) - 1.0)
    }

    /// The two ends of the series' life: `(earliest offset, latest offset)`, or
    /// `None` for an empty series.
    ///
    /// The earliest is the reference the dated MIRR discounts its outflows to and the
    /// latest is the horizon both it and [`net_future_value`](Self::net_future_value)
    /// compound to, so the two operations cannot disagree about where the series
    /// begins and ends (ADR-0065). Offsets are finite by construction
    /// ([`DatedCashflow::new`]), so the comparisons cannot see a `NaN`.
    fn span_ends(self) -> Option<(f64, f64)> {
        let mut rest = self.flows.iter();
        let first = rest.next()?.offset_years;
        let mut earliest = first;
        let mut latest = first;
        for cf in rest {
            if cf.offset_years < earliest {
                earliest = cf.offset_years;
            }
            if cf.offset_years > latest {
                latest = cf.offset_years;
            }
        }
        Some((earliest, latest))
    }

    /// The XNPV at a candidate annual `rate`: `Σᵢ CFᵢ (1 + r)^(−tᵢ)`, with `tᵢ`
    /// the offset in years rebased to the first flow. Empty series → `0`.
    fn xnpv_at(self, rate: f64) -> root::Residual {
        let Some(first) = self.flows.first() else {
            return root::Residual {
                value: 0.0,
                scale: 0.0,
            };
        };
        let reference = first.offset_years;
        let base = 1.0 + rate;
        let mut npv = 0.0;
        let mut scale = 0.0;
        for cf in self.flows {
            let years = cf.offset_years - reference;
            let factor = powf(base, -years);
            npv += cf.amount.value() * factor;
            scale += abs(cf.amount.value()) * abs(factor);
        }
        root::Residual { value: npv, scale }
    }

    /// The XNPV, the scale it is judged against (`Σᵢ |CFᵢ| (1+r)^(−tᵢ)`), and its
    /// derivative d(XNPV)/dr at a candidate annual `rate`.
    ///
    /// `XNPV(r) = Σᵢ CFᵢ (1+r)^(−tᵢ)`, `XNPV'(r) = Σᵢ −tᵢ CFᵢ (1+r)^(−tᵢ−1)`.
    fn xnpv_and_derivative(self, rate: f64) -> (root::Residual, f64) {
        let Some(first) = self.flows.first() else {
            return (
                root::Residual {
                    value: 0.0,
                    scale: 0.0,
                },
                0.0,
            );
        };
        let reference = first.offset_years;
        let base = 1.0 + rate;
        let mut npv = 0.0;
        let mut scale = 0.0;
        let mut derivative = 0.0;
        for cf in self.flows {
            let years = cf.offset_years - reference;
            let amount = cf.amount.value();
            let factor = powf(base, -years); // (1+r)^(−t)
            npv += amount * factor;
            scale += abs(amount) * abs(factor);
            derivative += -years * amount * factor / base; // (1+r)^(−t−1)
        }
        (root::Residual { value: npv, scale }, derivative)
    }
}

/// An **owned** dated cashflow series — the allocating complement to the borrowed
/// [`DatedCashflows`], behind the `alloc` feature (implied by `std`; ADR-0043,
/// ADR-0065).
///
/// [`DatedCashflows`] borrows a `&[DatedCashflow]` and stays allocation-free
/// (ADR-0013). `OwnedDatedCashflows` owns a `Vec<DatedCashflow>`, so it can be
/// **built from an iterator** or handed around without keeping the source slice
/// alive — at the cost of an allocation. It is the exact counterpart of
/// [`OwnedCashflows`](crate::OwnedCashflows) for irregularly dated flows, with no
/// periodicity tag to carry (the dated discount is intrinsically annual, ADR-0029).
///
/// The operations are **not reimplemented** here: an owned series lends a borrowed
/// [`DatedCashflows`] view via [`as_dated_cashflows`](Self::as_dated_cashflows), and
/// the methods below forward to it, so there is a single source of truth for the
/// math. (A new operation added to [`DatedCashflows`] should gain a one-line forward
/// here too.)
///
/// # Wire format
///
/// With the `serde` / `schemars` features this is the dated series type that crosses
/// a wire boundary — the borrowed [`DatedCashflows`] cannot, having no storage to
/// deserialize into. The shape is a **bare array of [`DatedCashflow`]**, each
/// `{ offset_years, amount }`, in slice order — which is *meaningful* order here,
/// since the first entry is the XNPV's valuation reference (ADR-0060, ADR-0065).
///
/// # Examples
///
/// ```
/// use time_value::{DatedCashflow, Money, OwnedDatedCashflows};
///
/// // Build straight from an iterator — no backing slice to keep alive.
/// let series: OwnedDatedCashflows = [(0.0, -100.0), (1.0, 110.0)]
///     .into_iter()
///     .map(|(t, amount)| DatedCashflow::new(t, Money::agnostic(amount)?))
///     .collect::<Result<_, _>>()?;
///
/// let irr = series.internal_rate_of_return()?;
/// assert!((irr.value() - 0.10).abs() < 1e-9);
/// # Ok::<(), time_value::TvmError>(())
/// ```
#[cfg(feature = "alloc")]
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedDatedCashflows {
    flows: alloc::vec::Vec<DatedCashflow>,
}

#[cfg(feature = "alloc")]
impl OwnedDatedCashflows {
    /// Takes ownership of `flows`; the first entry is the XNPV's valuation reference.
    #[must_use]
    pub const fn new(flows: alloc::vec::Vec<DatedCashflow>) -> Self {
        Self { flows }
    }

    /// Borrows the series as a [`DatedCashflows`] view — the bridge the forwarding
    /// operations go through, and the way to reach any [`DatedCashflows`] method not
    /// forwarded here.
    #[must_use]
    pub fn as_dated_cashflows(&self) -> DatedCashflows<'_> {
        DatedCashflows::new(&self.flows)
    }

    /// The underlying dated cashflows.
    #[must_use]
    pub fn as_slice(&self) -> &[DatedCashflow] {
        &self.flows
    }

    /// Consumes the series, returning the owned `Vec`.
    #[must_use]
    pub fn into_vec(self) -> alloc::vec::Vec<DatedCashflow> {
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

    /// The single [`Currency`](crate::Currency) the series is denominated in. See
    /// [`DatedCashflows::currency`].
    ///
    /// # Errors
    ///
    /// As [`DatedCashflows::currency`].
    pub fn currency(&self) -> Result<crate::Currency, TvmError> {
        self.as_dated_cashflows().currency()
    }

    /// The XNPV of the series discounted at an annual `rate`. See
    /// [`DatedCashflows::net_present_value`].
    ///
    /// # Errors
    ///
    /// As [`DatedCashflows::net_present_value`].
    pub fn net_present_value(&self, rate: Rate<Annual>) -> Result<Money, TvmError> {
        self.as_dated_cashflows().net_present_value(rate)
    }

    /// The net future value of the series at its horizon, compounded at an annual
    /// `rate`. See [`DatedCashflows::net_future_value`].
    ///
    /// # Errors
    ///
    /// As [`DatedCashflows::net_future_value`].
    pub fn net_future_value(&self, rate: Rate<Annual>) -> Result<Money, TvmError> {
        self.as_dated_cashflows().net_future_value(rate)
    }

    /// The XIRR from a default 10% guess. See
    /// [`DatedCashflows::internal_rate_of_return`].
    ///
    /// # Errors
    ///
    /// As [`DatedCashflows::internal_rate_of_return`].
    pub fn internal_rate_of_return(&self) -> Result<Rate<Annual>, TvmError> {
        self.as_dated_cashflows().internal_rate_of_return()
    }

    /// The XIRR seeded with `guess`. See
    /// [`DatedCashflows::internal_rate_of_return_from`].
    ///
    /// # Errors
    ///
    /// As [`DatedCashflows::internal_rate_of_return_from`].
    pub fn internal_rate_of_return_from(&self, guess: f64) -> Result<Rate<Annual>, TvmError> {
        self.as_dated_cashflows()
            .internal_rate_of_return_from(guess)
    }

    /// The dated modified internal rate of return. See
    /// [`DatedCashflows::modified_internal_rate_of_return`].
    ///
    /// # Errors
    ///
    /// As [`DatedCashflows::modified_internal_rate_of_return`].
    pub fn modified_internal_rate_of_return(
        &self,
        finance_rate: Rate<Annual>,
        reinvestment_rate: Rate<Annual>,
    ) -> Result<Rate<Annual>, TvmError> {
        self.as_dated_cashflows()
            .modified_internal_rate_of_return(finance_rate, reinvestment_rate)
    }
}

#[cfg(feature = "alloc")]
impl From<alloc::vec::Vec<DatedCashflow>> for OwnedDatedCashflows {
    fn from(flows: alloc::vec::Vec<DatedCashflow>) -> Self {
        Self::new(flows)
    }
}

#[cfg(feature = "alloc")]
impl From<DatedCashflows<'_>> for OwnedDatedCashflows {
    /// Copies a borrowed dated series into an owned one.
    fn from(borrowed: DatedCashflows<'_>) -> Self {
        Self::new(borrowed.as_slice().to_vec())
    }
}

#[cfg(feature = "alloc")]
impl FromIterator<DatedCashflow> for OwnedDatedCashflows {
    /// Collects dated cashflows in slice order — the first one is the XNPV's
    /// valuation reference.
    fn from_iter<I: IntoIterator<Item = DatedCashflow>>(iter: I) -> Self {
        Self::new(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::math::powf;
    use crate::root::within;
    use crate::{Annual, DatedCashflow, DatedCashflows, Money, Rate, TvmError};

    /// `no_std`-safe approximate equality (no `f64::abs`).
    fn approx(a: f64, b: f64) -> bool {
        within(a - b, 1e-6)
    }

    fn flow(offset_years: f64, amount: f64) -> DatedCashflow {
        DatedCashflow::new(offset_years, Money::agnostic(amount).unwrap()).unwrap()
    }

    fn annual(rate: f64) -> Rate<Annual> {
        Rate::<Annual>::new(rate).unwrap()
    }

    #[test]
    fn xnpv_over_one_year_is_the_annual_discount() {
        // -100 now, +110 in one year, discounted at 10% → exactly 0.
        let flows = [flow(0.0, -100.0), flow(1.0, 110.0)];
        let npv = DatedCashflows::new(&flows)
            .net_present_value(annual(0.10))
            .unwrap();
        assert!(approx(npv.value(), 0.0));
    }

    #[test]
    fn xirr_recovers_a_whole_year_rate() {
        let flows = [flow(0.0, -100.0), flow(1.0, 110.0)];
        let irr = DatedCashflows::new(&flows)
            .internal_rate_of_return()
            .unwrap();
        assert!(approx(irr.value(), 0.10));
    }

    #[test]
    fn xirr_recovers_a_fractional_year_rate() {
        // (1 + r)^0.5 = 1.05  ⇒  1 + r = 1.1025  ⇒  r = 0.1025.
        let flows = [flow(0.0, -100.0), flow(0.5, 105.0)];
        let irr = DatedCashflows::new(&flows)
            .internal_rate_of_return()
            .unwrap();
        assert!(approx(irr.value(), 0.1025));
        // …and discounting at that rate zeroes the XNPV.
        let npv = DatedCashflows::new(&flows).net_present_value(irr).unwrap();
        assert!(approx(npv.value(), 0.0));
    }

    #[test]
    fn xirr_matches_the_excel_reference() {
        // Microsoft's XIRR example (values on ACT/365 year-offsets from the first
        // date 2008-01-01): dates 2008-03-01, 2008-10-30, 2009-02-15, 2009-04-01
        // are 60, 303, 411, 456 days out. Excel returns 0.373362535.
        let flows = [
            flow(0.0, -10_000.0),
            flow(60.0 / 365.0, 2_750.0),
            flow(303.0 / 365.0, 4_250.0),
            flow(411.0 / 365.0, 3_250.0),
            flow(456.0 / 365.0, 2_750.0),
        ];
        let irr = DatedCashflows::new(&flows)
            .internal_rate_of_return()
            .unwrap();
        assert!(within(irr.value() - 0.373_362_535, 1e-5));
        // The located rate zeroes the XNPV.
        let npv = DatedCashflows::new(&flows).net_present_value(irr).unwrap();
        assert!(within(npv.value(), 1e-3));
    }

    #[test]
    fn xirr_is_invariant_to_shifting_the_reference() {
        // Rebasing to the first flow means a uniform shift of every offset leaves
        // the rate unchanged.
        let base = [flow(0.0, -100.0), flow(0.5, 40.0), flow(1.25, 80.0)];
        let shifted = [flow(10.0, -100.0), flow(10.5, 40.0), flow(11.25, 80.0)];
        let a = DatedCashflows::new(&base)
            .internal_rate_of_return()
            .unwrap();
        let b = DatedCashflows::new(&shifted)
            .internal_rate_of_return()
            .unwrap();
        assert!(approx(a.value(), b.value()));
    }

    #[test]
    fn xirr_falls_back_to_bisection_from_a_bad_guess() {
        let flows = [flow(0.0, -100.0), flow(0.5, 105.0)];
        let irr = DatedCashflows::new(&flows)
            .internal_rate_of_return_from(1e6)
            .unwrap();
        assert!(approx(irr.value(), 0.1025));
    }

    #[test]
    fn empty_xnpv_is_zero_and_xirr_errors() {
        let empty: [DatedCashflow; 0] = [];
        let series = DatedCashflows::new(&empty);
        assert_eq!(series.net_present_value(annual(0.05)).unwrap(), Money::ZERO);
        assert_eq!(
            series.internal_rate_of_return(),
            Err(TvmError::EmptyCashflows)
        );
    }

    /// XIRR shares the solver, so it shared the defect: `XNPV(r) → CF₀` as
    /// `r → ∞` just as `NPV` does, and a near-zero first flow made every large
    /// enough rate pass the old fixed tolerance (ADR-0054). Dated version of the
    /// `[0, 0, -100, 110]` reproduction, at whole-year offsets so the answer is the
    /// same `0.1`.
    #[test]
    fn xirr_rejects_a_root_that_is_only_zero_because_everything_discounted_away() {
        let flows = [
            flow(0.0, 0.0),
            flow(1.0, 0.0),
            flow(2.0, -100.0),
            flow(3.0, 110.0),
        ];
        for guess in [0.1, 0.9, 5.0, 50.0] {
            let irr = DatedCashflows::new(&flows)
                .internal_rate_of_return_from(guess)
                .unwrap();
            assert!(
                approx(irr.value(), 0.1),
                "guess {guess} found {} instead of the unique XIRR 0.1",
                irr.value(),
            );
        }
    }

    #[test]
    fn all_inflows_have_no_xirr() {
        let flows = [flow(0.0, 100.0), flow(0.5, 60.0), flow(1.0, 60.0)];
        assert_eq!(
            DatedCashflows::new(&flows).internal_rate_of_return(),
            Err(TvmError::IrrDidNotConverge)
        );
    }

    #[test]
    fn a_flow_before_the_reference_compounds_forward() {
        // First flow is the reference at t=0; an earlier-dated flow gets a negative
        // rebased offset, so it is compounded (not discounted). -100 at reference,
        // and a +? one half-year *earlier*: with the second flow at offset -0.5.
        let flows = [flow(0.0, -100.0), flow(-0.5, 105.0)];
        // XNPV(r) = -100 + 105·(1+r)^{0.5}; zero at (1+r)^0.5 = 100/105.
        let irr = DatedCashflows::new(&flows)
            .internal_rate_of_return()
            .unwrap();
        let base = 100.0 / 105.0;
        let expected = base * base - 1.0; // (1+r) = (100/105)^2
        assert!(approx(irr.value(), expected));
    }

    /// `net_present_value`'s `# Errors` names `CurrencyMismatch`, but nothing
    /// executed it — the dated series had no currency test at all (ADR-0045 rule 2).
    /// The fold is over the slice in order, so the payload is deterministic: `left`
    /// is what accumulated before the clash, `right` the flow that broke it.
    #[test]
    fn xnpv_rejects_a_series_of_mixed_currencies() {
        use crate::Currency;
        let flows = [
            DatedCashflow::new(0.0, Money::new(-100.0, Currency::Usd).unwrap()).unwrap(),
            DatedCashflow::new(1.0, Money::new(110.0, Currency::Eur).unwrap()).unwrap(),
        ];
        let expected = TvmError::CurrencyMismatch {
            left: Currency::Usd,
            right: Currency::Eur,
        };
        assert_eq!(
            DatedCashflows::new(&flows).net_present_value(annual(0.10)),
            Err(expected.clone())
        );
        // …and directly, through the accessor the fold lives in (issue #104).
        assert_eq!(DatedCashflows::new(&flows).currency(), Err(expected));
    }

    /// ADR-0034's identity rule over a dated series, exhaustively across the closed
    /// currency set: `Xxx` plus one real currency adopts it, a uniform series keeps
    /// its own, and the `Xxx` iteration covers the wholly-agnostic case.
    #[test]
    fn a_dated_series_adopts_the_one_currency_it_names() {
        use crate::Currency;
        for &currency in Currency::ALL {
            let mixed = [
                DatedCashflow::new(0.0, Money::agnostic(-100.0).unwrap()).unwrap(),
                DatedCashflow::new(1.0, Money::new(110.0, currency).unwrap()).unwrap(),
            ];
            assert_eq!(
                DatedCashflows::new(&mixed).currency().unwrap(),
                currency,
                "the accessor lost the denomination of an Xxx-and-{} dated series",
                currency.code(),
            );
            assert_eq!(
                DatedCashflows::new(&mixed)
                    .net_present_value(annual(0.05))
                    .unwrap()
                    .currency(),
                currency,
                "an Xxx-and-{} dated series lost the denomination",
                currency.code(),
            );

            let uniform = [
                DatedCashflow::new(0.0, Money::new(-100.0, currency).unwrap()).unwrap(),
                DatedCashflow::new(1.0, Money::new(110.0, currency).unwrap()).unwrap(),
            ];
            assert_eq!(DatedCashflows::new(&uniform).currency().unwrap(), currency);
            assert_eq!(
                DatedCashflows::new(&uniform)
                    .net_present_value(annual(0.05))
                    .unwrap()
                    .currency(),
                currency,
            );
        }
    }

    /// An empty dated series is `Xxx` — named explicitly, rather than inferred from
    /// `Money::ZERO` equality.
    #[test]
    fn an_empty_dated_series_is_currency_agnostic() {
        let empty: [DatedCashflow; 0] = [];
        assert_eq!(
            DatedCashflows::new(&empty).currency().unwrap(),
            crate::Currency::Xxx
        );
        assert_eq!(
            DatedCashflows::new(&empty)
                .net_present_value(annual(0.05))
                .unwrap()
                .currency(),
            crate::Currency::Xxx,
        );
    }

    /// XIRR makes the same choice as the periodic IRR (ADR-0057): it returns a
    /// rate, so it never folds the currencies, and a series `net_present_value`
    /// rejects still has an XIRR — the magnitude-only one.
    #[test]
    fn xirr_ignores_the_currencies_xnpv_rejects() {
        use crate::Currency;
        let mixed = [
            DatedCashflow::new(0.0, Money::new(-100.0, Currency::Usd).unwrap()).unwrap(),
            DatedCashflow::new(1.0, Money::new(110.0, Currency::Eur).unwrap()).unwrap(),
        ];
        let series = DatedCashflows::new(&mixed);
        assert!(matches!(
            series.net_present_value(annual(0.10)),
            Err(TvmError::CurrencyMismatch { .. })
        ));
        // The accessor gives a caller the strict reading without the XNPV (issue #104).
        assert!(matches!(
            series.currency(),
            Err(TvmError::CurrencyMismatch { .. })
        ));

        let agnostic = [flow(0.0, -100.0), flow(1.0, 110.0)];
        assert_eq!(
            series.internal_rate_of_return().unwrap(),
            DatedCashflows::new(&agnostic)
                .internal_rate_of_return()
                .unwrap(),
        );
    }

    #[test]
    fn non_finite_offset_is_rejected() {
        assert_eq!(
            DatedCashflow::new(f64::INFINITY, Money::agnostic(1.0).unwrap()),
            Err(TvmError::NonFiniteOffset)
        );
        assert_eq!(
            DatedCashflow::new(f64::NAN, Money::agnostic(1.0).unwrap()),
            Err(TvmError::NonFiniteOffset)
        );
    }

    // ---- The dated net future value (ADR-0065) -----------------------------

    /// The worked case, against an **independent** 60-digit reference: `Σ CFᵢ
    /// (1+r)^(T − tᵢ)` for the three flows below at 10% a year, computed in Python's
    /// `decimal` at 60 significant digits for exactly these `f64` inputs —
    /// `10.311474146468699192831302143775095060042…`. Not derived from any function
    /// in this crate, so it tests the arithmetic rather than a round trip.
    #[test]
    fn xnfv_compounds_every_flow_to_the_latest_date() {
        let flows = [flow(0.0, -100.0), flow(0.5, 40.0), flow(1.25, 80.0)];
        let nfv = DatedCashflows::new(&flows)
            .net_future_value(annual(0.10))
            .unwrap();
        assert!(
            within(nfv.value() - 10.311_474_146_468_699, 1e-12),
            "{} is not the reference 10.311474146468699",
            nfv.value(),
        );
    }

    /// `net_future_value`'s rustdoc states `XNFV = XNPV · (1 + r)^(T − t₀)`. The two
    /// are computed by separate loops, so this is a real cross-check of the horizon,
    /// not a restatement — and the reference XNPV `9.153346455373549884…` is from the
    /// same 60-digit computation.
    #[test]
    fn xnfv_is_the_xnpv_compounded_over_the_series_life() {
        let flows = [flow(0.0, -100.0), flow(0.5, 40.0), flow(1.25, 80.0)];
        let series = DatedCashflows::new(&flows);
        let present = series.net_present_value(annual(0.10)).unwrap().value();
        assert!(within(present - 9.153_346_455_373_55, 1e-12), "{present}");
        let future = series.net_future_value(annual(0.10)).unwrap().value();
        // (1 + r)^(T − t₀) with T = 1.25, t₀ = 0.
        assert!(within(future - present * powf(1.10, 1.25), 1e-12));
    }

    /// The horizon is the **latest offset**, not the last slice entry, so permuting
    /// the flows cannot change the answer — while the XNPV, whose reference *is* the
    /// first entry, does change (ADR-0065). Both halves matter: the first is the
    /// property, the second shows it is not vacuous.
    #[test]
    fn xnfv_ignores_the_order_of_the_flows() {
        let sorted = [flow(0.0, -100.0), flow(0.5, 40.0), flow(1.25, 80.0)];
        // Newest first, and a middle-first rotation: neither is sorted.
        let reversed = [flow(1.25, 80.0), flow(0.5, 40.0), flow(0.0, -100.0)];
        let rotated = [flow(0.5, 40.0), flow(1.25, 80.0), flow(0.0, -100.0)];

        let expected = DatedCashflows::new(&sorted)
            .net_future_value(annual(0.10))
            .unwrap()
            .value();
        for unsorted in [&reversed, &rotated] {
            let reordered = DatedCashflows::new(unsorted)
                .net_future_value(annual(0.10))
                .unwrap()
                .value();
            // The *terms* are identical; only the order they are summed in changes, so
            // the answers agree to rounding rather than bit-for-bit.
            assert!(
                approx(reordered, expected),
                "the horizon moved with the slice order: {reordered} vs {expected}",
            );
        }
        // …and the present value, which is anchored to the first entry, does move.
        let sorted_npv = DatedCashflows::new(&sorted)
            .net_present_value(annual(0.10))
            .unwrap();
        let reversed_npv = DatedCashflows::new(&reversed)
            .net_present_value(annual(0.10))
            .unwrap();
        assert!(!approx(sorted_npv.value(), reversed_npv.value()));
    }

    /// Where the latest flow happens to be listed **first**, the XNPV's reference and
    /// the XNFV's horizon are the same date, so the two operations must return the
    /// same amount. This pins the horizon choice from the other side: any other
    /// horizon would break the identity.
    #[test]
    fn xnfv_equals_the_xnpv_when_the_latest_flow_is_listed_first() {
        let flows = [flow(1.25, 80.0), flow(0.0, -100.0), flow(0.5, 40.0)];
        let series = DatedCashflows::new(&flows);
        assert_eq!(
            series.net_future_value(annual(0.10)).unwrap(),
            series.net_present_value(annual(0.10)).unwrap(),
        );
    }

    /// Negative offsets are ordinary input (ADR-0029), and they compound *further*
    /// than the reference flow rather than being discounted: with the horizon at
    /// `1.0`, the flow at `−0.5` is compounded over 1.5 years.
    #[test]
    fn xnfv_compounds_a_flow_before_the_reference_over_the_longer_span() {
        let flows = [flow(-0.5, -100.0), flow(0.0, 60.0), flow(1.0, 60.0)];
        let expected = -100.0 * powf(1.10, 1.5) + 60.0 * powf(1.10, 1.0) + 60.0;
        let nfv = DatedCashflows::new(&flows)
            .net_future_value(annual(0.10))
            .unwrap();
        assert!(approx(nfv.value(), expected));
    }

    /// The empty convention (ADR-0021): nothing to compound is `Ok(0 Xxx)`, matching
    /// the periodic `net_future_value` and the dated `net_present_value`.
    #[test]
    fn empty_xnfv_is_zero() {
        let empty: [DatedCashflow; 0] = [];
        let series = DatedCashflows::new(&empty);
        assert_eq!(series.net_future_value(annual(0.05)).unwrap(), Money::ZERO);
        assert_eq!(
            series.net_future_value(annual(0.05)).unwrap().currency(),
            crate::Currency::Xxx,
        );
    }

    /// The `# Errors` section names `Overflow`; reach it. Two near-max flows on the
    /// same date sum past `f64::MAX` at a zero rate, so the total is surfaced as an
    /// error rather than a silent infinity.
    #[test]
    fn xnfv_overflows_to_a_non_finite_result() {
        let flows = [flow(0.0, f64::MAX), flow(0.0, f64::MAX)];
        assert_eq!(
            DatedCashflows::new(&flows).net_future_value(annual(0.0)),
            Err(TvmError::Overflow)
        );
    }

    /// The dated net future value returns a `Money`, so it **folds** the currencies
    /// (ADR-0057) — the same fold, with the same deterministic payload, that the
    /// XNPV runs.
    #[test]
    fn xnfv_rejects_a_series_of_mixed_currencies() {
        use crate::Currency;
        let flows = [
            DatedCashflow::new(0.0, Money::new(-100.0, Currency::Usd).unwrap()).unwrap(),
            DatedCashflow::new(1.0, Money::new(110.0, Currency::Eur).unwrap()).unwrap(),
        ];
        assert_eq!(
            DatedCashflows::new(&flows).net_future_value(annual(0.10)),
            Err(TvmError::CurrencyMismatch {
                left: Currency::Usd,
                right: Currency::Eur,
            })
        );
    }

    /// ADR-0034's identity rule through the *future* value, exhaustively over the
    /// closed currency set — the companion to
    /// `a_dated_series_adopts_the_one_currency_it_names`, which covers the XNPV.
    #[test]
    fn the_dated_future_value_adopts_the_one_currency_it_names() {
        use crate::Currency;
        for &currency in Currency::ALL {
            let mixed = [
                DatedCashflow::new(0.0, Money::agnostic(-100.0).unwrap()).unwrap(),
                DatedCashflow::new(1.0, Money::new(110.0, currency).unwrap()).unwrap(),
            ];
            assert_eq!(
                DatedCashflows::new(&mixed)
                    .net_future_value(annual(0.05))
                    .unwrap()
                    .currency(),
                currency,
                "an Xxx-and-{} dated series lost the denomination",
                currency.code(),
            );
        }
    }

    // ---- The dated MIRR (ADR-0065) -----------------------------------------

    mod dated_mirr {
        use super::{annual, approx, flow, DatedCashflow, DatedCashflows, Money, TvmError};
        use crate::root::within;

        /// The four-flow worked case, against the same **independent** 60-digit
        /// `decimal` computation of
        /// `(TVᵢₙ / −PVₒᵤₜ)^(1/Y) − 1 = 0.0951029241431681260854387741605867860…`
        /// (`PVₒᵤₜ = −1476.731294622796156520…`, `TVᵢₙ = 1770.970617132655864657…`,
        /// `Y = 2`). The doctest asserts the same number to nine places.
        #[test]
        fn matches_the_independent_reference() {
            let flows = [
                flow(0.0, -1000.0),
                flow(0.5, -500.0),
                flow(1.25, 800.0),
                flow(2.0, 900.0),
            ];
            let mirr = DatedCashflows::new(&flows)
                .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                .unwrap();
            assert!(
                within(mirr.value() - 0.095_102_924_143_168_13, 1e-14),
                "{} is not the reference 0.09510292414316813",
                mirr.value(),
            );
        }

        /// On whole-year offsets the dated MIRR *is* the periodic MIRR — the two
        /// share no code, so this is corroboration. The expected value is the one
        /// `cashflows::tests::mirr::matches_the_manual_formula` already pins for
        /// `[−1000, −500, 800, 900]` at 10%/12%, `0.0728187246`, which the same
        /// 60-digit reference confirms as `0.07281872462958623405…`.
        #[test]
        fn agrees_with_the_periodic_mirr_on_whole_year_offsets() {
            let flows = [
                flow(0.0, -1000.0),
                flow(1.0, -500.0),
                flow(2.0, 800.0),
                flow(3.0, 900.0),
            ];
            let mirr = DatedCashflows::new(&flows)
                .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                .unwrap();
            assert!(approx(mirr.value(), 0.072_818_724_6), "{}", mirr.value());
        }

        /// The span runs from the **earliest** offset to the latest, so — like the
        /// XIRR, and unlike the XNPV — the answer does not depend on the order of the
        /// slice (ADR-0065). Were the first *entry* the reference, the reversed
        /// series below would span no time at all and error instead.
        #[test]
        fn ignores_the_order_of_the_flows() {
            let sorted = [
                flow(0.0, -1000.0),
                flow(0.5, -500.0),
                flow(1.25, 800.0),
                flow(2.0, 900.0),
            ];
            let reversed = [
                flow(2.0, 900.0),
                flow(1.25, 800.0),
                flow(0.5, -500.0),
                flow(0.0, -1000.0),
            ];
            let rotated = [
                flow(1.25, 800.0),
                flow(2.0, 900.0),
                flow(0.0, -1000.0),
                flow(0.5, -500.0),
            ];
            let expected = DatedCashflows::new(&sorted)
                .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                .unwrap();
            for unsorted in [&reversed, &rotated] {
                let reordered = DatedCashflows::new(unsorted)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                    .unwrap();
                // Same terms, different summation order: equal to rounding.
                assert!(
                    approx(reordered.value(), expected.value()),
                    "{} vs {}",
                    reordered.value(),
                    expected.value(),
                );
            }
        }

        /// A flow dated before the first-listed one still sits inside the span: the
        /// reference is the earliest offset, so it is discounted like any other
        /// outflow rather than compounded.
        #[test]
        fn a_flow_before_the_first_entry_sets_the_reference() {
            let listed = [flow(0.0, -1000.0), flow(-0.5, -500.0), flow(1.0, 1800.0)];
            let sorted = [flow(-0.5, -500.0), flow(0.0, -1000.0), flow(1.0, 1800.0)];
            let as_listed = DatedCashflows::new(&listed)
                .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                .unwrap();
            let as_sorted = DatedCashflows::new(&sorted)
                .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                .unwrap();
            assert!(approx(as_listed.value(), as_sorted.value()));
        }

        #[test]
        fn empty_series_errors() {
            let empty: [DatedCashflow; 0] = [];
            assert_eq!(
                DatedCashflows::new(&empty)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::EmptyCashflows)
            );
        }

        /// A lone outflow spans no time and cannot grow into anything, so **no** rate
        /// satisfies it. Note the variant: not the periodic operation's
        /// `ZeroPeriods` — there is no `Period<P>` here to be zero — but the outcome
        /// (ADR-0064's reasoning, ADR-0065).
        #[test]
        fn a_single_outflow_has_no_span_and_no_solution() {
            let flows = [flow(1.5, -1000.0)];
            assert_eq!(
                DatedCashflows::new(&flows)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::NoRealSolution)
            );
        }

        /// The other half of the zero-span row: a series dated entirely on one day
        /// whose outflows and inflows already match is satisfied by **every** rate.
        #[test]
        fn one_date_with_matching_flows_is_indeterminate() {
            let flows = [flow(0.25, -1000.0), flow(0.25, 1000.0)];
            assert_eq!(
                DatedCashflows::new(&flows)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::IndeterminateRate)
            );
        }

        /// …and where they do not match, no rate does — the same date, the opposite
        /// answer. Reporting one variant for both would collapse ADR-0056's
        /// distinction.
        #[test]
        fn one_date_with_mismatched_flows_has_no_solution() {
            let flows = [flow(0.25, -1000.0), flow(0.25, 1500.0)];
            assert_eq!(
                DatedCashflows::new(&flows)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::NoRealSolution)
            );
        }

        /// "Satisfied" is the solver's own root test, not `==` (ADR-0056): a target a
        /// hair from the outflows is still satisfied at every rate, so an
        /// exact-equality guard would call this near-miss `NoRealSolution`. This is
        /// the one case that tells the two guards apart.
        #[test]
        fn a_near_miss_on_one_date_is_still_indeterminate() {
            // 1e-7 against 1000 is 1e-10 relative — inside `is_root`'s 1e-9.
            let flows = [flow(0.25, -1000.0), flow(0.25, 1_000.000_000_1)];
            assert_eq!(
                DatedCashflows::new(&flows)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::IndeterminateRate)
            );
        }

        /// No outflows: there is no present value to grow from, so the ratio does not
        /// exist. Reported **before** the span question — a single inflow is both
        /// zero-span and outflow-free, and this is the variant it gets (ADR-0065).
        #[test]
        fn no_outflows_has_no_present_value_to_grow_from() {
            let spread = [flow(0.0, 1000.0), flow(0.5, 500.0), flow(2.0, 500.0)];
            assert_eq!(
                DatedCashflows::new(&spread)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::NoOutflows)
            );
            let lone_inflow = [flow(1.5, 1000.0)];
            assert_eq!(
                DatedCashflows::new(&lone_inflow)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::NoOutflows),
                "the zero-span check took precedence over the missing outflows",
            );
        }

        /// No inflows: the terminal value is zero, so the implied rate is `−100%`,
        /// which `Rate::from_operation` refuses. Same answer as the periodic MIRR.
        #[test]
        fn no_inflows_is_a_total_loss() {
            let flows = [flow(0.0, -1000.0), flow(0.5, -500.0), flow(2.0, -500.0)];
            assert_eq!(
                DatedCashflows::new(&flows)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.10)),
                Err(TvmError::RateOutOfRange)
            );
        }

        /// The dated MIRR returns a rate, so it makes the same choice XIRR and the
        /// periodic MIRR do (ADR-0057): the currencies are never folded, and the
        /// answer is the one the bare magnitudes give.
        #[test]
        fn ignores_the_currencies_xnpv_rejects() {
            use crate::Currency;
            let mixed = [
                DatedCashflow::new(0.0, Money::new(-1000.0, Currency::Usd).unwrap()).unwrap(),
                DatedCashflow::new(0.5, Money::new(-500.0, Currency::Eur).unwrap()).unwrap(),
                DatedCashflow::new(1.25, Money::new(800.0, Currency::Jpy).unwrap()).unwrap(),
                DatedCashflow::new(2.0, Money::new(900.0, Currency::Gbp).unwrap()).unwrap(),
            ];
            let agnostic = [
                flow(0.0, -1000.0),
                flow(0.5, -500.0),
                flow(1.25, 800.0),
                flow(2.0, 900.0),
            ];
            let series = DatedCashflows::new(&mixed);
            assert!(matches!(
                series.net_future_value(annual(0.10)),
                Err(TvmError::CurrencyMismatch { .. })
            ));
            assert_eq!(
                series
                    .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                    .unwrap(),
                DatedCashflows::new(&agnostic)
                    .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                    .unwrap(),
            );
        }
    }

    // ---- The owned dated series (ADR-0065) ---------------------------------

    #[cfg(feature = "alloc")]
    mod owned {
        use alloc::vec::Vec;

        use super::{annual, flow, DatedCashflow, DatedCashflows, Money};
        use crate::OwnedDatedCashflows;

        fn flows() -> Vec<DatedCashflow> {
            alloc::vec![
                flow(0.0, -1000.0),
                flow(0.5, -500.0),
                flow(1.25, 800.0),
                flow(2.0, 900.0),
            ]
        }

        /// Every forward answers exactly what the borrowed view does — the whole
        /// contract of a forwarding type, and the thing a future reimplementation
        /// here would break.
        #[test]
        fn owned_operations_match_the_borrowed_view() {
            let v = flows();
            let borrowed = DatedCashflows::new(&v);
            let owned = OwnedDatedCashflows::new(v.clone());

            assert_eq!(
                owned.net_present_value(annual(0.10)).unwrap(),
                borrowed.net_present_value(annual(0.10)).unwrap()
            );
            assert_eq!(
                owned.net_future_value(annual(0.10)).unwrap(),
                borrowed.net_future_value(annual(0.10)).unwrap()
            );
            assert_eq!(
                owned.internal_rate_of_return().unwrap(),
                borrowed.internal_rate_of_return().unwrap()
            );
            assert_eq!(
                owned.internal_rate_of_return_from(0.5).unwrap(),
                borrowed.internal_rate_of_return_from(0.5).unwrap()
            );
            assert_eq!(
                owned
                    .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                    .unwrap(),
                borrowed
                    .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                    .unwrap()
            );
            assert_eq!(owned.currency().unwrap(), borrowed.currency().unwrap());
        }

        #[test]
        fn builds_from_an_iterator() {
            let owned: OwnedDatedCashflows = [(0.0, -100.0), (1.0, 110.0)]
                .into_iter()
                .map(|(t, amount)| DatedCashflow::new(t, Money::agnostic(amount).unwrap()).unwrap())
                .collect();
            assert_eq!(owned.len(), 2);
            assert!(crate::root::within(
                owned.internal_rate_of_return().unwrap().value() - 0.10,
                1e-9
            ));
        }

        #[test]
        fn from_vec_and_from_a_borrowed_view_agree() {
            let v = flows();
            let from_vec = OwnedDatedCashflows::from(v.clone());
            let from_borrowed = OwnedDatedCashflows::from(DatedCashflows::new(&v));
            assert_eq!(from_vec, from_borrowed);
            assert_eq!(from_vec.as_slice(), &v[..]);
        }

        #[test]
        fn the_bridge_lends_a_view_and_into_vec_recovers() {
            let v = flows();
            let owned = OwnedDatedCashflows::new(v.clone());
            assert_eq!(owned.as_dated_cashflows().len(), 4);
            assert!(!owned.is_empty());
            assert_eq!(owned.into_vec(), v);
        }

        #[test]
        fn an_empty_owned_dated_series_is_zero_and_agnostic() {
            let owned = OwnedDatedCashflows::new(Vec::new());
            assert!(owned.is_empty());
            assert_eq!(owned.len(), 0);
            assert_eq!(owned.net_present_value(annual(0.05)).unwrap(), Money::ZERO);
            assert_eq!(owned.net_future_value(annual(0.05)).unwrap(), Money::ZERO);
            assert_eq!(owned.currency().unwrap(), crate::Currency::Xxx);
        }

        /// The forwards inherit the currency fold *because* they forward — worth
        /// pinning, since a future reimplementation here could quietly drop it
        /// (ADR-0034, ADR-0057, ADR-0045 rule 2).
        #[test]
        fn owned_operations_inherit_the_currency_split() {
            use crate::{Currency, TvmError};

            let owned = OwnedDatedCashflows::new(alloc::vec![
                DatedCashflow::new(0.0, Money::new(-100.0, Currency::Usd).unwrap()).unwrap(),
                DatedCashflow::new(1.0, Money::new(110.0, Currency::Eur).unwrap()).unwrap(),
            ]);
            let expected = TvmError::CurrencyMismatch {
                left: Currency::Usd,
                right: Currency::Eur,
            };
            assert_eq!(owned.net_present_value(annual(0.10)), Err(expected.clone()));
            assert_eq!(owned.net_future_value(annual(0.10)), Err(expected.clone()));
            assert_eq!(owned.currency(), Err(expected));
            // The rate-returning forwards still answer (ADR-0057).
            assert!(owned.internal_rate_of_return().is_ok());
            assert!(owned
                .modified_internal_rate_of_return(annual(0.10), annual(0.12))
                .is_ok());
        }
    }

    #[test]
    fn accessors_round_trip() {
        let cf = flow(1.5, -42.0);
        assert!(approx(cf.offset_years(), 1.5));
        assert_eq!(cf.amount(), Money::agnostic(-42.0).unwrap());

        let flows = [cf];
        let series = DatedCashflows::new(&flows);
        assert_eq!(series.len(), 1);
        assert!(!series.is_empty());
        assert_eq!(series.as_slice(), &flows);
    }
}
