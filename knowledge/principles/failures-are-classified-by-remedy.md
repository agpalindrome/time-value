---
type: Principle
title: Failures are classified by remedy
description:
  A failure is named for what would fix it, and where two rules could both apply
  the unfixable one wins.
tags: [standing-rule, errors]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T18:50:30Z }
generated: { by: claude/opus-5, at: 2026-08-08T16:30:02Z }
---

# The rule

A failure is named for **what would fix it**, not for what the code noticed
first. Where two rules could both describe the same input, the one that cannot
be fixed wins.

# Why the name is the remedy

An error is read by someone deciding what to do next. Two failures this library
already draws apart:

- **Domain** — the inputs were each valid and jointly meaningless. Change the
  model. No wider representation helps.
- **Representation** — the arithmetic left what the numbers can hold. Rescale,
  or carry more precision. The model was fine.

Those prescribe opposite actions, so a misclassification is not a cosmetic
defect. It sends a reader confidently in the wrong direction, which is worse
than a vague error that sends them nowhere.

# Where two rules race, the unfixable one wins

**Decided — check the condition that no wider representation can rescue,
first.**

This was learned twice on the same day, in two types, from the same mistake:

| input                    | both true                                | reported       | should be                                                                                    |
| ------------------------ | ---------------------------------------- | -------------- | -------------------------------------------------------------------------------------------- |
| `r = -1e300`, `t = 1e10` | the factor overflows **and** is negative | representation | **domain** — its sign is known from the operands, and `-∞` is not made positive by more bits |
| a span of `-∞`           | not finite **and** negative              | representation | **domain** — negative is negative at any width                                               |

In both, the code tested finiteness before sign because finiteness is the more
obvious check to write. The fix in both was to reorder, and the general form is
the rule above.

The test to apply: **would a wider representation change the answer?** If not,
the failure is about the model, whatever else is also true of the value.

# Naming a failure is not the same as counting them

A variant that several unrelated operations share tells a caller only that
something went wrong somewhere. This library's `NotFinite` reached five callers
before it carried anything identifying which quantity — an amount that was never
a number and a product that outgrew the range are different problems with
different fixes, and one message served both.

**Decided — a shared variant carries enough to say which case it is.** Splitting
into many variants is one way; a field naming the quantity is another and is
what this library chose. What is not acceptable is a single name standing for
several remedies.

**Derived — that field makes the class computable, not merely readable.** It was
adopted so a person reading a message could tell which case they had. It turns
out to be what lets the library _answer_ the question: an accessor returning the
class is a function of the variant and, for the shared one, of that field.
Proposing to split the variant instead — on the grounds that one name cannot
carry two remedies — would have been a breaking change to work already done, one
reason to check what the existing structure answers before breaking it.

**Derived — the classification belongs to the library, not to each caller.** An
error type that grows keeps its variants open, so a consumer matching them needs
a catch-all and a variant added later lands silently in whatever bucket that arm
picked. Inside the crate the match is exhaustive and a new variant fails to
compile until somebody classifies it. That is the difference between the rule
holding by construction and holding by attention.

# Limits

- **Not every failure has a distinct remedy.** Where two conditions really do
  prescribe the same action, one variant is right and splitting is ceremony.
- **This says nothing about how many variants an error type should have.** It
  says that whatever variants exist must partition by remedy rather than by
  where in the code the check happened to sit.
- **The classification is a claim, so it earns a test** — see
  [a claim earns a test](a-claim-earns-a-test.md). Both misclassifications above
  were live and untested; nothing failed when they were wrong.
- **Reading the class off a field rests on that field's values partitioning, and
  nothing enforces it.** The library's identifying field names a quantity, and
  the class follows because the quantities a caller supplies arrive as
  non-values while the computed ones outgrow a range. An operation that computed
  a value and handed it to a supplied quantity's constructor without checking
  the range first would report a representation failure wearing a domain label.
  Today none does, and the live paths are tested in both directions — but the
  partition is a convention among constructors rather than something a type
  holds.
