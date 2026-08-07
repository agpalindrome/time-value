# Directory Update Log

## 2026-08-07

- **Update**: Recorded in
  [a claim earns a test](principles/a-claim-earns-a-test.md) that a completeness
  claim — "the only X", "all the Y", "that cannot happen" — is an assertion like
  any other and is the one least likely to be tested, since it is about an
  absence. Two were falsified in a single review: `check.sh` was not the only
  definition of what must pass, and the float_cmp guarantee was not
  unconditional. A claim of a complete list is checked by deriving the list; a
  claim of an unreachable state, by trying to reach it.
- **Update**: Corrected [amount](domain/amount.md) and
  [a claim earns a test](principles/a-claim-earns-a-test.md) — both stated that
  clippy's `float_cmp` makes `assert_eq!` on two computed floats unreachable in
  a test, and neither said that the lint exempts itself by the enclosing
  function's name. Measured: a test named `eq`, `ne`, `is_nan`, or one starting
  `eq_` or ending `_eq`, compiles clean. Latent, since nothing in `crates/` sits
  in that shape, and an instance of the bundle's own rule that a check is
  believed only in the shape it has been seen to fail in.
- **Update**: Recorded in
  [a claim earns a test](principles/a-claim-earns-a-test.md) that a check is not
  believed until it has been seen to go red, and that a parser beats a pattern
  for anything structured. The bundle's own invariants are now a test rather
  than a shell pipeline, each confirmed to fail against a deliberately broken
  bundle.
- **Update**: Decided in [amount](domain/amount.md) that the read accessor
  stays, is a deliberate hole, and is renamed when a currency lands — at which
  point its name is the only warning left. Recorded the derived reading that
  reaching through the hatch is evidence an operation is missing rather than
  that the hatch is acceptable, which is what put Amount ÷ Amount into the
  library.
- **Restructure**: Split the flat `concepts/` directory into
  [principles](principles/) and [domain](domain/), each with its own index, and
  gave the root index a routing table. §8's stated purpose for index files is
  progressive disclosure — letting an agent see what is available before opening
  documents — which the previous single flat listing did not serve.
- **Creation**: Added
  [code and bundle change together](principles/code-and-bundle-change-together.md),
  a standing rule that every change asks what it means for the other side, and
  that anything learned asks whether it is a lesson rather than a local fix.
- **Creation**: Added
  [failures are classified by remedy](principles/failures-are-classified-by-remedy.md)
  and [a claim earns a test](principles/a-claim-earns-a-test.md), both extracted
  from an adversarial review of the first implementation rather than composed in
  the abstract.
- **Update**: Corrected [amount](domain/amount.md) — the tolerance is a type
  built one named term at a time, underflow is recorded as a failure alongside
  overflow, and the claim that reflexive equality makes sorting, deduplication
  and key use available was too strong on two counts.
- **Update**: Corrected
  [simple accumulation factor](domain/simple-accumulation-factor.md) — its
  two-way failure table was falsified by the implementation, and fused
  multiply-add is recorded as required rather than incidental.
- **Update**: Recorded in [future value](domain/future-value.md) that the source
  gives one period referred to twice rather than two that must agree, so the
  formula is agnostic to what a period is.
- **Creation**: Added the quantities — [amount](domain/amount.md),
  [simple interest rate](domain/simple-interest-rate.md),
  [elapsed periods](domain/elapsed-periods.md),
  [simple accumulation factor](domain/simple-accumulation-factor.md) — and the
  operation's shape, completing the model of `FV = PV(1 + rt)`.
- **Initialization**: Started the bundle with
  [future value](domain/future-value.md) under simple interest, and
  [illegal states are unrepresentable](principles/illegal-states-unrepresentable.md)
  and [the bundle is revisable](principles/the-bundle-is-revisable.md) as the
  first standing rules.
