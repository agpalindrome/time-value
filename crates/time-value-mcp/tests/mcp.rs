//! End-to-end test of the MCP server: spawn the binary and drive a real stdio
//! JSON-RPC session — initialize, tools/list, tools/call (ADR-0011).

use assert_cmd::Command;
use predicates::prelude::*;

/// Wrap one or more `tools/call` request lines in a full initialised session.
fn session(calls: &str) -> String {
    format!(
        concat!(
            r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"test","version":"0"}}}}}}"#,
            "\n",
            r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#,
            "\n",
            "{calls}",
        ),
        calls = calls
    )
}

#[test]
fn stdio_session_lists_tools_and_computes_npv() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"npv","arguments":{"rate":0.01,"cashflows":[-100,60,60]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // The server identifies itself (not the rmcp crate) on initialise.
        .stdout(predicate::str::contains("\"name\":\"time-value-mcp\""))
        // tools/list exposes every tool with a JSON-Schema input.
        .stdout(predicate::str::contains("npv"))
        .stdout(predicate::str::contains("irr"))
        .stdout(predicate::str::contains("single_sum_present_value"))
        .stdout(predicate::str::contains("annuity_payment"))
        .stdout(predicate::str::contains("inputSchema"))
        // tools/call returns the computed NPV (~18.2237).
        .stdout(predicate::str::contains("18.22"));
}

#[test]
fn irr_tool_solves_the_series() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"irr","arguments":{"cashflows":[-100,60,60]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("0.130"));
}

#[test]
fn xirr_tool_solves_dated_flows() {
    // Microsoft's XIRR example over ISO dates -> ~0.3734.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"xirr","arguments":{"flows":[{"date":"2008-01-01","amount":-10000},{"date":"2008-03-01","amount":2750},{"date":"2008-10-30","amount":4250},{"date":"2009-02-15","amount":3250},{"date":"2009-04-01","amount":2750}]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("0.373"));
}

#[test]
fn xnpv_tool_lists_and_computes() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"xnpv","arguments":{"rate":0.10,"flows":[{"date":"2020-01-01","amount":-100},{"date":"2021-01-01","amount":110}]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // The new tools are advertised alongside the originals.
        .stdout(predicate::str::contains("mirr"))
        .stdout(predicate::str::contains("xnpv"))
        .stdout(predicate::str::contains("xirr"));
}

#[test]
fn an_invalid_date_is_an_error() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"xirr","arguments":{"flows":[{"date":"2020-02-30","amount":-100},{"date":"2021-01-01","amount":110}]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("invalid date"));
}

#[test]
fn single_sum_periods_tool_solves() {
    // 1000 → 1126.825 at 1%/period ≈ 12 periods.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"single_sum_periods","arguments":{"rate":0.01,"present":1000,"future":1126.825}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("single_sum_periods"))
        .stdout(predicate::str::contains("11.9").or(predicate::str::contains("12.0")));
}

#[test]
fn annuity_perpetuity_and_due_tools() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_perpetuity","arguments":{"rate":0.05,"payment":100}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_due_present_value","arguments":{"rate":0.01,"periods":12,"payment":100}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // Perpetuity 100/0.05 = 2000; annuity-due PV ≈ 1136.76.
        .stdout(predicate::str::contains("2000"))
        .stdout(predicate::str::contains("1136.7"));
}

#[test]
fn annuity_growing_tools() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_growing_present_value","arguments":{"rate":0.05,"growth":0.02,"periods":12,"payment":100}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_growing_due_future_value","arguments":{"rate":0.05,"growth":0.02,"periods":12,"payment":100}}}"#,
        "\n",
        // Growth above the rate is priced, not rejected — the deliberate
        // difference from `annuity_growing_perpetuity` (ADR-0048).
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"annuity_growing_present_value","arguments":{"rate":0.02,"growth":0.05,"periods":12,"payment":100}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // Growing PV ≈ 979.32; the growing annuity-due FV ≈ 1846.65.
        .stdout(predicate::str::contains("979.3"))
        .stdout(predicate::str::contains("1846.6"))
        // r < g still converges over a finite term: ≈ 1386.73.
        .stdout(predicate::str::contains("1386.7"));
}

/// The sinking-fund payment and the perpetuity-due forms (ADR-0062).
#[test]
fn annuity_sinking_fund_and_perpetuity_due_tools() {
    let calls = concat!(
        // 12 contributions reaching 1268.25 at 1%/month -> ~100 each.
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_payment","arguments":{"rate":0.01,"periods":12,"future":1268.250}}}"#,
        "\n",
        // The same contribution reaches the larger start-of-period total.
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_due_payment","arguments":{"rate":0.01,"periods":12,"future":1280.933}}}"#,
        "\n",
        // 100/0.05 × 1.05 = 2100.
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"annuity_due_perpetuity","arguments":{"rate":0.05,"payment":100}}}"#,
        "\n",
        // 100/(0.05 − 0.02) × 1.05 = 3500.
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"annuity_growing_due_perpetuity","arguments":{"rate":0.05,"growth":0.02,"payment":100}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("99.99").or(predicate::str::contains("100.0")))
        .stdout(predicate::str::contains("2100"))
        .stdout(predicate::str::contains("3500"));
}

/// The `annuity_payment` tools take the same mutually-exclusive anchor the solves
/// do, so omitting both is an `invalid_params` error rather than a wrong answer.
#[test]
fn annuity_payment_requires_exactly_one_anchor() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_payment","arguments":{"rate":0.01,"periods":12}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_due_payment","arguments":{"rate":0.01,"periods":12,"present":1000,"future":1268.25}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("present").or(predicate::str::contains("future")))
        .stdout(predicate::str::contains("mutually exclusive"));
}

/// A perpetuity-due diverges on exactly the ordinary perpetuity's condition:
/// bringing every payment forward one period rescales a convergent sum, it does not
/// make a divergent one converge.
#[test]
fn the_perpetuity_due_tools_reject_divergence() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_due_perpetuity","arguments":{"rate":0,"payment":100}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_growing_due_perpetuity","arguments":{"rate":0.02,"growth":0.05,"payment":100}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("diverges"));
}

#[test]
fn annuity_periods_requires_exactly_one_anchor() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_periods","arguments":{"rate":0.01,"payment":100}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("present").or(predicate::str::contains("future")));
}

#[test]
fn rate_conversion_tools() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"rate_effective_annual","arguments":{"rate":0.01,"periodicity":"monthly"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"rate_convert","arguments":{"rate":0.01,"from":"monthly","to":"quarterly"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // EAR of 1%/month = 0.126825…; monthly→quarterly = 0.030301…
        .stdout(predicate::str::contains("0.1268"))
        .stdout(predicate::str::contains("0.0303"));
}

#[test]
fn rate_rejects_an_unknown_periodicity() {
    // Periodicity is a typed enum (ADR-0039), so an unknown value is refused by
    // deserialization at the boundary, before the handler runs.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"rate_effective_annual","arguments":{"rate":0.01,"periodicity":"fortnightly"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // The deserialize error names the bad value and lists the valid set.
        .stdout(predicate::str::contains("unknown variant"))
        .stdout(predicate::str::contains("fortnightly"))
        .stdout(predicate::str::contains("semi-annual"));
}

#[test]
fn amortize_tool_returns_a_schedule() {
    // 1000 at 10% paying 500 → three installments, last clears the balance.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"amortize","arguments":{"rate":0.10,"principal":1000,"payment":500}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("amortize"))
        .stdout(predicate::str::contains("\"period\":3"))
        .stdout(predicate::str::contains("\"balance\":0"));
}

#[test]
fn amortize_requires_periods_or_payment() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"amortize","arguments":{"rate":0.10,"principal":1000}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("periods").or(predicate::str::contains("payment")));
}

#[test]
fn an_overflowing_result_is_an_error_not_null() {
    // Previously this returned `{"future_value":null}` with isError:false — a
    // silent non-answer. Now it is a proper error (ADR-0021).
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"single_sum_future_value","arguments":{"rate":1,"periods":2000,"present":1000000}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success() // the process exits cleanly; the error is in the JSON-RPC response
        .stdout(predicate::str::contains("finite"))
        .stdout(predicate::str::contains("\"single_sum_future_value\":null").not());
}

#[test]
fn an_invalid_rate_is_an_error() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"npv","arguments":{"rate":-1.5,"cashflows":[-100,60]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success() // the process exits cleanly; the JSON-RPC response carries the error
        .stdout(predicate::str::contains("error"))
        .stdout(predicate::str::contains("rate"));
}

// ---- Currency (the `currency` input field): ADR-0034 ---------------------

#[test]
fn npv_echoes_the_currency_when_given() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"npv","arguments":{"rate":0.01,"cashflows":[-100,60,60],"currency":"USD"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("18.22"))
        .stdout(predicate::str::contains("\"currency\":\"USD\""));
}

#[test]
fn npv_without_currency_has_no_currency_field() {
    // Omitting `currency` (XXX) keeps the pre-currency output shape.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"npv","arguments":{"rate":0.01,"cashflows":[-100,60,60]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"currency\"").not());
}

#[test]
fn a_rate_result_carries_no_currency() {
    // IRR is a rate, not money: the `currency` input is accepted but not echoed.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"irr","arguments":{"cashflows":[-100,60,60],"currency":"USD"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("0.130"))
        .stdout(predicate::str::contains("\"currency\"").not());
}

#[test]
fn an_unknown_currency_code_is_an_error() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"npv","arguments":{"rate":0.01,"cashflows":[-100,60],"currency":"ZZZ"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success() // process exits cleanly; the response carries the error
        // An unknown code is now rejected at parameter deserialization by the core
        // `Currency` serde impl (ADR-0044), with the friendly ISO-code message.
        .stdout(predicate::str::contains("unknown ISO 4217 currency code"))
        .stdout(predicate::str::contains("ZZZ"));
}

#[test]
fn currency_input_advertises_the_code_enum() {
    // The `currency` input schema lists the ISO 4217 codes as an `enum` — now the
    // core `Currency`'s own JsonSchema (ADR-0044), inlined — so a consumer
    // discovers the valid set from tools/list. `ZWG` only occurs in that enum.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("ZWG"));
}

// ---- Foreign exchange (the `convert` tool): ADR-0034/0037, #67 -----------

#[test]
fn convert_restates_an_amount_in_the_target_currency() {
    // 100 USD at 0.9 USD->EUR = 90 EUR; the result is tagged the target code.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"convert","arguments":{"amount":100,"from":"USD","to":"EUR","rate":0.9}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"value\":90"))
        .stdout(predicate::str::contains("\"currency\":\"EUR\""));
}

#[test]
fn convert_is_listed_as_a_tool() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"convert\""));
}

#[test]
fn convert_rejects_a_non_positive_rate() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"convert","arguments":{"amount":100,"from":"USD","to":"EUR","rate":0}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"error\""))
        .stdout(predicate::str::contains("exchange rate"));
}

/// The narrowed `FxRate` domain (ADR-0053) reaches the MCP surface too: a
/// subnormal rate is an exchange-rate error rather than a subnormal result.
#[test]
fn convert_rejects_a_rate_outside_the_invertible_band() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"convert","arguments":{"amount":100,"from":"USD","to":"EUR","rate":5e-324}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"error\""))
        .stdout(predicate::str::contains("exchange rate"));
}

#[test]
fn convert_rejects_an_unknown_currency_code() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"convert","arguments":{"amount":100,"from":"ZZZ","to":"EUR","rate":0.9}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("ZZZ"));
}

// ---- Continuous compounding (the `continuous_*` tools): ADR-0036/0041, #68

#[test]
fn continuous_future_value_grows_and_echoes_currency() {
    // 1000 at δ=0.05 over 3y ≈ 1161.83, tagged USD.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"continuous_future_value","arguments":{"rate":0.05,"years":3,"amount":1000,"currency":"USD"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("1161.83"))
        .stdout(predicate::str::contains("\"currency\":\"USD\""));
}

#[test]
fn continuous_present_value_inverts_future_value() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"continuous_present_value","arguments":{"rate":0.05,"years":3,"amount":1000}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("860.7"));
}

#[test]
fn continuous_bridge_tools_round_trip_and_are_listed() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"continuous_from_effective","arguments":{"rate":0.05}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"continuous_effective","arguments":{"rate":0.05}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"continuous_future_value\""))
        .stdout(predicate::str::contains("\"continuous_effective\""))
        // δ = ln(1.05) ≈ 0.04879, and e^0.05 − 1 ≈ 0.05127.
        .stdout(predicate::str::contains("0.04879"))
        .stdout(predicate::str::contains("0.05127"));
}

#[test]
fn continuous_rejects_a_non_finite_force() {
    // JSON has no infinity literal; a force is rejected only when non-finite,
    // which the amount value cannot express — so exercise the amount overflow
    // path instead: an enormous growth factor overflows to a `TvmError`.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"continuous_future_value","arguments":{"rate":700,"years":2,"amount":1e300}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"error\""));
}
