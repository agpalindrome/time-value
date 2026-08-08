//! The knowledge bundle's invariants, asserted against the real tree.
//!
//! These ran as shell one-liners before they ran as tests, and misreported
//! repeatedly — always by producing a plausible answer rather than an error.
//! Every assertion here replaces one of those.
//!
//! The invariants themselves live in `check`, so a deliberately broken fixture
//! can be pointed at them; `fixtures.rs` is that red side, and carries the
//! spec-versus-house-rule reasoning alongside each. What is left here is the
//! green side: one test per rule, named for the rule, so a failure says which
//! invariant broke without anyone reading a list of violations to find out.
//!
//! Two of the rules are `okf-graph`'s verdict rather than this repo's, which is
//! why the bundle's structure and links no longer have a separate step in
//! `scripts/check.sh`: it ran the same checker as a nix-provided binary, and
//! one copy of a check is better than two that can drift.

use std::path::{Path, PathBuf};

use bundle_check::{Rule, Violation, check};

fn bundle_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/bundle-check.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

/// Every violation of `rule` in the real bundle. Empty is the passing state.
fn broken(rule: Rule) -> Vec<Violation> {
    check(&bundle_root())
        .expect("the bundle should be readable")
        .into_iter()
        .filter(|violation| violation.rule == rule)
        .collect()
}

/// Asserts nothing breaks `rule`, printing what did.
///
/// The message is the whole point of the helper: a bare count says a check
/// failed, and the run then has to be repeated by hand to learn what it caught.
#[track_caller]
fn nothing_breaks(rule: Rule) {
    let violations = broken(rule);
    assert!(
        violations.is_empty(),
        "{}:\n{}",
        rule.name(),
        violations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn the_bundle_conforms_to_the_spec() {
    nothing_breaks(Rule::SpecDefect);
}

/// The findings the spec says to tolerate — a dangling link above all. Failing
/// on one is this repo's own choice and the strictest thing here: `okf-graph`
/// exits zero on them, and a passing test prints nothing, so tolerating them
/// silently is how a broken cross-link stays broken.
#[test]
fn nothing_the_spec_merely_tolerates_is_left_unread() {
    nothing_breaks(Rule::SpecReport);
}

#[test]
fn the_bundle_holds_concepts_at_all() {
    // The guard on every test below: each one passes trivially over an empty
    // set, so the emptiness has to be a failure in its own right.
    nothing_breaks(Rule::EmptyBundle);
}

#[test]
fn every_concept_records_who_generated_it_and_when() {
    nothing_breaks(Rule::MissingGenerated);
}

#[test]
fn a_stable_concept_carries_a_verification() {
    nothing_breaks(Rule::StableUnverified);
}

#[test]
fn no_stable_concept_has_changed_since_it_was_verified() {
    nothing_breaks(Rule::StaleVerification);
}
