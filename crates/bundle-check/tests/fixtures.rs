//! The red side: every requirement, watched failing for its own reason.
//!
//! `bundle.rs` asserts the real tree is clean, which is worth having and proves
//! nothing about the checks themselves — a requirement that stopped examining
//! anything would pass it in silence. The bundle's own rule is that a check is
//! not believed until it has been seen to go red, and until #147 that had been
//! done once, by hand, against a bundle broken on purpose and then thrown away.
//!
//! Each fixture below is a minimal bundle breaking exactly one requirement, so
//! the assertion is both that the right code fires *and* that nothing else
//! does. The second half is the one that catches an over-broad check.
//!
//! **Spec codes appear here for two reasons only.** Their red side is
//! `okf-graph`'s own test suite, so re-testing the dependency is not the point:
//! it is to prove the wiring is live, and — for `dangling-link` — that this
//! repo's *denial* of a tolerated finding actually fails a run. That denial is
//! ours, so its red side is ours too.
//!
//! The fixtures are `.md` files in the tree, so `prettier` and `typos` run over
//! them like any other. Break the frontmatter, never the markdown: a formatter
//! that decides to repair a fixture would quietly rewrite the thing under test,
//! and a hook does it in place without being asked.

use std::path::{Path, PathBuf};

use bundle_check::{EMPTY_BUNDLE, HOUSE_CODES, Violation, check, house_checks};
use okf_graph::Level;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

fn violations(name: &str) -> Vec<Violation> {
    check(&fixture(name)).expect("a fixture should be readable")
}

/// The codes a fixture reports, in order.
fn codes(name: &str) -> Vec<String> {
    violations(name)
        .into_iter()
        .map(|violation| violation.code)
        .collect()
}

/// Asserts a fixture reports exactly `expected`, at `level`.
#[track_caller]
fn only(name: &str, expected: &[&str], level: Level) {
    let found = violations(name);
    assert_eq!(codes(name), expected, "{name}: {found:?}");
    for violation in &found {
        assert_eq!(violation.level, level, "{name}: {violation}");
    }
}

/// The green case the red fixtures are measured against. Without it, a checker
/// that reported everything would pass every test below.
#[test]
fn a_clean_bundle_reports_nothing() {
    assert_eq!(
        codes("clean"),
        Vec::<String>::new(),
        "{:?}",
        violations("clean")
    );
}

/// Registering the checks cannot fail for a fixed list of unique literals that
/// avoid the spec's codes — and `house_checks` panics rather than proving it,
/// so this is where the claim is exercised instead of asserted in a doc
/// comment.
#[test]
fn registers_every_house_check() {
    let checks = house_checks();
    assert_eq!(checks.len(), HOUSE_CODES.len() - 1, "TV-0 is not a Check");
    for code in HOUSE_CODES {
        if code != EMPTY_BUNDLE {
            assert!(checks.contains(code), "{code} is not registered");
        }
    }
}

/// A `.md` that is not a concept document at all — and note it leaves the
/// bundle with no concepts without being empty, which is why the empty-bundle
/// guard asks for silence as well as emptiness.
#[test]
fn a_document_with_no_frontmatter_is_unparsable() {
    only("unparsable", &["CONCEPT-1"], Level::Defect);
}

/// A bundle of nothing but reserved files holds no concepts, and a check that
/// examines nothing reports success — which is the failure this crate exists to
/// prevent, so the emptiness is itself a violation.
#[test]
fn a_bundle_with_no_concepts_is_a_violation() {
    only("reserved-only", &[EMPTY_BUNDLE], Level::Defect);
}

/// §11 rule 2, the spec's one required key.
#[test]
fn a_concept_with_no_type_is_reported() {
    only("no-type", &["CONCEPT-2"], Level::Defect);
}

/// A status outside the three §5.4 names. The fixture says `provisional` rather
/// than a misspelling of `stable`: a plausible typo is what this rule is for,
/// but writing one into a tracked file would fail the `typos` check that
/// `scripts/check.sh` gained in #145. The code path is the same.
#[test]
fn a_status_the_spec_does_not_define_is_reported() {
    only("invalid-status", &["CONCEPT-3"], Level::Defect);
}

/// `status: true` is not a string, and reads as no recognised status at all.
/// Kept beside `invalid-status` even though both land on `CONCEPT-3`: two input
/// shapes, one rule, and `Frontmatter::scalar` is now public if the distinction
/// is ever worth drawing again.
#[test]
fn a_status_that_is_not_a_string_is_reported() {
    only("non-string-status", &["CONCEPT-3"], Level::Defect);
}

/// A bare token is neither `<producer>/<version>` nor `<scheme>:<id>`.
#[test]
fn an_actor_matching_no_accepted_form_is_reported() {
    only("malformed-actor", &["CONCEPT-5"], Level::Defect);
}

/// `2026-W01-1T00:00:00Z` is twenty characters with separators at 4 and 10 and
/// a trailing `Z`, so the length-and-separators check this crate once used
/// passed it — and `W` sorts above every digit, so it compared as newer than
/// any calendar date and defeated the staleness check at the same time. It
/// stays now that the comparison is on parsed instants: what a week date must
/// never do is compare at all.
#[test]
fn a_week_date_timestamp_is_reported() {
    only("week-date-timestamp", &["CONCEPT-12"], Level::Defect);
}

/// The denial, made visible. `okf-graph` calls a dangling link a tolerated
/// report and exits zero on it; here it is a **defect**, because the link is
/// one this repo wrote. That is the principle's rule with a fixture behind it
/// rather than only a table.
#[test]
fn a_dangling_link_is_a_defect_because_the_link_is_ours() {
    only("dangling-link", &["BUNDLE-2"], Level::Defect);
}

/// The spec requires only `generated.by`; this repo also requires `at`, because
/// staleness is measured against it. The fixture carries the spec's half alone,
/// so it is conformant and still rejected here — the deviation made visible.
#[test]
fn a_generated_block_without_at_is_reported() {
    only("missing-generated-at", &["TV-2"], Level::Defect);
}

/// No `generated` family at all, which §4.1 permits outright. Both house halves
/// fire and the spec's `CONCEPT-4` does not, because that one keys off the
/// family being declared — which is what keeps the two from saying the same
/// thing twice.
#[test]
fn a_concept_with_no_generated_family_reports_both_halves() {
    only("no-generated", &["TV-1", "TV-2"], Level::Defect);
}

/// Stable means ready for consumption. Saying so with nobody having confirmed
/// it is the claim this catches.
#[test]
fn a_stable_concept_nobody_verified_is_reported() {
    only("stable-unverified", &["TV-3"], Level::Defect);
}

/// The requirement the whole crate exists for.
#[test]
fn a_verification_older_than_the_content_is_reported() {
    only("stale-verification", &["TV-4"], Level::Defect);
}

/// Every code this crate reports on its own authority has a fixture that fires
/// it. One added without one is a check nobody has watched fail, which is the
/// state this file exists to end.
///
/// What this does *not* check is that `HOUSE_CODES` is complete — a code left
/// out of that list is invisible here, and no test can see what it was never
/// given. `HOUSE_CODES` says so at its definition. Stating the scope is the
/// point: an unqualified "every rule" is the completeness claim this bundle now
/// has a principle about.
#[test]
fn every_house_code_has_a_fixture_that_triggers_it() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let triggered: Vec<String> = std::fs::read_dir(&directory)
        .expect("the fixtures directory should be readable")
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .filter_map(|path| check(&path).ok())
        .flatten()
        .map(|violation| violation.code)
        .collect();

    for code in HOUSE_CODES {
        assert!(
            triggered.iter().any(|fired| fired == code),
            "no fixture triggers {code}"
        );
    }
}
