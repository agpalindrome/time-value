---
okf_version: "0.2"
---

# time_value

The knowledge behind this library: the finance it implements, and where each
claim came from. It grows one concept at a time.

# Concepts

- [Future value](concepts/future-value.md) - The value at a future date of a
  single present amount earning simple interest.
- [Amount](concepts/amount.md) - A quantity of money located at a point in time.
- [Simple interest rate](concepts/simple-interest-rate.md) - The rate per time
  period at which simple interest accrues.
- [Elapsed periods](concepts/elapsed-periods.md) - A length of time counted in
  the periods a rate is stated against.
- [Simple accumulation factor](concepts/simple-accumulation-factor.md) - The
  dimensionless multiplier by which simple interest grows an amount over a span.

# Principles

- [Illegal states are unrepresentable](concepts/illegal-states-unrepresentable.md) -
  A value that would be invalid should be impossible to construct, not merely
  detectable afterwards.
- [The bundle is revisable](concepts/the-bundle-is-revisable.md) - Content,
  structure and decomposition all change as understanding improves; none of it
  is settled by having been written.
