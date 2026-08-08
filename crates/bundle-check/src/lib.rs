//! The requirements this repo holds its knowledge bundle to, checked rather
//! than eyeballed.
//!
//! Every one of these was, at some point, checked by hand with a shell
//! pipeline — and those pipelines misreported repeatedly: a quoted `yq` value
//! broke a string comparison and called every concept stale, a `grep -A5` ran
//! past a list and read the wrong field, and columns were transposed by being
//! read positionally rather than by name. `grep` and friends fail *soft*: they
//! produce plausible output instead of an error, which is the worst way for a
//! check to fail.
//!
//! **What this crate is responsible for has narrowed twice.** It began as its
//! own frontmatter parser, its own rule set and its own tree walk.
//! [`okf-graph`] 0.1 took over the parsing and every rule the OKF spec states.
//! 0.2 added a [`Check`](okf_graph::Check) trait and rule
//! [`Level`](okf_graph::Level)s, so this repo's own requirements are now checks
//! *registered with* that loader rather than a second pipeline beside it, and
//! its stricter reading of the spec's tolerances is a
//! [`Policy`](okf_graph::Policy) rather than a blanket rule of its own.
//!
//! Narrower is not shorter: `check.rs` grew by about thirty lines across that
//! second step, because four checks each carrying a code, a level and a reason
//! cost more text than one enum did. What went away is the second pipeline and
//! the parallel taxonomy, which is the part that could disagree with itself.
//!
//! What is left is the part that was always this repo's: five requirements the
//! spec does not make, and a decision about which of the spec's tolerated
//! findings are ours to fix. Both are the bundle's own claims, not this crate's
//! — see [we are this bundle's producer, not its consumer].
//!
//! [`okf-graph`]: https://crates.io/crates/okf-graph
//! [we are this bundle's producer, not its consumer]: ../../../../knowledge/principles/producer-not-consumer.md

mod check;

pub use check::{EMPTY_BUNDLE, HOUSE_CODES, Violation, check, house_checks, policy};
