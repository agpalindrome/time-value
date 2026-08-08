# Directory Update Log

## 2026-08-08

- **Update**: Recorded in
  [failures are classified by remedy](principles/failures-are-classified-by-remedy.md)
  that the field a shared variant carries makes its class **computable** rather
  than merely readable, so an accessor answering "what would fix this" is a
  function of the variant and that field — no variant split, which is what had
  been proposed. Also recorded that the classification belongs to the library
  rather than each caller, since only inside the crate is the match exhaustive,
  and the limit that reading a class off a field rests on a partition nothing
  enforces.
- **Creation**: Added
  [we are this bundle's producer, not its consumer](principles/producer-not-consumer.md),
  which states the rule four house rules had each been arguing separately: a
  tolerance the spec addresses to a consumer is not a licence for the producer,
  so a finding about material this repo owns is a defect here. It fixes the
  policy `bundle-check` will apply once `okf-graph` 0.2 lands rule levels —
  dangling links and an out-of-order log become defects, while a tool's vintage,
  an unsettled upstream question and a surface we do not use stay reports. The
  distinction is ownership rather than severity. First concept here to cite the
  OKF specification as a source, which is the point of it.
- **Correction**: Reversed a claim in
  [simple accumulation factor](domain/simple-accumulation-factor.md) that a
  decimal representation without a fused multiply-add "computes different
  answers about which inputs are legal". It conflates having no fused operation
  with rounding twice. A decimal holds `0.05` exactly and multiplies it exactly
  within its scale, so it rounds once with nothing fused — and as written the
  claim disqualified the representation
  [Amount](domain/amount.md#the-representation-is-a-parameter)'s parameter
  exists to admit.
- **Update**: Recorded in [Amount](domain/amount.md) that the requirement a
  representation must meet is **one rounding at most** in `1 + rt`, not a fused
  multiply-add — fusion being how binary achieves it — and that the guarantee is
  therefore per-representation: two representations may each round once and
  still disagree about a factor near cancellation, because the guard's verdict
  is a question about how each stored the rate.
- **Update**: Recorded in
  [the bundle is revisable](principles/the-bundle-is-revisable.md) that a
  constraint adopted for an implementation's convenience is labelled with the
  implementation or it outlives it. The instance: the frontmatter check demanded
  every timestamp end in `Z`, which the spec does not, purely because staleness
  was compared on strings — and it came out cleanly today only because the rule
  said so where it was defined.
- **Update**: Corrected
  [code and bundle change together](principles/code-and-bundle-change-together.md),
  which told a reader to run `okf-graph` after touching the bundle. `okf-graph`
  is now a crate rather than a binary in the devshell, so the instruction named
  something no longer there; the checks it belongs to are
  `./scripts/check.sh test`.

## 2026-08-07

- **Update**: Recorded in
  [a claim earns a test](principles/a-claim-earns-a-test.md) that a confirmation
  done once, by hand, expires the moment the code changes — a check is believed
  while it is watched failing, not because it once was. The bundle's invariants
  now carry fixture bundles that re-run the red side, and the gap was
  demonstrated before it was closed: deleting one invariant's report left every
  assertion against the real bundle green.
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
