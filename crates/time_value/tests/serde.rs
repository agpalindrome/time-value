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
