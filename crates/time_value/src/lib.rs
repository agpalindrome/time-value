//! # `time_value`
//!
//! Type-safe time-value-of-money (TVM) calculations.
//!
//! This crate is a deliberately type-heavy redesign of `time_value`, rebuilt
//! from scratch for the `1.0` line. The design goal is to make TVM mistakes —
//! applying an annual rate to monthly cashflows, discounting with an
//! economically meaningless rate — *compile errors* rather than silent
//! arithmetic, while keeping the common path ergonomic.
//!
//! The crate is `#![no_std]` and dependency-free by default.
//!
//! ## Model
//!
//! - [`Money`] is a validated monetary amount — an `f64` magnitude plus the
//!   [`Currency`] it is denominated in — always finite, because every operation
//!   that could overflow returns a `Result` instead; cashflows are signed (outflow
//!   negative, inflow positive). Currency is a *runtime value*, not a type tag
//!   ([`Currency::Xxx`] is the agnostic identity); a mismatch is a runtime
//!   [`TvmError::CurrencyMismatch`].
//! - [`Rate<P>`] is a per-period interest rate tagged with a [`Periodicity`]
//!   marker (`P` — e.g. [`Monthly`], [`Annual`]). The tag is zero-sized.
//! - [`Period<P>`] is a periodicity-tagged count of periods — "how many periods
//!   *of periodicity `P`*". Periodicity is the crate's sole compile-time tag,
//!   applied uniformly to `Rate<P>`, `Period<P>`, and `Cashflows<P>`, so pairing a
//!   duration with a rate of a different periodicity is a compile error
//!   everywhere, not just for series (`docs/adr/0035-periodicity-tagged-time.md`).
//! - [`Cashflows<P>`] is a periodicity-tagged series of cashflows at consecutive
//!   periods. Discounting a [`Cashflows<P>`] requires a [`Rate<P>`] of the *same*
//!   periodicity, so a mismatch is a compile error.
//! - Where an operation takes **two arguments of the same type** that a caller
//!   could transpose — two [`Money`] amounts, or a rate and a growth rate — the
//!   ambiguous positions are tagged with a zero-cost *role* newtype:
//!   [`Payment`], [`PresentValue`], [`FutureValue`], [`Principal`], and
//!   [`Growth<P>`]. They validate nothing (the inner value already did) and cost
//!   nothing; they make the swap a compile error
//!   (`docs/adr/0050-role-newtypes-for-ambiguous-arguments.md`).
//!
//! ## Operations
//!
//! The discrete operations — [`net_present_value`], [`net_future_value`], and
//! [`internal_rate_of_return`] — need only elementary arithmetic and are
//! available in the default `no_std`, zero-dependency build, as is the
//! allocation-free [`amortization`] schedule iterator (from an explicit payment;
//! its term-based constructor needs a feature).
//!
//! Operations that require transcendental functions (`powf`, `ln`, `exp`) live
//! behind the optional `std` / `libm` features (see
//! `docs/adr/0009-no_std-and-optional-libm.md`): the [`single_sum`] module
//! (present/future value and the solve-for `periods` / `rate` inverses, with the
//! [`Period<P>`] type), the [`annuity`] module (ordinary, [annuity-due](annuity::due),
//! [perpetuity](annuity::perpetuity), and [growing-perpetuity](annuity::growing_perpetuity)
//! forms, plus the `periods` / `rate` solves), the modified internal rate of return
//! ([`Cashflows::modified_internal_rate_of_return`]), the term-based
//! [`amortization`] constructor, effective rate conversions between
//! periodicities ([`Rate::convert`] / [`Rate::effective_annual`]),
//! [`DatedCashflows`] (XNPV/XIRR over irregularly dated flows), and the
//! [`continuous`] module (continuous compounding at a periodicity-free
//! [`ContinuousRate`], with the `Rate<Annual>` bridge). Nominal-rate
//! conversion ([`Rate::from_nominal_annual`] / [`Rate::nominal_annual`]) is plain
//! arithmetic and needs no feature.
//!
//! The optional `alloc` feature (off by default, implied by `std`) adds the owned
//! [`OwnedCashflows`] series — built from a `Vec` or an iterator — complementing
//! the borrowed, allocation-free [`Cashflows`] (`docs/adr/0043-owned-cashflows.md`).
//!
//! The optional `serde` feature (off by default, `no_std`-compatible) derives
//! `Serialize`/`Deserialize` for the public value types — bare numbers for the
//! newtypes, `{ amount, currency }` for [`Money`], the ISO 4217 code for
//! [`Currency`] — validating through the fallible constructors on the way in
//! (`docs/adr/0042-serde-support.md`). The optional `schemars` feature (off by
//! default, `no_std`-compatible, implies `alloc`) implements `JsonSchema` for
//! those same types, describing the identical shapes
//! (`docs/adr/0044-schemars-support.md`).
//!
//! ## Thread safety
//!
//! The public types are plain data with no interior mutability, so the owned
//! value types ([`Money`], [`Rate<P>`], [`Currency`], [`TvmError`], the
//! [`amortization`] types, and the feature-gated [`Period<P>`], [`ContinuousRate`],
//! [`OwnedCashflows`], …) are **`Send + Sync + 'static`**, and the borrowing views
//! ([`Cashflows<P>`], [`DatedCashflows`]) are **`Send + Sync`** (they are not
//! `'static` only because they borrow). This is a maintained part of the API — it
//! lets callers move values across threads, share them by `&`/`Arc`, and hold them
//! across `.await` in a `Send` future — and it is locked by `tests/thread_safety.rs`
//! (`docs/adr/0046-thread-safety-of-the-public-types.md`).
//!
//! ```
//! use time_value::{Cashflows, Money, Monthly, Rate};
//!
//! // A project: pay 100 now, receive 60 next month and 60 the month after.
//! // Pure-number TVM is currency-agnostic (`Money::agnostic`).
//! let flows = [Money::agnostic(-100.0)?, Money::agnostic(60.0)?, Money::agnostic(60.0)?];
//! let project = Cashflows::<Monthly>::new(&flows);
//!
//! let npv = project.net_present_value(Rate::<Monthly>::new(0.01)?)?;
//! assert!(npv.value() > 0.0); // worth doing at 1%/month
//!
//! let irr = project.internal_rate_of_return()?;
//! assert!((irr.value() - 0.1307).abs() < 1e-4); // ~13.07% per month
//! # Ok::<(), time_value::TvmError>(())
//! ```
//!
//! [`Cashflows<P>`]: Cashflows
//! [`Rate<P>`]: Rate
//! [`Period<P>`]: Period
//! [`net_present_value`]: Cashflows::net_present_value
//! [`net_future_value`]: Cashflows::net_future_value
//! [`internal_rate_of_return`]: Cashflows::internal_rate_of_return

// `no_std` unless the `std` feature is enabled — the `std` feature turns this
// into an ordinary `std` crate so it can use `f64`'s transcendental methods.
#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]

// The owned `OwnedCashflows` series needs a `Vec`; pull in the `alloc` crate
// (without requiring `std`) when the `alloc` feature — implied by `std` — is on
// (ADR-0043).
#[cfg(feature = "alloc")]
extern crate alloc;

pub mod amortization;
mod cashflows;
mod currency;
mod money;
mod periodicity;
mod rate;
mod roles;
mod root;

pub use cashflows::Cashflows;
#[cfg(feature = "alloc")]
pub use cashflows::OwnedCashflows;
pub use currency::Currency;
pub use money::{FxRate, Money};
pub use periodicity::{Annual, Daily, Monthly, Periodicity, Quarterly, SemiAnnual, Weekly};
pub use rate::Rate;
pub use roles::{FutureValue, Growth, Payment, PresentValue, Principal};

// Operations that need transcendental math (`powf`) are available only with the
// `std` or `libm` feature (see `docs/adr/0014-transcendental-single-sum-operations.md`).
#[cfg(any(feature = "std", feature = "libm"))]
pub mod annuity;
#[cfg(any(feature = "std", feature = "libm"))]
pub mod continuous;
#[cfg(any(feature = "std", feature = "libm"))]
mod dated;
#[cfg(any(feature = "std", feature = "libm"))]
mod math;
#[cfg(any(feature = "std", feature = "libm"))]
mod period;
#[cfg(any(feature = "std", feature = "libm"))]
pub mod single_sum;

// The private `*Wire` structs shared by the `serde` and `schemars` impls, so the
// two describe one wire format (ADR-0042 / ADR-0044).
#[cfg(any(feature = "serde", feature = "schemars"))]
mod wire;

// `serde` support for the public value types, behind the off-by-default feature
// (ADR-0042). The impls compose from the types' public API, so this is a leaf
// module with nothing re-exported.
#[cfg(feature = "serde")]
mod serde_impls;

// `schemars` (JsonSchema) support — the JSON-Schema companion to `serde`
// (ADR-0044), also a leaf module of impls.
#[cfg(feature = "schemars")]
mod schemars_impls;

pub use amortization::{Installment, Schedule};
#[cfg(any(feature = "std", feature = "libm"))]
pub use continuous::ContinuousRate;
#[cfg(any(feature = "std", feature = "libm"))]
pub use dated::{DatedCashflow, DatedCashflows};
#[cfg(any(feature = "std", feature = "libm"))]
pub use period::Period;

use core::fmt;

/// Errors produced when constructing or operating on time-value types.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TvmError {
    /// A rate was not finite, or was less than or equal to `-1.0` (i.e. ≤ −100%),
    /// which is economically meaningless for discounting and compounding.
    RateOutOfRange,
    /// A [`ContinuousRate`] (a force of interest) supplied to a constructor was not
    /// finite. Unlike [`RateOutOfRange`](Self::RateOutOfRange), a continuous rate has
    /// no `> -1` floor — any *finite* force of interest is valid (its effective
    /// growth factor `e^δ` is always positive) — so only non-finiteness is rejected
    /// (`docs/adr/0036-continuous-compounding-force-of-interest.md`).
    NonFiniteRate,
    /// A monetary amount supplied to a constructor was not finite (`NaN` or an
    /// infinity). For a non-finite value *produced by an operation*, see
    /// [`Overflow`](Self::Overflow) and [`Undefined`](Self::Undefined).
    NonFiniteAmount,
    /// An arithmetic or TVM operation combined two amounts denominated in distinct
    /// non-[`Xxx`](Currency::Xxx) currencies. An agnostic [`Xxx`](Currency::Xxx)
    /// amount adopts the currency it is combined with, but two different real
    /// currencies cannot be added, subtracted, or discounted together
    /// (`docs/adr/0034-money-and-currency.md`).
    CurrencyMismatch,
    /// An exchange rate supplied to [`FxRate::new`](crate::FxRate::new) was not
    /// finite or was not strictly positive; a non-positive price has no economic
    /// meaning (ADR-0034).
    InvalidExchangeRate,
    /// An operation's `f64` arithmetic overflowed the finite range — a genuine
    /// result exists mathematically but is too large to represent, so it became an
    /// infinity or `NaN` (e.g. compounding an enormous rate over a long horizon).
    /// Distinct from [`Undefined`](Self::Undefined), where the operation has no
    /// answer for the inputs at all, and from
    /// [`NonFiniteAmount`](Self::NonFiniteAmount), a non-finite value passed *in*
    /// (ADR-0021, ADR-0031).
    Overflow,
    /// An operation is mathematically undefined for the given inputs — a
    /// degenerate case with no answer, not an overflow of a real one. Examples:
    /// an [`annuity::payment`] over zero periods (nothing to amortise over), a
    /// solved period count whose logarithm has a non-positive argument, or a
    /// [`Cashflows::modified_internal_rate_of_return`] on a series with no span or
    /// no outflows. Distinct from [`Overflow`](Self::Overflow), which is a real
    /// result that exceeds `f64` range (ADR-0031).
    Undefined,
    /// A period count was negative or not finite.
    NegativePeriods,
    /// A duration in years, given as a plain `f64`, was not finite (`NaN` or an
    /// infinity). Used for a [`DatedCashflow`]'s year-offset (ADR-0029) and for the
    /// [`continuous`] operations' `years` duration (ADR-0036). The value may be
    /// negative or zero, but must be finite.
    NonFiniteOffset,
    /// An operation that requires at least one cashflow was given an empty
    /// series (e.g. [`Cashflows::internal_rate_of_return`]).
    EmptyCashflows,
    /// [`Cashflows::internal_rate_of_return`] did not converge to a root within
    /// its iteration budget, or the iteration left the valid rate domain.
    IrrDidNotConverge,
    /// A solve-for-rate operation did not converge to a root — no rate satisfies
    /// the relationship over the valid domain (e.g. [`annuity::rate`] when no rate
    /// prices the given payment stream at the target value). Distinct from
    /// [`IrrDidNotConverge`](Self::IrrDidNotConverge), which is specific to
    /// [`Cashflows::internal_rate_of_return`].
    SolveDidNotConverge,
    /// A perpetuity's present value diverges because its rate does not exceed its
    /// growth rate (`r <= g`; for a level perpetuity, `r <= 0`). The closed form
    /// `PMT / (r - g)` would return either an infinity (`r = g`) or a finite but
    /// economically meaningless value (`r < g`) for a series that does not
    /// converge, so [`annuity::perpetuity`] / [`annuity::growing_perpetuity`]
    /// reject it instead (ADR-0015).
    DivergentPerpetuity,
}

impl fmt::Display for TvmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::RateOutOfRange => {
                f.write_str("rate must be finite and greater than -1.0 (-100%)")
            }
            Self::NonFiniteRate => {
                f.write_str("continuous rate (force of interest) must be finite")
            }
            Self::NonFiniteAmount => f.write_str("monetary amount must be finite"),
            Self::CurrencyMismatch => {
                f.write_str("amounts are in distinct currencies and cannot be combined")
            }
            Self::InvalidExchangeRate => {
                f.write_str("exchange rate must be finite and greater than zero")
            }
            Self::Overflow => f.write_str("operation overflowed the finite range"),
            Self::Undefined => f.write_str("operation is undefined for these inputs"),
            Self::NegativePeriods => f.write_str("period count must be finite and non-negative"),
            Self::NonFiniteOffset => f.write_str("dated cashflow year-offset must be finite"),
            Self::EmptyCashflows => f.write_str("cashflow series is empty"),
            Self::IrrDidNotConverge => f.write_str("internal rate of return did not converge"),
            Self::SolveDidNotConverge => f.write_str("solve for rate did not converge"),
            Self::DivergentPerpetuity => {
                f.write_str("perpetuity present value diverges: rate does not exceed growth rate")
            }
        }
    }
}

impl core::error::Error for TvmError {}
