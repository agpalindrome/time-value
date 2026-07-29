# ADR-0048: The finite growing annuity

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Follows:** [ADR-0015](0015-annuities.md) (annuities, and its amendment adding
  annuity-due and the perpetuities),
  [ADR-0014](0014-transcendental-single-sum-operations.md) (`std`/`libm` gating),
  [ADR-0021](0021-fallible-operations-on-non-finite-results.md) /
  [ADR-0031](0031-split-non-finite-result-into-overflow-and-undefined.md) (fallible
  non-finite results),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (testing and design
  discipline)

## Context

The [`annuity`](../../crates/time_value/src/annuity.rs) module models a payment
that repeats each period. Growth arrived with ADR-0015's amendment, but only in
the **infinite** case: `growing_perpetuity` prices a payment growing at `g`
forever, and `perpetuity` is its `g = 0` case. The **finite** growing annuity —
`n` payments, each `(1 + g)` times the last — has no counterpart, so the module
can price a growing stream that never ends but not one that ends in ten years.

That is a real hole rather than a deliberate omission. ADR-0015 does not
consider the finite growing case at all: its amendment adds the perpetuities and
annuity-due as the two extensions it judged worth having, and is silent here. The
gap is visible in ordinary use — an escalating lease or salary, a dividend stream
with a terminal horizon, and an inflation-indexed payment are all finite growing
annuities, and each currently has to be assembled by hand from `Cashflows`.

Three constraints bound the decision:

- **The `r = g` case is not an error.** Unlike a growing *perpetuity*, which
  genuinely diverges when `r ≤ g` (and which the constructors therefore reject
  with `DivergentPerpetuity`), a *finite* growing annuity converges for every
  `r` and `g` — a finite sum of finite terms. At `r = g` the closed form is
  `0/0`, but the limit is perfectly ordinary: every discounted payment is worth
  the same, so `PV = n · PMT / (1 + r)`.
- **The module already has a house pattern for exactly this.** `RATE_NEAR_ZERO`
  and `near_zero` switch the ordinary factors to their `r → 0` limit, because the
  closed form is `0/0` at zero and ill-conditioned near it. The growing factors
  need the same treatment, on the **spread** `r − g` rather than on `r`.
- **The surface must stay symmetric.** The module currently offers present and
  future value for both ordinary and annuity-due. Adding growth to only some of
  those four cells would leave an asymmetry a user has to memorise.

## Decision

**We will add the finite growing annuity across the full present/future ×
ordinary/due matrix — four functions**, `std`/`libm`-gated like every other
transcendental annuity operation (ADR-0014):

- `annuity::growing_present_value(rate, growth, periods, payment)`
- `annuity::growing_future_value(rate, growth, periods, payment)`
- `annuity::due::growing_present_value(rate, growth, periods, payment)`
- `annuity::due::growing_future_value(rate, growth, periods, payment)`

`payment` is the **first** payment — the one at the end of period 1 (ordinary) or
at the start of period 1 (due) — and each subsequent payment is `(1 + g)` times
the one before, so the `k`-th payment is `PMT · (1 + g)^(k−1)`.

**Two private factor helpers carry the mathematics**, mirroring the existing
`present_value_factor` / `future_value_factor`:

```text
growing_present_value_factor(r, g, n) = (1 − ((1+g)/(1+r))ⁿ) / (r − g)
growing_future_value_factor(r, g, n)  = ((1+r)ⁿ − (1+g)ⁿ) / (r − g)
```

**The `r → g` limit is taken on the spread**, reusing `near_zero` and so the
existing `RATE_NEAR_ZERO` threshold:

```text
growing_present_value_factor(r, r, n) = n / (1 + r)
growing_future_value_factor(r, r, n)  = n · (1 + r)^(n−1)
```

**The due variants scale by `(1 + r)`**, exactly as ADR-0015's amendment
established for the level case — every payment is brought forward one period.

**`rate` and `growth` share the periodicity `P`**, as `growing_perpetuity`
already requires, so a monthly rate with an annual growth is a compile error
rather than a silent unit mismatch (ADR-0005, ADR-0045).

**The only error is `Overflow`.** There is no `DivergentPerpetuity` here and no
rejection of `r ≤ g`: a finite growing annuity is defined for every rate and
growth pair. The factors compound with `powf`, so extreme magnitudes can still
overflow to a non-finite `Money`, which stays fallible per ADR-0021/0031.

**Every relation asserted above earns a test** (ADR-0045). The reductions are
properties over ranges, not worked examples: at `g = 0` each growing function
agrees with its level counterpart; the growing present value tends to
`growing_perpetuity` as `n` grows when `r > g`; growing future value is growing
present value compounded forward by `(1 + r)ⁿ`; and each due variant is its
ordinary counterpart times `(1 + r)`. The `r = g` limit is pinned against a
directly-summed reference.

## Consequences

- The annuity module becomes complete over its own two axes: {level, growing} ×
  {ordinary, due} × {present, future}. There is no longer a growing stream the
  module can price only when it runs forever.
- Purely additive. No existing signature, factor, or result changes;
  `growing_perpetuity` keeps its name and its `DivergentPerpetuity` rejection,
  and is now documented as the `n → ∞` limit of the new present value.
- The `r ≈ g` band inherits `RATE_NEAR_ZERO`'s trade-off: within `1e-9` of the
  spread the limit is used instead of the closed form, so the result is the limit
  rather than the (ill-conditioned) exact value. This is the same accuracy bargain
  the module already makes at `r ≈ 0`, and it is now made in one more place.
- The binaries do **not** yet surface these. ADR-0028 requires that every core
  operation reach both binaries, so this leaves a surfacing obligation, tracked as
  the immediate follow-up — the same core-first sequencing continuous compounding
  used (ADR-0036 core, then ADR-0041 binaries).
- Four more `powf` call sites, so four more places the `std`/`libm` gate applies.
  The default `no_std` build is unchanged.

## Alternatives considered

- **Present value only.** The single most-used form, and the smallest addition.
  Rejected: it would leave growing FV as a hole against the existing
  `present_value`/`future_value` pairing, and the future-value factor is three
  lines given the present-value one.
- **Ordinary only, deferring the due variants to a later amendment** — the
  precedent ADR-0015 itself set when it shipped ordinary first. Rejected here
  because the due variants are a single `(1 + r)` scaling of factors this ADR
  already introduces; deferring them buys nothing and leaves the matrix ragged.
- **Reject `r ≤ g`, as `growing_perpetuity` does.** Superficially consistent, and
  it would let both growing functions share one guard. Rejected because it is
  mathematically wrong for the finite case: the sum converges, and refusing to
  price a ten-year lease whose escalation outruns the discount rate would be a
  bug dressed as a validation.
- **A `growing` submodule mirroring `due`.** Reads well on its own
  (`annuity::growing::present_value`), but growth and timing are independent
  axes, so it forces either `annuity::growing::due::present_value` or a split
  where half the matrix lives in each module. Rejected in favour of flat
  `growing_`-prefixed names, which also match the existing `growing_perpetuity`.
- **A `growth: Option<Rate<P>>` parameter on the existing functions.** Avoids new
  names entirely, but changes four existing signatures for every caller who never
  wanted growth, and makes the common level case pay ceremony for the rare one.
  Rejected — ADR-0005's "type-heavy *and* friendly" cuts against it.
