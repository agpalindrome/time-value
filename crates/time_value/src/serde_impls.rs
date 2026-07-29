//! `serde` support for the public value types (ADR-0042), behind the off-by-default
//! `serde` feature.
//!
//! The whole wire contract lives here, in one module, built only on the types'
//! **public** API (accessors out, fallible constructors in) — so it needs no
//! access to private fields and the format is reviewable in one place.
//!
//! Two shapes, by the type:
//!
//! - **Bare numbers.** The periodicity-tagged newtypes ([`Rate`], [`Period`]) and
//!   [`ContinuousRate`] serialize as a plain `f64` — the `PhantomData` tag is not
//!   on the wire (ADR-0019's original intent). The impls are hand-written so the
//!   phantom parameter `P` does not pick up a spurious `Serialize`/`Deserialize`
//!   bound.
//! - **Structs / strings.** [`Money`] is `{ amount, currency }` (the currency is
//!   *always* present — `"XXX"` for the agnostic amount — so it round-trips
//!   losslessly), [`Currency`] is its ISO 4217 code string, and [`FxRate`],
//!   [`DatedCashflow`], [`Installment`] are their field structs. Each defers to a
//!   small private `*Wire` struct so the derive does the field plumbing while the
//!   public impl still runs the constructor.
//!
//! **Deserialization validates.** Every value is rebuilt through its fallible
//! constructor ([`Rate::try_from`], [`Money::new`], [`Currency::from_code`], …),
//! so an out-of-domain number or an unknown currency code is a deserialization
//! error, not a silently-constructed invalid value. This is the whole reason the
//! newtypes cannot use a naive `#[serde(transparent)]` derive, which would bypass
//! the check.

use core::fmt;

use serde::de::{Error as DeError, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::wire::{FxRateWire, InstallmentWire, MoneyWire};
use crate::{amortization::Installment, Currency, FxRate, Money, Periodicity, Rate};
// `Period`, `ContinuousRate`, and `DatedCashflow` live in modules gated behind
// `std`/`libm` (they need transcendental math), so they do not exist in a pure
// `no_std` build — their serde impls carry the same gate.
#[cfg(any(feature = "std", feature = "libm"))]
use crate::wire::DatedCashflowWire;
#[cfg(any(feature = "std", feature = "libm"))]
use crate::{ContinuousRate, DatedCashflow, Period};

// ---- Bare-number newtypes -------------------------------------------------
//
// `serialize_f64(value())` out; `f64` in, rebuilt through the validating
// `TryFrom<f64>` so the domain invariant (finite, and `> −1` for a rate / `≥ 0`
// for a period) holds on the wire boundary too.

macro_rules! bare_number_serde {
    ($ty:ty $(, $param:ident)?) => {
        impl$(<$param: Periodicity>)? Serialize for $ty {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_f64(self.value())
            }
        }

        impl<'de $(, $param: Periodicity)?> Deserialize<'de> for $ty {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = f64::deserialize(deserializer)?;
                Self::try_from(value).map_err(DeError::custom)
            }
        }
    };
}

bare_number_serde!(Rate<P>, P);
#[cfg(any(feature = "std", feature = "libm"))]
bare_number_serde!(Period<P>, P);
#[cfg(any(feature = "std", feature = "libm"))]
bare_number_serde!(ContinuousRate);

// ---- Currency: an ISO 4217 code string ------------------------------------

impl Serialize for Currency {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.code())
    }
}

impl<'de> Deserialize<'de> for Currency {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CodeVisitor;

        impl Visitor<'_> for CodeVisitor {
            type Value = Currency;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an ISO 4217 currency code (e.g. `USD`)")
            }

            fn visit_str<E: DeError>(self, code: &str) -> Result<Currency, E> {
                Currency::from_code(code).ok_or_else(|| {
                    E::custom(format_args!("unknown ISO 4217 currency code `{code}`"))
                })
            }
        }

        deserializer.deserialize_str(CodeVisitor)
    }
}

// ---- Money: { amount, currency } (currency always present) ----------------

impl Serialize for Money {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        MoneyWire {
            amount: self.value(),
            currency: self.currency(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Money {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = MoneyWire::deserialize(deserializer)?;
        Money::new(wire.amount, wire.currency).map_err(DeError::custom)
    }
}

// ---- FxRate: { from, to, rate } -------------------------------------------

impl Serialize for FxRate {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        FxRateWire {
            from: self.source(),
            to: self.to(),
            rate: self.rate(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FxRate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = FxRateWire::deserialize(deserializer)?;
        FxRate::new(wire.from, wire.to, wire.rate).map_err(DeError::custom)
    }
}

// ---- DatedCashflow: { offset_years, amount } ------------------------------
// Gated with its type (std/libm), like the bare-number types above.

#[cfg(any(feature = "std", feature = "libm"))]
impl Serialize for DatedCashflow {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        DatedCashflowWire {
            offset_years: self.offset_years(),
            amount: self.amount(),
        }
        .serialize(serializer)
    }
}

#[cfg(any(feature = "std", feature = "libm"))]
impl<'de> Deserialize<'de> for DatedCashflow {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = DatedCashflowWire::deserialize(deserializer)?;
        DatedCashflow::new(wire.offset_years, wire.amount).map_err(DeError::custom)
    }
}

// ---- Installment: a plain record (no invariant beyond its Moneys) ----------

impl Serialize for Installment {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        InstallmentWire {
            period: self.period,
            payment: self.payment,
            interest: self.interest,
            principal: self.principal,
            balance: self.balance,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Installment {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = InstallmentWire::deserialize(deserializer)?;
        Ok(Installment {
            period: wire.period,
            payment: wire.payment,
            interest: wire.interest,
            principal: wire.principal,
            balance: wire.balance,
        })
    }
}
