//! The CLI's surface, exercised as a user does: by running the binary.
//!
//! Every assertion here is about the boundary rather than the arithmetic — the
//! library's own tests cover what the numbers should be. What can only break
//! out here is argument parsing, exit codes, and what reaches stdout.
//!
//! No `assert_cmd`: cargo sets `CARGO_BIN_EXE_<name>` for every binary in the
//! package, so locating it needs no dependency at all.

use std::process::Command;

/// Runs the binary and returns `(stdout, stderr, exit code)`.
fn run(args: &[&str]) -> (String, String, i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_time-value"))
        .args(args)
        .output()
        .expect("the binary should run");
    (
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
        String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        output.status.code().expect("the process should exit"),
    )
}

#[test]
fn future_value_prints_the_number_in_full() {
    // `114.99999999999999`, not `115`. The product of 100 and a factor of 1.15
    // is not 115 in binary floating point, and rounding the display would be
    // the CLI inventing precision the library refuses to invent. This is the
    // test that fails if someone "tidies" the output.
    let (stdout, _, code) = run(&[
        "simple",
        "fv",
        "--amount",
        "100",
        "--rate",
        "0.05",
        "--periods",
        "3",
    ]);
    assert_eq!(stdout, "114.99999999999999", "unexpected stdout");
    assert_eq!(code, 0, "should succeed");
}

#[test]
fn a_percentage_and_a_fraction_of_the_same_rate_agree() {
    let fraction = run(&[
        "simple",
        "fv",
        "--amount",
        "100",
        "--rate",
        "0.05",
        "--periods",
        "3",
    ]);
    let percent = run(&[
        "simple",
        "fv",
        "--amount",
        "100",
        "--rate-percent",
        "5",
        "--periods",
        "3",
    ]);
    assert_eq!(fraction.0, percent.0, "5% and 0.05 are the same rate");
}

#[test]
fn the_factor_is_its_own_command() {
    let (stdout, _, code) = run(&["simple", "factor", "--rate", "0.05", "--periods", "3"]);
    assert_eq!(stdout, "1.15", "unexpected stdout");
    assert_eq!(code, 0, "should succeed");
}

#[test]
fn json_keys_the_answer_by_the_operation() {
    let (fv, _, _) = run(&[
        "--json",
        "simple",
        "fv",
        "--amount",
        "100",
        "--rate",
        "0.05",
        "--periods",
        "3",
    ]);
    assert_eq!(fv, r#"{"fv":114.99999999999999}"#, "unexpected stdout");

    let (factor, _, _) = run(&[
        "--json",
        "simple",
        "factor",
        "--rate",
        "0.05",
        "--periods",
        "3",
    ]);
    assert_eq!(factor, r#"{"factor":1.15}"#, "unexpected stdout");
}

#[test]
fn a_negative_rate_is_an_argument_and_not_a_flag() {
    // The trap this exists for: without `allow_hyphen_values`, clap reads
    // `-0.05` as a cluster of short flags and rejects it. Negative rates are
    // legal in this library, so the whole surface would be unusable for them.
    let (stdout, stderr, code) = run(&[
        "simple",
        "fv",
        "--amount",
        "-100",
        "--rate",
        "-0.05",
        "--periods",
        "3",
    ]);
    assert_eq!(stdout, "-85", "unexpected stdout: {stderr}");
    assert_eq!(code, 0, "should succeed: {stderr}");
}

/// A computed value that outgrew the range, which is the *other* class. Exit 3,
/// not 1: nothing about the model is wrong, and the remedy is to rescale — so a
/// shell branching on `$?` gets told which of the two it is.
#[test]
fn a_representation_failure_exits_three_and_names_the_class() {
    let (_, stderr, code) = run(&[
        "simple",
        "fv",
        "--amount",
        "1.7976931348623157e308",
        "--rate",
        "1",
        "--periods",
        "1",
    ]);
    assert!(
        stderr.contains("(representation)"),
        "unexpected stderr: {stderr}"
    );
    assert_eq!(code, 3, "a representation failure exits 3");
}

#[test]
fn a_domain_failure_exits_one_and_says_why() {
    // Each argument is valid alone; the pair is not. The message is the
    // library's own — the CLI paraphrasing it would be a second place to keep
    // the reasoning right.
    let (stdout, stderr, code) = run(&["simple", "factor", "--rate", "-0.5", "--periods", "3"]);
    assert!(stdout.is_empty(), "nothing should reach stdout: {stdout}");
    assert!(
        stderr.contains("not positive"),
        "unexpected stderr: {stderr}"
    );
    assert!(stderr.contains("(domain)"), "unexpected stderr: {stderr}");
    assert_eq!(code, 1, "a domain failure exits 1");
}

#[test]
fn an_invalid_span_is_refused_by_the_library_and_not_by_the_parser() {
    // `--periods -1` parses fine and is refused by `ElapsedPeriods`. Exit 1,
    // not 2: the arguments were well-formed and the model was not.
    let (_, stderr, code) = run(&[
        "simple",
        "fv",
        "--amount",
        "100",
        "--rate",
        "0.05",
        "--periods",
        "-1",
    ]);
    assert!(stderr.contains("negative"), "unexpected stderr: {stderr}");
    assert_eq!(code, 1, "a domain failure exits 1");
}

#[test]
fn giving_both_spellings_of_the_rate_is_refused_by_the_parser() {
    // Half of what makes the impossible arms of `RateArg::resolve` impossible.
    let (_, stderr, code) = run(&[
        "simple",
        "fv",
        "--amount",
        "100",
        "--rate",
        "0.05",
        "--rate-percent",
        "5",
        "--periods",
        "3",
    ]);
    assert!(
        stderr.contains("cannot be used with"),
        "unexpected stderr: {stderr}"
    );
    assert_eq!(code, 2, "a usage error exits 2");
}

#[test]
fn giving_neither_spelling_of_the_rate_is_refused_by_the_parser() {
    // The other half, and the one that matters most: a surface where omitting
    // the rate silently defaulted is how a caller ends up computing at the
    // wrong scale.
    let (_, stderr, code) = run(&["simple", "fv", "--amount", "100", "--periods", "3"]);
    assert!(
        stderr.contains("required arguments were not provided"),
        "unexpected stderr: {stderr}"
    );
    assert_eq!(code, 2, "a usage error exits 2");
}
