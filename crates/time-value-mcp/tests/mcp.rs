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

/// The structured results of a session, keyed by request id.
fn structured(calls: &str) -> std::collections::HashMap<i64, serde_json::Value> {
    let output = Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .output()
        .expect("run server");
    assert!(output.status.success(), "server exited non-zero");

    let mut results = std::collections::HashMap::new();
    for line in String::from_utf8(output.stdout).expect("utf8").lines() {
        let value: serde_json::Value = serde_json::from_str(line).expect("json-rpc line");
        if let (Some(id), Some(content)) = (
            value.get("id").and_then(serde_json::Value::as_i64),
            value.pointer("/result/structuredContent"),
        ) {
            results.insert(id, content.clone());
        }
    }
    results
}

#[test]
fn xnfv_tool_compounds_to_the_latest_date() {
    // The horizon is the latest date, not the last flow listed (ADR-0065), so the
    // reversed call must return the same amount: −27.8267360… at 10% a year.
    let sorted = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"xnfv","arguments":{"rate":0.10,"flows":[{"date":"2020-01-01","amount":-1000},{"date":"2020-07-01","amount":-500},{"date":"2021-04-01","amount":800},{"date":"2022-01-01","amount":900}]}}}"#;
    let reversed = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"xnfv","arguments":{"rate":0.10,"flows":[{"date":"2022-01-01","amount":900},{"date":"2021-04-01","amount":800},{"date":"2020-07-01","amount":-500},{"date":"2020-01-01","amount":-1000}]}}}"#;
    let results = structured(&format!("{sorted}\n{reversed}\n"));

    let value = |id: i64| results[&id]["value"].as_f64().expect("a number");
    assert!(
        (value(2) - -27.826_736_031_298_8).abs() < 1e-9,
        "{} is not the reference −27.8267360312988",
        value(2),
    );
    assert!(
        (value(2) - value(3)).abs() < 1e-9,
        "the horizon moved with the order: {} vs {}",
        value(2),
        value(3),
    );
}

#[test]
fn xmirr_tool_annualises_over_the_dated_span() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"xmirr","arguments":{"finance":0.10,"reinvest":0.12,"flows":[{"date":"2020-01-01","amount":-1000},{"date":"2020-07-01","amount":-500},{"date":"2021-04-01","amount":800},{"date":"2022-01-01","amount":900}]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("0.0950481"));
}

#[test]
fn the_dated_tools_are_advertised_and_follow_the_currency_split() {
    // `xnfv` produces money, so it echoes the currency; `xmirr` produces a rate, so
    // it does not (ADR-0057). Both are listed alongside the originals.
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"xnfv","arguments":{"rate":0.10,"currency":"USD","flows":[{"date":"2020-01-01","amount":-100},{"date":"2021-01-01","amount":110}]}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"xmirr","arguments":{"finance":0.10,"reinvest":0.12,"currency":"USD","flows":[{"date":"2020-01-01","amount":-1000},{"date":"2022-01-01","amount":1500}]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"xnfv\""))
        .stdout(predicate::str::contains("\"xmirr\""));

    // …and in the structured results, only the monetary one carries the code.
    let results = structured(calls);
    assert_eq!(results[&3]["currency"], serde_json::json!("USD"));
    assert!(
        results[&4].get("currency").is_none(),
        "the dated MIRR echoed a currency: {}",
        results[&4],
    );
}

#[test]
fn xmirr_reports_a_series_dated_on_one_day() {
    // Zero span: with the flows matching, every rate satisfies them; with them
    // mismatched, none does (ADR-0056, ADR-0065).
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"xmirr","arguments":{"finance":0.10,"reinvest":0.10,"flows":[{"date":"2020-01-01","amount":-1000},{"date":"2020-01-01","amount":1000}]}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"xmirr","arguments":{"finance":0.10,"reinvest":0.10,"flows":[{"date":"2020-01-01","amount":-1000},{"date":"2020-01-01","amount":1500}]}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"xmirr","arguments":{"finance":0.10,"reinvest":0.10,"flows":[{"date":"2020-01-01","amount":1000}]}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "every rate satisfies these inputs",
        ))
        .stdout(predicate::str::contains("no real solution"))
        .stdout(predicate::str::contains("no outflows"));
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

/// The annuity-due period and rate solves, from both anchors (ADR-0063). Every value
/// here comes from twelve start-of-month payments of 100 at 1%/month, so both anchors
/// return the term or the rate that produced it.
#[test]
fn annuity_due_solve_tools() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_due_periods","arguments":{"rate":0.01,"payment":100,"present":1136.763}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_due_periods","arguments":{"rate":0.01,"payment":100,"future":1280.933}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"annuity_due_rate","arguments":{"periods":12,"payment":100,"present":1136.763}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"annuity_due_rate","arguments":{"periods":12,"payment":100,"future":1280.933}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("12.0"))
        .stdout(predicate::str::contains("0.00999").or(predicate::str::contains("0.01")));
}

/// A single start-of-period *payment* is never discounted, so the due present-value
/// factor is `1` at every rate and the rate solve is under-determined — the row
/// ADR-0056's table has for the *ordinary future* factor, moved here by the `(1 + r)`
/// scaling. A single start-of-period *contribution* is the mirror image: its factor is
/// `1 + r`, so it is a determined solve where the ordinary one is not (ADR-0063).
#[test]
fn a_single_period_annuity_due_rate_solve_reports_which_side_it_is_on() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_due_rate","arguments":{"periods":1,"payment":100,"present":100}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_due_rate","arguments":{"periods":1,"payment":100,"present":150}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"annuity_due_rate","arguments":{"periods":1,"payment":100,"future":125}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        // Satisfied by every rate, so none is the answer…
        .stdout(predicate::str::contains("every rate satisfies"))
        // …satisfied by none, which is a different failure…
        .stdout(predicate::str::contains("no real solution"))
        // …and the future anchor solves it outright: 125/100 − 1.
        .stdout(predicate::str::contains("0.2499").or(predicate::str::contains("0.25")));
}

/// The three growing-annuity inverses (ADR-0063), each recovering the argument that
/// produced a growing present value of 979.318 (twelve payments from 100 escalating
/// 2%/month, discounted at 5%/month).
#[test]
fn annuity_growing_inverse_tools() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_growing_payment","arguments":{"rate":0.05,"growth":0.02,"periods":12,"present":979.318,"currency":"USD"}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_growing_periods","arguments":{"rate":0.05,"growth":0.02,"payment":100,"present":979.318}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"annuity_growing_rate","arguments":{"growth":0.02,"periods":12,"payment":100,"present":979.318}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("99.99").or(predicate::str::contains("100.0")))
        .stdout(predicate::str::contains("\"USD\""))
        .stdout(predicate::str::contains("11.99").or(predicate::str::contains("12.0")))
        .stdout(predicate::str::contains("0.05"));
}

/// With the rate above the growth a growing annuity's present value is capped by the
/// growing perpetuity, so a target at or above `payment / (rate − growth)` is reached
/// by no finite term — while growth above the rate has no cap at all (ADR-0063).
#[test]
fn annuity_growing_periods_respects_the_perpetuity_ceiling() {
    let calls = concat!(
        // 100/(0.05 − 0.02) = 3333.33, so 4000 is unreachable.
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"annuity_growing_periods","arguments":{"rate":0.05,"growth":0.02,"payment":100,"present":4000}}}"#,
        "\n",
        // The same target with the growth above the rate: no ceiling, so it solves.
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"annuity_growing_periods","arguments":{"rate":0.02,"growth":0.05,"payment":100,"present":4000}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("never amortised"))
        .stdout(predicate::str::contains("\"value\":2"));
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

// ---- The continuous solves (`continuous_rate` / `continuous_years`): ADR-0064

#[test]
fn the_continuous_solve_tools_read_the_relation_back_and_are_listed() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"continuous_rate","arguments":{"years":3,"present":1000,"future":1161.834242728283}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"continuous_years","arguments":{"rate":0.05,"present":1000,"future":1161.834242728283}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("\"continuous_rate\""))
        .stdout(predicate::str::contains("\"continuous_years\""))
        .stdout(predicate::str::contains("0.0499999999999999"))
        .stdout(predicate::str::contains("2.9999999999999"));
}

/// The span is signed (ADR-0036/0064) — a discount answers in the past — and the
/// scalar result never carries a currency, even when the amounts were denominated
/// (ADR-0057).
#[test]
fn continuous_years_answers_negatively_for_a_discount_and_carries_no_currency() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"continuous_years","arguments":{"rate":0.05,"present":1161.834242728283,"future":1000,"currency":"USD"}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains("-2.9999999999999"))
        .stdout(predicate::str::contains("\"currency\"").not());
}

/// Every degenerate and out-of-domain input surfaces as a tool error carrying the
/// library's own message — including the two that say *every* value satisfies the
/// inputs, which name different unknowns.
#[test]
fn the_continuous_solves_report_their_degeneracies() {
    let calls = concat!(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"continuous_rate","arguments":{"years":0,"present":1000,"future":1000}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"continuous_years","arguments":{"rate":0,"present":1000,"future":1000}}}"#,
        "\n",
        r#"{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"continuous_rate","arguments":{"years":3,"present":1000,"future":-500}}}"#,
        "\n",
    );

    Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(session(calls))
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "every rate satisfies these inputs",
        ))
        .stdout(predicate::str::contains(
            "every span satisfies these inputs",
        ))
        .stdout(predicate::str::contains("no real solution"));
}
