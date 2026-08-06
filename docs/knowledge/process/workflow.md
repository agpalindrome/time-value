---
type: Reference
title: Workflow
status: stable
generated: { by: claude/opus-5, at: 2026-08-06T23:40:05Z }
sources:
  - id: okf-spec
    resource: https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/3fcbb9f828c2f23d109c855ee403c3a4c81f3a96/okf/SPEC.md
    author: team:knowledge-catalog
---

# Workflow

One formula per effort. The library grows in small, reviewable increments, each
of which is a complete piece of knowledge rather than a slice of one.

## The bundle is the documentation

There are no ADRs. This bundle is the living record — both of what the library
knows and of how it is built — and the code implements it. A formula that is not
in the bundle is not in the library.

`okf-graph` validates the bundle's structure on every commit that touches it and
in CI, pinned by revision. The OKF spec has no tags or releases and is edited in
place, so anything read against it is pinned by SHA.

Being a real corpus for that checker is a deliberate second purpose. Its
Attested-Computation fixtures are synthetic; these are not.

## What a formula effort contains

1. The Concept, as an **Attested Computation** — the definition, its parameters,
   and the source it came from. Where sources disagree on a convention, the
   Concept records which one was followed and that the others exist.
2. The implementation in `f64`, with tests capturing the behaviour the Concept
   states.
3. The types that make the misuse the formula invites a compile error — in the
   same pull request, as separate commits, so no untyped API is ever published.

Step 3 is where the type design is *earned*. A type that catches no real failure
mode for this formula does not belong; the pressure has to come from the
formula, not from a design decided in advance.

### Carried debt

CI passes `--no-tests=pass` to nextest, which is only correct while the crate is
empty — nextest failing on zero tests *is* the check working. **Remove the flag
with the first formula**, in the same pull request.

## Sequencing

The library first. The CLI and the MCP server follow as separate efforts once
the core operation exists, in that order — each targets only the formula just
landed, never a backlog.

## Branches and merging

Branch names match `^(feat|fix|chore|docs|refactor)/.*`. Commits are
Conventional Commits. `main` merges go through a merge queue, so
`gh pr merge <n> --squash` **enqueues** rather than merges: a pull request needs
green CI and a clean rebase to land. The job id `ci` is the required status
check — do not rename it, give it a custom `name:`, or drop the `merge_group`
trigger, since a required-check merge queue with no `merge_group` CI deadlocks.

## Releases

There are none, and none scheduled. Cutting one — bumping a version, flipping
`publish`, tagging — is the owner's call and is never inferred from the work
looking finished. The published `0.1.0`–`0.8.0` series is a separate, immutable
history that this line does not continue.

## Comments

The code, this bundle, and the structure carry the information. A comment
supplements them, and earns its place by naming a trap a reader would otherwise
walk into — not by restating what the line below it already says.
