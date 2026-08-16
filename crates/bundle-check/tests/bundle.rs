//! The real bundle, held to everything this repo asks of it.
//!
//! These ran as shell one-liners before they ran as tests, and misreported
//! repeatedly — always by producing a plausible answer rather than an error.
//! Every assertion here replaces one of those.
//!
//! The requirements themselves live in `check`, so a deliberately broken
//! fixture can be pointed at them; `fixtures.rs` is that red side. What is left
//! here is the green side: one test per code, named for what it asks, so a
//! failure says which requirement broke without anyone reading a list of
//! violations to find out.

use std::path::{Path, PathBuf};

use bundle_check::{DENIED, EMPTY_BUNDLE, TOLERATED, Violation, check, house_checks, policy};
use okf_graph::{Level, Rule, RuleId, Severity};

fn bundle_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/bundle-check.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

fn violations() -> Vec<Violation> {
    check(&bundle_root()).expect("the bundle should be readable")
}

/// Asserts nothing reports `code`, printing what did.
///
/// The message is the whole point of the helper: a bare count says a check
/// failed, and the run then has to be repeated by hand to learn what it caught.
#[track_caller]
fn nothing_reports(code: &str) {
    let found: Vec<Violation> = violations()
        .into_iter()
        .filter(|violation| violation.code == code)
        .collect();
    assert!(
        found.is_empty(),
        "{code}:\n{}",
        found
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Everything at once, whatever its code. The per-code tests below say *which*
/// requirement broke; this one is what makes a rule added without a test of its
/// own still gate the bundle.
#[test]
fn the_bundle_has_no_defects() {
    let defects: Vec<Violation> = violations()
        .into_iter()
        .filter(|violation| violation.level == Level::Defect)
        .collect();
    assert!(
        defects.is_empty(),
        "{}",
        defects
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_bundle_holds_concepts_at_all() {
    // The guard on every test below: each one passes trivially over an empty
    // set, so the emptiness has to be a failure in its own right.
    nothing_reports(EMPTY_BUNDLE);
}

#[test]
fn every_concept_declares_a_generated_family() {
    nothing_reports("TV-1");
}

#[test]
fn every_concept_records_when_it_was_generated() {
    nothing_reports("TV-2");
}

#[test]
fn a_stable_concept_carries_a_verification() {
    nothing_reports("TV-3");
}

#[test]
fn no_stable_concept_has_changed_since_it_was_verified() {
    nothing_reports("TV-4");
}

/// The policy the bundle decided, asserted rather than described.
///
/// [we are this bundle's producer, not its consumer] fixes which of the spec's
/// tolerated findings this repo treats as defects, on the test that they are
/// about material it wrote. Without this the table in that concept would be
/// prose beside code that could drift from it silently — which is exactly the
/// shape the bundle has a principle about.
///
/// [we are this bundle's producer, not its consumer]: ../../../../knowledge/principles/producer-not-consumer.md
#[test]
fn the_policy_matches_the_principle() {
    let policy = policy(&house_checks());
    let level = |rule: Rule| policy.level(&RuleId::Spec(rule));

    for ours in DENIED {
        assert_eq!(
            level(ours),
            Level::Defect,
            "{} is about material this repo wrote",
            ours.code()
        );
    }

    // Not ours to fix, and therefore not defects: a tool's vintage, a question
    // parked upstream, and two surfaces this bundle does not use.
    for theirs in TOLERATED {
        assert_eq!(
            level(theirs),
            Level::Report,
            "{} is not this repo's to fix",
            theirs.code()
        );
    }
}

/// Every tolerated rule the checker has is one this bundle placed.
///
/// The test above pins the two lists against the policy, and until 2026-08-16
/// that was the whole of it — which pins *agreement* and not *coverage*. A rule
/// added upstream was in neither list, contradicted neither assertion, and took
/// the spec's default in silence; `CONCEPT-15` did exactly that for three
/// releases. Only a rule the spec already calls a defect needs no decision
/// here, because it fails whatever this repo thinks.
///
/// This is the failing direction that looks like success, which is the reason
/// [we are this bundle's producer, not its consumer] exists at all.
///
/// [we are this bundle's producer, not its consumer]: ../../../../knowledge/principles/producer-not-consumer.md
#[test]
fn every_tolerated_rule_has_been_placed() {
    let placed: Vec<Rule> = DENIED.into_iter().chain(TOLERATED).collect();

    for rule in Rule::ALL {
        if rule.severity() != Severity::Report {
            continue;
        }
        assert!(
            placed.contains(rule),
            "{} is tolerated by the spec and placed by neither list — okf-graph \
             has a rule this bundle has not decided about, so it is running at \
             the spec's default rather than at a level anybody chose",
            rule.code()
        );
    }

    for rule in placed {
        assert_eq!(
            rule.severity(),
            Severity::Report,
            "{} is a defect to the spec, so placing it here claims a decision \
             that is not this repo's to make",
            rule.code()
        );
    }
}
