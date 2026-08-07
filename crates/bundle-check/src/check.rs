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
//! [a claim earns a test]: ../../../../knowledge/principles/a-claim-earns-a-test.md

use std::{fmt, path::Path};

use crate::{Frontmatter, actor_is_well_formed, concept_documents, parse};

/// Which invariant a [`Violation`] breaks.
///
/// Two kinds live here and they are not the same authority. A few are the OKF
/// spec's conformance rules (§11); the rest are this repo's own discipline,
/// stricter than the spec and deliberately so — each says **house rule**,
/// because §11 spends most of its text telling consumers *not* to reject a
/// bundle for exactly what is demanded here. A reader who cannot tell the two
/// apart will take a local choice for an external requirement and will not know
/// which ones are ours to change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    /// A non-reserved `.md` that does not parse as a concept (§11 rule 1).
    ///
    /// Reported rather than skipped: a document nobody can read is not a
    /// document that passes.
    Unparsable,
    /// The bundle holds no concepts at all.
    ///
    /// Not a spec rule — a guard on the checker itself. A check that examines
    /// nothing reports success, which is the quiet pass this crate exists to
    /// prevent.
    EmptyBundle,
    /// A concept with no non-empty `type` (§11 rule 2, the spec's one required
    /// key).
    MissingType,
    /// `status` is present and is not `draft`, `stable` or `deprecated` (§5.4),
    /// or is present and not a string at all.
    ///
    /// **House rule**, despite appearances: §5.4 does enumerate the three
    /// values, but §11 leaves rejecting a fourth to the consumer — "all other
    /// constraints" are soft guidance there. The enumeration is the spec's; the
    /// rejection is ours. Absent is legal and means stable; a typo would
    /// otherwise read as an unknown value rather than an error.
    InvalidStatus,
    /// An actor — `generated.by`, a `verified[].by`, or a source `author` —
    /// matching no accepted form.
    ///
    /// §7, as far as it goes. It contradicts §5.1's own example, and
    /// [`actor_is_well_formed`](crate::actor_is_well_formed) says which side
    /// this took.
    MalformedActor,
    /// A concept missing `generated.by` or `generated.at`.
    ///
    /// **House rule.** §4.1 makes the whole `generated` family optional and
    /// §5.2 marks only `by` required within it; `at` is described, not
    /// demanded. Both are demanded here because `generated.at` is what
    /// [`Rule::StaleVerification`] compares against — a concept without one can
    /// never be stale.
    MissingGenerated,
    /// A stable concept nobody has verified.
    ///
    /// **House rule, and one the spec argues against**: §5.3 says a concept
    /// with no trust frontmatter is still consumable and consumers MUST NOT
    /// reject it, and §11 repeats it. That rule governs a consumer reading a
    /// bundle it did not write. This is the producer gating its own, where
    /// `stable` is a claim this repo makes about its own work.
    StableUnverified,
    /// A stable concept whose newest verification predates its `generated.at`.
    ///
    /// **House rule, and the widest departure here**: §5.2 states that
    /// `verified` is independent of `generated.at` — "content can change
    /// without re-confirmation" — and describes that state as ordinary. This
    /// repo makes it fatal, because a verification is the only thing separating
    /// a concept someone read from a concept something wrote.
    StaleVerification,
    /// A `generated.at` or `verified[].at` that is not `YYYY-MM-DDThh:mm:ssZ`.
    ///
    /// **House rule, and a precondition rather than a conformance rule**: §5.2
    /// asks only for "an ISO 8601 datetime", which admits `+00:00` — §10's own
    /// `timestamp` example is written that way. The narrow shape is what
    /// [`Rule::StaleVerification`]'s string comparison needs, not what the spec
    /// requires, so a bundle this rejects may still be conformant. okf-tools#72
    /// proposes okf-graph parse these into datetimes instead, which would
    /// remove the need for the narrowing.
    MalformedTimestamp,
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
    pub const ALL: [Self; 9] = [
        Self::Unparsable,
        Self::EmptyBundle,
        Self::MissingType,
        Self::InvalidStatus,
        Self::MalformedActor,
        Self::MissingGenerated,
        Self::StableUnverified,
        Self::StaleVerification,
        Self::MalformedTimestamp,
    ];

    /// A short, stable name for the rule, used in messages.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Unparsable => "unparsable",
            Self::EmptyBundle => "empty-bundle",
            Self::MissingType => "missing-type",
            Self::InvalidStatus => "invalid-status",
            Self::MalformedActor => "malformed-actor",
            Self::MissingGenerated => "missing-generated",
            Self::StableUnverified => "stable-unverified",
            Self::StaleVerification => "stale-verification",
            Self::MalformedTimestamp => "malformed-timestamp",
        }
    }
}

/// A broken invariant, located.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The file it was found in, or the bundle root for [`Rule::EmptyBundle`].
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

/// Whether a timestamp is the fixed-offset shape the staleness comparison
/// assumes.
///
/// Every position, not four of them. An earlier version checked length, the
/// trailing `Z`, and the separators at 4 and 10 — which an ISO 8601 *week date*
/// satisfies: `2026-W01-1T00:00:00Z` is twenty characters with the right
/// separators, and `W` sorts above every digit, so it compared as newer than
/// any calendar date. It defeated the staleness check while passing this one.
fn is_iso_utc(timestamp: &str) -> bool {
    let shape = "dddd-dd-ddTdd:dd:ddZ";
    timestamp.len() == shape.len()
        && timestamp
            .chars()
            .zip(shape.chars())
            .all(|(actual, expected)| match expected {
                'd' => actual.is_ascii_digit(),
                other => actual == other,
            })
}

/// Every invariant this repo holds its bundle to, over the tree at `root`.
///
/// Returns each violation rather than the first: a run that stops at one hides
/// the rest until the next attempt.
///
/// # Errors
///
/// Any I/O failure reading the tree.
pub fn check(root: &Path) -> std::io::Result<Vec<Violation>> {
    let documents = concept_documents(root)?;

    if documents.is_empty() {
        return Ok(vec![Violation {
            path: root.display().to_string(),
            rule: Rule::EmptyBundle,
            detail: "no concepts found — a check that examines nothing reports success".to_owned(),
        }]);
    }

    let mut violations = Vec::new();
    for (path, text) in documents {
        match parse(&text) {
            Ok(front) => check_one(&path, &front, &mut violations),
            Err(error) => violations.push(Violation {
                path,
                rule: Rule::Unparsable,
                detail: format!("{error:?}"),
            }),
        }
    }
    Ok(violations)
}

/// The per-concept invariants, appending to `violations`.
fn check_one(path: &str, front: &Frontmatter, violations: &mut Vec<Violation>) {
    let mut report = |rule: Rule, detail: String| {
        violations.push(Violation {
            path: path.to_owned(),
            rule,
            detail,
        });
    };

    if front
        .concept_type
        .as_ref()
        .is_none_or(|t| t.trim().is_empty())
    {
        report(Rule::MissingType, "no `type`".to_owned());
    }

    // A `status` present but not a string — `status: true` — reads as `None`
    // exactly like an absent key, so treating the two alike passes over the
    // malformed one in silence.
    match (front.has_status_key, front.status.as_deref()) {
        (true, None) => report(
            Rule::InvalidStatus,
            "`status` is present but not a string".to_owned(),
        ),
        (_, Some(status)) if !matches!(status, "draft" | "stable" | "deprecated") => report(
            Rule::InvalidStatus,
            format!("status `{status}` is not draft, stable or deprecated"),
        ),
        _ => {}
    }

    for actor in &front.actors {
        if !actor_is_well_formed(actor) {
            report(
                Rule::MalformedActor,
                format!("actor `{actor}` matches no accepted form"),
            );
        }
    }

    if front.generated_by.is_none() {
        report(Rule::MissingGenerated, "no `generated.by`".to_owned());
    }
    if front.generated_at.is_none() {
        report(Rule::MissingGenerated, "no `generated.at`".to_owned());
    }

    for at in front
        .generated_at
        .iter()
        .chain(front.verified_at.iter())
        .filter(|at| !is_iso_utc(at))
    {
        report(
            Rule::MalformedTimestamp,
            format!("`{at}` is not ISO 8601 UTC"),
        );
    }

    // Draft and deprecated are both exempt, and this says `!= stable` rather
    // than `== draft` deliberately. A concept being worked on is expected to
    // sit unverified, and a deprecated one is "kept for links and history; no
    // longer current" in the spec's words — staleness against a verification is
    // not a question worth asking of either.
    if front.status.as_deref().unwrap_or("stable") != "stable" {
        return;
    }

    let Some(newest) = front.verified_at.iter().max() else {
        report(
            Rule::StableUnverified,
            "stable, but nobody has verified it".to_owned(),
        );
        return;
    };

    // Only meaningful while every timestamp is the fixed-offset shape above, so
    // a malformed one is left to `MalformedTimestamp` rather than compared and
    // reported twice.
    if let Some(generated) = front.generated_at.as_deref()
        && is_iso_utc(generated)
        && is_iso_utc(newest)
        && newest.as_str() < generated
    {
        report(
            Rule::StaleVerification,
            format!("changed at {generated}, last verified {newest} — stale"),
        );
    }
}
