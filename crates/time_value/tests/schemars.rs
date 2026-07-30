//! The `schemars` `JsonSchema` shapes (ADR-0044) — the JSON-Schema companion to the
//! `serde` wire format (ADR-0042). Asserts each type's generated schema matches
//! the shape its `serde` impl serializes to.
//!
//! Gated on `schemars` **and** `std`/`libm` (the `Period` / `ContinuousRate` /
//! `DatedCashflow` types live behind the transcendental-math feature). The
//! published `schemars` support is `no_std`; that build is covered by CI's
//! `--no-default-features --features schemars` clippy check.
#![cfg(all(feature = "schemars", any(feature = "std", feature = "libm")))]

use schemars::schema_for;
use serde_json::Value;
use time_value::{
    Annual, ContinuousRate, Currency, DatedCashflow, FxRate, Money, Monthly, OwnedCashflows,
    Period, Rate,
};

/// The generated schema for `T`, as a `serde_json::Value`.
macro_rules! schema {
    ($ty:ty) => {
        serde_json::to_value(schema_for!($ty)).unwrap()
    };
}

#[test]
fn bare_number_types_are_plain_numbers() {
    for schema in [
        schema!(Rate<Monthly>),
        schema!(Period<Monthly>),
        schema!(ContinuousRate),
    ] {
        assert_eq!(schema["type"], Value::from("number"), "schema: {schema}");
    }
}

#[test]
fn currency_is_a_string_enum_of_iso_codes() {
    let schema = schema!(Currency);
    assert_eq!(schema["type"], Value::from("string"));
    let codes = schema["enum"].as_array().expect("an enum array");
    assert!(codes.contains(&Value::from("USD")));
    assert!(codes.contains(&Value::from("XXX")));
    // The whole closed set, from `Currency::ALL`.
    assert_eq!(codes.len(), Currency::ALL.len());
}

#[test]
fn money_is_an_object_with_amount_and_currency() {
    let schema = schema!(Money);
    assert_eq!(schema["type"], Value::from("object"));
    let props = &schema["properties"];
    assert_eq!(props["amount"]["type"], Value::from("number"));
    // The currency sub-schema is the inlined code enum.
    assert_eq!(props["currency"]["type"], Value::from("string"));
    assert!(props["currency"]["enum"].is_array());
    let required = schema["required"].as_array().expect("required array");
    assert!(required.contains(&Value::from("amount")));
    assert!(required.contains(&Value::from("currency")));
}

/// The owned series is an `array` whose items are the `Money` schema, and — because
/// the periodicity tag is not on the wire (ADR-0060) — the schema is the *same*
/// document whatever `P` is. The `schemars` feature implies `alloc`, so
/// `OwnedCashflows` always exists in this build.
#[test]
fn owned_cashflows_is_an_array_of_money() {
    let schema = schema!(OwnedCashflows<Monthly>);
    assert_eq!(schema["type"], Value::from("array"));

    // `Money` stays the one named definition; the items point at it.
    let items = &schema["items"];
    let money = items["$ref"]
        .as_str()
        .and_then(|reference| reference.rsplit('/').next())
        .and_then(|name| schema["$defs"].get(name))
        .unwrap_or(items);
    assert_eq!(money["type"], Value::from("object"), "items: {items}");
    assert!(money["properties"].get("amount").is_some());
    assert!(money["properties"].get("currency").is_some());

    // Nothing in the schema distinguishes the periodicity.
    assert_eq!(schema, schema!(OwnedCashflows<Annual>));
}

/// The schema describes what `serde` actually writes: a serialized series is a JSON
/// array whose every element carries exactly the keys the `Money` definition
/// requires. Gated on `serde` as well, since it exercises both halves.
#[cfg(feature = "serde")]
#[test]
fn a_serialized_series_conforms_to_its_schema() {
    let series = OwnedCashflows::<Monthly>::new(vec![
        Money::new(-100.0, Currency::Usd).unwrap(),
        Money::agnostic(60.0).unwrap(),
    ]);
    let document = serde_json::to_value(&series).unwrap();
    let elements = document.as_array().expect("an array, per the schema");
    assert_eq!(elements.len(), series.len());

    let money_schema = schema!(Money);
    let required = money_schema["required"]
        .as_array()
        .expect("required array")
        .iter()
        .map(|key| key.as_str().expect("a string key"))
        .collect::<Vec<_>>();
    for element in elements {
        let object = element.as_object().expect("an object element");
        for key in &required {
            assert!(
                object.contains_key(*key),
                "element missing `{key}`: {element}"
            );
        }
        assert_eq!(object.len(), required.len(), "extra keys in {element}");
    }
}

#[test]
fn composite_structs_are_objects_with_their_fields() {
    let fx = schema!(FxRate);
    assert_eq!(fx["type"], Value::from("object"));
    for field in ["from", "to", "rate"] {
        assert!(fx["properties"].get(field).is_some(), "FxRate.{field}");
    }

    let dated = schema!(DatedCashflow);
    assert_eq!(dated["type"], Value::from("object"));
    assert!(dated["properties"].get("offset_years").is_some());
    assert!(dated["properties"].get("amount").is_some());
}
