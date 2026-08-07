# Directory Update Log

## 2026-08-07

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
