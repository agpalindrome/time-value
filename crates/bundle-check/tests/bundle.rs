//! The knowledge bundle's invariants, asserted against the real tree.
//!
//! These ran as shell one-liners before they ran as tests, and misreported
//! repeatedly — always by producing a plausible answer rather than an error.
//! Every assertion here replaces one of those.

use std::path::{Path, PathBuf};

use bundle_check::{actor_is_well_formed, concept_documents, parse};

fn bundle_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/bundle-check.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("knowledge")
}

// The `allow-panic-in-tests` exemption keys on the `#[test]` attribute, so a
// helper in a test file is not covered by it. Failing loudly is the point: a
// document that will not parse must stop the run, not be skipped into silence.
#[expect(
    clippy::panic,
    reason = "an unparsable concept must fail the run, and this helper is not a #[test] fn"
)]
fn concepts() -> Vec<(String, bundle_check::Frontmatter)> {
    let documents = concept_documents(&bundle_root()).expect("the bundle should be readable");
    assert!(
        !documents.is_empty(),
        "no concepts found — a check that examines nothing reports success"
    );
    documents
        .into_iter()
        .map(|(path, text)| {
            let front = parse(&text).unwrap_or_else(|e| panic!("{path}: {e:?}"));
            (path, front)
        })
        .collect()
}

#[test]
fn every_concept_declares_a_type() {
    // The spec's one required key, and the whole of its conformance rule 2.
    for (path, front) in concepts() {
        let declared = front.concept_type.unwrap_or_default();
        assert!(!declared.trim().is_empty(), "{path}: no `type`");
    }
}

#[test]
fn every_status_is_one_the_spec_defines() {
    // Absent is legal and means stable. A typo is not, and would otherwise be
    // read as an unknown value rather than rejected.
    for (path, front) in concepts() {
        if !front.has_status_key {
            continue;
        }
        // A `status` that is present but not a string — `status: true` — read
        // as absent, so this test skipped it entirely and passed vacuously.
        let status = front
            .status
            .unwrap_or_else(|| panic!("{path}: `status` is present but not a string"));
        assert!(
            matches!(status.as_str(), "draft" | "stable" | "deprecated"),
            "{path}: status `{status}` is not draft, stable or deprecated"
        );
    }
}

#[test]
fn every_actor_is_well_formed() {
    for (path, front) in concepts() {
        for actor in front.actors {
            assert!(
                actor_is_well_formed(&actor),
                "{path}: actor `{actor}` matches no accepted form"
            );
        }
    }
}

#[test]
fn every_concept_records_who_generated_it_and_when() {
    for (path, front) in concepts() {
        assert!(front.generated_by.is_some(), "{path}: no `generated.by`");
        assert!(front.generated_at.is_some(), "{path}: no `generated.at`");
    }
}

#[test]
fn a_stable_concept_carries_a_verification() {
    // Stable means ready for consumption. Saying so without anyone having
    // confirmed it is the claim this catches.
    for (path, front) in concepts() {
        if front.status.as_deref().unwrap_or("stable") == "stable" {
            assert!(
                !front.verified_at.is_empty(),
                "{path}: stable, but nobody has verified it"
            );
        }
    }
}

#[test]
fn no_stable_concept_has_changed_since_it_was_verified() {
    // The invariant this whole crate exists for. `generated.at` moves whenever
    // a concept changes materially, so a newest verification that predates it
    // means the concept says something nobody has read.
    //
    // Draft and deprecated are both exempt, and the code below says `!= stable`
    // rather than `== draft` deliberately. A concept being worked on is
    // expected to sit unverified, and a deprecated one is "kept for links and
    // history; no longer current" in the spec's words — staleness against a
    // verification is not a question worth asking of either.
    //
    // The timestamps are ISO 8601 with a fixed `Z` offset throughout, so
    // comparing them as strings is comparing them chronologically. Anything
    // else here would need a date library; the assertion below pins the format
    // so this stays true.
    for (path, front) in concepts() {
        if front.status.as_deref().unwrap_or("stable") != "stable" {
            continue;
        }
        let generated = front.generated_at.expect("checked above");
        let newest = front
            .verified_at
            .iter()
            .max()
            .expect("checked above")
            .clone();
        assert!(
            newest >= generated,
            "{path}: changed at {generated}, last verified {newest} — stale"
        );
    }
}

#[test]
fn every_timestamp_is_the_format_the_comparison_assumes() {
    // The test above compares timestamps as strings, which is only
    // chronological while every one is the same fixed-offset ISO 8601 shape.
    // A `+01:00` offset or a missing `Z` would silently make that comparison
    // meaningless, so the assumption is pinned rather than trusted.
    // Every position, not four of them. The earlier version checked length,
    // the trailing Z, and the separators at 4 and 10 — which an ISO 8601 *week
    // date* satisfies: `2026-W01-1T00:00:00Z` is twenty characters with the
    // right separators, and `W` sorts above every digit, so it compared as
    // newer than any calendar date and defeated the staleness check while
    // passing this one.
    let iso_utc = |t: &str| {
        let shape = "dddd-dd-ddTdd:dd:ddZ";
        t.len() == shape.len()
            && t.chars()
                .zip(shape.chars())
                .all(|(actual, expected)| match expected {
                    'd' => actual.is_ascii_digit(),
                    other => actual == other,
                })
    };
    for (path, front) in concepts() {
        if let Some(at) = &front.generated_at {
            assert!(
                iso_utc(at),
                "{path}: generated.at `{at}` is not ISO 8601 UTC"
            );
        }
        for at in &front.verified_at {
            assert!(
                iso_utc(at),
                "{path}: verified.at `{at}` is not ISO 8601 UTC"
            );
        }
    }
}
