//! `schemars` (`JsonSchema`) support for the public value types (ADR-0044),
//! behind the off-by-default `schemars` feature — the JSON-Schema companion to the
//! `serde` wire format (ADR-0042).
//!
//! The schemas **describe the same shapes** the `serde` impls produce, and the
//! composites reuse the very same private `*Wire` structs (`crate::wire`) that
//! back serde, so the two cannot drift:
//!
//! - **Bare numbers.** [`Rate`], [`Period`], [`ContinuousRate`] → `{ "type":
//!   "number" }`, inlined (the `PhantomData` tag is not on the wire).
//! - **[`Currency`]** → a `string` with the ISO 4217 code `enum` from
//!   [`Currency::ALL`], inlined — the schema this crate's consumers (the MCP
//!   server) advertise for a currency field.
//! - **Composites** ([`Money`], [`FxRate`], [`DatedCashflow`], [`Installment`])
//!   delegate to their `*Wire` struct's derived schema.
//! - **[`OwnedCashflows`]** → an inlined `array` of [`Money`] (ADR-0060). The
//!   `schemars` feature implies `alloc`, so the owned series always exists here.
//! - **`OwnedDatedCashflows`** → an inlined `array` of [`DatedCashflow`] (ADR-0065),
//!   behind std/libm with the dated types.

use alloc::borrow::Cow;
use alloc::vec::Vec;

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};

#[cfg(any(feature = "std", feature = "libm"))]
use crate::wire::DatedCashflowWire;
#[cfg(any(feature = "std", feature = "libm"))]
use crate::wire::OwnedDatedCashflowsWire;
use crate::wire::{FxRateWire, InstallmentWire, MoneyWire, OwnedCashflowsWire};
use crate::{
    amortization::Installment, Currency, FxRate, Money, OwnedCashflows, Periodicity, Rate,
};
#[cfg(any(feature = "std", feature = "libm"))]
use crate::{ContinuousRate, DatedCashflow, OwnedDatedCashflows, Period};

// ---- Bare-number newtypes: a plain `number`, inlined ----------------------

macro_rules! bare_number_schema {
    ($ty:ty, $name:literal $(, $param:ident)?) => {
        impl$(<$param: Periodicity>)? JsonSchema for $ty {
            /// Trivial, so inline it rather than emit a `$ref`/`$def`.
            fn inline_schema() -> bool {
                true
            }

            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed($name)
            }

            fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
                json_schema!({ "type": "number" })
            }
        }
    };
}

bare_number_schema!(Rate<P>, "Rate", P);
#[cfg(any(feature = "std", feature = "libm"))]
bare_number_schema!(Period<P>, "Period", P);
#[cfg(any(feature = "std", feature = "libm"))]
bare_number_schema!(ContinuousRate, "ContinuousRate");

// ---- Currency: a string with the ISO 4217 code enum, inlined --------------

impl JsonSchema for Currency {
    /// Inlined, so a `currency` field carries the code `enum` directly (matching
    /// the string schema the MCP server used to hand-write on its `CurrencyCode`).
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Currency")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        // The closed code set, generated from the core table — no second list to
        // keep in sync (mirrors the `serde` `Deserialize`, which resolves via
        // `Currency::from_code`).
        let codes: Vec<&str> = Currency::ALL.iter().map(|c| c.code()).collect();
        json_schema!({
            "type": "string",
            "enum": codes,
            "description": "An ISO 4217 currency code (e.g. `USD`); `XXX` is the \
                            currency-agnostic identity.",
        })
    }
}

// ---- Composites: delegate to the shared `*Wire` shape ---------------------

macro_rules! delegate_schema {
    ($ty:ty, $wire:ty, $name:literal) => {
        impl JsonSchema for $ty {
            fn schema_name() -> Cow<'static, str> {
                Cow::Borrowed($name)
            }

            fn json_schema(generator: &mut SchemaGenerator) -> Schema {
                <$wire as JsonSchema>::json_schema(generator)
            }
        }
    };
}

// ---- OwnedCashflows: an array of Money, inlined ---------------------------

impl<P: Periodicity> JsonSchema for OwnedCashflows<P> {
    /// Inlined, like `schemars`' own sequence schemas — and for the same reason the
    /// bare-number newtypes are: the periodicity tag is not on the wire (ADR-0060),
    /// so a `$ref` to a definition named `OwnedCashflows` would imply a
    /// per-periodicity schema that does not exist. `Money` remains the named
    /// definition the `items` point at.
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("OwnedCashflows")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        <OwnedCashflowsWire<'_> as JsonSchema>::json_schema(generator)
    }
}

// ---- OwnedDatedCashflows: an array of DatedCashflow, inlined ---------------

#[cfg(any(feature = "std", feature = "libm"))]
impl JsonSchema for OwnedDatedCashflows {
    /// Inlined, like `schemars`' own sequence schemas and like the periodic
    /// [`OwnedCashflows`] above. `DatedCashflow` remains the named definition the
    /// `items` point at (ADR-0065).
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("OwnedDatedCashflows")
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        <OwnedDatedCashflowsWire<'_> as JsonSchema>::json_schema(generator)
    }
}

delegate_schema!(Money, MoneyWire, "Money");
delegate_schema!(FxRate, FxRateWire, "FxRate");
#[cfg(any(feature = "std", feature = "libm"))]
delegate_schema!(DatedCashflow, DatedCashflowWire, "DatedCashflow");
delegate_schema!(Installment, InstallmentWire, "Installment");
