//! The invariants this repo holds its knowledge bundle to, checked rather than
//! eyeballed.
//!
//! Every one of these was, at some point, checked by hand with a shell
//! pipeline — and those pipelines misreported repeatedly: a quoted `yq` value
//! broke a string comparison and called every concept stale, a `grep -A5` ran
//! past a list and read the wrong field, and columns were transposed by being
//! read positionally rather than by name. `grep` and friends fail *soft*: they
//! produce plausible output instead of an error, which is the worst way for a
//! check to fail.
//!
//! **This crate no longer reads the frontmatter itself.** Until 2026-08-08 it
//! carried its own `saphyr`-based parser, a `Frontmatter` type, an actor-shape
//! predicate and a tree walk, because the only other reader of an OKF bundle
//! was a binary this repo could not call into. [`okf-graph`] is now a crate, so
//! the parsing — and every rule the OKF spec itself states — comes from there,
//! and what is left here is the half that was always this repo's own: the house
//! rules in [`check`], stricter than the spec on purpose.
//!
//! Three things the local parser was written for are now measured facts about
//! the dependency rather than code here (all observed 2026-08-08, against
//! `okf-graph` 0.1.0):
//!
//! - a duplicate top-level key is rejected, so a second `generated` can no
//!   longer hide the first — `serde_yaml` errors where `saphyr` silently took
//!   the last;
//! - a bare `verified: { by, at }` mapping counts as one event, not zero, which
//!   is the shape most of this bundle uses;
//! - `generated.at` and `verified[].at` are parsed as RFC 3339 instants, which
//!   retires the narrow `YYYY-MM-DDThh:mm:ssZ` shape this crate used to demand.
//!   That narrowing existed only to make a *string* comparison safe, and it
//!   rejected the conformant `+00:00` form; staleness now compares instants.
//!
//! [`okf-graph`]: https://crates.io/crates/okf-graph

mod check;

pub use check::{Rule, Violation, check};
