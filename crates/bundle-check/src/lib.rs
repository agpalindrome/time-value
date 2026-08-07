//! Invariants of the knowledge bundle, checked rather than eyeballed.
//!
//! Every one of these was, at some point, checked by hand with a shell
//! pipeline — and those pipelines misreported repeatedly: a quoted `yq` value
//! broke a string comparison and called every concept stale, a `grep -A5` ran
//! past a list and read the wrong field, and columns were transposed by being
//! read positionally rather than by name. `grep` and friends fail *soft*: they
//! produce plausible output instead of an error, which is the worst way for a
//! check to fail.
//!
//! So the invariants live here, parsed properly and asserted in CI.

use std::{fs, path::Path};

use saphyr::{LoadableYamlNode, Yaml};

mod check;

pub use check::{Rule, Violation, check};

/// A concept's frontmatter, so far as these invariants care about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontmatter {
    /// The `type` field. Required by the spec, and the only required key.
    pub concept_type: Option<String>,
    /// `status`, absent meaning `stable` per the spec — recorded as written so
    /// an invalid value can be reported rather than silently defaulted.
    pub status: Option<String>,
    /// Whether a `status` key was present at all, which `status` alone cannot
    /// say: a non-string value reads as `None` exactly like an absent key, and
    /// a check that treats the two alike passes over the malformed one.
    pub has_status_key: bool,
    /// `generated.by`, an actor.
    pub generated_by: Option<String>,
    /// `generated.at`, an ISO 8601 datetime.
    pub generated_at: Option<String>,
    /// Every `verified[].at`, in the order written. The spec permits a bare
    /// mapping as well as a list and requires consumers to treat the bare form
    /// as one element, so both shapes land here identically.
    pub verified_at: Vec<String>,
    /// Every actor named anywhere — `generated.by`, each `verified[].by`, and
    /// each source's `author`.
    pub actors: Vec<String>,
}

/// Why a document could not be read as a concept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// No frontmatter block delimited by `---`.
    MissingFrontmatter,
    /// The frontmatter is not parseable YAML.
    Malformed(String),
    /// The frontmatter parsed, but not to a mapping.
    NotAMapping,
    /// A field that must be a list is something else.
    NotASequence(String),
    /// A key appears more than once at the top level.
    ///
    /// YAML 1.2 makes this an error; the parser used here takes the last
    /// silently, so a duplicate `generated` hides the earlier one and with it
    /// whatever it said. That is the shape a bad merge or an appended edit
    /// produces, so it is rejected rather than resolved.
    DuplicateKey(String),
}

/// Top-level keys, in order, as written.
///
/// Read from the text rather than the parsed value because the parse has
/// already discarded the duplicates by then.
fn top_level_keys(block: &str) -> Vec<String> {
    block
        .lines()
        .filter(|line| !line.starts_with([' ', '\t', '-', '#']))
        .filter_map(|line| line.split_once(':'))
        .map(|(key, _)| key.trim().to_owned())
        .filter(|key| !key.is_empty())
        .collect()
}

fn text(node: &Yaml<'_>) -> Option<String> {
    node.as_str().map(ToOwned::to_owned)
}

/// Splits a document into its frontmatter and reads the fields above.
///
/// # Errors
///
/// [`ParseError`] if the document has no frontmatter block, the block is not
/// parseable YAML, or it does not parse to a mapping.
pub fn parse(document: &str) -> Result<Frontmatter, ParseError> {
    // `split_once` rather than slicing by index: `string_slice` is denied
    // because an index into a string can land inside a UTF-8 character, and
    // this bundle is full of em dashes.
    let (block, _body) = document
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n"))
        .ok_or(ParseError::MissingFrontmatter)?;

    let keys = top_level_keys(block);
    for (index, key) in keys.iter().enumerate() {
        if keys.iter().skip(index + 1).any(|later| later == key) {
            return Err(ParseError::DuplicateKey(key.clone()));
        }
    }

    let docs = Yaml::load_from_str(block).map_err(|e| ParseError::Malformed(e.to_string()))?;
    let doc = docs.first().ok_or(ParseError::NotAMapping)?;
    let map = doc.as_mapping().ok_or(ParseError::NotAMapping)?;

    let get = |key: &str| map.get(&Yaml::value_from_str(key)).cloned();

    let mut actors = Vec::new();

    let generated = get("generated");
    let (generated_by, generated_at) =
        generated
            .as_ref()
            .and_then(Yaml::as_mapping)
            .map_or((None, None), |g| {
                (
                    g.get(&Yaml::value_from_str("by")).and_then(text),
                    g.get(&Yaml::value_from_str("at")).and_then(text),
                )
            });
    if let Some(by) = generated_by.clone() {
        actors.push(by);
    }

    // A bare mapping counts as a one-element list; the spec requires consumers
    // to treat it so, and this bundle contains both shapes.
    let mut verified_at = Vec::new();
    if let Some(verified) = get("verified") {
        let entries: Vec<Yaml<'_>> = verified
            .as_sequence()
            .map_or_else(|| vec![verified.clone()], Clone::clone);
        for entry in &entries {
            if let Some(entry_map) = entry.as_mapping() {
                if let Some(at) = entry_map.get(&Yaml::value_from_str("at")).and_then(text) {
                    verified_at.push(at);
                }
                if let Some(by) = entry_map.get(&Yaml::value_from_str("by")).and_then(text) {
                    actors.push(by);
                }
            }
        }
    }

    // A `sources` that is present but not a sequence was previously skipped in
    // silence, so every author in it went unchecked — and the malformed shape
    // was the one that passed.
    let sources = get("sources");
    if let Some(node) = sources.as_ref() {
        let seq = node
            .as_sequence()
            .ok_or_else(|| ParseError::NotASequence("sources".to_owned()))?;
        for source in seq {
            if let Some(author) = source
                .as_mapping()
                .and_then(|s| s.get(&Yaml::value_from_str("author")))
                .and_then(text)
            {
                actors.push(author);
            }
        }
    }

    Ok(Frontmatter {
        concept_type: get("type").as_ref().and_then(text),
        status: get("status").as_ref().and_then(text),
        has_status_key: get("status").is_some(),
        generated_by,
        generated_at,
        verified_at,
        actors,
    })
}

/// Whether an actor matches the spec's convention.
///
/// §7 lists exactly three forms, but §5.1's own worked example uses a
/// `team:` prefix that none of them admit — so the general shape is accepted:
/// `<producer>/<version>`, or `<scheme>:<id>` with both sides non-empty.
/// Rejecting a bare token is the point; guessing which schemes are sanctioned
/// is not.
#[must_use]
pub fn actor_is_well_formed(actor: &str) -> bool {
    let two_parts = |sep: char| {
        actor.split_once(sep).is_some_and(|(a, b)| {
            !a.is_empty() && !b.is_empty() && !a.contains(' ') && !b.contains(' ')
        })
    };
    two_parts('/') || two_parts(':')
}

/// Every concept document under `root`, as (path, contents).
///
/// Reserved filenames are skipped: `index.md` and `log.md` are not concepts.
///
/// # Errors
///
/// Any I/O failure reading the tree.
pub fn concept_documents(root: &Path) -> std::io::Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(directory) = stack.pop() {
        for entry in fs::read_dir(&directory)? {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            let is_markdown = path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("md"));
            if !is_markdown || name == "index.md" || name == "log.md" {
                continue;
            }
            found.push((path.display().to_string(), fs::read_to_string(&path)?));
        }
    }
    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::{ParseError, actor_is_well_formed, parse};

    const BARE_VERIFIED: &str = "---\ntype: Principle\nstatus: stable\nverified: { by: human:someone, at: 2026-01-02T03:04:05Z }\ngenerated: { by: agent/v1, at: 2026-01-01T00:00:00Z }\n---\n\nbody\n";

    const LIST_VERIFIED: &str = "---\ntype: Quantity\nstatus: stable\nverified:\n  - { by: human:someone, at: 2026-01-02T03:04:05Z }\n  - { by: human:someone, at: 2026-01-03T03:04:05Z }\ngenerated: { by: agent/v1, at: 2026-01-01T00:00:00Z }\nsources:\n  - id: s\n    resource: https://example.invalid\n    author: team:some-team\n---\n\nbody\n";

    #[test]
    fn reads_a_bare_verified_mapping_as_one_entry() {
        // The spec requires consumers to treat the bare form as a one-element
        // list, and this bundle contains both shapes — a reader handling only
        // the list form silently sees no verification at all.
        let front = parse(BARE_VERIFIED).expect("parses");
        assert_eq!(front.verified_at, vec!["2026-01-02T03:04:05Z"]);
    }

    #[test]
    fn reads_a_verified_list_in_order() {
        let front = parse(LIST_VERIFIED).expect("parses");
        assert_eq!(
            front.verified_at,
            vec!["2026-01-02T03:04:05Z", "2026-01-03T03:04:05Z"]
        );
    }

    #[test]
    fn collects_actors_from_every_field_that_names_one() {
        let front = parse(LIST_VERIFIED).expect("parses");
        assert!(front.actors.contains(&"agent/v1".to_owned()));
        assert!(front.actors.contains(&"human:someone".to_owned()));
        assert!(front.actors.contains(&"team:some-team".to_owned()));
    }

    #[test]
    fn reads_type_and_status() {
        let front = parse(BARE_VERIFIED).expect("parses");
        assert_eq!(front.concept_type.as_deref(), Some("Principle"));
        assert_eq!(front.status.as_deref(), Some("stable"));
    }

    #[test]
    fn refuses_a_document_with_no_frontmatter() {
        assert_eq!(parse("just a body\n"), Err(ParseError::MissingFrontmatter));
        assert_eq!(
            parse("---\nunterminated\n"),
            Err(ParseError::MissingFrontmatter)
        );
    }

    #[test]
    fn accepts_the_actor_forms_in_use_and_rejects_a_bare_token() {
        for actor in [
            "human:ojhermann",
            "claude/opus-5",
            "team:wikipedia",
            "process:x",
        ] {
            assert!(actor_is_well_formed(actor), "{actor} should be accepted");
        }
        for actor in ["alice", "", "human:", ":id", "a b/c"] {
            assert!(!actor_is_well_formed(actor), "{actor} should be rejected");
        }
    }

    #[test]
    fn rejects_a_space_on_either_side_of_the_separator() {
        // The check tested only the left half, so `human:x y` and
        // `claude/ opus 5` were accepted. One-sided validation looks complete
        // until the mirror case is written down.
        for actor in ["human:x y", "claude/ opus 5", "team: ", "a b:c"] {
            assert!(!actor_is_well_formed(actor), "{actor} should be rejected");
        }
    }

    #[test]
    fn rejects_a_duplicate_top_level_key() {
        // YAML 1.2 calls this an error; the parser takes the last silently, so
        // a second `generated` hides the first and whatever it said. It is the
        // shape a bad merge produces.
        let document = concat!(
            "---\n",
            "type: Principle\n",
            "generated: { by: a/1, at: 2026-06-01T00:00:00Z }\n",
            "generated: { by: a/1, at: 2025-01-01T00:00:00Z }\n",
            "---\n\nbody\n"
        );
        assert_eq!(
            parse(document),
            Err(ParseError::DuplicateKey("generated".to_owned()))
        );
    }

    #[test]
    fn rejects_sources_that_is_not_a_list() {
        // Previously skipped in silence, so every author inside went
        // unchecked — and the malformed shape was the one that passed.
        let document = concat!(
            "---\n",
            "type: Principle\n",
            "sources:\n",
            "  an-id:\n",
            "    author: alice\n",
            "---\n\nbody\n"
        );
        assert_eq!(
            parse(document),
            Err(ParseError::NotASequence("sources".to_owned()))
        );
    }

    #[test]
    fn distinguishes_an_absent_status_from_an_unreadable_one() {
        // `status: true` reads as `None` exactly like an absent key, so a check
        // treating the two alike passes over the malformed one.
        let absent = parse("---\ntype: Principle\n---\n\nbody\n").expect("parses");
        assert!(!absent.has_status_key && absent.status.is_none());

        let unreadable =
            parse("---\ntype: Principle\nstatus: true\n---\n\nbody\n").expect("parses");
        assert!(unreadable.has_status_key && unreadable.status.is_none());
    }
}
