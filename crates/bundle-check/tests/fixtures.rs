//! The red side: every invariant, watched failing for its own reason.
//!
//! `bundle.rs` asserts the real tree is clean, which is worth having and proves
//! nothing about the checks themselves — an invariant that stopped examining
//! anything would pass it in silence. The bundle's own rule is that a check is
//! not believed until it has been seen to go red, and until now that had been
//! done once, by hand, against a bundle broken on purpose and then thrown away.
//!
//! Each fixture below is a minimal bundle breaking exactly one invariant, so
//! the assertion is both that the right rule fires *and* that nothing else
//! does. The second half is the one that catches an over-broad check.
//!
//! Six of them fire [`Rule::SpecDefect`], which is one rule covering every
//! conformance failure `okf-graph` finds — so those assert the finding's code
//! as well, or a fixture would only prove that *something* was wrong. Their
//! value is not to test the dependency, which has its own suite: it is to prove
//! the wiring is live, and that each shape this repo cares about still reaches
//! a failing assertion here.
//!
//! The fixtures are `.md` files in the tree, so `prettier` and `typos` run over
//! them like any other. Break the frontmatter, never the markdown: a formatter
//! that decides to repair a fixture would quietly rewrite the thing under test,
//! and a hook does it in place without being asked.

use std::path::{Path, PathBuf};

use bundle_check::{Rule, Violation, check};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// The rules a fixture reports, in order.
fn rules(name: &str) -> Vec<Rule> {
    violations(name).into_iter().map(|v| v.rule).collect()
}

fn violations(name: &str) -> Vec<Violation> {
    check(&fixture(name)).expect("a fixture should be readable")
}

/// Asserts a fixture reports exactly one spec finding, of `rule`, and that it
/// is the `okf-graph` rule `code` names.
#[track_caller]
fn only_spec_finding(name: &str, rule: Rule, code: &str) {
    let found = violations(name);
    assert_eq!(rules(name), [rule], "{name}: {found:?}");
    let detail = found
        .first()
        .map(|violation| violation.detail.as_str())
        .expect("the assertion above found exactly one violation");
    assert!(
        detail.starts_with(code),
        "{name}: expected {code}, got {found:?}"
    );
}

/// The green case the red fixtures are measured against. Without it, a checker
/// that reported everything would pass every test below.
#[test]
fn a_clean_bundle_reports_nothing() {
    assert_eq!(
        rules("clean"),
        [],
        "clean fixture: {:?}",
        violations("clean")
    );
}

/// A `.md` that is not a concept document at all. Reported rather than skipped:
/// a document nobody can read is not a document that passes — and note it
/// leaves the bundle with no concepts without being empty, which is why
/// [`Rule::EmptyBundle`] asks for silence as well as emptiness.
#[test]
fn a_document_with_no_frontmatter_is_unparsable() {
    only_spec_finding("unparsable", Rule::SpecDefect, "CONCEPT-1");
}

/// A bundle of nothing but reserved files holds no concepts, and a check that
/// examines nothing reports success — which is the failure this crate exists to
/// prevent, so the emptiness is itself a violation.
#[test]
fn a_bundle_with_no_concepts_is_a_violation() {
    assert_eq!(rules("reserved-only"), [Rule::EmptyBundle]);
}

/// §11 rule 2, the spec's one required key.
#[test]
fn a_concept_with_no_type_is_reported() {
    only_spec_finding("no-type", Rule::SpecDefect, "CONCEPT-2");
}

/// A status outside the three §5.4 names. The fixture says `provisional` rather
/// than a misspelling of `stable`: a plausible typo is what this rule is for,
/// but writing one into a tracked file would fail the `typos` check that
/// `scripts/check.sh` gained in #145. The code path is the same.
#[test]
fn a_status_the_spec_does_not_define_is_reported() {
    only_spec_finding("invalid-status", Rule::SpecDefect, "CONCEPT-3");
}

/// `status: true` is not a string, and reads as no recognised status at all.
/// Kept as a second fixture beside `invalid-status` even though both now land
/// on `CONCEPT-3`: the local checker used to distinguish them, and the two
/// input shapes are the reason the distinction was written in the first place.
#[test]
fn a_status_that_is_not_a_string_is_reported() {
    only_spec_finding("non-string-status", Rule::SpecDefect, "CONCEPT-3");
}

/// A bare token is neither `<producer>/<version>` nor `<scheme>:<id>`.
#[test]
fn an_actor_matching_no_accepted_form_is_reported() {
    only_spec_finding("malformed-actor", Rule::SpecDefect, "CONCEPT-5");
}

/// `2026-W01-1T00:00:00Z` is twenty characters with separators at 4 and 10 and
/// a trailing `Z`, so the length-and-separators version of this check passed it
/// — and `W` sorts above every digit, so it compared as newer than any calendar
/// date and defeated the staleness check at the same time. It is here because
/// it is the case that actually got through, and it stays here now that the
/// comparison is on parsed instants: what a week date must never do is compare
/// at all.
#[test]
fn a_week_date_timestamp_is_reported() {
    only_spec_finding("week-date-timestamp", Rule::SpecDefect, "CONCEPT-12");
}

/// A body link to a concept nobody wrote. The spec says to tolerate it and
/// `okf-graph` exits zero on it; this repo fails, because a report from a
/// passing test is a report nobody reads.
#[test]
fn a_dangling_link_is_reported_though_the_spec_tolerates_it() {
    only_spec_finding("dangling-link", Rule::SpecReport, "BUNDLE-2");
}

/// The spec requires only `generated.by`; this repo also requires `at`, because
/// staleness is measured against it. The fixture carries the spec's half alone,
/// so it is conformant and still rejected here — which is the deviation stated
/// on the rule, made visible.
#[test]
fn a_generated_block_without_at_is_reported() {
    assert_eq!(rules("missing-generated-at"), [Rule::MissingGenerated]);
}

/// No `generated` family at all, which §4.1 permits outright. Its own fixture
/// because it is a different branch: a *declared* block missing `by` is
/// `okf-graph`'s `CONCEPT-4`, so the `by` half of this rule fires only here,
/// and a branch nothing exercises is a branch nobody has seen work.
#[test]
fn a_concept_with_no_generated_family_is_reported_twice() {
    assert_eq!(
        rules("no-generated"),
        [Rule::MissingGenerated, Rule::MissingGenerated],
        "{:?}",
        violations("no-generated")
    );
}

/// Stable means ready for consumption. Saying so with nobody having confirmed
/// it is the claim this catches.
#[test]
fn a_stable_concept_nobody_verified_is_reported() {
    assert_eq!(rules("stable-unverified"), [Rule::StableUnverified]);
}

/// The invariant the whole crate exists for.
#[test]
fn a_verification_older_than_the_content_is_reported() {
    assert_eq!(rules("stale-verification"), [Rule::StaleVerification]);
}

/// Every rule has a fixture that fires it. A rule added without one is a check
/// nobody has watched fail, which is the state this file exists to end.
///
/// What this does *not* check is that `Rule::ALL` is complete — a variant left
/// out of that list is invisible here, and no test can see what it was never
/// given. `Rule::ALL` says so at its definition, and the compiler puts whoever
/// adds a variant two lines from it. Stating the scope is the point: an
/// unqualified "every rule" is the completeness claim this bundle now has a
/// principle about.
#[test]
fn every_rule_has_a_fixture_that_triggers_it() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let triggered: Vec<Rule> = std::fs::read_dir(&directory)
        .expect("the fixtures directory should be readable")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .filter_map(|path| check(&path).ok())
        .flatten()
        .map(|violation| violation.rule)
        .collect();

    for rule in Rule::ALL {
        assert!(
            triggered.contains(&rule),
            "no fixture triggers {}",
            rule.name()
        );
    }
}
