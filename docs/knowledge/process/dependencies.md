---
type: Reference
title: Dependencies
status: stable
generated: { by: claude/opus-5, at: 2026-08-06T23:40:05Z }
sources:
  - id: rust-defaults
    resource: https://github.com/ojhermann-org/claude/blob/main/rust.md
    author: human:ojhermann
---

# Dependencies

The core is `no_std` and carries **no dependencies by default**.
`[workspace.dependencies]` is empty, and each arrives with the formula that
needs it — not in advance.

## The bar

A dependency is justified when at least one holds: correctness is hard and being
wrong is expensive; it is a de-facto interface rather than a convenience; or
reimplementing it would need its own test suite.

Not justified when std has grown the feature, when it is a thin wrapper used
once, or when the transitive tree costs more than the direct benefit. **Evaluate
the tree, not the crate** — a 50-line crate with twelve transitive dependencies
is worse than a 2000-line crate with none.

## Anticipated

- **`libm`** — `powf`, `ln` and `exp` are `std`-only, so any compounding formula
  needs them under `no_std`. It arrives as an optional feature with the first
  operation that requires one, never unconditionally.
- **`proptest`** — a dev-dependency, arriving with the first property. See
  [Testing](testing.md).

## Features must be additive

Features are unified across the whole graph, so a feature that removes or
changes behaviour is broken by construction — anyone in the tree can enable it
for everyone. `std` (additive, and here default-off), never `no-std`.

Optional dependencies always use `dep:` inside an explicit feature. Bare
`optional = true` implicitly creates a public feature named after the
dependency, which is a semver commitment to that name.

## Supply chain

`cargo deny check all` — advisories, bans, licences, sources. The `[sources]`
table is written out because without it the check reports `sources ok` having
done nothing. Dev-dependencies are deliberately **not** excluded: an advisory in
a test dependency still executes on the CI machine.

`unsafe_code = "forbid"` is a crate-local guarantee and says nothing about the
dependency tree. With no dependencies the two currently coincide; that stops
being true the moment one is added.
