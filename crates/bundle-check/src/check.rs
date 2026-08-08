//! This repo's own requirements on its bundle, as `okf-graph` checks.
//!
//! They lived in a hand-rolled pipeline beside `okf-graph`'s until 0.2.0 added
//! a [`Check`] trait — a caller registers its own rules, and the same loader
//! that finds the spec's findings runs them. So there is one finding stream
//! rather than two mapped into a common type, and each rule's code, level and
//! reason sit together. The trait's own documentation gives the argument:
//! "every check is somebody's house rule, and the question a year later is
//! rarely what it checks — it is who wanted it and why."
//!
//! Every rule here is stricter than the spec, and
//! [we are this bundle's producer, not its consumer] is why: a tolerance the
//! spec addresses to a consumer is not a licence for the producer. That
//! principle also fixes [`policy`], which is where the spec's own findings are
//! re-levelled.
//!
//! Codes are `TV-*`. [`Checks::add`] refuses a collision with an OKF code, so
//! the namespaces cannot silently overlap.
//!
//! [we are this bundle's producer, not its consumer]: ../../../../knowledge/principles/producer-not-consumer.md

use std::{fmt, path::Path};

use okf_graph::{Bundle, Check, Checks, Concept, Level, Policy, Rule, Status, Timestamp};

/// The whole-bundle guard's code.
///
/// Not a [`Check`]: the trait runs against one concept, and this is a question
/// about the bundle. `okf-graph` 0.2 makes an empty bundle a usage error on its
/// *binary* and deliberately not a rule, leaving `Bundle::is_empty` to a
/// library consumer — which this crate is.
pub const EMPTY_BUNDLE: &str = "TV-0";

/// Every code this crate can report on its own authority, so a test can assert
/// each has a fixture that fires it.
///
/// Hand-written, and the compiler does not check it is complete — the honest
/// scope of it. What it does not cover is the spec's own codes: their red side
/// is `okf-graph`'s test suite, and duplicating it here would be testing the
/// dependency. The one exception is a report this repo *denies*, which has a
/// fixture because the denial is ours.
pub const HOUSE_CODES: [&str; 5] = [EMPTY_BUNDLE, "TV-1", "TV-2", "TV-3", "TV-4"];

/// The spec findings this repo treats as defects rather than tolerating.
///
/// Each is about material this repo wrote, which is the whole test — see the
/// principle. Named as a list so [`policy`] and the test pinning it read from
/// one place.
const DENIED: [Rule; 5] = [
    Rule::DanglingLink,
    Rule::DanglingPath,
    Rule::DanglingIndexEntry,
    Rule::DanglingLogEntry,
    Rule::LogOutOfOrder,
];

/// TV-1 — a concept declares a `generated` family at all.
///
/// **House rule.** §4.1 makes the family optional outright. It is required here
/// because [`VerificationIsCurrent`] measures staleness against `generated.at`,
/// and a concept without one can never be stale.
///
/// Fires only when the family is *absent*. A declared family missing its `by`
/// is `CONCEPT-4`, which fires on `declares("generated")` — measured by reading
/// `okf-graph` 0.2.0's `bundle.rs`, so the two cannot both report.
struct GeneratedFamily;

impl Check for GeneratedFamily {
    fn code(&self) -> &'static str {
        "TV-1"
    }

    fn check(&self, _id: &str, concept: &Concept) -> Result<(), String> {
        if concept.frontmatter().declares("generated") {
            Ok(())
        } else {
            Err("no `generated` family, which §4.1 permits and this repo does not".to_owned())
        }
    }
}

/// TV-2 — that family carries an `at`.
///
/// **House rule.** §5.2 marks only `by` required within `generated`; `at` is
/// described, not demanded. It is demanded here for the same reason as TV-1.
struct GeneratedAt;

impl Check for GeneratedAt {
    fn code(&self) -> &'static str {
        "TV-2"
    }

    fn check(&self, _id: &str, concept: &Concept) -> Result<(), String> {
        match concept.frontmatter().generated().and_then(|g| g.at) {
            Some(_) => Ok(()),
            None => {
                Err("no `generated.at`, which is what staleness is measured against".to_owned())
            }
        }
    }
}

/// TV-3 — a `stable` concept carries a verification.
///
/// **House rule, and one the spec argues against**: §5.3 says a concept with no
/// trust frontmatter is still consumable and a consumer MUST NOT reject it, and
/// §11 repeats it. That governs a consumer reading a bundle it did not write.
/// This is the producer gating its own, where `stable` is a claim this repo
/// makes about its own work.
struct StableIsVerified;

impl Check for StableIsVerified {
    fn code(&self) -> &'static str {
        "TV-3"
    }

    fn check(&self, _id: &str, concept: &Concept) -> Result<(), String> {
        let front = concept.frontmatter();
        if front
            .status()
            .is_some_and(|status| status != Status::Stable)
        {
            return Ok(());
        }
        if front.verified().is_empty() {
            return Err("stable, but nobody has verified it".to_owned());
        }
        Ok(())
    }
}

/// TV-4 — a `stable` concept's newest verification is not older than its
/// content.
///
/// **House rule, and the widest departure in substance**: §5.2 states that
/// `verified` is independent of `generated.at` — "content can change without
/// re-confirmation" — and describes that state as ordinary. It is fatal here,
/// because a verification is the only thing separating a concept someone read
/// from a concept something wrote.
///
/// Compares [`Timestamp`]s, not strings. The text comparison this replaced
/// needed every timestamp narrowed to a `...Z` shape first, and a week date
/// slipped through the narrowing and sorted above every calendar date.
struct VerificationIsCurrent;

impl Check for VerificationIsCurrent {
    fn code(&self) -> &'static str {
        "TV-4"
    }

    fn check(&self, _id: &str, concept: &Concept) -> Result<(), String> {
        let front = concept.frontmatter();
        if front
            .status()
            .is_some_and(|status| status != Status::Stable)
        {
            return Ok(());
        }

        // A malformed `at` is CONCEPT-12's to report. Passing here rather than
        // guessing is the trait's own instruction inverted: there is nothing this
        // check cannot evaluate, because a timestamp nobody can read is already a
        // finding under its own code.
        let Some(generated) = front
            .generated()
            .and_then(|g| g.at)
            .and_then(|at| Timestamp::parse(&at).map(|when| (when, at)))
        else {
            return Ok(());
        };
        let Some(newest) = front
            .verified()
            .into_iter()
            .filter_map(|event| event.at)
            .filter_map(|at| Timestamp::parse(&at).map(|when| (when, at)))
            .max_by(|left, right| left.0.cmp(&right.0))
        else {
            return Ok(());
        };

        if newest.0 < generated.0 {
            return Err(format!(
                "changed at {}, last verified {} — stale",
                generated.1, newest.1
            ));
        }
        Ok(())
    }
}

/// This repo's checks, with their codes verified unique against the spec's.
///
/// # Panics
///
/// If two of the codes above collide, or one collides with an OKF code. Both
/// are impossible for a fixed list of literals, and
/// `registers_every_house_check` exercises this on every run rather than
/// leaving that a comment.
#[must_use]
pub fn house_checks() -> Checks {
    let mut checks = Checks::new();
    // Boxed, and the array's type written out rather than cast: `trivial_casts`
    // is denied, and coercion does the same job.
    let registry: [Box<dyn Check>; 4] = [
        Box::new(GeneratedFamily),
        Box::new(GeneratedAt),
        Box::new(StableIsVerified),
        Box::new(VerificationIsCurrent),
    ];
    for check in registry {
        checks
            .add(check)
            .expect("the house codes are literals, unique, and not the spec's");
    }
    checks
}

/// How this repo levels every rule: its own at `Defect`, and the spec's own
/// findings re-levelled where the material is ours.
///
/// The five denials are the bundle's decision, not a preference — see
/// [we are this bundle's producer, not its consumer]. Everything else keeps
/// `okf-graph`'s default, which is §11's verdict.
///
/// [we are this bundle's producer, not its consumer]: ../../../../knowledge/principles/producer-not-consumer.md
#[must_use]
pub fn policy(checks: &Checks) -> Policy {
    let mut policy = Policy::for_checks(checks);
    for rule in DENIED {
        policy.set(rule, Level::Defect);
    }
    policy
}

/// A finding this repo acts on: where, which rule, at what level, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// The bundle-relative file it is about.
    pub path: String,
    /// The rule's code — `TV-*` for this repo's, an OKF code for the spec's.
    pub code: String,
    /// What this repo does about it.
    pub level: Level,
    /// What is wrong, in the words a failing test should print.
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: [{} {:?}] {}",
            self.path, self.code, self.level, self.detail
        )
    }
}

/// Everything this repo has to say about the bundle at `root`, spec and house
/// alike, with anything the policy allows already dropped.
///
/// Returns each violation rather than the first: a run that stops at one hides
/// the rest until the next attempt.
///
/// # Errors
///
/// Any I/O failure reading the tree.
pub fn check(root: &Path) -> std::io::Result<Vec<Violation>> {
    let bundle = Bundle::load(root)?;
    let checks = house_checks();
    let policy = policy(&checks);

    // Nothing examined and nothing to say. Both halves matter: a bundle whose
    // only documents failed to parse is not empty, it is broken, and CONCEPT-1
    // already says so.
    if bundle.is_empty() && bundle.findings().is_empty() {
        return Ok(vec![Violation {
            path: root.display().to_string(),
            code: EMPTY_BUNDLE.to_owned(),
            level: Level::Defect,
            detail: "no concepts found — a check that examines nothing reports success".to_owned(),
        }]);
    }

    let spec = bundle.findings_at(&policy).into_iter().cloned();
    let house = bundle.check(&checks).into_iter();

    Ok(spec
        .chain(house)
        .map(|finding| Violation {
            path: finding.file,
            code: finding.rule.code().to_owned(),
            level: policy.level(&finding.rule),
            detail: finding.detail,
        })
        .collect())
}
