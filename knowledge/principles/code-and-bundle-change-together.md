---
type: Principle
title: Code and bundle change together
description:
  Any change to one asks what it means for the other, and anything learned asks
  whether it is a lesson rather than a local fix.
tags: [standing-rule, process]
status: stable
verified:
  - { by: human:ojhermann, at: 2026-08-07T18:50:30Z }
  - { by: human:ojhermann, at: 2026-08-08T14:25:43Z }
generated: { by: claude/opus-5, at: 2026-08-08T14:01:42Z }
---

# The rule

The bundle is the specification and the code implements it, so the two drifting
apart is not a tidiness problem — it means one of them is lying, and a reader
cannot tell which.

Three questions, asked on every change:

1. **Code changed — what does the bundle now say that is false?** Behaviour that
   contradicts a recorded claim means one of them is wrong. Decide which,
   deliberately, and change that one.
2. **A concept changed — what code now contradicts it?** A decision recorded and
   not implemented is a decision only in appearance.
3. **Something was learned — is it a lesson or a local fix?** This is the one
   that gets skipped, and it is the one that pays.

# The third question is the point

A defect is usually an instance of a rule nobody wrote down. Fixing the instance
leaves the rule unlearned, so the next instance arrives unrecognised.

Both principles this bundle gained from its first adversarial review were
already true and simply unrecorded —
[failures are classified by remedy](failures-are-classified-by-remedy.md) was
violated twice, in two types, by the same reordering mistake, and
[a claim earns a test](a-claim-earns-a-test.md) came from discovering that
swapping the single most load-bearing line in the crate passed every test.
Neither was invented; both were extracted from what had gone wrong.

So the question to ask of any fix is: **would a rule have prevented this, and is
that rule written down?** If the answer is yes and no, the fix is not finished.

# What "in sync" does not mean

**Not that the bundle describes the code.** It records what is _true_ of the
domain and what has been _decided_, including operations deliberately not
implemented and questions deliberately left open. The implementation is a subset
that has been earned. Discovering the bundle states more than the code does is
expected; discovering it states something the code _contradicts_ is a defect.

**Not that every claim is implemented.** Deferred work is recorded with a
trigger and is not a gap.

# Mechanics

- **`generated.at` moves on any substantive edit.** That is what makes a
  verification visibly stale. Skipping it to keep a concept looking fresh
  defeats the only staleness signal there is.
- **Never write a `verified` entry.** It asserts a human read and confirmed the
  content; an agent writing one is fabricating provenance.
- **Run the checks after touching the bundle** — `./scripts/check.sh test`,
  which runs `okf-graph` over it as a library rather than the binary this bullet
  named until 2026-08-08. They check structure, links and frontmatter, not
  claims — a green run says the bundle is well-formed, never that it is true.
- **A correction is written as a correction.** See
  [the bundle is revisable](the-bundle-is-revisable.md): the earlier position
  and why it failed stay readable.

# Limits

- **Nothing enforces this.** No check compares prose against behaviour, and none
  could in general. It is a discipline, and its failure mode is silent — which
  is why it is written down rather than assumed.
- **Not every code change touches the bundle.** A refactor that preserves
  behaviour, a formatting pass, a dependency bump: the question is asked and the
  answer is no. Asking is cheap; the rule is about not skipping the question.
