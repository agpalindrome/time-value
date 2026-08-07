//! The public contract, exercised from outside the crate.
//!
//! These see only what a caller sees. A unit test cannot detect a breaking
//! change to the public surface, because it can reach past it.

use time_value::{
    Amount, ElapsedPeriods, Error, SimpleAccumulationFactor, SimpleInterestRate, future_value,
};

/// Values chosen to be awkward: the boundaries, the classic binary-float
/// embarrassments, and both ends of the representable range.
fn awkward_magnitudes() -> Vec<f64> {
    vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.1,
        0.1 + 0.2,
        1.0 / 3.0,
        1e-300,
        f64::MIN_POSITIVE,
        1e20,
        f64::MAX,
        -f64::MAX,
        123_456_789.987_654_32,
    ]
}

#[test]
fn rendering_round_trips_through_parsing() {
    // The law the exact-rendering decision buys: parse(render(a)) == a, for
    // every Amount. Stating it needs no reimplementation of anything, which is
    // what makes it worth more than a table of expected outputs.
    for magnitude in awkward_magnitudes() {
        let original = Amount::new(magnitude).expect("a finite magnitude is an Amount");
        let rendered = original.to_string();
        let parsed: Amount = rendered
            .parse()
            .unwrap_or_else(|_| panic!("`{rendered}` should read back as an Amount"));
        assert_eq!(parsed, original, "round trip lost `{rendered}`");
    }
}

#[test]
fn round_tripping_does_not_witness_the_sign_of_zero() {
    // An honest limit of the law above. Negative zero and zero are equal by
    // decision — they are the same sum of money — so the round trip cannot
    // distinguish them, and passing it is not evidence the sign survived.
    let negative = Amount::new(-0.0).expect("valid");
    let parsed: Amount = negative.to_string().parse().expect("valid");
    assert_eq!(parsed, negative);
    assert_eq!(negative, Amount::new(0.0).expect("valid"));
}

#[test]
fn the_two_failures_are_told_apart_through_the_public_api() {
    let rate = SimpleInterestRate::from_fraction(-0.5).expect("a finite rate");
    let periods = ElapsedPeriods::new(3.0).expect("a non-negative span");

    // Each argument valid, the pair meaningless: a domain failure.
    let domain = SimpleAccumulationFactor::new(rate, periods).expect_err("1 + rt is -0.5");
    assert!(
        matches!(domain, Error::NonPositiveFactor { .. }),
        "{domain}"
    );

    // A valid factor whose application does not fit: a representation failure.
    let doubling = SimpleAccumulationFactor::new(
        SimpleInterestRate::from_fraction(1.0).expect("valid"),
        ElapsedPeriods::new(1.0).expect("valid"),
    )
    .expect("2.0 is a valid factor");
    let representation = doubling
        .apply(Amount::new(f64::MAX).expect("valid"))
        .expect_err("2 * MAX does not fit");
    assert!(
        matches!(representation, Error::NotFinite),
        "{representation}"
    );
}

#[test]
fn a_worked_example_from_the_source() {
    // The source's own worked example is a simple-interest one: 100 at 5% for
    // three years gives 115.
    let future = future_value(
        Amount::new(100.0).expect("valid"),
        SimpleInterestRate::from_percent(5.0).expect("valid"),
        ElapsedPeriods::new(3.0).expect("valid"),
    )
    .expect("valid");

    let expected = Amount::new(115.0).expect("valid");
    assert!(future.is_close(expected, 1e-9, 1e-9), "got {future}");
}

#[test]
fn errors_render_as_sentence_fragments() {
    // They are joined into a chain by whatever reports them, so each is a
    // fragment: lowercase, no trailing stop.
    let error = SimpleAccumulationFactor::new(
        SimpleInterestRate::from_fraction(-0.5).expect("valid"),
        ElapsedPeriods::new(3.0).expect("valid"),
    )
    .expect_err("invalid");
    let rendered = error.to_string();

    assert!(!rendered.ends_with('.'), "`{rendered}` ends with a stop");
    assert!(
        rendered.starts_with(|c: char| c.is_lowercase()),
        "`{rendered}` does not start lowercase"
    );
}
