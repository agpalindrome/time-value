//! The server, exercised the way a client uses it: JSON-RPC over the real
//! binary's stdin and stdout.
//!
//! Nothing here re-tests the arithmetic — the library owns that. What can only
//! break out here is the protocol surface: the schema a client downloads, the
//! annotations it reads, whether a malformed argument is refused, and whether a
//! failure carries the class an agent would branch on.
//!
//! No MCP client library: a handful of framed JSON lines is the whole protocol
//! this needs, and `CARGO_BIN_EXE_<name>` locates the binary without a
//! dependency.
#![expect(
    clippy::indexing_slicing,
    reason = "read-only Index on serde_json::Value returns Null rather than \
              panicking — read from serde_json 1.0.151's value/index.rs, where \
              the panics are in IndexMut only. Chained .get() would make a \
              wire-shape assertion unreadable for no safety gained."
)]

use std::{
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
};

use serde_json::{Value, json};

/// A server process, and a line-buffered reader over its stdout.
struct Server {
    child: Child,
}

impl Server {
    fn start() -> Self {
        let child = Command::new(env!("CARGO_BIN_EXE_time-value-mcp"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the server should start");
        Self { child }
    }

    /// Sends the initialize handshake and then `requests`, returning one parsed
    /// response per request that carried an id.
    ///
    /// Everything goes in one write and stdin is then closed, which is what
    /// makes this deterministic: the server reads to EOF and exits rather
    /// than waiting on a client that has nothing more to say.
    fn exchange(mut self, requests: &[Value]) -> Vec<Value> {
        let mut script = vec![
            json!({
                "jsonrpc": "2.0", "id": 0, "method": "initialize",
                "params": {
                    "protocolVersion": "2025-06-18",
                    "capabilities": {},
                    "clientInfo": { "name": "time-value-mcp tests", "version": "0" }
                }
            }),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        ];
        script.extend(requests.iter().cloned());

        let mut stdin = self.child.stdin.take().expect("stdin was piped");
        for message in &script {
            writeln!(stdin, "{message}").expect("the server should accept a line");
        }
        drop(stdin);

        let stdout = self.child.stdout.take().expect("stdout was piped");
        let responses: Vec<Value> = BufReader::new(stdout)
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
            .filter(|value| value.get("id").is_some_and(|id| id != 0))
            .collect();
        self.child.wait().expect("the server should exit");
        responses
    }
}

/// The one response to a single request.
fn call(tool: &str, arguments: &Value) -> Value {
    let responses = Server::start().exchange(&[json!({
        "jsonrpc": "2.0", "id": 1, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments.clone() }
    })]);
    responses
        .into_iter()
        .next()
        .expect("one request, one response")
}

fn tools() -> Vec<Value> {
    let responses =
        Server::start().exchange(&[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})]);
    let listing = responses.first().expect("one response").clone();
    listing["result"]["tools"]
        .as_array()
        .expect("a tools array")
        .clone()
}

/// The text a failed tool call carries, whether it failed as a protocol error
/// or as an error-flagged result. MCP allows both shapes and this crate
/// produces one of each — a refused argument is a result, a refused value is an
/// error.
fn failure_text(response: &Value) -> String {
    if let Some(error) = response.get("error") {
        return format!(
            "{} {}",
            error["message"],
            error.get("data").unwrap_or(&Value::Null)
        );
    }
    response["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn both_spellings_of_a_rate_give_the_same_answer() {
    let fraction = call(
        "simple_future_value",
        &json!({"amount": 100, "rate": {"fraction": 0.05}, "periods": 3}),
    );
    let percent = call(
        "simple_future_value",
        &json!({"amount": 100, "rate": {"percent": 5}, "periods": 3}),
    );
    assert_eq!(
        fraction["result"]["structuredContent"], percent["result"]["structuredContent"],
        "5% and 0.05 are the same rate: {fraction:?} vs {percent:?}"
    );
    assert_eq!(
        fraction["result"]["structuredContent"]["future_value"],
        json!(114.999_999_999_999_99),
        "the answer is not rounded: {fraction:?}"
    );
}

#[test]
fn the_factor_is_its_own_tool() {
    let response = call(
        "simple_accumulation_factor",
        &json!({"rate": {"fraction": 0.05}, "periods": 3}),
    );
    assert_eq!(
        response["result"]["structuredContent"]["accumulation_factor"],
        json!(1.15),
        "{response:?}"
    );
}

#[test]
fn a_bare_number_is_not_a_rate() {
    // The failure this surface exists to prevent: an earlier version accepted `5`
    // for five percent and computed at 500%.
    let response = call(
        "simple_future_value",
        &json!({"amount": 100, "rate": 5, "periods": 3}),
    );
    let text = failure_text(&response);
    assert!(text.contains("invalid type"), "unexpected failure: {text}");
}

#[test]
fn naming_a_rate_twice_is_refused() {
    let response = call(
        "simple_future_value",
        &json!({"amount": 100, "rate": {"fraction": 0.05, "percent": 5}, "periods": 3}),
    );
    let text = failure_text(&response);
    assert!(text.contains("single key"), "unexpected failure: {text}");
}

#[test]
fn an_unknown_field_is_refused_rather_than_dropped() {
    // Silently dropping it is how a misspelled argument computes a confident wrong
    // answer, which is what `deny_unknown_fields` is for.
    let response = call(
        "simple_future_value",
        &json!({"amount": 100, "rate": {"fraction": 0.05}, "periods": 3, "period": 4}),
    );
    let text = failure_text(&response);
    assert!(
        text.contains("unknown field `period`"),
        "unexpected failure: {text}"
    );
}

#[test]
fn a_domain_failure_says_so_in_data() {
    let response = call(
        "simple_accumulation_factor",
        &json!({"rate": {"fraction": -0.5}, "periods": 3}),
    );
    assert_eq!(
        response["error"]["data"]["kind"],
        json!("domain"),
        "{response:?}"
    );
}

#[test]
fn a_representation_failure_says_so_in_data() {
    // The other class, and the reason the field is worth carrying: an agent's next
    // move differs, so it should not have to read prose to tell them apart.
    let response = call(
        "simple_future_value",
        &json!({"amount": 1.797_693_134_862_315_7e308, "rate": {"fraction": 1}, "periods": 1}),
    );
    assert_eq!(
        response["error"]["data"]["kind"],
        json!("representation"),
        "{response:?}"
    );
}

#[test]
fn every_tool_declares_itself_read_only_and_idempotent() {
    // Each is a pure function, and saying so is free. An earlier version of this
    // surface shipped 45 tools with no annotations at all.
    let tools = tools();
    assert_eq!(tools.len(), 2, "{tools:?}");
    for tool in &tools {
        let annotations = &tool["annotations"];
        assert_eq!(annotations["readOnlyHint"], json!(true), "{tool:?}");
        assert_eq!(annotations["idempotentHint"], json!(true), "{tool:?}");
        assert_eq!(annotations["openWorldHint"], json!(false), "{tool:?}");
    }
}

#[test]
fn the_rate_schema_expresses_the_choice_as_a_one_of() {
    // Mutual exclusivity encoded rather than described. An earlier version stated
    // it in prose across 45 schemas and used `oneOf` in none of them.
    let tools = tools();
    for tool in &tools {
        let rate = &tool["inputSchema"]["$defs"]["Rate"];
        assert!(
            rate["oneOf"].is_array(),
            "{}: {rate:?}",
            tool["name"].as_str().unwrap_or_default()
        );
    }
}

#[test]
fn the_listing_stays_small_enough_to_read() {
    // A client downloads this on every connection. 45 tools once cost 103 KB, 41%
    // of it one enum repeated — so the budget is asserted while it is cheap to
    // hold, not after it is expensive to fix.
    let listing = serde_json::to_string(&tools()).expect("the listing serialises");
    assert!(
        listing.len() < 4_096,
        "the two-tool listing is {} bytes; trim the schema-visible doc comments \
         rather than raising this",
        listing.len()
    );
}
