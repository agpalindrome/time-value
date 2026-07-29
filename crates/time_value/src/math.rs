//! Transcendental helpers behind the `std` / `libm` features.
//!
//! The core is `no_std` and dependency-free by default
//! (`docs/adr/0009-no_std-and-optional-libm.md`); the operations here need
//! `powf`, which lives in `std` or — for `no_std` builds — in `libm`. `std` is
//! preferred when both features are enabled.

/// `base` raised to the power `exponent`, via `std`.
#[cfg(feature = "std")]
#[inline]
pub(crate) fn powf(base: f64, exponent: f64) -> f64 {
    base.powf(exponent)
}

/// `base` raised to the power `exponent`, via `libm` (for `no_std` builds).
#[cfg(all(not(feature = "std"), feature = "libm"))]
#[inline]
pub(crate) fn powf(base: f64, exponent: f64) -> f64 {
    libm::pow(base, exponent)
}

/// `e` raised to the power `x`, via `std`.
#[cfg(feature = "std")]
#[inline]
pub(crate) fn exp(x: f64) -> f64 {
    x.exp()
}

/// `eˣ − 1`, computed without the cancellation of `exp(x) - 1`, via `std`.
///
/// For small `x`, `exp(x)` is `1 + x + …`, so subtracting `1` throws away the
/// leading digits of the answer. `exp_m1` keeps them, which is what makes the
/// annuity factors well-conditioned near `r = 0`
/// (`docs/adr/0054-numeric-robustness-of-the-core-operations.md`).
#[cfg(feature = "std")]
#[inline]
pub(crate) fn exp_m1(x: f64) -> f64 {
    x.exp_m1()
}

/// `eˣ − 1`, via `libm` (for `no_std` builds).
#[cfg(all(not(feature = "std"), feature = "libm"))]
#[inline]
pub(crate) fn exp_m1(x: f64) -> f64 {
    libm::expm1(x)
}

/// `ln(1 + x)`, computed without the cancellation of `ln(1.0 + x)`, via `std`.
///
/// The companion to [`exp_m1`]: for small `x`, forming `1.0 + x` first rounds
/// away the low bits of `x`, and the logarithm cannot recover them.
#[cfg(feature = "std")]
#[inline]
pub(crate) fn ln_1p(x: f64) -> f64 {
    x.ln_1p()
}

/// `ln(1 + x)`, via `libm` (for `no_std` builds).
#[cfg(all(not(feature = "std"), feature = "libm"))]
#[inline]
pub(crate) fn ln_1p(x: f64) -> f64 {
    libm::log1p(x)
}

/// `e` raised to the power `x`, via `libm` (for `no_std` builds).
#[cfg(all(not(feature = "std"), feature = "libm"))]
#[inline]
pub(crate) fn exp(x: f64) -> f64 {
    libm::exp(x)
}

/// The natural logarithm of `x`, via `std`.
#[cfg(feature = "std")]
#[inline]
pub(crate) fn ln(x: f64) -> f64 {
    x.ln()
}

/// The natural logarithm of `x`, via `libm` (for `no_std` builds).
#[cfg(all(not(feature = "std"), feature = "libm"))]
#[inline]
pub(crate) fn ln(x: f64) -> f64 {
    libm::log(x)
}

/// `x` rounded to the nearest integer (half away from zero), via `std`.
#[cfg(feature = "std")]
#[inline]
pub(crate) fn round(x: f64) -> f64 {
    x.round()
}

/// `x` rounded to the nearest integer (half away from zero), via `libm` (for
/// `no_std` builds).
#[cfg(all(not(feature = "std"), feature = "libm"))]
#[inline]
pub(crate) fn round(x: f64) -> f64 {
    libm::round(x)
}
