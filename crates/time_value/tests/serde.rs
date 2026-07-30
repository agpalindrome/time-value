//! The `serde` wire format (ADR-0042): round-trips and validation-on-deserialize.
//!
//! Gated on `serde` **and** `std`/`libm`, because the value types beyond
//! `Rate`/`Money`/`Currency`/`FxRate`/`Installment` (namely `Period`,
//! `ContinuousRate`, `DatedCashflow`) live behind the transcendental-math
//! feature. The published `serde` support itself is `no_std`; that build is
//! covered by CI's `--no-default-features --features serde` clippy check, not
//! here.
#![cfg(all(feature = "serde", any(feature = "std", feature = "libm")))]

use serde_json::{from_str, json, to_value};
use time_value::{
    amortization::Schedule, Annual, ContinuousRate, Currency, DatedCashflow, FxRate, Money,
    Monthly, Payment, Period, Principal, Rate,
};

// ---- Bare numbers: serialize as a plain `f64`, no phantom tag on the wire ---

#[test]
fn rate_is_a_bare_number() {
    let rate = Rate::<Monthly>::new(0.01).unwrap();
    assert_eq!(to_value(rate).unwrap(), json!(0.01));
    assert_eq!(from_str::<Rate<Monthly>>("0.01").unwrap(), rate);
}

#[test]
fn period_is_a_bare_number() {
    let period = Period::<Monthly>::new(12.0).unwrap();
    assert_eq!(to_value(period).unwrap(), json!(12.0));
    assert_eq!(from_str::<Period<Monthly>>("12.0").unwrap(), period);
}

#[test]
fn continuous_rate_is_a_bare_number() {
    let force = ContinuousRate::new(0.05).unwrap();
    assert_eq!(to_value(force).unwrap(), json!(0.05));
    assert_eq!(from_str::<ContinuousRate>("0.05").unwrap(), force);
}

// ---- Money & Currency ------------------------------------------------------

#[test]
fn money_carries_its_currency() {
    let money = Money::new(100.0, Currency::Usd).unwrap();
    assert_eq!(
        to_value(money).unwrap(),
        json!({"amount": 100.0, "currency": "USD"})
    );
    assert_eq!(
        from_str::<Money>(r#"{"amount":100.0,"currency":"USD"}"#).unwrap(),
        money
    );
}

#[test]
fn agnostic_money_is_denominated_xxx_not_omitted() {
    // Unlike the binaries' presentation shape, the core wire format always carries
    // the currency, so it round-trips losslessly — `XXX` is spelled out.
    let money = Money::agnostic(5.0).unwrap();
    assert_eq!(
        to_value(money).unwrap(),
        json!({"amount": 5.0, "currency": "XXX"})
    );
    assert_eq!(
        from_str::<Money>(r#"{"amount":5.0,"currency":"XXX"}"#).unwrap(),
        money
    );
}

#[test]
fn currency_is_its_iso_code() {
    assert_eq!(to_value(Currency::Jpy).unwrap(), json!("JPY"));
    assert_eq!(from_str::<Currency>(r#""EUR""#).unwrap(), Currency::Eur);
}

// ---- Composite value structs ----------------------------------------------

#[test]
fn fx_rate_round_trips() {
    let fx = FxRate::new(Currency::Usd, Currency::Eur, 0.9).unwrap();
    assert_eq!(
        to_value(fx).unwrap(),
        json!({"from": "USD", "to": "EUR", "rate": 0.9})
    );
    assert_eq!(
        from_str::<FxRate>(r#"{"from":"USD","to":"EUR","rate":0.9}"#).unwrap(),
        fx
    );
}

#[test]
fn dated_cashflow_round_trips() {
    let flow = DatedCashflow::new(1.5, Money::new(100.0, Currency::Usd).unwrap()).unwrap();
    assert_eq!(
        to_value(flow).unwrap(),
        json!({"offset_years": 1.5, "amount": {"amount": 100.0, "currency": "USD"}})
    );
    let back: DatedCashflow =
        from_str(r#"{"offset_years":1.5,"amount":{"amount":100.0,"currency":"USD"}}"#).unwrap();
    assert_eq!(back, flow);
}

#[test]
fn installment_round_trips() {
    // First row of a level-payment amortization of 1000 at 10%/period paying 500.
    let installment = Schedule::<Monthly>::with_payment(
        Rate::new(0.10).unwrap(),
        Payment(Money::new(500.0, Currency::Usd).unwrap()),
        Principal(Money::new(1000.0, Currency::Usd).unwrap()),
    )
    .unwrap()
    .next()
    .unwrap();

    let json = to_value(installment).unwrap();
    assert_eq!(json["period"], json!(1));
    assert_eq!(json["balance"]["currency"], json!("USD"));

    let back: time_value::Installment = serde_json::from_value(json).unwrap();
    assert_eq!(back, installment);
}

// ---- OwnedCashflows: a bare array of Money --------------------------------
//
// Gated on `alloc` (the owned series' own gate) — `std`, which this file already
// requires, implies it, but the gate says so rather than relying on that. The
// imports sit inside each test so no other feature configuration sees them.

/// The series is an array of `Money` in period order, and nothing else — no
/// wrapper object, and no periodicity: the tag is a compile-time marker, so it is
/// absent from the wire (ADR-0060).
#[cfg(feature = "alloc")]
#[test]
fn owned_cashflows_is_a_bare_array_of_money() {
    use time_value::OwnedCashflows;

    let series = OwnedCashflows::<Monthly>::new(vec![
        Money::new(-100.0, Currency::Usd).unwrap(),
        Money::new(60.0, Currency::Usd).unwrap(),
    ]);
    assert_eq!(
        to_value(&series).unwrap(),
        json!([
            {"amount": -100.0, "currency": "USD"},
            {"amount": 60.0, "currency": "USD"},
        ])
    );
    let back: OwnedCashflows<Monthly> =
        from_str(r#"[{"amount":-100.0,"currency":"USD"},{"amount":60.0,"currency":"USD"}]"#)
            .unwrap();
    assert_eq!(back, series);
}

/// The periodicity is *not* on the wire, so one serialized series deserializes
/// into whichever `P` the caller asks for — the deliberate trade ADR-0060 records:
/// a mismatch is silent, exactly as it is for the bare-number `Rate<P>`.
#[cfg(feature = "alloc")]
#[test]
fn the_periodicity_is_not_recorded_on_the_wire() {
    use time_value::{Annual, OwnedCashflows};

    let monthly = OwnedCashflows::<Monthly>::new(vec![Money::agnostic(1.0).unwrap()]);
    let json = to_value(&monthly).unwrap();
    // The same document reads back as an annual series, with no error to flag it.
    let annual: OwnedCashflows<Annual> = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(annual.as_slice(), monthly.as_slice());
    assert_eq!(to_value(&annual).unwrap(), json);
}

/// An empty series is an empty array, both ways round.
#[cfg(feature = "alloc")]
#[test]
fn an_empty_series_round_trips_as_an_empty_array() {
    use time_value::OwnedCashflows;

    let empty = OwnedCashflows::<Monthly>::new(Vec::new());
    assert_eq!(to_value(&empty).unwrap(), json!([]));
    let back: OwnedCashflows<Monthly> = from_str("[]").unwrap();
    assert_eq!(back, empty);
    assert!(back.is_empty());
}

/// A series is only as valid as its elements: each one is rebuilt through
/// [`Money`]'s validating impl, so a single bad element fails the whole document
/// rather than materialising an invalid series.
#[cfg(feature = "alloc")]
#[test]
fn one_invalid_element_rejects_the_whole_series() {
    use time_value::OwnedCashflows;

    // A non-finite amount — `1e999` overflows `f64` — must not become a `Money`.
    assert!(from_str::<Money>(r#"{"amount":1e999,"currency":"USD"}"#).is_err());
    assert!(from_str::<OwnedCashflows<Monthly>>(
        r#"[{"amount":1.0,"currency":"USD"},{"amount":1e999,"currency":"USD"}]"#
    )
    .is_err());
    // ...and the same for an unknown currency code mid-series.
    assert!(from_str::<OwnedCashflows<Monthly>>(
        r#"[{"amount":1.0,"currency":"USD"},{"amount":2.0,"currency":"ZZZ"}]"#
    )
    .is_err());
}

/// A deserialized series is a *working* series, not just a container: the
/// operations that make it useful run on the value that came off the wire.
#[cfg(feature = "alloc")]
#[test]
fn a_deserialized_series_computes() {
    use time_value::OwnedCashflows;

    let series: OwnedCashflows<Monthly> = from_str(
        r#"[{"amount":-100.0,"currency":"XXX"},
            {"amount":60.0,"currency":"XXX"},
            {"amount":60.0,"currency":"XXX"}]"#,
    )
    .unwrap();
    let npv = series
        .net_present_value(Rate::<Monthly>::new(0.01).unwrap())
        .unwrap();
    assert!((npv.value() - 18.2237).abs() < 1e-3);
}

/// Mixed currencies are representable in process (`OwnedCashflows::new` accepts
/// any `Vec<Money>`; the *operation* reports the mismatch), so the wire accepts
/// them too — deserialization is exactly as permissive as the constructor, no more
/// and no less (ADR-0060).
#[cfg(feature = "alloc")]
#[test]
fn a_mixed_currency_series_deserializes_and_fails_at_the_operation() {
    use time_value::OwnedCashflows;

    let series: OwnedCashflows<Monthly> =
        from_str(r#"[{"amount":-100.0,"currency":"USD"},{"amount":60.0,"currency":"EUR"}]"#)
            .unwrap();
    assert!(series.currency().is_err());
    assert!(series
        .net_present_value(Rate::<Monthly>::new(0.01).unwrap())
        .is_err());
}

// ---- Deserialization validates: an out-of-domain value is an error ---------

#[test]
fn a_rate_at_or_below_minus_one_is_rejected() {
    // A rate must be finite and greater than −100%.
    assert!(from_str::<Rate<Monthly>>("-5").is_err());
    assert!(from_str::<Rate<Monthly>>("-1").is_err());
}

#[test]
fn a_negative_period_is_rejected() {
    assert!(from_str::<Period<Monthly>>("-1").is_err());
}

#[test]
fn an_unknown_currency_code_is_rejected() {
    assert!(from_str::<Currency>(r#""ZZZ""#).is_err());
    // ...and it propagates through a nested Money.
    assert!(from_str::<Money>(r#"{"amount":1.0,"currency":"ZZZ"}"#).is_err());
}

#[test]
fn a_non_positive_exchange_rate_is_rejected() {
    assert!(from_str::<FxRate>(r#"{"from":"USD","to":"EUR","rate":0}"#).is_err());
    assert!(from_str::<FxRate>(r#"{"from":"USD","to":"EUR","rate":-0.5}"#).is_err());
}

#[test]
fn an_effective_annual_bridge_round_trips_through_serde() {
    // A ContinuousRate deserialized from the wire is a usable, validated value.
    let force: ContinuousRate = from_str("0.05").unwrap();
    let effective = force.effective_annual().unwrap();
    let back =
        ContinuousRate::from_effective_annual(Rate::<Annual>::new(effective.value()).unwrap());
    assert!((back.value() - force.value()).abs() < 1e-12);
}
