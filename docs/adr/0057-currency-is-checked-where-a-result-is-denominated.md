# ADR-0057: Currency is checked where a result is denominated

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0034](0034-money-and-currency.md) (currency as a runtime value
  on `Money`, and the `Xxx` identity rule)
- **Follows:** [ADR-0045](0045-make-illegal-states-unrepresentable.md) (pin every
  stated assumption), [ADR-0052](0052-tvmerror-variant-granularity.md) (the
  `CurrencyMismatch` payload)

## Context

ADR-0034 makes currency a runtime value and gives `Currency::Xxx` the role of the
identity element: `Xxx` combined with `C` yields `C`, equal currencies pass
through, and two distinct non-`Xxx` currencies are a `CurrencyMismatch`. The
series operations fold that rule over their flows to find the one currency their
result is denominated in.

Not every series operation folds it, and the split has never been stated:

| operation | returns | folds the currencies? |
| --- | --- | --- |
| `Cashflows::net_present_value` / `net_future_value` | `Money` | yes |
| `DatedCashflows::net_present_value` | `Money` | yes |
| `Schedule::with_payment` (and `for_term`) | denominated rows | yes |
| `Cashflows::internal_rate_of_return{,_from}` | `Rate<P>` | **no** |
| `Cashflows::modified_internal_rate_of_return` | `Rate<P>` | **no** |
| `DatedCashflows::internal_rate_of_return{,_from}` | `Rate<Annual>` | **no** |

So the same mixed-currency series is a `CurrencyMismatch` from `net_present_value`
and an `Ok(rate)` from `internal_rate_of_return`. An adversarial pre-publication
review found this asymmetry (issue #108) and observed that it was neither
documented nor tested — it was true by construction rather than by decision, which
is exactly the state ADR-0045 exists to end.

## Decision

**The currency fold is part of *producing* a `Money`, not of consuming one. State
that as the rule, document it on the rate-returning operations, and pin it with
tests. The behaviour does not change.**

Concretely:

- An operation whose result is a `Money` (or a row of them) folds its inputs'
  currencies by ADR-0034's identity rule, because it has to: the result needs one
  denomination to be stamped with, and there is no honest answer when the inputs
  name two. `CurrencyMismatch` is the only thing it can return.
- An operation whose result is a `Rate` does not, because there is nothing to
  derive. A rate has no denomination, so no currency of the inputs is privileged
  and none is contradicted. The answer it gives is the rate that zeroes the sum of
  the **bare magnitudes** — which is what a caller passing mixed currencies into a
  rate solve has, in fact, asked for.
- Each rate-returning operation carries a `# Currency` rustdoc section saying so,
  and pointing a caller who wants the strict reading at the fix: fold the
  currencies yourself, or call `net_present_value` once and discard the result.

The rule reads off the type of the result, so it needs no per-operation memory and
extends by itself: a future operation returning `Money` folds, a future operation
returning `Rate` or `Period` does not.

**Every clause above earns a test** (ADR-0045 rule 2), which is the substance of
this ADR's PR: the fold is exercised for all four series types across the whole
closed currency set, the `CurrencyMismatch` payload is pinned (the fold stops at
the first clash, so `left` is what accumulated before it and `right` is the flow
that broke it — deterministic, because the slice order is), and each rate-returning
operation is pinned to agree with its magnitude-only twin on a series its
`net_present_value` rejects.

## Consequences

- The asymmetry is a documented rule with a one-line test of the type signature
  behind it, rather than an accident a refactor could flip in either direction.
- A caller who wants a mixed series rejected everywhere has a stated way to get
  it, and it costs one call.
- `Cashflows::currency` and `DatedCashflows::currency` stay private: they exist to
  serve the denominated operations, and exposing them is a separate question
  (issue #104).
- The cost is accepted, not hidden: `npv` and `irr` disagree about whether a mixed
  series is well-formed, and the docs say so in the place a caller reading `irr`
  will see it.
- The rule is stated in terms of the *result* type, so it constrains new work: an
  operation returning `Money` may not skip the fold to save a pass over the flows.

## Alternatives considered

- **Make the rate solves reject a mixed series too.** The consistent-looking
  option, and there is a real argument for it: `Σ CFₜ x^t` mixing USD and EUR is
  as meaningless as the NPV of the same flows. Rejected here for two reasons.
  It is a behaviour change turning an `Ok` into an `Err` — the owner's call, not a
  tests-and-docs PR's — and it would put a currency check on an operation with no
  currency in its signature, which is where the confusion started. If it is ever
  wanted, it wants its own ADR and a note in the changelog.
- **Fold the currencies and discard the result**, purely to inherit the error. The
  cheapest route to the strict reading, and it is what a caller can already do in
  one line. Rejected as a default: it makes the operation fail for a reason its
  return type cannot express.
- **Leave it undocumented.** It is arguably obvious from the return types.
  Rejected: it was obvious to nobody in the review, and ADR-0045 rule 2 is that a
  behaviour worth relying on is a behaviour worth pinning.
- **A type-level fix** — a `Cashflows` that cannot hold mixed currencies at all,
  so the question never arises. Genuinely in the spirit of ADR-0045 rule 1, and
  rejected by that rule's own boundary: currency is a runtime value (ADR-0034), so
  a uniformly-denominated series type would need either a currency type tag (the
  thing ADR-0033 declined) or a validating constructor that makes `Cashflows::new`
  fallible — ceremony on every construction to catch a mistake the denominated
  operations already catch at the point it matters.
