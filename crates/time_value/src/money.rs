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
/// Negation is a [`Neg`] operator: negating a finite amount is always finite, so
/// it cannot fail. Addition, subtraction and scaling *can* leave `f64` range (and
/// addition/subtraction can find mismatched currencies), so they are fallible
/// [`try_add`](Self::try_add), [`try_sub`](Self::try_sub),
/// [`try_mul`](Self::try_mul) and [`try_div`](Self::try_div) methods rather than
/// operators — an operator cannot return a `Result`, and silently yielding an
/// infinity is the foot-gun this crate exists to avoid
/// (`docs/adr/0023-money-arithmetic-surface.md`).
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

/// Formats the bare magnitude. Currency-aware formatting (with the code and
/// minor-unit rounding) is a presentation concern left to the caller.
impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.magnitude.fmt(f)
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
