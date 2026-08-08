//! The invariants themselves, as a function over a bundle root.
//!
//! They lived in `#[test]` bodies until they needed a red side. A test that
//! walks the real tree can only ever assert the bundle is *currently* fine —
//! nothing stops an edit making the assertion vacuous, and by the bundle's own
//! rule ([a claim earns a test]) an unpinned check is documentation with a
//! `#[test]` on it. Moving each into a function that takes a root lets a
//! deliberately broken fixture be pointed at it, so every invariant has been
//! watched going red for the reason it exists.
//!
//! [`okf_graph`] does the reading and reports the spec's own rules; this adds
//! what the spec does not ask for. Both arrive as [`Violation`]s from one
//! [`check`] call, because a caller with two lists to consult reads one of
//! them.
//!
//! [a claim earns a test]: ../../../../knowledge/principles/a-claim-earns-a-test.md

use std::{fmt, path::Path};

use okf_graph::{Bundle, Frontmatter, Severity, Status, Timestamp};

/// Which invariant a [`Violation`] breaks.
///
/// Two kinds live here and they are not the same authority.
/// [`Rule::SpecDefect`] is the OKF spec's own conformance verdict, delegated to
/// `okf-graph`; the rest are this repo's own discipline, stricter than the spec
/// and deliberately so — each says **house rule**, because §11 spends most of
/// its text telling consumers *not* to reject a bundle for exactly what is
/// demanded here. A reader who cannot tell the two apart will take a local
/// choice for an external requirement and will not know which ones are ours to
/// change.
///
/// Five rules that were once here are gone, not weakened: an unparsable
/// document, a missing `type`, an invalid `status`, a malformed actor and a
/// malformed timestamp are `CONCEPT-1`, `-2`, `-3`, `-5` and `-12` in
/// `okf-graph`, which reports each as a defect. Keeping local copies would mean
/// two implementations of one rule, and the day they disagreed the local one
/// would be believed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// A finding `okf-graph` calls a **defect** — a §11 conformance failure,
    /// whatever its rule. The [`Violation::detail`] carries the code, so a
    /// fixture still pins which one.
    ///
    /// One variant rather than one per rule: `okf_graph::Rule` is
    /// `#[non_exhaustive]` and grows as the spec and the checker do, so mapping
    /// it exhaustively here would put this crate in the business of tracking
    /// somebody else's enum. The spec's own binary — defect or report — is what
    /// this crate needs, and that is settled.
    SpecDefect,
    /// A finding `okf-graph` calls a **report** — a dangling link, a tolerated
    /// out-of-order log entry, an unusable credibility signal.
    ///
    /// **House rule, and the strictest departure here**: §6 and §11 say a
    /// consumer MUST NOT reject a bundle for these, and `okf-graph` accordingly
    /// exits zero on them. It is a violation here because the alternative is
    /// worse than either extreme: this crate is a test, `cargo nextest` shows
    /// nothing from a test that passes, and a finding printed where nobody
    /// looks is a finding nobody acts on. That is a statement about our own
    /// bundle, never about the spec — accepting a report means editing this
    /// rule, on purpose, with the reason written down.
    SpecReport,
    /// The bundle holds no concepts and nothing was reported about it.
    ///
    /// Not a spec rule — a guard on the checker itself. A check that examines
    /// nothing reports success, which is the quiet pass this crate exists to
    /// prevent. Both halves matter: a bundle whose only documents failed to
    /// parse is not empty, it is broken, and `SpecDefect` already says so.
    EmptyBundle,
    /// A concept with no `generated.by` or no `generated.at`.
    ///
    /// **House rule.** §4.1 makes the whole `generated` family optional and
    /// §5.2 marks only `by` required within it; `at` is described, not
    /// demanded. Both are demanded here because `generated.at` is what
    /// [`Rule::StaleVerification`] compares against — a concept without one can
    /// never be stale.
    ///
    /// The `by` half fires only when the family is absent altogether. A
    /// declared `generated` with no `by` is `okf-graph`'s `CONCEPT-4`, and
    /// reporting it twice would say the same thing in two vocabularies.
    MissingGenerated,
    /// A stable concept nobody has verified.
    ///
    /// **House rule, and one the spec argues against**: §5.3 says a concept
    /// with no trust frontmatter is still consumable and consumers MUST NOT
    /// reject it, and §11 repeats it. That rule governs a consumer reading
    /// a bundle it did not write. This is the producer gating its own,
    /// where `stable` is a claim this repo makes about its own work.
    StableUnverified,
    /// A stable concept whose newest verification predates its `generated.at`.
    ///
    /// **House rule, and the widest departure in substance**: §5.2 states that
    /// `verified` is independent of `generated.at` — "content can change
    /// without re-confirmation" — and describes that state as ordinary.
    /// This repo makes it fatal, because a verification is the only thing
    /// separating a concept someone read from a concept something wrote.
    StaleVerification,
}

impl Rule {
    /// Every rule, so a test can assert each one has a fixture that fires it.
    ///
    /// Hand-written, and the compiler does not check it is complete — that is
    /// the honest scope of it. What the compiler does force is that a new
    /// variant breaks [`Rule::name`] below until it is handled, which puts the
    /// author two lines from this list at exactly the moment it needs
    /// extending. Deriving it instead would want a dependency, and those arrive
    /// when something needs them.
    pub const ALL: [Self; 6] = [
        Self::SpecDefect,
        Self::SpecReport,
        Self::EmptyBundle,
        Self::MissingGenerated,
        Self::StableUnverified,
        Self::StaleVerification,
    ];

    /// A short, stable name for the rule, used in messages.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::SpecDefect => "spec-defect",
            Self::SpecReport => "spec-report",
            Self::EmptyBundle => "empty-bundle",
            Self::MissingGenerated => "missing-generated",
            Self::StableUnverified => "stable-unverified",
            Self::StaleVerification => "stale-verification",
        }
    }
}

/// A broken invariant, located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Where it was found: a bundle-relative file for a spec finding, a concept
    /// id for a house rule, or the bundle root for [`Rule::EmptyBundle`].
    pub path: String,
    /// Which invariant it breaks.
    pub rule: Rule,
    /// What is wrong, in the words a failing test should print.
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: [{}] {}", self.path, self.rule.name(), self.detail)
    }
}

/// Every invariant this repo holds its bundle to, over the tree at `root`.
///
/// Returns each violation rather than the first: a run that stops at one hides
/// the rest until the next attempt. Order is `okf-graph`'s findings, then the
/// house rules by concept id — deterministic, because a bundle is loaded into a
/// `BTreeMap` (observed in `okf-graph` 0.1.0, whose API does not promise it).
///
/// # Errors
///
/// Any I/O failure reading the tree.
pub fn check(root: &Path) -> std::io::Result<Vec<Violation>> {
    let bundle = Bundle::load(root)?;

    let mut violations: Vec<Violation> = bundle
        .findings()
        .iter()
        .map(|finding| Violation {
            path: finding.file.clone(),
            rule: match finding.severity() {
                Severity::Defect => Rule::SpecDefect,
                Severity::Report => Rule::SpecReport,
            },
            detail: format!(
                "{} ({}): {}",
                finding.rule.code(),
                finding.rule.title(),
                finding.detail
            ),
        })
        .collect();

    if bundle.is_empty() && violations.is_empty() {
        violations.push(Violation {
            path: root.display().to_string(),
            rule: Rule::EmptyBundle,
            detail: "no concepts found — a check that examines nothing reports success".to_owned(),
        });
        return Ok(violations);
    }

    for (id, concept) in bundle.concepts() {
        check_one(id, concept.frontmatter(), &mut violations);
    }
    Ok(violations)
}

/// The per-concept house rules, appending to `violations`.
fn check_one(id: &str, front: &Frontmatter, violations: &mut Vec<Violation>) {
    let mut report = |rule: Rule, detail: String| {
        violations.push(Violation {
            path: id.to_owned(),
            rule,
            detail,
        });
    };

    let generated = front.generated();
    if generated.is_none() {
        report(Rule::MissingGenerated, "no `generated.by`".to_owned());
    }
    let generated_at = generated.as_ref().and_then(|g| g.at.clone());
    if generated_at.is_none() {
        report(Rule::MissingGenerated, "no `generated.at`".to_owned());
    }

    // Draft and deprecated are both exempt, and this says `!= stable` rather
    // than `== draft` deliberately. A concept being worked on is expected to
    // sit unverified, and a deprecated one is "kept for links and history; no
    // longer current" in the spec's words — staleness against a verification is
    // not a question worth asking of either.
    //
    // An unreadable `status` is a `CONCEPT-3` defect and defaults to stable
    // here, so a typo is judged by the stricter reading rather than excused by
    // being unreadable.
    let verified = front.verified();
    if front
        .status()
        .is_some_and(|status| status != Status::Stable)
    {
        return;
    }

    // Emptiness is asked of the events, not of the parsed instants: a concept
    // whose only `verified.at` is malformed has been verified and has a
    // `CONCEPT-12` defect, and calling that unverified would name the wrong
    // problem.
    if verified.is_empty() {
        report(
            Rule::StableUnverified,
            "stable, but nobody has verified it".to_owned(),
        );
        return;
    }

    // Instants, not strings. The text comparison this replaced needed every
    // timestamp narrowed to `...Z` first, and a week date slipped through the
    // narrowing and sorted above every calendar date.
    let newest = verified
        .iter()
        .filter_map(|event| event.at.as_deref())
        .filter_map(|at| Timestamp::parse(at).map(|instant| (instant, at)))
        .max_by_key(|&(instant, _)| instant);

    if let Some(at) = generated_at.as_deref()
        && let Some(generated) = Timestamp::parse(at)
        && let Some((newest, newest_text)) = newest
        && newest < generated
    {
        report(
            Rule::StaleVerification,
            format!("changed at {at}, last verified {newest_text} — stale"),
        );
    }
}
