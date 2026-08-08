//! `time-value` — the command-line interface for the [`time_value`] library.
//!
//! A thin calculator, and thin is the design rather than a stage it will grow
//! out of: every value is validated by the library's constructors, so this
//! crate parses text, calls one operation, and renders the answer. Nothing here
//! decides what a valid rate or span is.
//!
//! Commands are grouped noun-then-verb — `simple fv`, `simple factor` — because
//! the noun is the interest model. Compound interest, when it arrives, is a
//! sibling group rather than a flag on these verbs.
//!
//! **A rate is never inferred.** `--rate` is always a fraction and
//! `--rate-percent` is always a percentage; one is required and they conflict.
//! The library draws the same distinction in its two constructors, and a
//! surface that guessed from magnitude is how an earlier version of this repo
//! computed at 500% for a caller who wrote `5` and meant five percent.

use std::process::ExitCode;

use clap::{Args, Parser, Subcommand};
use time_value::{
    Amount, ElapsedPeriods, Kind, SimpleAccumulationFactor, SimpleInterestRate, future_value,
};

/// Type-safe time-value-of-money calculations.
#[derive(Debug, Parser)]
#[command(name = "time-value", version, about)]
struct Cli {
    /// Print a JSON object keyed by the operation instead of a bare number.
    #[arg(long, global = true)]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Simple interest: the operations that accumulate by `1 + rt`.
    Simple {
        #[command(subcommand)]
        command: SimpleCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SimpleCommand {
    /// Future value: `FV = PV(1 + rt)`.
    Fv {
        /// The present value. May be negative — a liability accumulates into a
        /// larger liability.
        #[arg(long, allow_hyphen_values = true)]
        amount: f64,

        #[command(flatten)]
        rate: RateArg,

        #[command(flatten)]
        periods: PeriodsArg,
    },

    /// The accumulation factor `1 + rt`, on its own.
    ///
    /// Its own command because the two failures separate onto the two steps:
    /// building the factor can fail on the pair `(r, t)`, applying it can only
    /// overflow. A caller who wants to know which occurred asks for the factor.
    Factor {
        #[command(flatten)]
        rate: RateArg,

        #[command(flatten)]
        periods: PeriodsArg,
    },
}

/// A rate, as a fraction or as a percentage, never inferred from magnitude.
///
/// `required = true, multiple = false` is the whole rule, enforced by the
/// parser: exactly one of the two reaches [`RateArg::resolve`]. See the note
/// there about the arms that cannot happen.
#[derive(Debug, Args)]
#[group(required = true, multiple = false)]
struct RateArg {
    /// The rate as a fraction, per period: `0.05` is five percent.
    #[arg(long, allow_hyphen_values = true)]
    rate: Option<f64>,

    /// The rate as a percentage, per period: `5` is five percent.
    #[arg(long, allow_hyphen_values = true)]
    rate_percent: Option<f64>,
}

/// A span, counted in the same period the rate is quoted per.
///
/// Its own type only so the doc comment lives in one place; the formula is
/// agnostic to what the period is, and the CLI does not name one either.
#[derive(Debug, Args)]
struct PeriodsArg {
    /// Elapsed periods, in the same period the rate is quoted per.
    #[arg(long, allow_hyphen_values = true)]
    periods: f64,
}

/// Why the run failed, and what a caller does about it.
#[derive(Debug)]
enum Failure {
    /// The library refused the inputs, or could not represent the answer. Its
    /// own message already says which, and says it better than a paraphrase
    /// would.
    Library(time_value::Error),

    /// Arguments the parser is expected to have rejected already. Reachable
    /// only if `clap`'s group stops holding — see [`RateArg::resolve`].
    Usage(&'static str),
}

impl RateArg {
    /// The rate this pair of flags names.
    ///
    /// The impossible arms return a usage failure rather than panicking. Two
    /// reasons: `panic` and `unreachable` are denied workspace-wide, and a
    /// panic would be a second, weaker statement of a rule the `[group]`
    /// attribute already makes. `tests/cli.rs` pins both shapes being
    /// rejected by the parser, so the claim that these arms are dead is
    /// tested from outside rather than asserted here.
    fn resolve(&self) -> Result<SimpleInterestRate, Failure> {
        match (self.rate, self.rate_percent) {
            (Some(fraction), None) => SimpleInterestRate::from_fraction(fraction),
            (None, Some(percent)) => SimpleInterestRate::from_percent(percent),
            (Some(_), Some(_)) | (None, None) => {
                return Err(Failure::Usage(
                    "exactly one of --rate or --rate-percent is required",
                ));
            }
        }
        .map_err(Failure::Library)
    }
}

/// What to print: the number, and the key it takes under `--json`.
struct Answer {
    key: &'static str,
    value: f64,
}

fn run(cli: &Cli) -> Result<Answer, Failure> {
    let Command::Simple { command } = &cli.command;
    match command {
        SimpleCommand::Fv {
            amount,
            rate,
            periods,
        } => {
            let value = future_value(
                Amount::new(*amount).map_err(Failure::Library)?,
                rate.resolve()?,
                ElapsedPeriods::new(periods.periods).map_err(Failure::Library)?,
            )
            .map_err(Failure::Library)?;
            Ok(Answer {
                key: "fv",
                value: value.magnitude(),
            })
        }
        SimpleCommand::Factor { rate, periods } => {
            let factor = SimpleAccumulationFactor::new(
                rate.resolve()?,
                ElapsedPeriods::new(periods.periods).map_err(Failure::Library)?,
            )
            .map_err(Failure::Library)?;
            Ok(Answer {
                key: "factor",
                value: factor.value(),
            })
        }
    }
}

/// Prints the answer, bare or as JSON.
///
/// **The number is written in full, not rounded.** `100` at five percent over
/// three periods is `114.99999999999999`, and printing `115` would be this
/// crate inventing precision the library refuses to invent — it has a
/// `Tolerance` type and an `is_close` because that gap is real. Written this
/// way the output also round-trips: it is exactly what `Amount`'s `FromStr`
/// reads back.
///
/// Emitting JSON by hand rather than through `serde_json` is deliberate while
/// the shape is one key and one number, both finite by construction, so there
/// is nothing to escape and no NaN to render unrepresentably. The first
/// structured result — a schedule, a row, anything with a nested value — is
/// when the dependency is earned.
#[expect(
    clippy::print_stdout,
    reason = "a calculator's answer belongs on stdout; the lint guards libraries"
)]
fn print_answer(answer: &Answer, json: bool) {
    if json {
        println!("{{\"{}\":{}}}", answer.key, answer.value);
    } else {
        println!("{}", answer.value);
    }
}

/// Explains the failure, naming the class where the library knows one.
///
/// The word is the library's own [`Kind`], not a paraphrase: a reader deciding
/// what to do next is choosing between changing the model and rescaling it, and
/// that is exactly what the class says.
#[expect(
    clippy::print_stderr,
    reason = "a failed run explains itself on stderr; the lint guards libraries"
)]
fn report(failure: &Failure) {
    match failure {
        Failure::Library(error) => eprintln!("error ({}): {error}", error.kind()),
        Failure::Usage(message) => eprintln!("error: {message}"),
    }
}

/// The exit code for a failure: what would fix it, in one number.
fn code(failure: &Failure) -> ExitCode {
    match failure {
        Failure::Library(error) => match error.kind() {
            Kind::Domain => ExitCode::from(1),
            Kind::Representation => ExitCode::from(3),
        },
        // Not reachable through `clap`, which exits 2 itself before this runs.
        // Kept consistent with that rather than inventing a fourth code.
        Failure::Usage(_) => ExitCode::from(2),
    }
}

/// Exit `0` on an answer, `2` on a usage error — from `clap`, before anything
/// is computed — and, on a failed run, a code naming **what would fix it**: `1`
/// for a domain failure, `3` for a representation one.
///
/// The split is the point rather than a nicety. Those two prescribe opposite
/// actions — change the model, or rescale it — so a shell handed one code for
/// both learned only that something went wrong.
///
/// It became possible when `Error::kind` landed. Until then this crate could
/// only match a `#[non_exhaustive]` enum and guess where a future variant
/// belonged, so it deliberately reported `1` for everything and said so.
fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(&cli) {
        Ok(answer) => {
            print_answer(&answer, cli.json);
            ExitCode::SUCCESS
        }
        Err(failure) => {
            report(&failure);
            code(&failure)
        }
    }
}
