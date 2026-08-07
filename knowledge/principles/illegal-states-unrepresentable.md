---
type: Principle
title: Illegal states are unrepresentable
description:
  A value that would be invalid should be impossible to construct, not merely
  detectable afterwards.
tags: [standing-rule]
status: stable
verified: { by: human:ojhermann, at: 2026-08-07T02:37:35Z }
generated: { by: claude/opus-5, at: 2026-08-07T02:21:28Z }
---

# The rule

A value that would be invalid should be impossible to construct, rather than
possible to construct and detectable afterwards.

This is the default posture for everything in this bundle. Concepts should say
how they satisfy it. Exceptions are allowed and must be recorded with their
reason — an exception nobody wrote down is indistinguishable from an oversight.

The phrasing is a known idiom, commonly attributed to Yaron Minsky. No source is
cited here because the attribution has not been checked against an original.

# Why it carries weight here

This library computes numbers other people act on. A wrong number that announces
itself is a bug; a wrong number that does not is a liability, and the failure
surfaces far from its cause.

The [Amount](../domain/amount.md) case is the concrete one: NaN propagates
through every subsequent operation without a signal, so a single invalid
construction silently voids every result downstream of it. No amount of care at
the point of _use_ recovers that, because by then the information that something
was wrong is gone.

# The ladder

Preferred to least preferred. Reach for the highest rung the situation allows:

1. **Structurally impossible** — the type has no invalid inhabitants at all, so
   nothing needs checking. The strongest form, and free at run time.
2. **Validated construction** — invalid inhabitants exist in the representation,
   but the only way in rejects them. This is the usual rung.
3. **Checked at use** — the value can be invalid and each operation must cope.
   Weaker, and it moves the burden onto every call site.
4. **Documented** — a comment saying what is required. This is not enforcement;
   it is a note about what will not be enforced.

Most of this library will sit on rung 2, because its representations carry
non-values that no type discipline can remove.

# What rung 2 actually requires

Three things, and dropping any one of them collapses to rung 4:

- **Construction validates, and can therefore fail.** An invariant that cannot
  reject an input is not an invariant.
- **There is no path around it.** Privacy is the mechanism, not a stylistic
  preference — an exposed field can be written to directly, and every guarantee
  becomes a comment. This is why [Amount](../domain/amount.md) requires a
  private magnitude.
- **Operations re-establish the invariant.** Where operations are partial,
  results must be validated too, so they also return something that can fail.
  The rule propagates through the API rather than stopping at the constructor.

# Limits

Stated so the rule is not applied where it does not belong:

- **It constrains representable states, not representable mistakes.** Two values
  of the same type remain interchangeable at a call site; nothing here prevents
  passing one where the other was meant. That is a separate question about roles
  and tagging.
- **A distinction that catches no real failure is ceremony, not safety.** The
  rule is not "wrap everything". A type earns its place by making a mistake
  impossible that would otherwise happen.
- **Genuinely runtime-chosen sets stay values.** Where a distinction is not
  known until the program runs — a currency parsed from input — it cannot live
  in a type, and enforcing it at run time is the highest rung available. See
  [Amount](../domain/amount.md).
- **It costs ergonomics.** A fallible constructor obliges every caller to handle
  failure. The cost is worth paying at a chokepoint, where it is paid once, and
  not worth spreading across every call site.
