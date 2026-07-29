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
    // A payment below the first period's interest never retires the balance.
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
        .stderr(predicate::str::contains("undefined"));
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
