---
okf_version: "0.2"
---

The knowledge behind this library: the finance it implements, where each claim
came from, and the standing rules the code is built under. The code implements
this bundle — where the two disagree, one of them is wrong, and it is not
automatically the bundle.

Claims are marked by where they come from. A footnote means a source supports
it; otherwise it is labelled **Decided** (a choice that could have gone
otherwise) or **Derived** (a consequence of something already established).

# Start here

Read the principles once; they govern everything. Read the domain concepts for
the work in hand.

| if you are…                                  | read                                                                                                                                                  |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| changing any code at all                     | every [principle](principles/), then the domain concepts your change touches                                                                          |
| implementing or altering a formula           | that formula's concept, then every quantity it names                                                                                                  |
| adding a new formula                         | [future value](domain/future-value.md), as the worked example of how one is modelled, then the quantities it reuses                                   |
| deciding how a value is represented          | [amount](domain/amount.md) — representation, equality, ordering, comparison and rendering are all settled there                                       |
| touching an error type or its classification | [failures are classified by remedy](principles/failures-are-classified-by-remedy.md)                                                                  |
| writing or reviewing tests                   | [a claim earns a test](principles/a-claim-earns-a-test.md)                                                                                            |
| recording a decision or a correction         | [the bundle is revisable](principles/the-bundle-is-revisable.md) and [code and bundle change together](principles/code-and-bundle-change-together.md) |

# Groups

- [Principles](principles/) - Standing rules that govern every concept and every
  change to the code.
- [Domain](domain/) - The finance: the formulas, and the quantities they are
  written in.

# Tags

Two axes, so a consumer scanning frontmatter can filter on either. A concept's
kind is already its `type` and is not repeated as a tag.

- **Topic** — the area of the domain: `simple-interest`, `money`, `time`.
- **Concern** — the cross-cutting question it bears on: `errors`, `testing`,
  `numerics`, `process`, `standing-rule`.
