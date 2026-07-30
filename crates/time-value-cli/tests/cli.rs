//! Integration tests: drive the compiled `time-value` binary and assert on its
//! stdout / stderr / exit status (ADR-0010, ADR-0011 testing strategy).

use assert_cmd::Command;
use predicates::prelude::*;

fn time_value() -> Command {
    Command::cargo_bin("time-value").unwrap()
}

#[test]
fn npv_of_a_simple_series() {
    // -100 now, +60, +60 at 1% per period -> ~18.22.
    time_value()
        .args(["series", "npv", "--rate", "0.01", "-100", "60", "60"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("18.22"));
}

#[test]
fn nfv_of_a_simple_series() {
    time_value()
        .args(["series", "nfv", "--rate", "0.01", "-100", "60", "60"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("18.5"));
}

#[test]
fn irr_of_a_simple_series() {
    time_value()
        .args(["series", "irr", "-100", "60", "60"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.130"));
}

#[test]
fn mirr_of_a_simple_series() {
    // Outflows -1000, -500; inflows 800, 900 at finance 10% / reinvest 12%.
    time_value()
        .args([
            "series",
            "mirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.12",
            "-1000",
            "-500",
            "800",
            "900",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.072"));
}

#[test]
fn xnpv_of_dated_flows() {
    // -100 now, +110 exactly one year later at 10%/yr -> ~0.
    time_value()
        .args([
            "series",
            "xnpv",
            "--rate",
            "0.10",
            "2020-01-01:-100",
            "2021-01-01:110",
        ])
        .assert()
        .success()
        // 2020 is a leap year (366 days), so the offset is 366/365 -> XNPV slightly
        // above zero, but small.
        .stdout(predicate::str::starts_with("0.0").or(predicate::str::starts_with("-0.0")));
}

#[test]
fn xirr_of_the_excel_reference() {
    // Microsoft's XIRR example -> ~0.3734.
    time_value()
        .args([
            "series",
            "xirr",
            "2008-01-01:-10000",
            "2008-03-01:2750",
            "2008-10-30:4250",
            "2009-02-15:3250",
            "2009-04-01:2750",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.373"));
}

// ---- The dated counterparts: `series xnfv` / `series xmirr` (ADR-0065) ----

#[test]
fn xnfv_of_dated_flows() {
    // -100 on 2020-01-01, +110 on 2021-01-01, compounded at 10%/yr to the later
    // date. 2020 is a leap year, so the span is 366/365 and the outflow grows to a
    // shade over 110: the value at the horizon is −0.0287…
    time_value()
        .args([
            "series",
            "xnfv",
            "--rate",
            "0.10",
            "2020-01-01:-100",
            "2021-01-01:110",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("-0.028"));
}

#[test]
fn xnfv_compounds_to_the_latest_date_whatever_the_order() {
    // The horizon is the latest date, not the last argument (ADR-0065), so the same
    // flows given newest-first must print the same amount.
    let sorted = time_value()
        .args([
            "series",
            "xnfv",
            "--rate",
            "0.10",
            "2020-01-01:-1000",
            "2020-07-01:-500",
            "2021-04-01:800",
            "2022-01-01:900",
        ])
        .assert()
        .success();
    let reversed = time_value()
        .args([
            "series",
            "xnfv",
            "--rate",
            "0.10",
            "2022-01-01:900",
            "2021-04-01:800",
            "2020-07-01:-500",
            "2020-01-01:-1000",
        ])
        .assert()
        .success();
    assert_eq!(
        String::from_utf8_lossy(&sorted.get_output().stdout),
        String::from_utf8_lossy(&reversed.get_output().stdout),
    );
    // …and it is the value computed independently at 60 digits: −27.8267360312988…
    sorted.stdout(predicate::str::starts_with("-27.8267360"));
}

#[test]
fn xmirr_of_dated_flows() {
    // The same four flows at a 10% finance rate and 12% reinvestment rate, over the
    // 731/365-year span between the earliest and latest date: 0.09504815957…
    time_value()
        .args([
            "series",
            "xmirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.12",
            "2020-01-01:-1000",
            "2020-07-01:-500",
            "2021-04-01:800",
            "2022-01-01:900",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.0950481"));
}

#[test]
fn xmirr_reports_a_series_dated_on_one_day() {
    // The span is zero, so the growth factor is 1 whatever the rate. With the flows
    // matching, *every* rate satisfies them; with them mismatched, none does. The
    // arm interpolates the library's message rather than picking one statically.
    time_value()
        .args([
            "series",
            "xmirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.10",
            "2020-01-01:-1000",
            "2020-01-01:1000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "every rate satisfies these inputs",
        ));

    time_value()
        .args([
            "series",
            "xmirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.10",
            "2020-01-01:-1000",
            "2020-01-01:1500",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no real solution"));
}

#[test]
fn xmirr_without_outflows_says_so() {
    time_value()
        .args([
            "series",
            "xmirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.10",
            "2020-01-01:1000",
            "2021-01-01:500",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no outflows"));
}

#[test]
fn the_dated_counterparts_follow_the_currency_split() {
    // `xnfv` produces a `Money`, so it echoes the currency; `xmirr` produces a rate,
    // so it never does (ADR-0057).
    time_value()
        .args([
            "--currency",
            "USD",
            "series",
            "xnfv",
            "--rate",
            "0.10",
            "2020-01-01:-100",
            "2021-01-01:110",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("USD"));

    time_value()
        .args([
            "--currency",
            "USD",
            "series",
            "xmirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.12",
            "2020-01-01:-1000",
            "2022-01-01:1500",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("USD").not());
}

#[test]
fn the_dated_counterparts_emit_the_uniform_json_shape() {
    time_value()
        .args([
            "--json",
            "--currency",
            "EUR",
            "series",
            "xnfv",
            "--rate",
            "0.10",
            "2020-01-01:-100",
            "2021-01-01:110",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"value\"")
                .and(predicate::str::contains("\"currency\":\"EUR\"")),
        );

    time_value()
        .args([
            "--json",
            "series",
            "xmirr",
            "--finance",
            "0.10",
            "--reinvest",
            "0.12",
            "2020-01-01:-1000",
            "2022-01-01:1500",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"value\"").and(predicate::str::contains("currency").not()),
        );
}

#[test]
fn an_invalid_date_fails() {
    time_value()
        .args(["series", "xirr", "2020-02-30:-100", "2021-01-01:110"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("invalid date"));
}

#[test]
fn present_value_of_a_single_sum() {
    time_value()
        .args([
            "single-sum",
            "pv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--future",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("887.4"));
}

#[test]
fn future_value_of_a_single_sum() {
    time_value()
        .args([
            "single-sum",
            "fv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--present",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1126.8"));
}

#[test]
fn single_sum_nper_inverts_growth() {
    // 1000 grows to 1126.83 at 1%/period -> ~12 periods.
    time_value()
        .args([
            "single-sum",
            "nper",
            "--rate",
            "0.01",
            "--present",
            "1000",
            "--future",
            "1126.825",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("12.0").or(predicate::str::starts_with("11.9")));
}

#[test]
fn single_sum_rate_inverts_growth() {
    time_value()
        .args([
            "single-sum",
            "rate",
            "--periods",
            "12",
            "--present",
            "1000",
            "--future",
            "1126.825",
        ])
        .assert()
        .success()
        // The future is ~1000·1.01¹², so the solved rate is ~0.01 (printed as
        // 0.00999997…); accept either rounding face.
        .stdout(predicate::str::starts_with("0.0099").or(predicate::str::starts_with("0.01")));
}

#[test]
fn annuity_present_value() {
    time_value()
        .args([
            "annuity",
            "pv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1125.5"));
}

#[test]
fn annuity_payment_amortises_a_present_value() {
    time_value()
        .args([
            "annuity",
            "payment",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--present",
            "1125.508",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("99.99").or(predicate::str::starts_with("100")));
}

#[test]
fn annuity_nper_solves_from_present() {
    // A 100/period annuity priced at 1125.51 at 1% -> ~12 payments.
    time_value()
        .args([
            "annuity",
            "nper",
            "--rate",
            "0.01",
            "--payment",
            "100",
            "--present",
            "1125.508",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("12.0").or(predicate::str::starts_with("11.9")));
}

#[test]
fn annuity_rate_solves_from_present() {
    time_value()
        .args([
            "annuity",
            "rate",
            "--periods",
            "12",
            "--payment",
            "100",
            "--present",
            "1125.508",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.0099").or(predicate::str::starts_with("0.01")));
}

#[test]
fn annuity_nper_requires_a_basis() {
    time_value()
        .args(["annuity", "nper", "--rate", "0.01", "--payment", "100"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--present").or(predicate::str::contains("--future")));
}

#[test]
fn annuity_perpetuity_present_value() {
    // 100/period forever at 5% -> 2000.
    time_value()
        .args([
            "annuity",
            "perpetuity",
            "--rate",
            "0.05",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("2000"));
}

#[test]
fn annuity_growing_perpetuity_present_value() {
    // 100 growing 2%, discounted 5% -> 100 / (0.05 - 0.02) = 3333.33…
    time_value()
        .args([
            "annuity",
            "growing-perpetuity",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("3333"));
}

#[test]
fn annuity_growing_present_and_future_value() {
    // A first payment of 100 growing 2%/period for a year, discounted at 5%:
    // PV = 100·(1 − (1.02/1.05)¹²)/0.03 ≈ 979.32, and FV = PV·1.05¹² ≈ 1758.72.
    time_value()
        .args([
            "annuity",
            "growing",
            "pv",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("979.3"));

    time_value()
        .args([
            "annuity",
            "growing",
            "fv",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1758.7"));
}

#[test]
fn annuity_growing_prices_growth_above_the_rate() {
    // The deliberate difference from `growing-perpetuity`, which rejects this
    // pair as divergent: a *finite* growing annuity converges for any r and g
    // (ADR-0048), so this must succeed rather than error.
    time_value()
        .args([
            "annuity",
            "growing",
            "pv",
            "--rate",
            "0.02",
            "--growth",
            "0.05",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1386.7"));

    time_value()
        .args([
            "annuity",
            "growing-perpetuity",
            "--rate",
            "0.02",
            "--growth",
            "0.05",
            "--payment",
            "100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("diverges"));
}

#[test]
fn annuity_due_growing_is_the_ordinary_growing_scaled() {
    // Due = ordinary × (1 + r): 979.32 × 1.05 ≈ 1028.28, 1758.72 × 1.05 ≈ 1846.65.
    time_value()
        .args([
            "annuity",
            "growing",
            "due-pv",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1028.2"));

    time_value()
        .args([
            "annuity",
            "growing",
            "due-fv",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1846.6"));
}

#[test]
fn annuity_due_present_value_exceeds_ordinary() {
    // Annuity-due PV = ordinary PV * (1 + r); at 1% -> 1125.51 * 1.01 ≈ 1136.76.
    time_value()
        .args([
            "annuity",
            "due",
            "pv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1136.7"));
}

#[test]
fn annuity_payment_from_a_future_value_is_the_sinking_fund() {
    // 12 contributions at 1%/month reaching 1268.25 -> ~100 each (ADR-0062).
    time_value()
        .args([
            "annuity",
            "payment",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--future",
            "1268.250",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("99.99").or(predicate::str::starts_with("100")));
}

#[test]
fn annuity_due_payment_from_a_future_value_is_the_sinking_fund() {
    // Start-of-month contributions earn one period more, so reaching the larger
    // 1280.93 takes the same ~100 (the ordinary target × 1.01).
    time_value()
        .args([
            "annuity",
            "due",
            "payment",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--future",
            "1280.933",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("99.99").or(predicate::str::starts_with("100")));
}

#[test]
fn annuity_payment_requires_a_basis() {
    // The anchored `--present`/`--future` pair, exactly as `nper` and `rate` use it.
    time_value()
        .args(["annuity", "payment", "--rate", "0.01", "--periods", "12"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--present").or(predicate::str::contains("--future")));
}

#[test]
fn annuity_payment_rejects_both_bases() {
    time_value()
        .args([
            "annuity",
            "payment",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--present",
            "1000",
            "--future",
            "1268.25",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[test]
fn annuity_payment_over_zero_periods_is_rejected_from_either_basis() {
    // The factor is 0 whichever end the payment is solved from (ADR-0056).
    for basis in ["--present", "--future"] {
        time_value()
            .args([
                "annuity",
                "payment",
                "--rate",
                "0.01",
                "--periods",
                "0",
                basis,
                "1000",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("annuity payment"));
    }
}

#[test]
fn annuity_due_perpetuity_exceeds_the_ordinary_perpetuity() {
    // Perpetuity-due = ordinary × (1 + r): 100/0.05 × 1.05 = 2100.
    time_value()
        .args([
            "annuity",
            "due",
            "perpetuity",
            "--rate",
            "0.05",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("2100"));
}

#[test]
fn annuity_growing_due_perpetuity_present_value() {
    // 100/(0.05 − 0.02) × 1.05 = 3500.
    time_value()
        .args([
            "annuity",
            "growing",
            "due-perpetuity",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--payment",
            "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("3500"));
}

#[test]
fn the_perpetuity_due_forms_reject_divergence() {
    // Bringing every payment forward one period rescales a convergent sum; it
    // cannot rescue a divergent one, so the due forms reject what the ordinary ones
    // reject (ADR-0062).
    time_value()
        .args([
            "annuity",
            "due",
            "perpetuity",
            "--rate",
            "0",
            "--payment",
            "100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("diverges"));
    time_value()
        .args([
            "annuity",
            "growing",
            "due-perpetuity",
            "--rate",
            "0.02",
            "--growth",
            "0.05",
            "--payment",
            "100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("diverges"));
}

// ---- The annuity-due solves (ADR-0063) ----

#[test]
fn annuity_due_nper_solves_from_either_basis() {
    // The due present value 1136.763 and future value 1280.933 both come from 12
    // start-of-month payments of 100 at 1%/month, so both anchors return the term.
    for (basis, value) in [("--present", "1136.763"), ("--future", "1280.933")] {
        time_value()
            .args([
                "annuity",
                "due",
                "nper",
                "--rate",
                "0.01",
                "--payment",
                "100",
                basis,
                value,
            ])
            .assert()
            .success()
            .stdout(predicate::str::starts_with("12.0"));
    }
}

#[test]
fn annuity_due_rate_solves_from_either_basis() {
    for (basis, value) in [("--present", "1136.763"), ("--future", "1280.933")] {
        time_value()
            .args([
                "annuity",
                "due",
                "rate",
                "--periods",
                "12",
                "--payment",
                "100",
                basis,
                value,
            ])
            .assert()
            .success()
            .stdout(predicate::str::starts_with("0.00999").or(predicate::str::starts_with("0.01")));
    }
}

#[test]
fn annuity_due_solves_take_the_same_anchor_as_the_ordinary_ones() {
    for command in ["nper", "rate"] {
        let first = if command == "nper" {
            "--rate"
        } else {
            "--periods"
        };
        let first_value = if command == "nper" { "0.01" } else { "12" };
        time_value()
            .args([
                "annuity",
                "due",
                command,
                first,
                first_value,
                "--payment",
                "100",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("--present").or(predicate::str::contains("--future")));
        time_value()
            .args([
                "annuity",
                "due",
                command,
                first,
                first_value,
                "--payment",
                "100",
                "--present",
                "1136.763",
                "--future",
                "1280.933",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("mutually exclusive"));
    }
}

#[test]
fn annuity_due_rate_over_one_period_is_indeterminate_from_a_present_value() {
    // A single start-of-period payment is not discounted, so the due present-value
    // factor is 1 at every rate: every rate satisfies the equation and none is *the*
    // answer. The message interpolates the library error rather than claiming no rate
    // solves the inputs, which would be the opposite of the truth (ADR-0056/0063).
    time_value()
        .args([
            "annuity",
            "due",
            "rate",
            "--periods",
            "1",
            "--payment",
            "100",
            "--present",
            "100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("every rate satisfies"));

    // A single start-of-period *contribution*, by contrast, is a determined solve:
    // its factor is `1 + r`, so 100 growing to 125 in a period is 25%.
    time_value()
        .args([
            "annuity",
            "due",
            "rate",
            "--periods",
            "1",
            "--payment",
            "100",
            "--future",
            "125",
        ])
        .assert()
        .success()
        // The solver's root tolerance over the factor's derivative pins this to about
        // 2.3e-9, so it prints as 0.2499999976 rather than 0.25 exactly.
        .stdout(predicate::str::starts_with("0.2499").or(predicate::str::starts_with("0.25")));
}

#[test]
fn annuity_due_rate_over_zero_periods_is_rejected_from_either_basis() {
    for basis in ["--present", "--future"] {
        time_value()
            .args([
                "annuity",
                "due",
                "rate",
                "--periods",
                "0",
                "--payment",
                "100",
                basis,
                "1000",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("at least one period"));
    }
}

// ---- The growing-annuity inverses (ADR-0063) ----

#[test]
fn annuity_growing_payment_amortises_a_present_value() {
    // The growing PV of 12 payments from 100 escalating 2%/month at 5%/month is
    // 979.318, so amortising 979.318 returns the 100 it came from.
    time_value()
        .args([
            "annuity",
            "growing",
            "payment",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--present",
            "979.318",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("99.99").or(predicate::str::starts_with("100")));
}

#[test]
fn annuity_growing_nper_and_rate_invert_the_growing_present_value() {
    time_value()
        .args([
            "annuity",
            "growing",
            "nper",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--payment",
            "100",
            "--present",
            "979.318",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("11.99").or(predicate::str::starts_with("12")));

    time_value()
        .args([
            "annuity",
            "growing",
            "rate",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--payment",
            "100",
            "--present",
            "979.318",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.05"));
}

#[test]
fn annuity_growing_nper_rejects_a_target_above_the_perpetuity_ceiling() {
    // With the rate above the growth the present value is capped at
    // 100/(0.05 − 0.02) = 3333.33, so 4000 is reached by no finite term.
    time_value()
        .args([
            "annuity",
            "growing",
            "nper",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--payment",
            "100",
            "--present",
            "4000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("number of periods"));

    // Growth *above* the rate has no ceiling, so an arbitrarily large target solves.
    time_value()
        .args([
            "annuity",
            "growing",
            "nper",
            "--rate",
            "0.02",
            "--growth",
            "0.05",
            "--payment",
            "100",
            "--present",
            "4000",
        ])
        .assert()
        .success();
}

#[test]
fn annuity_growing_payment_over_zero_periods_is_rejected() {
    time_value()
        .args([
            "annuity",
            "growing",
            "payment",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "0",
            "--present",
            "979.318",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("growing annuity payment"));
}

#[test]
fn annuity_growing_payment_echoes_its_currency_as_json() {
    time_value()
        .args([
            "--json",
            "--currency",
            "USD",
            "annuity",
            "growing",
            "payment",
            "--rate",
            "0.05",
            "--growth",
            "0.02",
            "--periods",
            "12",
            "--present",
            "979.318",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"currency\":\"USD\""));
}

#[test]
fn rate_effective_annual_of_a_monthly_rate() {
    // (1.01)^12 - 1 = 0.126825…
    time_value()
        .args(["rate", "ear", "--rate", "0.01", "--periodicity", "monthly"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.1268"));
}

#[test]
fn rate_convert_between_periodicities() {
    // 1%/month -> quarterly at the same EAR = 0.030301…
    time_value()
        .args([
            "rate",
            "convert",
            "--rate",
            "0.01",
            "--from",
            "monthly",
            "--to",
            "quarterly",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.0303"));
}

#[test]
fn rate_nominal_and_from_nominal_are_inverses() {
    // nominal(0.01, monthly) = 0.12; from-nominal(0.12, monthly) = 0.01.
    time_value()
        .args([
            "rate",
            "nominal",
            "--rate",
            "0.01",
            "--periodicity",
            "monthly",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.12"));

    time_value()
        .args([
            "rate",
            "from-nominal",
            "--nominal",
            "0.12",
            "--periodicity",
            "monthly",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.01"));
}

#[test]
fn rate_rejects_an_unknown_periodicity() {
    time_value()
        .args([
            "rate",
            "ear",
            "--rate",
            "0.01",
            "--periodicity",
            "fortnightly",
        ])
        .assert()
        .failure()
        // Periodicity is a clap ValueEnum (ADR-0039): an unknown value is rejected
        // by parsing, and the error lists the valid set.
        .stderr(predicate::str::contains("fortnightly"))
        .stderr(predicate::str::contains("semi-annual"));
}

#[test]
fn amortize_over_a_term_prints_a_table() {
    // 1000 at 10% paying 500: three rows (500, 500, 176 stub), balance to 0.
    time_value()
        .args([
            "amortize",
            "--rate",
            "0.10",
            "--principal",
            "1000",
            "--payment",
            "500",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("period\tpayment"))
        // The final installment clears the balance.
        .stdout(predicate::str::contains("3\t176"));
}

#[test]
fn amortize_json_is_a_schedule_object() {
    // The typed output layer (ADR-0039) wraps the rows in `{ "schedule": [...] }`,
    // the uniform tabular shape; the rows themselves are unchanged.
    time_value()
        .args([
            "--json",
            "amortize",
            "--rate",
            "0.10",
            "--principal",
            "1000",
            "--payment",
            "500",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{\"schedule\":[{"))
        .stdout(predicate::str::contains("\"period\":1"))
        .stdout(predicate::str::contains("\"balance\":0"));
}

#[test]
fn amortize_requires_periods_or_payment() {
    time_value()
        .args(["amortize", "--rate", "0.01", "--principal", "1000"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("--periods").or(predicate::str::contains("--payment")));
}

#[test]
fn amortize_rejects_a_non_amortizing_payment() {
    // A payment below the first period's interest never retires the balance. The
    // message names *that* condition rather than a generic "undefined": the
    // library's `PaymentDoesNotAmortize` reaches the user (ADR-0052).
    time_value()
        .args([
            "amortize",
            "--rate",
            "0.10",
            "--principal",
            "1000",
            "--payment",
            "50",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("amortization schedule"))
        .stderr(predicate::str::contains("never amortised"));
}

#[test]
fn amortize_over_a_zero_term_names_the_zero_term() {
    // The other degenerate amortization case, now told apart from the one above
    // (ADR-0052): a zero term has nothing to amortise over.
    time_value()
        .args([
            "amortize",
            "--rate",
            "0.01",
            "--principal",
            "1000",
            "--periods",
            "0",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("at least one period"));
}

#[test]
fn json_output_uses_the_uniform_value_key() {
    // ADR-0039: the scalar `--json` shape is `{ "value": … }`, uniform across the
    // families (the operation is already named by the command), replacing the old
    // per-operation key.
    time_value()
        .args([
            "--json", "series", "npv", "--rate", "0.01", "-100", "60", "60",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\""))
        .stdout(predicate::str::contains("\"npv\"").not());
}

#[test]
fn json_scalar_shape_is_uniform_across_families() {
    // Every scalar operation, across every family, emits the same `{ "value": … }`
    // object under `--json` (ADR-0028 §4 as amended by ADR-0039).
    let cases: &[&[&str]] = &[
        &[
            "single-sum",
            "pv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--future",
            "1000",
        ],
        &[
            "annuity",
            "pv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--payment",
            "100",
        ],
        &[
            "annuity",
            "due",
            "pv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--payment",
            "100",
        ],
        &["rate", "ear", "--rate", "0.01", "--periodicity", "monthly"],
    ];
    for op_args in cases {
        let mut args = vec!["--json"];
        args.extend_from_slice(op_args);
        time_value()
            .args(&args)
            .assert()
            .success()
            .stdout(predicate::str::contains("\"value\""));
    }
}

#[test]
fn an_invalid_rate_fails() {
    time_value()
        .args(["series", "npv", "--rate", "-1.5", "-100", "60"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("rate"));
}

#[test]
fn an_overflowing_result_fails_instead_of_printing_inf() {
    // 2^2000 overflows f64; the CLI must error, not print `inf` with exit 0.
    time_value()
        .args([
            "single-sum",
            "fv",
            "--rate",
            "1",
            "--periods",
            "2000",
            "--present",
            "1e6",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("finite"));
}

#[test]
fn an_overflowing_result_fails_in_json_mode_too() {
    // Previously this printed `{"single_sum_future_value":null}` with exit 0; now it is an error.
    time_value()
        .args([
            "--json",
            "single-sum",
            "fv",
            "--rate",
            "1",
            "--periods",
            "2000",
            "--present",
            "1e6",
        ])
        .assert()
        .failure()
        .stdout(predicate::str::contains("null").not());
}

#[test]
fn a_nonconvergent_irr_fails() {
    // All inflows: NPV is positive for every rate, so there is no IRR.
    time_value()
        .args(["series", "irr", "100", "60", "60"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("internal rate of return"));
}

// ---- Currency (--currency): ADR-0034 -------------------------------------

#[test]
fn currency_echoes_the_code_after_a_monetary_result() {
    time_value()
        .args([
            "--currency",
            "USD",
            "series",
            "npv",
            "--rate",
            "0.01",
            "-100",
            "60",
            "60",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("18.22").and(predicate::str::contains("USD")));
}

#[test]
fn default_currency_stays_a_bare_number() {
    // No --currency (XXX): output is unchanged — no code appended.
    time_value()
        .args(["series", "npv", "--rate", "0.01", "-100", "60", "60"])
        .assert()
        .success()
        .stdout(predicate::str::contains("XXX").not());
}

#[test]
fn currency_is_added_as_a_json_field() {
    time_value()
        .args([
            "--json",
            "--currency",
            "JPY",
            "single-sum",
            "fv",
            "--rate",
            "0.01",
            "--periods",
            "12",
            "--present",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"currency\":\"JPY\""));
}

/// `--json`'s own help text said "a one-field JSON object", which
/// `currency_is_added_as_a_json_field` above disproves — a monetary result in a
/// non-`XXX` currency has two. Nothing tested the help string, so it went stale
/// while the module doc twenty lines away stayed right. Pin it to the shape the
/// binary actually emits (ADR-0045 rule 2).
#[test]
fn the_json_flag_help_describes_the_currency_field_it_can_emit() {
    time_value()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("one-field").not())
        .stdout(predicate::str::contains("`\"currency\"` field"));
}

#[test]
fn currency_is_not_echoed_for_a_rate_result() {
    // IRR is a rate, not money — the currency does not apply.
    time_value()
        .args(["--currency", "USD", "series", "irr", "-100", "60", "60"])
        .assert()
        .success()
        .stdout(predicate::str::contains("USD").not());
}

#[test]
fn an_unknown_currency_code_is_rejected() {
    time_value()
        .args([
            "--currency",
            "ZZZ",
            "series",
            "npv",
            "--rate",
            "0.01",
            "-100",
            "60",
            "60",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ZZZ"));
}

#[test]
fn amortize_echoes_currency_as_a_comment_line() {
    time_value()
        .args([
            "--currency",
            "USD",
            "amortize",
            "--rate",
            "0.10",
            "--payment",
            "500",
            "--principal",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("# currency: USD"));
}

// ---- Foreign exchange (convert): ADR-0034/0037, #67 ----------------------

#[test]
fn convert_restates_an_amount_in_the_target_currency() {
    // 100 USD at 0.9 USD->EUR = 90 EUR; the target code is echoed.
    time_value()
        .args([
            "convert", "--from", "USD", "--to", "EUR", "--rate", "0.9", "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("90").and(predicate::str::contains("EUR")));
}

#[test]
fn convert_json_carries_the_target_currency() {
    time_value()
        .args([
            "--json", "convert", "--from", "GBP", "--to", "USD", "--rate", "1.25", "80",
        ])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("\"value\":100.0")
                .and(predicate::str::contains("\"currency\":\"USD\"")),
        );
}

#[test]
fn convert_to_agnostic_stays_a_bare_number() {
    // Converting into XXX drops the code, matching every other monetary result.
    time_value()
        .args([
            "convert", "--from", "USD", "--to", "XXX", "--rate", "0.9", "100",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("XXX").not());
}

#[test]
fn convert_rejects_a_non_positive_rate() {
    time_value()
        .args([
            "convert", "--from", "USD", "--to", "EUR", "--rate", "0", "100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("exchange rate"));
}

/// A rate that is finite and positive but whose reciprocal is not — the band
/// `FxRate::new` narrowed to so that `inverse()` cannot lie (ADR-0053). It reaches
/// the CLI as an exchange-rate error, not as an out-of-range converted amount.
#[test]
fn convert_rejects_a_rate_outside_the_invertible_band() {
    for rate in ["5e-324", "1.7976931348623157e308"] {
        time_value()
            .args([
                "convert", "--from", "USD", "--to", "EUR", "--rate", rate, "100",
            ])
            .assert()
            .failure()
            .stderr(predicate::str::contains("exchange rate"));
    }

    // A realistically extreme rate is still fine — the excluded band is hundreds
    // of orders of magnitude beyond any real currency pair.
    time_value()
        .args([
            "convert", "--from", "USD", "--to", "EUR", "--rate", "1e-7", "100",
        ])
        .assert()
        .success();
}

#[test]
fn convert_rejects_an_unknown_currency_code() {
    time_value()
        .args([
            "convert", "--from", "ZZZ", "--to", "EUR", "--rate", "0.9", "100",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("ZZZ"));
}

// ---- Continuous compounding (continuous): ADR-0036/0041, #68 -------------

#[test]
fn continuous_future_value_grows_at_the_force_of_interest() {
    // 1000 at δ=0.05 over 3y = 1000·e^0.15 ≈ 1161.83.
    time_value()
        .args([
            "continuous",
            "fv",
            "--rate",
            "0.05",
            "--years",
            "3",
            "--present",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("1161.83"));
}

#[test]
fn continuous_present_value_inverts_future_value() {
    // The inverse of the fv above: 1000·e^-0.15 ≈ 860.71.
    time_value()
        .args([
            "continuous",
            "pv",
            "--rate",
            "0.05",
            "--years",
            "3",
            "--future",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("860.7"));
}

#[test]
fn continuous_value_echoes_the_currency() {
    time_value()
        .args([
            "--currency",
            "USD",
            "continuous",
            "fv",
            "--rate",
            "0.05",
            "--years",
            "3",
            "--present",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("USD"));
}

#[test]
fn continuous_from_effective_is_the_log_bridge() {
    // δ = ln(1 + 0.05) ≈ 0.048790.
    time_value()
        .args(["continuous", "from-effective", "--rate", "0.05"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.04879"));
}

#[test]
fn continuous_effective_inverts_from_effective() {
    // r = e^0.05 − 1 ≈ 0.051271.
    time_value()
        .args(["continuous", "effective", "--rate", "0.05"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.05127"));
}

#[test]
fn continuous_effective_carries_no_currency() {
    // The bridge is a rate, not money — the currency does not apply.
    time_value()
        .args([
            "--currency",
            "USD",
            "continuous",
            "effective",
            "--rate",
            "0.05",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("USD").not());
}

#[test]
fn continuous_rejects_a_non_finite_force() {
    time_value()
        .args([
            "continuous",
            "fv",
            "--rate",
            "inf",
            "--years",
            "3",
            "--present",
            "1000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("force of interest"));
}

// ---- The continuous solves (continuous rate / years): ADR-0064, #106 ------

#[test]
fn continuous_rate_solves_for_the_force_of_interest() {
    // The inverse of `continuous fv` above: the force that takes 1000 to 1161.83
    // over three years is 5%.
    time_value()
        .args([
            "continuous",
            "rate",
            "--years",
            "3",
            "--present",
            "1000",
            "--future",
            "1161.834242728283",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("0.0499999999999999"));
}

#[test]
fn continuous_years_solves_for_the_span() {
    time_value()
        .args([
            "continuous",
            "years",
            "--rate",
            "0.05",
            "--present",
            "1000",
            "--future",
            "1161.834242728283",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("2.9999999999999"));
}

/// The span is signed (ADR-0036/0064): reaching a *smaller* amount at a positive
/// force of interest is in the past, and the CLI prints a negative number rather
/// than failing the way `single-sum nper` does on the same shape of input.
#[test]
fn continuous_years_prints_a_negative_span_for_a_discount() {
    time_value()
        .args([
            "continuous",
            "years",
            "--rate",
            "0.05",
            "--present",
            "1161.834242728283",
            "--future",
            "1000",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("-2.9999999999999"));
}

/// A negative `--years` is accepted by the force solve and flips the sign of the
/// answer — the flag takes `allow_hyphen_values`, so this also pins that it parses.
#[test]
fn continuous_rate_accepts_a_negative_span() {
    time_value()
        .args([
            "continuous",
            "rate",
            "--years",
            "-3",
            "--present",
            "1000",
            "--future",
            "1161.834242728283",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("-0.0499999999999999"));
}

/// The solves are scalar, so they never echo a currency even though the amounts
/// are denominated by the global flag (ADR-0057: the fold belongs to operations
/// that *produce* a `Money`).
#[test]
fn the_continuous_solves_carry_no_currency() {
    for leaf in ["rate", "years"] {
        let (flag, value) = if leaf == "rate" {
            ("--years", "3")
        } else {
            ("--rate", "0.05")
        };
        time_value()
            .args([
                "--currency",
                "USD",
                "continuous",
                leaf,
                flag,
                value,
                "--present",
                "1000",
                "--future",
                "1161.834242728283",
            ])
            .assert()
            .success()
            .stdout(predicate::str::contains("USD").not());
    }
}

/// A zero span leaves the force under-determined when the amounts agree, and
/// unsolvable when they do not. The message interpolates the library error rather
/// than restating it, because a static "no rate solves these inputs" would invert
/// the first case (ADR-0052/0064).
#[test]
fn a_zero_span_reports_the_degeneracy_it_found() {
    time_value()
        .args([
            "continuous",
            "rate",
            "--years",
            "0",
            "--present",
            "1000",
            "--future",
            "1000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "force of interest: every rate satisfies these inputs",
        ));

    time_value()
        .args([
            "continuous",
            "rate",
            "--years",
            "0",
            "--present",
            "1000",
            "--future",
            "2000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no real solution"));
}

/// The mirror on the span solve — and the reason the two degeneracies do not share
/// one variant: the message names the *span*, not a rate.
#[test]
fn a_zero_force_reports_the_span_degeneracy() {
    time_value()
        .args([
            "continuous",
            "years",
            "--rate",
            "0",
            "--present",
            "1000",
            "--future",
            "1000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "span in years: every span satisfies these inputs",
        ));

    time_value()
        .args([
            "continuous",
            "years",
            "--rate",
            "0",
            "--present",
            "1000",
            "--future",
            "2000",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no real solution"));
}

/// Both solves reject the same out-of-domain amounts, with the same variant.
#[test]
fn the_continuous_solves_reject_amounts_outside_the_domain() {
    for (leaf, flag, value) in [("rate", "--years", "3"), ("years", "--rate", "0.05")] {
        for (present, future) in [("1000", "-500"), ("0", "1000"), ("1000", "0")] {
            time_value()
                .args([
                    "continuous",
                    leaf,
                    flag,
                    value,
                    "--present",
                    present,
                    "--future",
                    future,
                ])
                .assert()
                .failure()
                .stderr(predicate::str::contains("no real solution"));
        }
    }
}

/// `--json` wraps a solve in the uniform scalar shape, with no `currency` field
/// (ADR-0039).
#[test]
fn the_continuous_solves_print_the_scalar_json_shape() {
    time_value()
        .args([
            "--json",
            "continuous",
            "years",
            "--rate",
            "0.05",
            "--present",
            "1000",
            "--future",
            "1161.834242728283",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\""))
        .stdout(predicate::str::contains("currency").not());
}
