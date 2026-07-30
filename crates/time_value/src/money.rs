//! [`Money`] — a validated monetary amount denominated in a [`Currency`].

use core::fmt;
use core::ops::Neg;

use crate::{Currency, TvmError};

/// A monetary amount: an `f64` magnitude together with the [`Currency`] it is
/// denominated in.
///
/// Per `docs/adr/0033-core-domain-model-two-axes-and-an-f64-engine.md`, the crate
/// is a TVM *computation engine* whose magnitudes are `f64` (transcendental TVM
/// results are irrational, so an exact-decimal representation would promise a
/// precision the mathematics does not have). Currency is *dynamic* data, so it is
/// carried as a runtime value rather than a compile-time type tag
/// (`docs/adr/0034-money-and-currency.md`). `Money` stays `Copy`, `no_std`, and
/// allocation-free.
///
/// Every `Money` is finite. The [`new`](Money::new) constructor rejects `NaN`
/// and the infinities, and every operation that could leave the finite range —
/// the TVM operations and the arithmetic below — returns a `Result` whose `Err`
/// is [`TvmError::Overflow`] (a real result too large for `f64`), or a named
/// degenerate case such as [`TvmError::DivisionByZero`], rather than a non-finite
/// `Money`
/// (`docs/adr/0021-fallible-operations-on-non-finite-results.md`,
/// `docs/adr/0031-split-non-finite-result-into-overflow-and-undefined.md`,
/// `docs/adr/0052-tvmerror-variant-granularity.md`).
///
/// Cashflows are signed — an outflow is negative, an inflow positive.
///
/// # Currency
///
/// [`Currency::Xxx`] (ISO 4217 "no currency") is the **currency-agnostic** amount
/// and the identity on the currency axis: adding an `Xxx` amount to one in `C`
/// yields `C`, while adding two *distinct* non-`Xxx` currencies is a
/// [`TvmError::CurrencyMismatch`]. So pure-number TVM is all `Xxx` — construct it
/// with [`agnostic`](Money::agnostic) — and [`ZERO`](Money::ZERO) is `0 XXX`, a
/// neutral element that adds cleanly into any currency.
///
/// ```
/// use time_value::{Currency, Money};
///
/// let fee = Money::new(25.0, Currency::Usd)?;
/// let refund = -fee; // an inflow becomes an outflow
/// assert_eq!(refund.value(), -25.0);
/// assert_eq!(refund.currency(), Currency::Usd);
///
/// let total = fee.try_add(Money::new(75.0, Currency::Usd)?)?;
/// let doubled = total.try_mul(2.0)?;
/// assert_eq!(doubled.value(), 200.0);
///
/// // A currency-agnostic amount adopts whatever it is combined with.
/// let bonus = fee.try_add(Money::agnostic(5.0)?)?;
/// assert_eq!(bonus.currency(), Currency::Usd);
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Arithmetic
///
/// Negation is a [`Neg`] operator, and [`abs`](Self::abs) / [`signum`](Self::signum)
/// are plain methods: each is a sign operation on an already-finite amount, so none
/// of them can fail. Addition, subtraction and scaling *can* leave `f64` range (and
/// addition/subtraction can find mismatched currencies), so they are fallible
/// [`try_add`](Self::try_add), [`try_sub`](Self::try_sub),
/// [`try_mul`](Self::try_mul) and [`try_div`](Self::try_div) methods rather than
/// operators — an operator cannot return a `Result`, and silently yielding an
/// infinity is the foot-gun this crate exists to avoid
/// (`docs/adr/0023-money-arithmetic-surface.md`).
///
/// [`try_sum`](Self::try_sum) totals an iterator for the same reason there is no
/// [`Sum`](core::iter::Sum) impl, and [`try_min`](Self::try_min) /
/// [`try_max`](Self::try_max) exist because the ordering below is *partial*, so
/// `Money` has no [`Ord`] to take `min`/`max` from
/// (`docs/adr/0061-money-and-currency-ergonomics.md`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Money {
    magnitude: f64,
    currency: Currency,
}

impl Money {
    /// Zero money, denominated in [`Currency::Xxx`] — the additive identity
    /// (ADR-0032), and currency-agnostic so it adds cleanly into any currency.
    pub const ZERO: Self = Self {
        magnitude: 0.0,
        currency: Currency::Xxx,
    };

    /// Constructs `amount` denominated in `currency`.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::NonFiniteAmount`] if `amount` is not finite
    /// (`NaN`, `+∞`, or `-∞`).
    pub fn new(amount: f64, currency: Currency) -> Result<Self, TvmError> {
        if amount.is_finite() {
            Ok(Self {
                magnitude: amount,
                currency,
            })
        } else {
            Err(TvmError::NonFiniteAmount)
        }
    }

    /// Constructs a currency-agnostic amount ([`Currency::Xxx`]) — the pure-number
    /// path, for TVM that is not denominated in any particular currency.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::NonFiniteAmount`] if `amount` is not finite.
    pub fn agnostic(amount: f64) -> Result<Self, TvmError> {
        Self::new(amount, Currency::Xxx)
    }

    /// The magnitude as a plain `f64`.
    #[must_use]
    pub const fn value(self) -> f64 {
        self.magnitude
    }

    /// The currency this amount is denominated in.
    #[must_use]
    pub const fn currency(self) -> Currency {
        self.currency
    }

    /// Constructs from the `f64` result of an operation, tagging it `currency` and
    /// validating finiteness.
    ///
    /// This is the overflow funnel: a non-finite result reaching here is a real
    /// value that exceeded the representable `f64` range, so it is
    /// [`TvmError::Overflow`]. Mathematically degenerate cases (e.g. an annuity
    /// payment over zero periods) are guarded at their call sites and return their
    /// own named variant — [`TvmError::ZeroPeriods`] and the rest — before reaching
    /// this point (ADR-0021, ADR-0031, ADR-0052).
    /// Both are distinct from the [`TvmError::NonFiniteAmount`] that
    /// [`new`](Self::new) returns for a non-finite value supplied by a *caller*.
    pub(crate) fn from_operation(amount: f64, currency: Currency) -> Result<Self, TvmError> {
        if amount.is_finite() {
            Ok(Self {
                magnitude: amount,
                currency,
            })
        } else {
            Err(TvmError::Overflow)
        }
    }

    /// Adds `rhs`, combining currencies by the [`Currency::Xxx`] identity rule.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::CurrencyMismatch`] if the two amounts are in distinct
    /// non-`Xxx` currencies, or [`TvmError::Overflow`] if the sum leaves the finite
    /// `f64` range.
    pub fn try_add(self, rhs: Self) -> Result<Self, TvmError> {
        let currency = combine(self.currency, rhs.currency)?;
        Self::from_operation(self.magnitude + rhs.magnitude, currency)
    }

    /// Subtracts `rhs`, combining currencies by the [`Currency::Xxx`] identity rule.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::CurrencyMismatch`] if the two amounts are in distinct
    /// non-`Xxx` currencies, or [`TvmError::Overflow`] if the difference leaves the
    /// finite `f64` range.
    pub fn try_sub(self, rhs: Self) -> Result<Self, TvmError> {
        let currency = combine(self.currency, rhs.currency)?;
        Self::from_operation(self.magnitude - rhs.magnitude, currency)
    }

    /// Totals `amounts`, folding their currencies by the [`Currency::Xxx`] identity
    /// rule — the n-ary counterpart to [`try_add`](Self::try_add).
    ///
    /// An **empty** iterator sums to [`ZERO`](Self::ZERO) (`0 XXX`), the additive
    /// identity. The fold runs left to right from that identity, so an agnostic
    /// amount adopts whatever currency the rest of the series names, and a series of
    /// agnostic amounts stays agnostic.
    ///
    /// This is deliberately *not* a [`Sum`](core::iter::Sum) impl. `Sum::sum`
    /// returns `Self`, so it could only panic or hand back a non-finite `Money` on
    /// overflow, and it could not report a currency mismatch at all — the two ways
    /// summing money genuinely fails (ADR-0021, ADR-0034,
    /// `docs/adr/0061-money-and-currency-ergonomics.md`).
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::CurrencyMismatch`] if two amounts are in distinct
    /// non-`Xxx` currencies — `left` is the currency accumulated from the amounts so
    /// far and `right` the offending amount's, as everywhere else the crate folds a
    /// series (ADR-0052) — or [`TvmError::Overflow`] if a running total leaves the
    /// finite `f64` range.
    ///
    /// ```
    /// use time_value::{Currency, Money};
    ///
    /// let flows = [
    ///     Money::new(-100.0, Currency::Usd)?,
    ///     Money::new(60.0, Currency::Usd)?,
    ///     Money::new(60.0, Currency::Usd)?,
    /// ];
    /// let total = Money::try_sum(flows.iter().copied())?;
    /// assert_eq!(total.value(), 20.0);
    /// assert_eq!(total.currency(), Currency::Usd);
    ///
    /// // Nothing to total is zero, and currency-agnostic.
    /// assert_eq!(Money::try_sum([])?, Money::ZERO);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    pub fn try_sum<I: IntoIterator<Item = Self>>(amounts: I) -> Result<Self, TvmError> {
        amounts.into_iter().try_fold(Self::ZERO, Self::try_add)
    }

    /// Scales by `factor` — e.g. `payment.try_mul(12.0)` for an annual total. The
    /// currency is preserved.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::NonFiniteScalar`] if `factor` is itself `NaN` or infinite
    /// (no finite product is defined), or [`TvmError::Overflow`] if a finite factor
    /// pushes the product past the representable range.
    pub fn try_mul(self, factor: f64) -> Result<Self, TvmError> {
        if !factor.is_finite() {
            return Err(TvmError::NonFiniteScalar);
        }
        Self::from_operation(self.magnitude * factor, self.currency)
    }

    /// Divides by `divisor` — e.g. `total.try_div(12.0)` for a monthly share. The
    /// currency is preserved.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::DivisionByZero`] if `divisor` is zero (the quotient has
    /// no defined value, including `0 / 0`), [`TvmError::NonFiniteScalar`] if it is
    /// `NaN`, or [`TvmError::Overflow`] if dividing a large amount by a tiny one
    /// leaves the finite range. An *infinite* divisor is not an error: the quotient
    /// is zero, which is finite.
    pub fn try_div(self, divisor: f64) -> Result<Self, TvmError> {
        if divisor == 0.0 {
            return Err(TvmError::DivisionByZero);
        }
        if divisor.is_nan() {
            return Err(TvmError::NonFiniteScalar);
        }
        Self::from_operation(self.magnitude / divisor, self.currency)
    }

    /// The amount with its sign removed — `|−25 USD|` is `25 USD`. The currency is
    /// preserved.
    ///
    /// Infallible, like [`Neg`]: the absolute value of a finite amount is finite
    /// (ADR-0021), so this is an operator-grade operation by ADR-0023's test. It is
    /// a sign flip rather than a call to `f64::abs` (a `std`-only intrinsic), so it
    /// is available in the default `no_std`, zero-dependency build alongside the
    /// rest of `Money`'s arithmetic. A negative zero comes back as a positive one,
    /// exactly as `f64::abs` would give.
    ///
    /// ```
    /// use time_value::{Currency, Money};
    ///
    /// let outflow = Money::new(-25.0, Currency::Usd)?;
    /// assert_eq!(outflow.abs().value(), 25.0);
    /// assert_eq!(outflow.abs().currency(), Currency::Usd); // currency preserved
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    #[must_use]
    pub fn abs(self) -> Self {
        Self {
            // `is_sign_negative` rather than `< 0.0` so that `-0.0` normalises to
            // `0.0`, as `f64::abs` does; `Money::new(-0.0, c) == Money::new(0.0, c)`,
            // so leaving the sign bit on would make two equal amounts render
            // differently (`-0` vs `0`).
            magnitude: if self.magnitude.is_sign_negative() {
                -self.magnitude
            } else {
                self.magnitude
            },
            currency: self.currency,
        }
    }

    /// The sign of the amount as a plain number: `1.0` for an inflow, `-1.0` for an
    /// outflow, `0.0` for zero. A sign has no denomination, so the currency is not
    /// part of the answer — but it is not consulted either, so an amount's sign is
    /// the same in any currency.
    ///
    /// **This differs from `f64::signum` at zero**, deliberately. `f64::signum`
    /// reports `1.0` for `+0.0` and `-1.0` for `-0.0`, but `Money::new(0.0, c)` and
    /// `Money::new(-0.0, c)` are *equal* (`-0.0 == 0.0`), so delegating would let
    /// two equal amounts report opposite signs — the sign would depend on how the
    /// zero was spelled. Zero is also neither an inflow nor an outflow under the
    /// crate's signed-cashflow convention, so `0.0` is the honest third answer
    /// (`docs/adr/0061-money-and-currency-ergonomics.md`).
    ///
    /// ```
    /// use time_value::{Currency, Money};
    ///
    /// assert_eq!(Money::new(25.0, Currency::Usd)?.signum(), 1.0);
    /// assert_eq!(Money::new(-25.0, Currency::Usd)?.signum(), -1.0);
    /// assert_eq!(Money::ZERO.signum(), 0.0); // not `f64::signum`'s 1.0
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    #[must_use]
    pub fn signum(self) -> f64 {
        if self.magnitude > 0.0 {
            1.0
        } else if self.magnitude < 0.0 {
            -1.0
        } else {
            0.0 // both zeros; no `NaN` can reach here (every `Money` is finite)
        }
    }

    /// The smaller of the two amounts, denominated in the currency the two combine
    /// to.
    ///
    /// **Fallible because `Money`'s ordering is partial.** Two distinct non-`Xxx`
    /// currencies do not compare — `100 USD` against `100 EUR` has no answer, so
    /// [`PartialOrd`] returns `None` and `Money` has no [`Ord`] and none of `Ord`'s
    /// infallible `min`/`max`/`clamp` (ADR-0059). An infallible `Money::min` would
    /// have to invent an answer for that pair; this returns the mismatch instead,
    /// and the `try_` prefix says so at the call site, as it does for
    /// [`try_add`](Self::try_add).
    ///
    /// The currency is folded by the [`Currency::Xxx`] identity rule, exactly as
    /// [`try_add`](Self::try_add) folds it, so the result is denominated even when
    /// the selected side was the agnostic one: the smaller of `0 XXX` and `100 USD`
    /// is `0 USD`. That is also what makes the operation commutative — on a tie
    /// between an agnostic and a denominated amount, an unfolded currency would
    /// answer differently depending on the argument order.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::CurrencyMismatch`] if the two amounts are in distinct
    /// non-`Xxx` currencies. Nothing else can go wrong: the magnitude returned is
    /// one of the two given, so there is no arithmetic to overflow.
    ///
    /// ```
    /// use time_value::{Currency, Money};
    ///
    /// let floor = Money::new(50.0, Currency::Usd)?;
    /// let fee = Money::new(75.0, Currency::Usd)?;
    /// assert_eq!(fee.try_min(floor)?, floor);
    /// assert_eq!(fee.try_max(floor)?, fee);
    ///
    /// // The agnostic identity is folded away, so the result stays denominated.
    /// assert_eq!(Money::ZERO.try_min(fee)?.currency(), Currency::Usd);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    pub fn try_min(self, other: Self) -> Result<Self, TvmError> {
        self.select(other, other.magnitude < self.magnitude)
    }

    /// The larger of the two amounts, denominated in the currency the two combine
    /// to. The fallible counterpart to [`try_min`](Self::try_min), for the same
    /// reason: `Money`'s ordering is partial (ADR-0059).
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::CurrencyMismatch`] if the two amounts are in distinct
    /// non-`Xxx` currencies. It cannot overflow.
    ///
    /// ```
    /// use time_value::{Currency, Money};
    ///
    /// let cap = Money::new(100.0, Currency::Eur)?;
    /// let claim = Money::new(140.0, Currency::Eur)?;
    /// assert_eq!(claim.try_max(cap)?, claim);
    ///
    /// // Distinct currencies have no larger one.
    /// assert!(claim.try_max(Money::new(140.0, Currency::Usd)?).is_err());
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    pub fn try_max(self, other: Self) -> Result<Self, TvmError> {
        self.select(other, other.magnitude > self.magnitude)
    }

    /// The shared body of [`try_min`](Self::try_min) and [`try_max`](Self::try_max):
    /// fold the two currencies, then keep `other`'s magnitude if `take_other`.
    ///
    /// Both magnitudes are finite by construction, so the caller's comparison is a
    /// total one — no `NaN` arm to consider. A tie keeps `self`'s magnitude, which is
    /// immaterial: equal magnitudes in one folded currency are the same `Money`.
    fn select(self, other: Self, take_other: bool) -> Result<Self, TvmError> {
        let currency = combine(self.currency, other.currency)?;
        Ok(Self {
            magnitude: if take_other {
                other.magnitude
            } else {
                self.magnitude
            },
            currency,
        })
    }

    /// Rounds the magnitude to this amount's currency minor unit — a *presentation*
    /// step, never used during computation (ADR-0033, ADR-0034).
    ///
    /// Uses the currency's [minor-unit exponent][Currency::minor_unit_exponent]
    /// (`2` for `USD`, `0` for `JPY`, `3` for `BHD`), rounding half away from zero.
    /// A currency with no minor unit — [`Currency::Xxx`], the precious metals, the
    /// fund/testing codes — is returned unchanged. The currency is preserved.
    ///
    /// Because magnitudes are `f64`, a value that *looks* like it sits exactly on a
    /// rounding boundary (e.g. `1.005`) may round either way, since it is stored as
    /// the nearest representable double (here `1.00499…`, which rounds down). This
    /// is presentation-only and consistent with the crate's approximate-real
    /// precision contract (ADR-0033).
    ///
    /// Requires the `std` or `libm` feature (it rounds an `f64`).
    ///
    /// # Every result is still finite
    ///
    /// Rounding scales up, rounds, and scales back down, and the scaling up can
    /// overflow: `f64::MAX * 100.0` is `inf`, which written into a `Money` would
    /// break the crate's headline invariant (ADR-0054). A magnitude whose scaled
    /// form is not finite is therefore returned **unchanged**, which is not a
    /// fallback but the *exact* answer: overflow needs
    /// `|magnitude| > f64::MAX / scale`, at least `≈1.8e304`, and every `f64`
    /// above `2⁵³ ≈ 9.0e15` is already an integer — so such a magnitude is
    /// already an exact multiple of any minor unit and rounding it is the
    /// identity. Only the multiplication can overflow: if `magnitude · scale` is
    /// finite then so is `round(magnitude · scale) / scale`, because `scale ≥ 1`.
    ///
    /// ```
    /// use time_value::{Currency, Money};
    ///
    /// let usd = Money::new(2.348, Currency::Usd)?.round_to_currency();
    /// assert_eq!(usd.value(), 2.35);
    ///
    /// let jpy = Money::new(1234.9, Currency::Jpy)?.round_to_currency();
    /// assert_eq!(jpy.value(), 1235.0); // no minor unit
    ///
    /// // A magnitude too large to scale is already whole, so it is unchanged —
    /// // and, in particular, still finite.
    /// let huge = Money::new(f64::MAX, Currency::Usd)?.round_to_currency();
    /// assert!(huge.value().is_finite());
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    #[cfg(any(feature = "std", feature = "libm"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "std", feature = "libm"))))]
    #[must_use]
    pub fn round_to_currency(self) -> Self {
        let Some(exponent) = self.currency.minor_unit_exponent() else {
            return self; // no minor unit
        };
        let Some(scale) = minor_unit_scale(exponent) else {
            return self; // an exponent beyond the table — unreachable today
        };
        let scaled = self.magnitude * scale;
        if !scaled.is_finite() {
            return self; // already whole; see "Every result is still finite"
        }
        Self {
            magnitude: crate::math::round(scaled) / scale,
            currency: self.currency,
        }
    }

    /// Converts this amount into another currency using a caller-supplied
    /// [`FxRate`] (ADR-0034).
    ///
    /// The rate's [`source`](FxRate::source) must match this amount's currency;
    /// the result is tagged the rate's [`to`](FxRate::to) currency. To convert the
    /// other way, apply [`FxRate::inverse`].
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::CurrencyMismatch`] if `self.currency() != fx.source()`, or
    /// [`TvmError::Overflow`] if the converted magnitude leaves the finite range.
    ///
    /// ```
    /// use time_value::{Currency, FxRate, Money};
    ///
    /// let usd = Money::new(100.0, Currency::Usd)?;
    /// let usd_to_eur = FxRate::new(Currency::Usd, Currency::Eur, 0.9)?;
    /// let eur = usd.convert(usd_to_eur)?;
    /// assert_eq!(eur.value(), 90.0);
    /// assert_eq!(eur.currency(), Currency::Eur);
    ///
    /// // The same rate, inverted, converts back.
    /// let back = eur.convert(usd_to_eur.inverse())?;
    /// assert!((back.value() - 100.0).abs() < 1e-9);
    /// # Ok::<(), time_value::TvmError>(())
    /// ```
    pub fn convert(self, fx: FxRate) -> Result<Self, TvmError> {
        if self.currency != fx.from {
            return Err(TvmError::CurrencyMismatch {
                left: self.currency,
                right: fx.from,
            });
        }
        Self::from_operation(self.magnitude * fx.rate, fx.to)
    }
}

/// `10ᵉ` as an exact `f64`, for a currency's minor-unit exponent — the scale
/// [`Money::round_to_currency`] rounds at.
///
/// A **total** function, unlike the fixed array it replaces: that array had
/// exactly five slots for the exponents ISO 4217 actually uses (`{0, 2, 3, 4}`),
/// so it carried zero headroom and a future code with five decimals would have
/// been an out-of-bounds panic on a `#[must_use] -> Self` method that cannot
/// report failure (ADR-0054). An exponent past the table is `None`, and
/// `round_to_currency` then leaves the amount alone rather than panicking. The
/// `every_currency_has_a_minor_unit_scale` test keeps that arm unreachable by
/// checking every [`Currency::ALL`](crate::Currency::ALL) entry against it, so a
/// new currency that outgrows the table fails a test rather than shipping.
#[cfg(any(feature = "std", feature = "libm"))]
const fn minor_unit_scale(exponent: u8) -> Option<f64> {
    // Exact small integer scales — avoids a transcendental `powi`.
    Some(match exponent {
        0 => 1.0,
        1 => 10.0,
        2 => 100.0,
        3 => 1_000.0,
        4 => 10_000.0,
        _ => return None,
    })
}

/// Combines two currencies by the [`Currency::Xxx`] identity rule (ADR-0034): an
/// agnostic `Xxx` amount adopts the other currency, equal currencies pass through,
/// and two distinct non-`Xxx` currencies are a mismatch. Shared with the series
/// operations, which fold it over their flows to find the one currency a monetary
/// result is denominated in.
///
/// A mismatch names both currencies — `CurrencyMismatch { left: a, right: b }` —
/// so a caller can report which two clashed (ADR-0052). When folded over a series,
/// `left` is the currency accumulated from the flows so far and `right` is the
/// offending flow's.
pub(crate) fn combine(a: Currency, b: Currency) -> Result<Currency, TvmError> {
    match (a, b) {
        (Currency::Xxx, other) | (other, Currency::Xxx) => Ok(other),
        _ if a == b => Ok(a),
        _ => Err(TvmError::CurrencyMismatch { left: a, right: b }),
    }
}

/// A directional exchange rate: the price of one unit of [`source`](Self::source)
/// in units of [`to`](Self::to) (ADR-0034).
///
/// Rates are **caller-supplied** — the core carries no rate data and stays
/// `no_std`. Triangulation (via a base currency) and bid/ask spreads are out of
/// scope: those are rate-*sourcing* concerns, not core arithmetic. A rate can be
/// used in either direction via [`inverse`](Self::inverse), which is infallible
/// because [`new`](Self::new) admits only rates whose reciprocal is itself a valid
/// rate (ADR-0053).
///
/// ```
/// use time_value::{Currency, FxRate};
///
/// let gbp_to_usd = FxRate::new(Currency::Gbp, Currency::Usd, 1.25)?;
/// assert_eq!(gbp_to_usd.rate(), 1.25);
///
/// let usd_to_gbp = gbp_to_usd.inverse();
/// assert_eq!(usd_to_gbp.source(), Currency::Usd);
/// assert_eq!(usd_to_gbp.to(), Currency::Gbp);
/// assert_eq!(usd_to_gbp.rate(), 1.0 / 1.25);
/// # Ok::<(), time_value::TvmError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FxRate {
    from: Currency,
    to: Currency,
    rate: f64,
}

impl FxRate {
    /// Constructs the rate that prices one unit of `from` at `rate` units of `to`.
    ///
    /// The accepted domain is **closed under reciprocal**, so that
    /// [`inverse`](Self::inverse) can be infallible: `rate` must be a *normal*
    /// `f64` whose reciprocal is also normal. That excludes zero, the negatives,
    /// `NaN`, the infinities, and — the reason both halves of the test are needed —
    /// the subnormal fringes at either end of the range. `f64::MAX` is itself
    /// normal, but `1.0 / f64::MAX ≈ 5.6e-309` is *subnormal*, and inverting that
    /// again overflows to infinity, so `is_normal()` alone would not close the
    /// domain (ADR-0053).
    ///
    /// The excluded band is `rate < 2.3e-308` or `rate > 4.5e307`. No real exchange
    /// rate is within hundreds of orders of magnitude of it: the extremes of actual
    /// currency markets sit around `1e-7` to `1e7`.
    ///
    /// # Errors
    ///
    /// Returns [`TvmError::InvalidExchangeRate`] if `rate` is not finite, is not
    /// strictly positive (a non-positive exchange rate is economically
    /// meaningless), or lies in the subnormal band described above.
    pub fn new(from: Currency, to: Currency, rate: f64) -> Result<Self, TvmError> {
        if rate.is_normal() && rate > 0.0 && (1.0 / rate).is_normal() {
            Ok(Self { from, to, rate })
        } else {
            Err(TvmError::InvalidExchangeRate)
        }
    }

    /// The source currency (the unit being priced).
    ///
    /// Named `source` rather than `from` so that the name does not shadow
    /// [`From::from`] on this type: an *inherent* `FxRate::from` would win path-form
    /// resolution over any future `impl From<T> for FxRate` and make that
    /// constructor form permanently unreachable (ADR-0053).
    #[must_use]
    pub const fn source(self) -> Currency {
        self.from
    }

    /// The target currency (the unit the price is expressed in).
    #[must_use]
    pub const fn to(self) -> Currency {
        self.to
    }

    /// The exchange rate — units of [`to`](Self::to) per unit of
    /// [`source`](Self::source).
    #[must_use]
    pub const fn rate(self) -> f64 {
        self.rate
    }

    /// The reverse rate: swaps the two currencies and reciprocates the rate, so it
    /// converts [`to`](Self::to) back into [`source`](Self::source).
    ///
    /// Infallible: [`new`](Self::new) accepts only a normal rate whose reciprocal is
    /// also normal, so the reciprocal taken here is always a rate `new` would itself
    /// accept — and inverting twice returns to the accepted domain (ADR-0053).
    #[must_use]
    pub fn inverse(self) -> Self {
        Self {
            from: self.to,
            to: self.from,
            rate: 1.0 / self.rate,
        }
    }
}

/// The default `Money` is [`ZERO`](Money::ZERO) — the additive identity, `0 XXX`
/// (ADR-0032).
impl Default for Money {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Fallibly wraps an `f64` as a currency-agnostic amount, mirroring
/// [`Money::agnostic`]: lets a call site that expects a `Money` use
/// `f64::try_into()` on the pure-number path (ADR-0032).
///
/// # Errors
///
/// Returns [`TvmError::NonFiniteAmount`] if the value is not finite.
impl TryFrom<f64> for Money {
    type Error = TvmError;

    fn try_from(amount: f64) -> Result<Self, Self::Error> {
        Self::agnostic(amount)
    }
}

/// Extracts the plain magnitude, mirroring [`Money::value`] (ADR-0032) — the
/// currency is dropped.
///
/// Only `Money` gets a `From<_> for f64`: converting a [`Rate`](crate::Rate)
/// this way would silently drop its periodicity tag — the very safety the type
/// exists for — so rates keep `value()` explicit.
impl From<Money> for f64 {
    fn from(money: Money) -> Self {
        money.value()
    }
}

/// Flips the sign — an inflow becomes an outflow, and vice versa. The currency is
/// preserved.
///
/// Infallible: the negation of a finite amount is finite (ADR-0021).
impl Neg for Money {
    type Output = Self;

    fn neg(self) -> Self {
        Self {
            magnitude: -self.magnitude,
            currency: self.currency,
        }
    }
}

/// Orders amounts by magnitude *within a compatible currency*. Ordering is only
/// defined when the currencies combine (equal, or either is [`Currency::Xxx`]);
/// two distinct non-`Xxx` currencies are unordered, so comparison yields `None`
/// (ADR-0034).
impl PartialOrd for Money {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        combine(self.currency, other.currency).ok()?;
        self.magnitude.partial_cmp(&other.magnitude)
    }
}

/// Formats the magnitude, then — unless the amount is currency-agnostic — a space
/// and the ISO 4217 code: `100 USD`, `1234.5 JPY`. A [`Currency::Xxx`] amount
/// prints the bare magnitude, so the pure-number path is byte-for-byte unchanged
/// (`docs/adr/0058-money-display-carries-its-currency.md`).
///
/// Value first, qualifier second, matching every sibling in the crate:
/// [`Rate`](crate::Rate) prints `0.01 monthly`, `Period` prints `12 monthly`, and
/// `ContinuousRate` prints `0.05 continuous`.
///
/// ```
/// use time_value::{Currency, Money};
///
/// assert_eq!(Money::new(100.0, Currency::Usd)?.to_string(), "100 USD");
/// assert_eq!(Money::new(1234.5, Currency::Jpy)?.to_string(), "1234.5 JPY");
///
/// // The currency-agnostic amount stays a bare number.
/// assert_eq!(Money::agnostic(100.0)?.to_string(), "100");
/// assert_eq!(Money::ZERO.to_string(), "0");
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # No minor-unit rounding
///
/// `Display` does **not** round to the currency's minor unit. Rounding is an
/// explicit, opt-in presentation step — [`Money::round_to_currency`][round]
/// (ADR-0033, ADR-0034) — so doing it here would silently discard information the
/// caller never asked to lose, and leave no way to get the full magnitude back out
/// of the rendering. Ask for the rounding when you want it.
///
/// ```
/// # use time_value::{Currency, Money};
/// // Two cents is USD's minor unit; the third decimal survives anyway.
/// assert_eq!(Money::new(2.348, Currency::Usd)?.to_string(), "2.348 USD");
/// # Ok::<(), time_value::TvmError>(())
/// ```
///
/// # Format specifiers size the magnitude, not the whole rendering
///
/// The formatter is forwarded to the `f64` before the code is appended, so
/// precision (`{:.2}`), sign (`{:+}`), and width/fill/alignment (`{:>10}`,
/// `{:012}`) all behave exactly as they do on the bare number. The consequence
/// worth knowing: **padding sizes the number alone**, so a `{:>10}` rendering of a
/// denominated amount is four characters longer than ten. To lay out a column,
/// pad the finished string — `format!("{:>10}", money.to_string())`.
///
/// Only `Display` is forwarded. `Money` implements neither `LowerExp` nor
/// `UpperExp`, so `{:e}` does not compile against it; reach for
/// [`value`](Money::value) when you want the `f64`'s other formatting traits.
///
/// ```
/// # use time_value::{Currency, Money};
/// let fee = Money::new(1234.5678, Currency::Usd)?;
///
/// assert_eq!(format!("{fee:.2}"), "1234.57 USD");
/// assert_eq!(format!("{fee:+.1}"), "+1234.6 USD");
///
/// // The width applies to `1234.6`, and ` USD` follows it.
/// assert_eq!(format!("{fee:>12.1}"), "      1234.6 USD");
/// // Padding the rendering instead gives a column of the width asked for.
/// assert_eq!(format!("{:>12}", format!("{fee:.1}")), "  1234.6 USD");
/// # Ok::<(), time_value::TvmError>(())
/// ```
#[cfg_attr(
    not(any(feature = "std", feature = "libm")),
    doc = "

[round]: https://docs.rs/time_value/latest/time_value/struct.Money.html#method.round_to_currency
"
)]
#[cfg_attr(
    any(feature = "std", feature = "libm"),
    doc = "

[round]: Money::round_to_currency
"
)]
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The magnitude goes through the formatter first, so `{:.2}`, `{:+}` and
        // width/alignment keep applying to the number exactly as they did before
        // the code was appended (ADR-0058). Rendering into a `String` and writing
        // *that* would hand the specifier the whole rendering instead — and would
        // need `alloc`, which the core does not have.
        self.magnitude.fmt(f)?;
        if self.currency != Currency::Xxx {
            f.write_str(" ")?;
            f.write_str(self.currency.code())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // These tests round-trip exactly-representable values, so exact `==` is
    // correct here.
    #![allow(clippy::float_cmp)]

    use crate::{Currency, Money, TvmError};

    #[test]
    fn accepts_finite_values() {
        assert_eq!(Money::new(42.5, Currency::Usd).unwrap().value(), 42.5);
        assert_eq!(Money::new(-42.5, Currency::Usd).unwrap().value(), -42.5);
        assert_eq!(Money::ZERO.value(), 0.0);
    }

    #[test]
    fn carries_its_currency() {
        assert_eq!(
            Money::new(1.0, Currency::Eur).unwrap().currency(),
            Currency::Eur
        );
        assert_eq!(Money::agnostic(1.0).unwrap().currency(), Currency::Xxx);
        assert_eq!(Money::ZERO.currency(), Currency::Xxx);
    }

    #[test]
    fn rejects_non_finite_values() {
        assert_eq!(
            Money::new(f64::NAN, Currency::Usd),
            Err(TvmError::NonFiniteAmount)
        );
        assert_eq!(
            Money::agnostic(f64::INFINITY),
            Err(TvmError::NonFiniteAmount)
        );
        assert_eq!(
            Money::new(f64::NEG_INFINITY, Currency::Usd),
            Err(TvmError::NonFiniteAmount)
        );
    }

    /// The largest finite `f64`; doubling it overflows.
    fn huge() -> Money {
        Money::agnostic(f64::MAX).unwrap()
    }

    #[test]
    fn negation_flips_the_sign_and_keeps_currency() {
        let m = Money::new(42.5, Currency::Usd).unwrap();
        assert_eq!((-m).value(), -42.5);
        assert_eq!((-m).currency(), Currency::Usd);
        assert_eq!(-(-huge()), huge());
    }

    #[test]
    fn adds_and_subtracts() {
        let a = Money::new(100.0, Currency::Usd).unwrap();
        let b = Money::new(25.0, Currency::Usd).unwrap();
        assert_eq!(a.try_add(b).unwrap().value(), 125.0);
        assert_eq!(a.try_add(b).unwrap().currency(), Currency::Usd);
        assert_eq!(a.try_sub(b).unwrap().value(), 75.0);
        assert_eq!(b.try_sub(a).unwrap().value(), -75.0);
    }

    #[test]
    fn agnostic_amounts_adopt_the_other_currency() {
        let usd = Money::new(100.0, Currency::Usd).unwrap();
        let bonus = Money::agnostic(25.0).unwrap();
        assert_eq!(usd.try_add(bonus).unwrap().currency(), Currency::Usd);
        assert_eq!(bonus.try_add(usd).unwrap().currency(), Currency::Usd);
        // Two agnostic amounts stay agnostic.
        assert_eq!(bonus.try_add(bonus).unwrap().currency(), Currency::Xxx);
    }

    #[test]
    fn distinct_currencies_are_a_mismatch() {
        let usd = Money::new(100.0, Currency::Usd).unwrap();
        let eur = Money::new(100.0, Currency::Eur).unwrap();
        // The error names *both* currencies, in the order the operation combined
        // them, so a caller can report which two clashed (ADR-0052).
        let mismatch = TvmError::CurrencyMismatch {
            left: Currency::Usd,
            right: Currency::Eur,
        };
        assert_eq!(usd.try_add(eur), Err(mismatch.clone()));
        assert_eq!(usd.try_sub(eur), Err(mismatch));
        assert_eq!(
            eur.try_add(usd),
            Err(TvmError::CurrencyMismatch {
                left: Currency::Eur,
                right: Currency::Usd,
            })
        );
    }

    /// The payload is not merely carried — it reaches the rendered message
    /// (ADR-0045 rule 2: an assertion in the docs earns a test).
    #[test]
    #[cfg(feature = "alloc")]
    fn a_currency_mismatch_names_both_currencies_when_displayed() {
        use alloc::string::ToString as _;
        let usd = Money::new(100.0, Currency::Usd).unwrap();
        let eur = Money::new(100.0, Currency::Eur).unwrap();
        assert_eq!(
            usd.try_add(eur).unwrap_err().to_string(),
            "cannot combine USD with EUR"
        );
    }

    /// The currency-agnostic rendering is the bare magnitude — byte-for-byte what
    /// it was before the code was appended (ADR-0058). The pure-number path is the
    /// default and by far the most used, so this is the assertion that says the
    /// change did not reach it.
    #[test]
    #[cfg(feature = "alloc")]
    fn agnostic_money_displays_the_bare_magnitude() {
        use alloc::string::ToString as _;
        assert_eq!(Money::agnostic(100.0).unwrap().to_string(), "100");
        assert_eq!(Money::agnostic(1234.5).unwrap().to_string(), "1234.5");
        assert_eq!(Money::agnostic(-0.25).unwrap().to_string(), "-0.25");
        assert_eq!(Money::ZERO.to_string(), "0");
    }

    /// A denominated amount renders value-then-qualifier, matching `Rate`,
    /// `Period` and `ContinuousRate` (ADR-0058).
    #[test]
    #[cfg(feature = "alloc")]
    fn denominated_money_displays_its_code_after_the_magnitude() {
        use alloc::string::ToString as _;
        assert_eq!(
            Money::new(100.0, Currency::Usd).unwrap().to_string(),
            "100 USD"
        );
        assert_eq!(
            Money::new(1234.5, Currency::Jpy).unwrap().to_string(),
            "1234.5 JPY"
        );
        assert_eq!(
            Money::new(-42.0, Currency::Eur).unwrap().to_string(),
            "-42 EUR"
        );
    }

    /// `Currency` is a small closed set, so iterate it rather than sample
    /// (ADR-0045 rule 2): every non-`Xxx` code appears exactly once, at the end,
    /// after the magnitude — and `Xxx` appears not at all.
    #[test]
    #[cfg(feature = "alloc")]
    fn every_currency_appends_its_code_exactly_once_except_xxx() {
        use alloc::format;
        for currency in Currency::ALL {
            let rendered = format!("{}", Money::new(12.5, *currency).unwrap());
            let code = currency.code();
            if *currency == Currency::Xxx {
                assert_eq!(rendered, "12.5", "{code} should render bare");
            } else {
                assert_eq!(rendered, format!("12.5 {code}"));
                assert_eq!(
                    rendered.matches(code).count(),
                    1,
                    "{code} should appear exactly once in `{rendered}`"
                );
            }
        }
    }

    /// `Display` never applies minor-unit rounding — that is
    /// `round_to_currency`'s opt-in job (ADR-0058). `USD` has two minor digits and
    /// `JPY` none, and neither loses a decimal here.
    #[test]
    #[cfg(feature = "alloc")]
    fn display_does_not_round_to_the_minor_unit() {
        use alloc::string::ToString as _;
        assert_eq!(
            Money::new(2.348, Currency::Usd).unwrap().to_string(),
            "2.348 USD"
        );
        assert_eq!(
            Money::new(1234.9, Currency::Jpy).unwrap().to_string(),
            "1234.9 JPY"
        );
    }

    /// The formatter is forwarded to the `f64`, so a specifier keeps applying to
    /// the magnitude and the code is appended after it (ADR-0058). Building a
    /// `String` and writing *that* would break every case below.
    #[test]
    #[cfg(feature = "alloc")]
    fn format_specifiers_apply_to_the_magnitude() {
        use alloc::format;
        let fee = Money::new(1234.5678, Currency::Usd).unwrap();

        assert_eq!(format!("{fee:.2}"), "1234.57 USD");
        assert_eq!(format!("{fee:.0}"), "1235 USD");
        assert_eq!(format!("{fee:+.1}"), "+1234.6 USD");

        // Width and alignment size the number, not the whole rendering — the
        // documented wart. Twelve characters of number, then ` USD`.
        assert_eq!(format!("{fee:>12.1}"), "      1234.6 USD");
        assert_eq!(format!("{fee:<12.1}"), "1234.6       USD");
        assert_eq!(format!("{fee:^12.1}"), "   1234.6    USD");
        assert_eq!(format!("{fee:012.1}"), "0000001234.6 USD");

        // Padding the finished rendering is how a caller gets a real column.
        assert_eq!(format!("{:>12}", format!("{fee:.1}")), "  1234.6 USD");

        // The agnostic path keeps the plain `f64` behaviour untouched.
        assert_eq!(
            format!("{:>10.2}", Money::agnostic(1234.5678).unwrap()),
            "   1234.57"
        );
    }

    #[test]
    fn add_and_sub_report_overflow() {
        assert_eq!(huge().try_add(huge()), Err(TvmError::Overflow));
        assert_eq!(huge().try_sub(-huge()), Err(TvmError::Overflow));
    }

    #[test]
    fn scales_by_a_factor_preserving_currency() {
        let payment = Money::new(250.0, Currency::Usd).unwrap();
        assert_eq!(payment.try_mul(12.0).unwrap().value(), 3000.0);
        assert_eq!(payment.try_mul(12.0).unwrap().currency(), Currency::Usd);
        assert_eq!(payment.try_mul(0.0).unwrap().value(), 0.0);
        assert_eq!(payment.try_mul(-1.0).unwrap().value(), -250.0);
    }

    #[test]
    fn mul_rejects_a_non_finite_result() {
        // A finite factor that overflows the range is an Overflow; a non-finite
        // factor is a bad *input*, so it is NonFiniteScalar (ADR-0031, ADR-0052).
        assert_eq!(huge().try_mul(2.0), Err(TvmError::Overflow));
        assert_eq!(
            Money::agnostic(1.0).unwrap().try_mul(f64::INFINITY),
            Err(TvmError::NonFiniteScalar)
        );
        assert_eq!(
            Money::agnostic(1.0).unwrap().try_mul(f64::NAN),
            Err(TvmError::NonFiniteScalar)
        );
    }

    #[test]
    fn divides_by_a_divisor_preserving_currency() {
        let total = Money::new(3000.0, Currency::Usd).unwrap();
        assert_eq!(total.try_div(12.0).unwrap().value(), 250.0);
        assert_eq!(total.try_div(12.0).unwrap().currency(), Currency::Usd);
        assert_eq!(total.try_div(-12.0).unwrap().value(), -250.0);
        // An infinite divisor yields zero, which is finite — not an error.
        assert_eq!(total.try_div(f64::INFINITY).unwrap().value(), 0.0);
    }

    #[test]
    fn div_rejects_a_non_finite_result() {
        let total = Money::agnostic(3000.0).unwrap();
        // A zero divisor and a NaN divisor are different faults and now say so;
        // a finite divisor that overflows the range is an Overflow (ADR-0052).
        assert_eq!(total.try_div(0.0), Err(TvmError::DivisionByZero));
        assert_eq!(total.try_div(-0.0), Err(TvmError::DivisionByZero));
        assert_eq!(total.try_div(f64::NAN), Err(TvmError::NonFiniteScalar));
        // 0 / 0 is undefined, not zero.
        assert_eq!(Money::ZERO.try_div(0.0), Err(TvmError::DivisionByZero));
        assert_eq!(huge().try_div(0.5), Err(TvmError::Overflow));
    }

    #[test]
    fn abs_removes_the_sign_and_keeps_the_currency() {
        let outflow = Money::new(-25.0, Currency::Usd).unwrap();
        assert_eq!(outflow.abs().value(), 25.0);
        assert_eq!(outflow.abs().currency(), Currency::Usd);
        // Already positive, and the extremes: `abs` is closed over finite `f64`, so
        // it needs no `Result` (ADR-0021).
        assert_eq!(Money::new(25.0, Currency::Eur).unwrap().abs().value(), 25.0);
        assert_eq!(huge().abs(), huge());
        assert_eq!((-huge()).abs(), huge());
    }

    /// `abs` is idempotent and a fixed point of negation-then-`abs`, over every
    /// currency in the closed set — the currency is preserved, never folded away
    /// (ADR-0061). Iterated rather than sampled (ADR-0045 rule 2).
    #[test]
    fn abs_preserves_every_currency() {
        for &currency in Currency::ALL {
            let outflow = Money::new(-12.5, currency).unwrap();
            assert_eq!(outflow.abs().currency(), currency, "{}", currency.code());
            assert_eq!(outflow.abs(), (-outflow).abs());
            assert_eq!(outflow.abs().abs(), outflow.abs());
        }
    }

    /// `f64::abs(-0.0)` is `0.0`, and so is this: `Money::new(-0.0, c)` equals
    /// `Money::new(0.0, c)`, so leaving the sign bit on would let two *equal*
    /// amounts render differently (`-0` against `0`).
    #[test]
    fn abs_normalises_a_negative_zero() {
        let negative_zero = Money::new(-0.0, Currency::Usd).unwrap();
        assert!(negative_zero.value().is_sign_negative());
        assert!(negative_zero.abs().value().is_sign_positive());
        assert_eq!(negative_zero.abs().currency(), Currency::Usd);
    }

    #[test]
    fn signum_reports_the_direction_of_the_flow() {
        assert_eq!(Money::new(25.0, Currency::Usd).unwrap().signum(), 1.0);
        assert_eq!(Money::new(-25.0, Currency::Usd).unwrap().signum(), -1.0);
        assert_eq!(huge().signum(), 1.0);
        assert_eq!((-huge()).signum(), -1.0);
        // The currency is not consulted: the sign is the same in any of them.
        for &currency in Currency::ALL {
            assert_eq!(Money::new(-1.0, currency).unwrap().signum(), -1.0);
        }
    }

    /// The documented divergence from `f64::signum`, which answers `1.0` for `+0.0`
    /// and `-1.0` for `-0.0`. Two `Money` values that compare equal must not report
    /// opposite signs, and the two zeros *are* equal — so both answer `0.0`
    /// (ADR-0061).
    #[test]
    fn signum_of_either_zero_is_zero() {
        let positive = Money::new(0.0, Currency::Usd).unwrap();
        let negative = Money::new(-0.0, Currency::Usd).unwrap();
        assert_eq!(positive, negative);
        assert_eq!(positive.signum(), 0.0);
        assert_eq!(negative.signum(), 0.0);
        assert_eq!(Money::ZERO.signum(), 0.0);
        // The claim this test exists to defend.
        assert_eq!((-0.0f64).signum(), -1.0);
    }

    #[test]
    fn try_min_and_try_max_select_by_magnitude() {
        let small = Money::new(50.0, Currency::Usd).unwrap();
        let large = Money::new(75.0, Currency::Usd).unwrap();
        assert_eq!(small.try_min(large).unwrap(), small);
        assert_eq!(large.try_min(small).unwrap(), small);
        assert_eq!(small.try_max(large).unwrap(), large);
        assert_eq!(large.try_max(small).unwrap(), large);
        // Signed cashflows: an outflow is the smaller amount, not the smaller
        // magnitude.
        let outflow = Money::new(-100.0, Currency::Usd).unwrap();
        assert_eq!(outflow.try_min(small).unwrap(), outflow);
        assert_eq!(outflow.try_max(small).unwrap(), small);
    }

    /// Two distinct non-`Xxx` currencies are *unordered* (`partial_cmp` is `None`),
    /// so there is no smaller or larger one and the answer is the mismatch, named in
    /// argument order (ADR-0052). This is the whole reason the two are `try_`
    /// (ADR-0059, ADR-0061).
    #[test]
    fn try_min_and_try_max_report_unordered_currencies() {
        let usd = Money::new(100.0, Currency::Usd).unwrap();
        let eur = Money::new(200.0, Currency::Eur).unwrap();
        assert_eq!(usd.partial_cmp(&eur), None);

        let mismatch = TvmError::CurrencyMismatch {
            left: Currency::Usd,
            right: Currency::Eur,
        };
        assert_eq!(usd.try_min(eur), Err(mismatch.clone()));
        assert_eq!(usd.try_max(eur), Err(mismatch));
        assert_eq!(
            eur.try_min(usd),
            Err(TvmError::CurrencyMismatch {
                left: Currency::Eur,
                right: Currency::Usd,
            })
        );
    }

    /// The currency is folded by the `Xxx` identity rule, as `try_add` folds it, so
    /// selecting the agnostic side still yields a denominated result — and the
    /// operation is commutative even on a tie, which an unfolded currency would not
    /// be (ADR-0061).
    #[test]
    fn try_min_and_try_max_fold_the_currency() {
        let usd = Money::new(100.0, Currency::Usd).unwrap();
        let agnostic_lower = Money::agnostic(50.0).unwrap();

        let min = usd.try_min(agnostic_lower).unwrap();
        assert_eq!(min.value(), 50.0);
        assert_eq!(min.currency(), Currency::Usd); // not `Xxx`
        assert_eq!(usd.try_max(agnostic_lower).unwrap(), usd);

        // A tie: whichever side is selected, the answer is the same `Money`.
        let tie = Money::agnostic(100.0).unwrap();
        assert_eq!(usd.try_min(tie).unwrap(), tie.try_min(usd).unwrap());
        assert_eq!(usd.try_min(tie).unwrap().currency(), Currency::Usd);
        assert_eq!(usd.try_max(tie).unwrap().currency(), Currency::Usd);

        // Two agnostic amounts stay agnostic.
        assert_eq!(
            agnostic_lower.try_min(tie).unwrap().currency(),
            Currency::Xxx
        );
    }

    #[test]
    fn try_sum_totals_a_series() {
        let flows = [
            Money::new(-100.0, Currency::Usd).unwrap(),
            Money::new(60.0, Currency::Usd).unwrap(),
            Money::new(60.0, Currency::Usd).unwrap(),
        ];
        let total = Money::try_sum(flows.iter().copied()).unwrap();
        assert_eq!(total.value(), 20.0);
        assert_eq!(total.currency(), Currency::Usd);
        // An array, a `map`, and a single amount all work: any `IntoIterator`.
        assert_eq!(Money::try_sum(flows).unwrap(), total);
        assert_eq!(
            Money::try_sum(flows.iter().map(|m| -*m)).unwrap().value(),
            -20.0
        );
        assert_eq!(Money::try_sum([total]).unwrap(), total);
    }

    /// The documented empty case: nothing to total is `Money::ZERO`, the additive
    /// identity, and currency-agnostic (ADR-0061).
    #[test]
    fn try_sum_of_nothing_is_zero() {
        assert_eq!(Money::try_sum([]).unwrap(), Money::ZERO);
        assert_eq!(Money::try_sum([]).unwrap().currency(), Currency::Xxx);
        assert_eq!(Money::try_sum(core::iter::empty()).unwrap(), Money::ZERO);
    }

    /// `try_sum` folds the currency exactly as `try_add` does — the `Xxx` identity
    /// adopts, and the first clash is reported with `left` the currency accumulated
    /// so far and `right` the offending flow's (ADR-0052, ADR-0057).
    #[test]
    fn try_sum_folds_the_currency_and_names_the_first_clash() {
        let usd = Money::new(10.0, Currency::Usd).unwrap();
        let eur = Money::new(10.0, Currency::Eur).unwrap();
        let gbp = Money::new(10.0, Currency::Gbp).unwrap();

        // A leading agnostic amount adopts the currency that follows it.
        assert_eq!(
            Money::try_sum([Money::agnostic(5.0).unwrap(), usd])
                .unwrap()
                .currency(),
            Currency::Usd
        );
        // The fold stops at the *first* clash, whatever follows it.
        assert_eq!(
            Money::try_sum([usd, eur, gbp]),
            Err(TvmError::CurrencyMismatch {
                left: Currency::Usd,
                right: Currency::Eur,
            })
        );
        assert_eq!(
            Money::try_sum([usd, usd, gbp, eur]),
            Err(TvmError::CurrencyMismatch {
                left: Currency::Usd,
                right: Currency::Gbp,
            })
        );
    }

    /// A running total that leaves the finite range is an `Overflow`, through the
    /// same funnel as `try_add` (ADR-0021, ADR-0023) — `Money` never holds an
    /// infinity, not even mid-fold.
    #[test]
    fn try_sum_reports_overflow() {
        assert_eq!(Money::try_sum([huge(), huge()]), Err(TvmError::Overflow));
        // The overflow is in the *running* total: the mathematical sum here is
        // representable, but the left-to-right fold passes through `2 · f64::MAX`.
        assert_eq!(
            Money::try_sum([huge(), huge(), -huge()]),
            Err(TvmError::Overflow)
        );
    }

    /// `try_sum` is the `try_add` fold, so it must agree with one written by hand.
    #[test]
    fn try_sum_agrees_with_a_hand_written_fold() {
        let flows = [
            Money::new(-1000.0, Currency::Jpy).unwrap(),
            Money::new(250.5, Currency::Jpy).unwrap(),
            Money::agnostic(0.25).unwrap(),
            Money::new(-0.75, Currency::Jpy).unwrap(),
        ];
        let mut folded = Money::ZERO;
        for flow in flows {
            folded = folded.try_add(flow).unwrap();
        }
        assert_eq!(Money::try_sum(flows).unwrap(), folded);
    }

    #[test]
    fn ordering_is_within_a_compatible_currency() {
        let a = Money::new(100.0, Currency::Usd).unwrap();
        let b = Money::new(200.0, Currency::Usd).unwrap();
        assert!(a < b);
        // Agnostic amounts are comparable with any currency.
        assert!(Money::agnostic(50.0).unwrap() < a);
        // Distinct currencies are unordered.
        let eur = Money::new(200.0, Currency::Eur).unwrap();
        assert_eq!(a.partial_cmp(&eur), None);
    }

    #[test]
    fn equality_distinguishes_currency() {
        assert_ne!(
            Money::new(1.0, Currency::Usd).unwrap(),
            Money::new(1.0, Currency::Eur).unwrap()
        );
        assert_eq!(
            Money::new(1.0, Currency::Usd).unwrap(),
            Money::new(1.0, Currency::Usd).unwrap()
        );
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Money::default(), Money::ZERO);
    }

    #[test]
    fn try_from_mirrors_agnostic() {
        assert_eq!(Money::try_from(42.5).unwrap().value(), 42.5);
        assert_eq!(Money::try_from(42.5).unwrap().currency(), Currency::Xxx);
        assert_eq!(Money::try_from(f64::NAN), Err(TvmError::NonFiniteAmount));
        // Usable through the `TryInto` sugar at an inference site.
        let m: Money = 10.0.try_into().unwrap();
        assert_eq!(m.value(), 10.0);
    }

    #[test]
    fn into_f64_is_the_magnitude() {
        assert_eq!(f64::from(Money::new(42.5, Currency::Usd).unwrap()), 42.5);
        let x: f64 = Money::new(-7.0, Currency::Eur).unwrap().into();
        assert_eq!(x, -7.0);
    }

    #[cfg(any(feature = "std", feature = "libm"))]
    #[test]
    fn rounds_to_the_currency_minor_unit() {
        // 2 decimals for USD, 0 for JPY, 3 for BHD; half away from zero. Example
        // values are chosen to avoid f64 tie-representation ambiguity.
        assert_eq!(
            Money::new(2.348, Currency::Usd)
                .unwrap()
                .round_to_currency()
                .value(),
            2.35
        );
        assert_eq!(
            Money::new(2.344, Currency::Usd)
                .unwrap()
                .round_to_currency()
                .value(),
            2.34
        );
        assert_eq!(
            Money::new(1234.9, Currency::Jpy)
                .unwrap()
                .round_to_currency()
                .value(),
            1235.0
        );
        assert_eq!(
            Money::new(1.23456, Currency::Bhd)
                .unwrap()
                .round_to_currency()
                .value(),
            1.235
        );
        // No minor unit — unchanged; currency preserved.
        let gold = Money::new(1.23456, Currency::Xau)
            .unwrap()
            .round_to_currency();
        assert_eq!(gold.value(), 1.23456);
        assert_eq!(gold.currency(), Currency::Xau);
    }

    /// Rounding used to write `magnitude * scale` straight into the struct, so a
    /// large magnitude produced an **infinite** `Money` — the one thing the type
    /// promises never to hold (ADR-0054). The boundary is `f64::MAX / scale`, so
    /// it depends on the currency's minor unit: `JPY` (scale `1`) can never
    /// overflow, `BHD` (scale `1000`) overflows two decades sooner than `USD`.
    #[cfg(any(feature = "std", feature = "libm"))]
    #[test]
    fn rounding_a_huge_magnitude_stays_finite_and_unchanged() {
        for currency in [
            Currency::Usd,
            Currency::Jpy,
            Currency::Bhd,
            Currency::Clf,
            Currency::Xau,
        ] {
            for magnitude in [f64::MAX, -f64::MAX, 1.8e306, 1e300, 1e16] {
                let rounded = Money::new(magnitude, currency).unwrap().round_to_currency();
                assert!(
                    rounded.value().is_finite(),
                    "{magnitude:e} in {} rounded to a non-finite value",
                    currency.code(),
                );
                // Above 2^53 every f64 is already an integer, so it is already an
                // exact multiple of any minor unit: rounding is the identity.
                assert_eq!(
                    rounded.value(),
                    magnitude,
                    "{magnitude:e} in {} was altered by rounding",
                    currency.code(),
                );
                assert_eq!(rounded.currency(), currency);
            }
        }
    }

    /// `round_to_currency` looks its scale up by minor-unit exponent. The lookup
    /// must cover every currency the crate knows, or the amount would silently
    /// pass through unrounded (and, in the array form this replaced, panic).
    /// ADR-0045 rule 2: the finite domain is checked exhaustively, not sampled.
    #[cfg(any(feature = "std", feature = "libm"))]
    #[test]
    fn every_currency_has_a_minor_unit_scale() {
        for &currency in Currency::ALL {
            let Some(exponent) = currency.minor_unit_exponent() else {
                continue;
            };
            assert!(
                super::minor_unit_scale(exponent).is_some(),
                "{} has minor-unit exponent {exponent}, beyond the scale table",
                currency.code(),
            );
        }
    }

    #[test]
    fn fx_convert_round_trips() {
        use crate::FxRate;
        let usd = Money::new(100.0, Currency::Usd).unwrap();
        let fx = FxRate::new(Currency::Usd, Currency::Eur, 0.9).unwrap();
        let eur = usd.convert(fx).unwrap();
        assert_eq!(eur.value(), 90.0);
        assert_eq!(eur.currency(), Currency::Eur);
        let back = eur.convert(fx.inverse()).unwrap();
        assert!((back.value() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn fx_convert_requires_matching_from_currency() {
        let eur = Money::new(100.0, Currency::Eur).unwrap();
        let fx = crate::FxRate::new(Currency::Usd, Currency::Gbp, 0.8).unwrap();
        // `left` is the amount's own currency, `right` the rate's `from` (ADR-0052).
        assert_eq!(
            eur.convert(fx),
            Err(TvmError::CurrencyMismatch {
                left: Currency::Eur,
                right: Currency::Usd,
            })
        );
    }

    #[test]
    fn fx_rate_rejects_non_positive_or_non_finite() {
        use crate::FxRate;
        assert_eq!(
            FxRate::new(Currency::Usd, Currency::Eur, 0.0),
            Err(TvmError::InvalidExchangeRate)
        );
        assert_eq!(
            FxRate::new(Currency::Usd, Currency::Eur, -1.0),
            Err(TvmError::InvalidExchangeRate)
        );
        assert_eq!(
            FxRate::new(Currency::Usd, Currency::Eur, f64::NAN),
            Err(TvmError::InvalidExchangeRate)
        );
        assert_eq!(
            FxRate::new(Currency::Usd, Currency::Eur, f64::INFINITY),
            Err(TvmError::InvalidExchangeRate)
        );
    }

    /// The two ends of the band `new` excludes so that `inverse` cannot lie
    /// (ADR-0053). Each of these passed the old `is_finite() && > 0.0` test.
    #[test]
    fn fx_rate_rejects_rates_whose_reciprocal_escapes_the_domain() {
        use crate::FxRate;
        let rejected = |rate| FxRate::new(Currency::Usd, Currency::Eur, rate);

        // Subnormal: `1.0 / 5e-324` is `+∞`, and inverting twice gives `0.0` — a
        // value `new` itself rejects.
        assert_eq!(rejected(5e-324), Err(TvmError::InvalidExchangeRate));
        assert_eq!(
            rejected(f64::MIN_POSITIVE / 2.0),
            Err(TvmError::InvalidExchangeRate)
        );
        // Normal, but its reciprocal is subnormal: `1.0 / f64::MAX ≈ 5.6e-309`.
        // This is why `is_normal()` alone does not close the domain.
        assert!(f64::MAX.is_normal());
        assert!((1.0 / f64::MAX).is_subnormal());
        assert_eq!(rejected(f64::MAX), Err(TvmError::InvalidExchangeRate));

        // The exact boundaries are admitted: `f64::MIN_POSITIVE` is the smallest
        // normal, and its reciprocal is the largest rate whose own reciprocal is
        // still normal.
        assert!(rejected(f64::MIN_POSITIVE).is_ok());
        assert!(rejected(1.0 / f64::MIN_POSITIVE).is_ok());
    }

    /// The accepted domain is closed under reciprocal, so `inverse()` is honest at
    /// its extremes and not merely in the middle (ADR-0053).
    #[test]
    fn fx_rate_inverse_stays_in_the_accepted_domain() {
        use crate::FxRate;
        for rate in [
            f64::MIN_POSITIVE,
            1.0 / f64::MIN_POSITIVE,
            1e-300,
            1e300,
            1e-7,
            1e7,
            0.9,
            1.0,
            1.25,
        ] {
            let fx = FxRate::new(Currency::Usd, Currency::Eur, rate).unwrap();
            let inverted = fx.inverse();
            assert!(
                inverted.rate().is_normal() && inverted.rate() > 0.0,
                "inverse of {rate:e} left the domain: {}",
                inverted.rate()
            );
            // The inverse is a rate `new` would itself accept — the property the
            // constructor's domain exists to guarantee.
            assert!(FxRate::new(Currency::Eur, Currency::Usd, inverted.rate()).is_ok());
        }
    }

    /// A double inverse recovers the original rate and direction. The magnitude is
    /// only recovered to within a rounding error — `1.0 / (1.0 / x)` is not exact
    /// for every `x` — so the currencies are compared exactly and the rate
    /// relatively (ADR-0033's approximate-real precision contract).
    #[test]
    fn fx_rate_double_inverse_recovers_the_original() {
        use crate::FxRate;
        for rate in [f64::MIN_POSITIVE, 1.0 / f64::MIN_POSITIVE, 1e-7, 0.9, 1.25] {
            let fx = FxRate::new(Currency::Gbp, Currency::Jpy, rate).unwrap();
            let round_tripped = fx.inverse().inverse();
            assert_eq!(round_tripped.source(), Currency::Gbp);
            assert_eq!(round_tripped.to(), Currency::Jpy);
            let relative_error = (round_tripped.rate() - rate).abs() / rate;
            assert!(
                relative_error < 1e-15,
                "double inverse of {rate:e} gave {:e}",
                round_tripped.rate()
            );
        }
    }

    #[test]
    fn fx_rate_accessors_report_the_pair() {
        use crate::FxRate;
        let fx = FxRate::new(Currency::Gbp, Currency::Usd, 1.25).unwrap();
        assert_eq!(fx.source(), Currency::Gbp);
        assert_eq!(fx.to(), Currency::Usd);
        assert_eq!(fx.rate(), 1.25);
    }
}
