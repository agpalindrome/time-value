---
type: Principle
title: The bundle is revisable
description:
  Content, structure and decomposition all change as understanding improves;
  none of it is settled by having been written.
tags: [standing-rule, process]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T18:50:30Z }
generated: { by: claude/opus-5, at: 2026-08-07T16:20:15Z }
---

# The rule

This is a living document. Content, structure and decomposition are all expected
to change as understanding of the domain improves, and nothing here is settled
merely by having been written down.

Two kinds of change are equally ordinary:

- **Content.** A claim turns out to be wrong, or too strong, or true only under
  an assumption nobody stated. It gets corrected.
- **Structure.** How concepts are arranged, how a directory is laid out, where
  one concept ends and the next begins. A flat `concepts/` may become subtrees
  because that is easier to read or easier for a tool to traverse. A quantity
  split into two may turn out to be one.

# Decompositions are guesses until a second instance exists

The riskiest thing recorded here is not a claim about the world — those get
checked against sources. It is a **decomposition**: the decision that this is
one concept and that is another.

A decomposition drawn from a single example is a guess about a shape not yet
seen twice. It usually only becomes testable when a second instance arrives:
[the simple accumulation factor](../domain/simple-accumulation-factor.md) exists
as a concept because `(1 + rt)` looked like a thing in its own right, and
whether it was the right cut becomes answerable when `(1 + i)^n` appears beside
it, not before.

So a decomposition being revised later is the process working, not a defect in
the earlier reasoning.

# What this does not license

**Silent revision.** A claim that changes says it changed. This bundle already
carries corrections written that way — an ordering claim narrowed once currency
came into view, and a periodicity claim reversed once its argument was found to
apply equally to a case it had been contrasted against. In each the earlier
position and the reason it failed are still readable, because the reasoning is
the artifact and a version of it with the mistakes removed is worth less.

**Losing why.** Structure may be rearranged freely; the argument for a decision
travels with it. A concept moved into a subtree keeps its reasoning.

**Treating open questions as settled.** A recorded `Not decided` is a commitment
to answer it later, not a placeholder to be quietly dropped when the code turns
out not to need it.

# What makes revision safe

Each concept records where its claims come from — sourced, derived, or decided —
which is what makes a later reader able to tell a claim that a new source could
overturn from one that only a new preference could. And each records who
confirmed it and when, so a claim changed after it was last read is visible as
exactly that rather than having to be remembered.
