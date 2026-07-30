//! Private `*Wire` structs — the field layout of the composite value types'
//! serialized form, shared by the `serde` and `schemars` impls so the two cannot
//! describe different shapes for the same wire format (ADR-0042 / ADR-0044).
//!
//! Each derives `Serialize`/`Deserialize` under `serde` and `JsonSchema` under
//! `schemars`; the public impls in `serde_impls` / `schemars_impls` delegate to
//! these (serde routes deserialization through the fallible constructor; schemars
//! just borrows the shape). The field names *are* the wire keys.

// Under `schemars` alone the fields feed the `JsonSchema` derive at compile time
// (their types shape the schema) but are never *read* at runtime, so they look
// dead; under `serde` they are read. Conditionally-dead by design — allow it.
#![allow(dead_code)]

use crate::{Currency, Money};

/// `Money` → `{ amount, currency }` (the currency is always present).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) struct MoneyWire {
    pub(crate) amount: f64,
    pub(crate) currency: Currency,
}

/// `FxRate` → `{ from, to, rate }`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) struct FxRateWire {
    pub(crate) from: Currency,
    pub(crate) to: Currency,
    pub(crate) rate: f64,
}

/// `DatedCashflow` → `{ offset_years, amount }`. Gated with its type (std/libm).
#[cfg(any(feature = "std", feature = "libm"))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) struct DatedCashflowWire {
    pub(crate) offset_years: f64,
    pub(crate) amount: Money,
}

/// `OwnedCashflows` → a bare JSON **array** of `Money` in period order; the
/// periodicity tag is a compile-time marker and is not on the wire (ADR-0060).
/// Gated with its type (`alloc`).
///
/// The only sequence wire type, and the only one that is a *newtype*: both derives
/// see straight through it to the inner sequence (serde serializes a newtype struct
/// as its field; schemars derives the field's schema), so the shape stays "an array
/// of `Money`" while the two descriptions still come from one declaration. The
/// `Cow` is what lets that single declaration serve both directions — borrowed on
/// the way out, so serializing a series copies nothing, and owned on the way in.
#[cfg(feature = "alloc")]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) struct OwnedCashflowsWire<'a>(pub(crate) alloc::borrow::Cow<'a, [Money]>);

/// `OwnedDatedCashflows` → a bare JSON **array** of `DatedCashflow` in slice order,
/// which is meaningful order: the first entry is the XNPV's valuation reference
/// (ADR-0065). The dated sibling of [`OwnedCashflowsWire`], with the same newtype +
/// `Cow` construction and for the same reasons — one declaration for both derives,
/// borrowed out and owned in. Gated with its type (`alloc` **and** std/libm, since
/// `DatedCashflow` itself is).
#[cfg(all(feature = "alloc", any(feature = "std", feature = "libm")))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) struct OwnedDatedCashflowsWire<'a>(
    pub(crate) alloc::borrow::Cow<'a, [crate::DatedCashflow]>,
);

/// `Installment` → `{ period, payment, interest, principal, balance }`.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub(crate) struct InstallmentWire {
    pub(crate) period: u32,
    pub(crate) payment: Money,
    pub(crate) interest: Money,
    pub(crate) principal: Money,
    pub(crate) balance: Money,
}
