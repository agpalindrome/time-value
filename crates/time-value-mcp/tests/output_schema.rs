//! Output-schema conformance (ADR-0039): every tool's real `structuredContent`
//! must validate against the `outputSchema` that same tool declares in
//! `tools/list`. This is the drift guard the typed output layer promises — a
//! renamed, added, or forgotten field, or a type mismatch between what a tool
//! returns and what its schema says, fails here.
//!
//! The harness drives a real stdio JSON-RPC session (like `mcp.rs`) and carries a
//! small JSON-Schema validator covering the subset schemars emits for our DTOs
//! (objects, numbers, strings, arrays, `$ref`/`$defs`, and `type` unions). Cases
//! are added family by family as each is converted; this file currently covers
//! the **series** family.

use assert_cmd::Command;
use serde_json::{json, Value};

/// One `tools/call` case: the tool name and its arguments.
struct Case {
    tool: &'static str,
    args: Value,
}

/// Drive an initialised stdio session: a `tools/list` (id 100) followed by one
/// `tools/call` per case (id = its index), and return the parsed JSON-RPC
/// responses keyed by id.
fn run(cases: &[Case]) -> std::collections::HashMap<i64, Value> {
    let mut lines = vec![
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}).to_string(),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
        json!({"jsonrpc":"2.0","id":100,"method":"tools/list","params":{}}).to_string(),
    ];
    for (i, case) in cases.iter().enumerate() {
        let id = i64::try_from(i).expect("case index fits i64");
        lines.push(
            json!({"jsonrpc":"2.0","id": id,"method":"tools/call",
                   "params":{"name": case.tool, "arguments": case.args}})
            .to_string(),
        );
    }
    let input = format!("{}\n", lines.join("\n"));

    let output = Command::cargo_bin("time-value-mcp")
        .unwrap()
        .write_stdin(input)
        .output()
        .expect("run server");
    assert!(output.status.success(), "server exited non-zero");

    let stdout = String::from_utf8(output.stdout).expect("utf8");
    let mut responses = std::collections::HashMap::new();
    for line in stdout.lines().filter(|l| !l.trim().is_empty()) {
        let value: Value = serde_json::from_str(line).expect("json-rpc line");
        if let Some(id) = value.get("id").and_then(Value::as_i64) {
            responses.insert(id, value);
        }
    }
    responses
}

/// Resolve a `{"$ref": "#/$defs/Name"}` against the root schema's `$defs` /
/// `definitions`; any other schema is returned unchanged.
fn resolve<'a>(schema: &'a Value, root: &'a Value) -> &'a Value {
    let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
        return schema;
    };
    let name = reference
        .strip_prefix("#/$defs/")
        .or_else(|| reference.strip_prefix("#/definitions/"))
        .unwrap_or_else(|| panic!("unsupported $ref `{reference}`"));
    root.get("$defs")
        .or_else(|| root.get("definitions"))
        .and_then(|defs| defs.get(name))
        .unwrap_or_else(|| panic!("unresolved $ref `{reference}`"))
}

/// Does `instance` satisfy a JSON-Schema `type` token?
fn type_matches(token: &str, instance: &Value) -> bool {
    match token {
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "number" => instance.is_number(),
        "integer" => instance.as_i64().is_some() || instance.as_u64().is_some(),
        "boolean" => instance.is_boolean(),
        "null" => instance.is_null(),
        other => panic!("unknown schema type `{other}`"),
    }
}

/// Validate `instance` against `schema` (rooted at `root` for `$ref`). Panics
/// with a descriptive message on the first violation — enough to catch the drift
/// class ADR-0039 guards: a missing required field, an undeclared/renamed field,
/// or a type mismatch.
fn validate(instance: &Value, schema: &Value, root: &Value, path: &str) {
    let schema = resolve(schema, root);

    // `type`, as a single token or a union array.
    match schema.get("type") {
        Some(Value::String(token)) => assert!(
            type_matches(token, instance),
            "{path}: expected type `{token}`, got {instance}"
        ),
        Some(Value::Array(tokens)) => assert!(
            tokens
                .iter()
                .filter_map(Value::as_str)
                .any(|token| type_matches(token, instance)),
            "{path}: expected one of {tokens:?}, got {instance}"
        ),
        _ => {}
    }

    if let Some(properties) = schema.get("properties").and_then(Value::as_object) {
        let object = instance
            .as_object()
            .unwrap_or_else(|| panic!("{path}: expected an object, got {instance}"));

        for required in schema
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            assert!(
                object.contains_key(required),
                "{path}: missing required field `{required}`"
            );
        }

        let extras_allowed = schema
            .get("additionalProperties")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        for (key, sub_instance) in object {
            match properties.get(key) {
                Some(sub_schema) => {
                    validate(sub_instance, sub_schema, root, &format!("{path}.{key}"));
                }
                None => assert!(
                    extras_allowed,
                    "{path}: undeclared field `{key}` (not in the output schema)"
                ),
            }
        }
    }

    if let Some(items) = schema.get("items") {
        let array = instance
            .as_array()
            .unwrap_or_else(|| panic!("{path}: expected an array, got {instance}"));
        for (i, element) in array.iter().enumerate() {
            validate(element, items, root, &format!("{path}[{i}]"));
        }
    }
}

/// Run every case and assert its `structuredContent` validates against the
/// `outputSchema` the same tool advertises in `tools/list`. Shared by the
/// per-family conformance tests.
fn check_conformance(cases: &[Case]) {
    let responses = run(cases);

    let tools = responses[&100]["result"]["tools"]
        .as_array()
        .expect("tools array");
    let schema_for = |name: &str| -> Value {
        tools
            .iter()
            .find(|t| t["name"] == name)
            .and_then(|t| t.get("outputSchema"))
            .cloned()
            .unwrap_or_else(|| panic!("tool `{name}` declares no outputSchema"))
    };

    for (i, case) in cases.iter().enumerate() {
        let id = i64::try_from(i).expect("case index fits i64");
        let response = &responses[&id];
        let content = response["result"]
            .get("structuredContent")
            .unwrap_or_else(|| panic!("`{}` returned no structuredContent", case.tool));
        let schema = schema_for(case.tool);
        validate(content, &schema, &schema, case.tool);
    }
}

/// The series family (`npv`/`nfv`/`irr`/`mirr`/`xnpv`/`xirr`).
#[test]
fn series_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "npv",
            args: json!({"rate":0.01,"cashflows":[-100,60,60],"currency":"USD"}),
        },
        Case {
            tool: "nfv",
            args: json!({"rate":0.01,"cashflows":[-100,60,60]}),
        },
        Case {
            tool: "irr",
            args: json!({"cashflows":[-100,60,60]}),
        },
        Case {
            tool: "mirr",
            args: json!({"finance":0.10,"reinvest":0.12,"cashflows":[-100,60,60]}),
        },
        Case {
            tool: "xnpv",
            args: json!({"rate":0.10,"flows":[{"date":"2020-01-01","amount":-100},{"date":"2021-01-01","amount":110}],"currency":"EUR"}),
        },
        Case {
            tool: "xirr",
            args: json!({"flows":[{"date":"2020-01-01","amount":-100},{"date":"2021-01-01","amount":110}]}),
        },
    ]);
}

/// The single-sum family (present/future value, and the NPER/RATE solves).
#[test]
fn single_sum_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "single_sum_present_value",
            args: json!({"rate":0.01,"periods":12,"future":1000,"currency":"USD"}),
        },
        Case {
            tool: "single_sum_future_value",
            args: json!({"rate":0.01,"periods":12,"present":1000}),
        },
        Case {
            tool: "single_sum_periods",
            args: json!({"rate":0.01,"present":1000,"future":1126.825}),
        },
        Case {
            tool: "single_sum_rate",
            args: json!({"periods":12,"present":1000,"future":1126.825}),
        },
    ]);
}

/// The annuity family: ordinary, the NPER/RATE solves, perpetuities, the
/// annuity-due forms, and the finite growing forms (ADR-0048).
#[test]
fn annuity_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "annuity_present_value",
            args: json!({"rate":0.01,"periods":12,"payment":100,"currency":"USD"}),
        },
        Case {
            tool: "annuity_future_value",
            args: json!({"rate":0.01,"periods":12,"payment":100}),
        },
        Case {
            tool: "annuity_payment",
            args: json!({"rate":0.01,"periods":12,"present":1000,"currency":"GBP"}),
        },
        Case {
            tool: "annuity_periods",
            args: json!({"rate":0.01,"payment":100,"present":1000}),
        },
        Case {
            tool: "annuity_rate",
            args: json!({"periods":12,"payment":100,"present":1000}),
        },
        Case {
            tool: "annuity_perpetuity",
            args: json!({"rate":0.05,"payment":100,"currency":"JPY"}),
        },
        Case {
            tool: "annuity_growing_perpetuity",
            args: json!({"rate":0.05,"growth":0.02,"payment":100}),
        },
        Case {
            tool: "annuity_due_present_value",
            args: json!({"rate":0.01,"periods":12,"payment":100}),
        },
        Case {
            tool: "annuity_due_future_value",
            args: json!({"rate":0.01,"periods":12,"payment":100}),
        },
        Case {
            tool: "annuity_due_payment",
            args: json!({"rate":0.01,"periods":12,"present":1000}),
        },
        Case {
            tool: "annuity_growing_present_value",
            args: json!({"rate":0.05,"growth":0.02,"periods":12,"payment":100,"currency":"EUR"}),
        },
        Case {
            tool: "annuity_growing_future_value",
            args: json!({"rate":0.05,"growth":0.02,"periods":12,"payment":100}),
        },
        Case {
            tool: "annuity_growing_due_present_value",
            args: json!({"rate":0.05,"growth":0.02,"periods":12,"payment":100}),
        },
        Case {
            tool: "annuity_growing_due_future_value",
            args: json!({"rate":0.05,"growth":0.02,"periods":12,"payment":100}),
        },
    ]);
}

/// The rate family: EAR, cross-periodicity conversion, and nominal/APR quotes.
#[test]
fn rate_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "rate_effective_annual",
            args: json!({"rate":0.01,"periodicity":"monthly"}),
        },
        Case {
            tool: "rate_convert",
            args: json!({"rate":0.01,"from":"monthly","to":"quarterly"}),
        },
        Case {
            tool: "rate_from_nominal",
            args: json!({"nominal":0.12,"periodicity":"monthly"}),
        },
        Case {
            tool: "rate_nominal",
            args: json!({"rate":0.01,"periodicity":"monthly"}),
        },
    ]);
}

/// The amortize tool — a tabular result (`{ schedule: [rows], currency? }`),
/// exercising the harness's array + `$ref` resolution over the row DTO.
#[test]
fn amortize_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "amortize",
            args: json!({"rate":0.10,"principal":1000,"payment":500,"currency":"USD"}),
        },
        Case {
            tool: "amortize",
            args: json!({"rate":0.01,"principal":1000,"periods":12}),
        },
    ]);
}

/// The continuous family: future/present value (monetary) and the two
/// effective-annual bridges (scalar rates).
#[test]
fn continuous_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "continuous_future_value",
            args: json!({"rate":0.05,"years":3,"amount":1000,"currency":"USD"}),
        },
        Case {
            tool: "continuous_present_value",
            args: json!({"rate":0.05,"years":3,"amount":1000}),
        },
        Case {
            tool: "continuous_from_effective",
            args: json!({"rate":0.05}),
        },
        Case {
            tool: "continuous_effective",
            args: json!({"rate":0.05}),
        },
    ]);
}

/// The standalone `convert` tool — a monetary result carrying the target
/// currency (the result is never agnostic when `to` is a real code).
#[test]
fn convert_output_conforms_to_declared_schema() {
    check_conformance(&[
        Case {
            tool: "convert",
            args: json!({"amount":100,"from":"USD","to":"EUR","rate":0.9}),
        },
        Case {
            tool: "convert",
            args: json!({"amount":100,"from":"USD","to":"XXX","rate":0.9}),
        },
    ]);
}
