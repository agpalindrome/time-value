The finance this library implements: the formulas, and the quantities they are
written in.

A formula names quantities; each quantity has its own concept holding its
domain, its arithmetic and what has been decided about representing it. Read the
formula first, then the quantities it names.

# Formulas

- [Future value](future-value.md) - The value at a future date of a single
  present amount earning simple interest.

# Quantities

- [Amount](amount.md) - A quantity of money located at a point in time. The
  largest concept here, and the one that settles representation, equality,
  ordering, comparison and rendering for everything else.
- [Simple interest rate](simple-interest-rate.md) - The rate per time period at
  which simple interest accrues.
- [Elapsed periods](elapsed-periods.md) - A length of time counted in the
  periods a rate is stated against.
- [Simple accumulation factor](simple-accumulation-factor.md) - The
  dimensionless multiplier by which simple interest grows an amount over a span.
