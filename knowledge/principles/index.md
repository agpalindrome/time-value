Standing rules. They govern every concept in this bundle and every change to the
code, so they are read once rather than per task.

Each was drawn from something that actually happened here — a defect, a
measurement, or a review finding — rather than composed in the abstract. Where
one names an example, the example is real.

# Design

- [Illegal states are unrepresentable](illegal-states-unrepresentable.md) - A
  value that would be invalid should be impossible to construct, not merely
  detectable afterwards.
- [Failures are classified by remedy](failures-are-classified-by-remedy.md) - A
  failure is named for what would fix it, and where two rules could both apply
  the unfixable one wins.

# Practice

- [A claim earns a test](a-claim-earns-a-test.md) - Anything this bundle asserts
  about implemented behaviour has a test that fails when the code stops
  honouring it.
- [Code and bundle change together](code-and-bundle-change-together.md) - Any
  change to one asks what it means for the other, and anything learned asks
  whether it is a lesson rather than a local fix.
- [The bundle is revisable](the-bundle-is-revisable.md) - Content, structure and
  decomposition all change as understanding improves; none of it is settled by
  having been written.
