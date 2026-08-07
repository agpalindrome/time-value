---
type: Principle
title: A claim earns a test
description:
  Anything this bundle asserts about implemented behaviour has a test that fails
  when the code stops honouring it.
tags: [standing-rule, testing]
status: stable
verified:
  - { by: human:ojhermann, at: 2026-08-07T21:29:40Z }
  - { by: human:ojhermann, at: 2026-08-07T22:22:09Z }
  - { by: human:ojhermann, at: 2026-08-07T22:46:43Z }
generated: { by: claude/opus-5, at: 2026-08-07T22:43:29Z }
---

# The rule

Anything this bundle asserts about behaviour that exists has a test which fails
the moment the code stops honouring it. A claim with nothing pinning it is a
claim the code can abandon in silence, and the bundle would go on stating it.

This applies to claims about **implemented** behaviour. A recorded decision
about work deliberately deferred is not owed a test; it is owed a trigger, which
is a different thing.

# What "pinning" means, demonstrated

**A load-bearing choice can look tested and not be.** This library computes its
accumulation factor with a fused multiply-add rather than a separate multiply
and add. Adversarial review replaced the fused form with the obvious one:
**every test passed, and so did the linter.** The two forms disagree about which
inputs are valid for 63% of pairs near cancellation, so an innocent-looking
simplification would have silently changed which models the library accepts.

Forty-four tests, none of which touched the one line that decided the answer.
The lesson is not "write more tests" — it is that the tests were about the happy
path of the operation and not about the _decision inside it_.

**Decided — a claim is pinned by a test that fails under the plausible wrong
implementation**, not by one that passes under the right one. The question to
ask of a new test is: what change would this catch? If the answer is "none that
anyone would actually make", it is documentation with a `#[test]` on it.

# The kinds of claim that go unpinned

Each of these was found in this library, not imagined:

- **An implementation choice that changes behaviour.** The fused multiply-add
  above.
- **A classification.** Two failures were reported under the wrong kind, and
  nothing asserted which kind they should be — see
  [failures are classified by remedy](failures-are-classified-by-remedy.md).
- **A boundary the tests approach but never touch.** A sweep over non-values
  covered NaN and positive infinity and omitted negative infinity — which is
  exactly the input where two rules race, so the untested case was the one case
  that mattered.
- **A guarantee stated in prose in a doc comment.** "The result carries the sign
  of the present value" held for every tested magnitude and failed at
  subnormals, where the result underflows to negative zero and stops comparing
  as negative.
- **An absence.** That two arguments cannot be transposed, or that an operator
  is deliberately not implemented, is a claim like any other. A `compile_fail`
  test is the instrument.

# Prove the check can fail

**Decided — a check is not believed until it has been seen to go red.** A
verification script is code, and this rule applies to it before it applies to
anything it verifies.

The evidence is embarrassing and worth keeping. The script written to stop the
checks drifting shipped with two defects, both of which made its own summary
decorative and neither of which a green run could reveal:

- The exit status was captured one line late, so it recorded the success of an
  array append rather than the check. Every check reported a pass whatever
  happened.
- An unquoted `**/*.md` glob was expanded by the shell rather than the
  formatter, and a shell without `globstar` reads `**` as `*` — so it matched
  one directory deep and skipped every file at the repository root.

Both were found by breaking a file on purpose and watching the run stay green.
Neither would have been found any other way.

The same failure recurred all day in ad-hoc shell: a pipe swallowing an exit
code, a quoted value breaking a string comparison, a context flag reading past a
list into the wrong field. `grep`, `tail`, `$?` and globs fail **soft** — they
produce a plausible answer instead of an error, which is the worst way for a
check to fail, because the reader cannot tell.

**Derived — prefer a parser to a pattern for anything structured.** Every one of
those misreads was a shell pipeline interrogating YAML. The invariants they were
groping at are now a test that parses the frontmatter, and each one was
confirmed to fail against a deliberately broken bundle before being trusted.

# A completeness claim is a claim, and nothing pins it

**Decided — "this is the only X", "these are all the Y", "that cannot happen
here" are assertions like any other, and they are load-bearing precisely because
a reader who believes one stops looking.** They are also the assertions least
likely to be tested, because there is nothing concrete to point a test at: the
claim is about an absence.

Two were found in a single review, both stated confidently and both false:

- **"`scripts/check.sh` is the only definition of what must pass."** There were
  two. Seven pre-commit hooks in `flake.nix` appeared nowhere in the script, and
  two of those were content checks that a clone without hooks armed skipped in
  silence. The sentence was in the same paragraph that warns against keeping a
  list in two places.
- **"A test suite here cannot reach for equality even by accident."** It can,
  whenever the test is named after what it tests — see
  [Amount](../domain/amount.md#equality). The lint had been watched going red;
  it had never been watched in the shape being relied on.

**Derived — the two failure modes are enumeration and exhaustiveness, and they
want different answers.** A claim that a list is complete is checked by deriving
the list rather than writing it down twice — `git ls-files '*.nix'` instead of
naming `flake.nix`. A claim that some state is unreachable is checked by trying
to reach it, which is [prove the check can fail](#prove-the-check-can-fail)
again: construct the case the claim says cannot exist and watch what happens.

**Derived — a completeness claim should say what it was checked against.** Both
above would have survived as true statements with a scope on them: the only
definition _of what CI enforces_; unreachable _for any test not named `eq`_. The
scope is what a later reader needs in order to notice that the world has moved,
and it costs a clause.

# What a test may not stand in for

**Approximate comparison is not equality**, and reaching for equality on
computed values is the common error. The toolchain enforces this independently
here: comparing two binary floats with `assert_eq!` inside a test is a hard
error under the denied `pedantic` group, and the usual test exemptions have no
member covering it. Measured 2026-08-07.

**Corrected 2026-08-07 — that enforcement has a hole, and the earlier
unqualified claim hid it.** `float_cmp` exempts itself by the enclosing
function's name: a test called `eq`, `ne` or `is_nan`, or one whose name starts
`eq_` or ends `_eq`, compiles clean. Measured; the detail is in
[Amount](../domain/amount.md#equality). This is
[prove the check can fail](#prove-the-check-can-fail) pointed at a linter, which
is a check like any other: the guarantee was believed because the lint had been
seen to go red, never because it had been seen to go red **in the shape being
relied on**. A tool's silence is evidence only for the case it was watched on.

So a test comparing a computed amount against an expected one states its
tolerance explicitly, and states both halves of it — see
[Amount](../domain/amount.md#approximate-comparison-is-two-operations-not-one).

# Limits

- **Coverage is not the target.** A test that executes a line without asserting
  anything about it pins nothing, and counting such tests is worse than not
  counting.
- **Some claims cannot be tested from inside.** That a public API has not broken
  is invisible to a unit test, which can reach past it; that belongs in a test
  compiled as a separate crate.
- **Some claims are not testable at all**, and those say so rather than being
  quietly dropped. A claim about deferred work is one; a claim about a source's
  meaning is another.
