# ADR-0049: The growing annuity in the binaries

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Follows:** [ADR-0048](0048-finite-growing-annuity.md) (the finite growing
  annuity in the core), [ADR-0028](0028-binary-surface-conventions.md) (binary
  surface conventions), [ADR-0039](0039-typed-output-layer-for-the-binaries.md)
  (typed output layer), [ADR-0037](0037-currency-in-the-binaries.md) (currency in
  the binaries)

## Context

ADR-0048 added the finite growing annuity to the **core** — the full
present/future × ordinary/due matrix — and deliberately left the binaries alone,
following the core-first sequencing continuous compounding used (ADR-0036 core,
then ADR-0041 binaries). ADR-0028 §1 requires every non-trivial core operation to
be reachable from both binaries, so that ADR closed with an explicit surfacing
obligation. This ADR discharges it.

This is pure surface: the core API already exists and does not change. Two things
about the operation shape the grammar:

- It takes **two rates** — a discount `rate` and a `growth` — where every other
  annuity operation takes one. Only `growing_perpetuity` has faced this before,
  and it settled the vocabulary: a second `--growth` / `growth` alongside `rate`.
- Its **validity differs from the perpetuity's**. `growing_perpetuity` rejects
  `r ≤ g` as divergent; the finite annuity prices every pair. Two neighbouring
  commands therefore accept the same inputs and disagree about whether they are
  legal, which the help text has to make obvious or it reads as a bug.

## Decision

**Surface all four core functions on both binaries**, as ordinary members of the
existing `annuity` family rather than a new group — growth is a variation on an
annuity, not a separate relationship (ADR-0028 §2).

- **CLI**, as a `growing` subgroup of `annuity` holding all four:
  - `annuity growing pv --rate <r> --growth <g> --periods <n> --payment <PMT>`
  - `annuity growing fv …`
  - `annuity growing due-pv …`, `annuity growing due-fv …`
- **MCP**, family-prefixed (ADR-0028 §5): `annuity_growing_present_value`,
  `annuity_growing_future_value`, `annuity_growing_due_present_value`,
  `annuity_growing_due_future_value`.

**Both surfaces name growth first, and group the four together.** The core reaches
these through two paths (`annuity::growing_present_value` and
`annuity::due::growing_present_value`) because its modules are organised by
*timing*, with `due` as the submodule. The binaries organise by *variation*
instead: growth is one thing applied to all four values, so `growing` is the
grouping level and `due` becomes part of the leaf name. That keeps the CLI and the
MCP names in exact correspondence with each other — `annuity growing due-pv` ↔
`annuity_growing_due_present_value` — which matters more for a user moving between
the two binaries than matching an internal module path neither surface exposes.

**Grouping also keeps the dispatcher honest.** The `annuity` CLI group was already
the largest, and four more flat subcommands pushed `run_annuity` past the
workspace's function-length lint. A `growing` subgroup with its own
`run_annuity_growing` dispatcher is the structural answer the lint was pointing
at, not a suppression of it.

**All four take one shared input shape**, `GrowingAnnuityInput` on the MCP side —
`rate`, `growth`, `periods`, `payment`, optional `currency` — exactly as
`AnnuityValueInput` is shared by the ordinary and due value tools.

**`payment` is documented as the *first* payment** in every description and help
string, with each later one `(1 + growth)` times the last. This is the single most
misreadable thing about a growing annuity, and it is the same convention
`growing_perpetuity` already documents.

**Every description states that the rate need not exceed the growth.** This is the
deliberate contrast with the adjacent `growing_perpetuity` (ADR-0048), and without
it in the help text the difference looks like an inconsistency rather than a
decision.

**They honour the global `--currency` / optional `currency`** and return
`Json<MoneyResult>` with an auto-declared `outputSchema`, like every other monetary
operation (ADR-0037, ADR-0039). Validation and errors stay the core's: `Overflow`
is the only failure, surfacing as the usual CLI `error:` line or MCP
`invalid_params`.

## Consequences

- Every core operation is again reachable from both binaries, so ADR-0028 §1 holds
  and the surfacing backlog is empty once more.
- Purely additive: no existing command, tool, flag, output shape, or default
  changes.
- The `annuity` CLI group now has seven direct subcommands plus two subgroups
  (`due`, `growing`). Adding a variation as a subgroup rather than as more flat
  siblings is the pattern to repeat if a third one ever arrives.
- The four MCP tools conform to the ADR-0039 typed-output contract and are covered
  by the output-schema conformance test, which now spans fourteen annuity tools.
- The behavioural contrast with `growing_perpetuity` is pinned by tests on **both**
  binaries: the same rate/growth pair succeeds for the annuity and fails as
  divergent for the perpetuity, asserted side by side.

## Alternatives considered

- **A top-level `growing` command group**, a sibling of `annuity`. Rejected: it
  splits the annuity family across two top-level groups, so a user looking for
  annuity maths would have to know growth lives elsewhere. Nesting `growing`
  *inside* `annuity` keeps the family together while still grouping the variation.
- **Four flat subcommands** (`annuity growing-pv`, `annuity due growing-pv`),
  mirroring the core module paths. Rejected: it splits the four across two groups
  by timing, leaves the CLI and MCP names in different orders, and overflows the
  `annuity` dispatcher's length budget.
- **Reuse `annuity pv` with an optional `--growth` defaulting to 0.** No new
  subcommands at all, and level annuities are the `g = 0` case. Rejected for the
  reason ADR-0048 rejected the equivalent core signature: it makes the common case
  carry the rare one's ceremony, and an optional flag that silently changes the
  formula is exactly the kind of implicit behaviour this project encodes in names
  instead.
- **Surface the ordinary pair only, deferring the due variants.** Half the diff.
  Rejected — it would leave the binaries a strict subset of the core, which is the
  situation ADR-0028 §1 exists to prevent, and the due arms are four lines each.
- **A `growing` boolean flag on the existing tools** (`{"growing": true}`).
  Rejected: it makes the input schema's meaning depend on another field's value,
  which no other tool in the surface does.
