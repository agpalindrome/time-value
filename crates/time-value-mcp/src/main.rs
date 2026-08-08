//! `time-value-mcp` — a Model Context Protocol server over the [`time_value`]
//! library, speaking MCP over stdio.
//!
//! Third stage of the repo's standing cadence: a feature is modelled and built
//! in the library, exposed in the CLI, then exposed here. Nothing is validated
//! in this crate — every value is built by the library's constructors, so a
//! tool parses JSON, calls one operation and renders the answer.
//!
//! **Async lives only here.** The library is synchronous and the tools call it
//! directly; `tokio` and `rmcp` are this crate's dependencies and not features
//! of `time_value`.

mod params;
mod server;

use std::error::Error;

use rmcp::{ServiceExt, transport::stdio};

use crate::server::TimeValueServer;

/// Serves MCP over stdin/stdout until the client disconnects.
///
/// Nothing is printed: on stdio, stdout **is** the protocol, so a stray line
/// would corrupt the stream rather than inform anybody. Diagnostics would have
/// to go to stderr, and there are none worth sending yet.
#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let service = TimeValueServer::new().serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
