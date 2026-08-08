//! The tools, and how a failure reaches the caller.

use rmcp::{
    ErrorData, Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{Implementation, ProtocolVersion, ServerCapabilities, ServerInfo},
    serde_json::json,
    tool, tool_handler, tool_router,
};
use time_value::{
    Amount, ElapsedPeriods, SimpleAccumulationFactor, SimpleInterestRate, future_value,
};

use crate::params::{
    AccumulationFactorRequest, AccumulationFactorResult, FutureValueRequest, FutureValueResult,
    Rate,
};

/// What every tool tells a caller about the conventions it works under.
///
/// It goes in the server's instructions because that is the one place an agent
/// reliably reads, and because three of these were silent in an earlier version
/// of this surface: what a rate means, what a period is, and that a sign is
/// meaningful.
const INSTRUCTIONS: &str = "\
Time-value-of-money calculations under simple interest.

A rate is never a bare number: give `{\"fraction\": 0.05}` or `{\"percent\": 5}`, \
which are the same rate. There is no default and no inference from magnitude.

`periods` counts the same period the rate is quoted per — the formula never asks \
what that period is, so a monthly rate needs a count of months. Naming a period \
is the caller's job, not this server's.

Signs are meaningful. A negative amount is a liability and accumulates into a \
larger one; a negative rate shrinks an amount. Neither is an error.

A failure names what would fix it. `domain` means the inputs were jointly \
meaningless, so change the model; `representation` means the arithmetic left what \
a 64-bit float holds, so rescale. The two prescribe opposite actions.";

/// Turns a library failure into a tool error that says what would fix it.
///
/// The class comes from `Error::kind`, so this server reports the distinction
/// rather than deciding it — and puts it in the error's `data`, where an agent
/// can branch on it instead of matching on prose.
fn failure(error: &time_value::Error) -> ErrorData {
    ErrorData::invalid_params(
        error.to_string(),
        Some(json!({ "kind": error.kind().to_string() })),
    )
}

impl Rate {
    /// The library's rate, built by whichever constructor the caller named.
    fn resolve(self) -> Result<SimpleInterestRate, ErrorData> {
        match self {
            Self::Fraction(fraction) => SimpleInterestRate::from_fraction(fraction),
            Self::Percent(percent) => SimpleInterestRate::from_percent(percent),
        }
        .map_err(|error| failure(&error))
    }
}

/// The server. Stateless: every tool is a pure function of its arguments, which
/// is what `read_only_hint` and `idempotent_hint` advertise below.
#[derive(Debug, Clone)]
pub(crate) struct TimeValueServer {
    tool_router: ToolRouter<Self>,
}

impl Default for TimeValueServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(router = tool_router)]
impl TimeValueServer {
    /// A server with its tools registered.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }

    /// The value of a present amount after a span of simple interest:
    /// `FV = PV(1 + rt)`.
    #[tool(
        name = "simple_future_value",
        annotations(
            title = "Simple interest future value",
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn simple_future_value(
        &self,
        Parameters(request): Parameters<FutureValueRequest>,
    ) -> Result<Json<FutureValueResult>, ErrorData> {
        let value = future_value(
            Amount::new(request.amount).map_err(|error| failure(&error))?,
            request.rate.resolve()?,
            ElapsedPeriods::new(request.periods).map_err(|error| failure(&error))?,
        )
        .map_err(|error| failure(&error))?;
        Ok(Json(FutureValueResult {
            future_value: value.magnitude(),
        }))
    }

    /// The multiplier simple interest applies over a span: `1 + rt`. Ask for it
    /// when you need to know whether the rate and span are valid together,
    /// separately from whether the product fits.
    //
    // Its own tool because the two failures separate onto the two steps: building
    // the factor can fail on the pair `(rate, periods)`, applying it can only
    // leave the range.
    #[tool(
        name = "simple_accumulation_factor",
        annotations(
            title = "Simple interest accumulation factor",
            read_only_hint = true,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn simple_accumulation_factor(
        &self,
        Parameters(request): Parameters<AccumulationFactorRequest>,
    ) -> Result<Json<AccumulationFactorResult>, ErrorData> {
        let factor = SimpleAccumulationFactor::new(
            request.rate.resolve()?,
            ElapsedPeriods::new(request.periods).map_err(|error| failure(&error))?,
        )
        .map_err(|error| failure(&error))?;
        Ok(Json(AccumulationFactorResult {
            accumulation_factor: factor.value(),
        }))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for TimeValueServer {
    fn get_info(&self) -> ServerInfo {
        // Built field by field rather than with a struct literal: `ServerInfo`
        // is `#[non_exhaustive]`, so a literal cannot name it and a future field
        // arrives as a default rather than as a compile error.
        let mut info = ServerInfo::default();
        info.protocol_version = ProtocolVersion::LATEST;
        info.capabilities = ServerCapabilities::builder().enable_tools().build();
        info.server_info = Implementation::from_build_env();
        info.instructions = Some(INSTRUCTIONS.to_owned());
        info
    }
}
