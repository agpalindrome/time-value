# ADR-0052: `TvmError` variant granularity — a payload on `CurrencyMismatch`, and `Undefined` split into named degenerate cases

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0004](0004-error-handling.md) (its "one variant per
  distinguishable failure" claim, now actually honoured),
  [ADR-0031](0031-split-non-finite-result-into-overflow-and-undefined.md) (its
  `Undefined` half — the `Overflow` half is unchanged)
- **Follows:** [ADR-0021](0021-fallible-operations-on-non-finite-results.md)
  (operations are fallible when their result can be non-finite),
  [ADR-0034](0034-money-and-currency.md) (currency is a runtime value),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (make illegal states
  unrepresentable; test the class, not the instance)

## Context

`TvmError` is `#[non_exhaustive]`. That attribute is asymmetric, and the
asymmetry is the whole reason this ADR exists **now** rather than later:

- **Adding a variant is non-breaking.** Callers must already carry a wildcard
  arm, so a new variant compiles against existing `match`es.
- **Adding a field to an existing variant is breaking.** A unit variant
  `TvmError::CurrencyMismatch` is matched as `TvmError::CurrencyMismatch`;
  turning it into `CurrencyMismatch { .. }` breaks every one of those patterns.
  `#[non_exhaustive]` on the *enum* does nothing for this — only
  `#[non_exhaustive]` on the *variant* would, and that would in turn stop
  callers constructing the variant at all.

Every variant was a unit variant. Two consequences follow, both permanent the
moment the crate is published (nothing is published yet — ADR-0038).

### 1. `CurrencyMismatch` could never say which two currencies clashed

ADR-0034 made currency a **runtime value**, precisely because it is chosen at
runtime and is not knowable when the model is written. The error reporting that a
mismatch occurred, without naming the two values involved, discards exactly the
runtime information the design decided to carry. A caller rendering a message —
the CLI, the MCP server, or any downstream consumer — could say no more than
"amounts are in distinct currencies", which for a series of a hundred flows is
close to useless. The information exists at the construction site (`combine`
holds both currencies) and was thrown away one line later.

### 2. `Undefined` meant at least eight different things

ADR-0031 split `NonFiniteResult` into `Overflow` ("a real result, too large to
represent") and `Undefined` ("no answer exists"). The `Overflow` half of that
split has held up well. The `Undefined` half only moved the problem: it became
the single name for **thirteen guarded preconditions across six modules**, which
between them cover a zero term, a payment that cannot cover interest, a division
by zero, a non-finite scalar operand, a logarithm with no real value, and a
series with no outflows. A caller matching `Err(TvmError::Undefined)` learns that
*something* was degenerate and nothing else; the variant's own rustdoc had to
list examples because the name could not carry the meaning.

Its rustdoc also promised more than the type delivered — ADR-0004's stated design
is "one variant per distinguishable failure", and thirteen distinguishable
failures under one variant is the opposite.

Splitting later is *technically* non-breaking (new variants are allowed), but it
is **silently behaviour-changing**: an existing `match TvmError::Undefined => …`
arm keeps compiling while quietly ceasing to catch the conditions moved out from
under it. That is the worst kind of change to ship post-publication, and the
reason to do it while nothing is published.

## Decision

### `CurrencyMismatch` carries both currencies

```rust
CurrencyMismatch { left: Currency, right: Currency },
```

`left` and `right` are named for the order the operation combined them, which is
the only ordering that is true at every construction site:

- `combine(a, b)` — the shared fold used by `Money::try_add`/`try_sub`,
  `Cashflows::currency`, `DatedCashflows::currency`, and
  `Schedule::with_payment`. When folded over a series, `left` is the currency
  accumulated from the flows so far and `right` is the offending flow's.
- `Money::convert` — `left` is the amount's own currency, `right` is the
  `FxRate`'s `from`, which it must match.

`Display` uses them: `"cannot combine USD with EUR"`. `Currency` is already in
the `no_std`, zero-dependency core and is `Copy`, so the payload costs nothing
and keeps `TvmError` `Clone + PartialEq + Eq`.

Alternative field names were considered and rejected. `{ expected, found }`
reads well for `convert` but is a fiction for `combine`, where neither side is
expected. `{ first, second }` says less than `left`/`right` about a binary
operation.

### `Undefined` is **removed**, replaced by six named variants

Every one of the thirteen sites maps to a variant that says what actually went
wrong, so no residual catch-all remains. Removing it is a breaking change and
that is the point: a caller matching `Undefined` today should be forced to look
at what it now catches, rather than silently catching less.

| Old `Undefined` site | Condition | New variant |
| --- | --- | --- |
| `annuity::payment` | `periods == 0` — the annuity factor is `0`; nothing to amortise over | `ZeroPeriods` |
| `annuity::due::payment` | `periods == 0`, same factor | `ZeroPeriods` |
| `single_sum::rate` | `periods <= 0` — no elapsed time, so no rate is implied | `ZeroPeriods` |
| `Schedule::for_term` (via `annuity::payment`) | zero term | `ZeroPeriods` |
| `Cashflows::modified_internal_rate_of_return` | fewer than two flows, so `N = 0`: no span to annualise over | `ZeroPeriods` |
| `Schedule::with_payment` | `PMT ≤ principal · r` — the payment never exceeds the first period's interest | `PaymentDoesNotAmortize` |
| `annuity::periods` (general branch) | `1 − PV·r/PMT ≤ 0` — i.e. `PMT ≤ PV·r`, the same condition, reached through the logarithm | `PaymentDoesNotAmortize` |
| `annuity::periods` (`r → 0` branch) | zero payment against a balance — retires nothing at any rate | `PaymentDoesNotAmortize` |
| `single_sum::periods` | `rate == 0` (nothing grows), or `FV/PV` not positive-finite (no real logarithm; includes a zero `present`) | `NoRealSolution` |
| `annuity::periods_from_future` (general branch) | `1 + FV·r/PMT ≤ 0` — payment and target inconsistent in sign or magnitude | `NoRealSolution` |
| `annuity::periods_from_future` (`r → 0` branch) | zero payment — nothing ever accumulates | `NoRealSolution` |
| `Cashflows::modified_internal_rate_of_return` | `present_outflows == 0` — no investment to measure a return on | `NoOutflows` |
| `Money::try_div` | `divisor == 0.0` — no defined quotient, including `0 / 0` | `DivisionByZero` |
| `Money::try_div` | `divisor.is_nan()` | `NonFiniteScalar` |
| `Money::try_mul` | `!factor.is_finite()` | `NonFiniteScalar` |

(Fifteen rows for thirteen sites: `single_sum::periods` and `Money::try_div`
each guarded two conditions in one `if`, and one of the `try_div` pair now
routes elsewhere.)

The variants, and what a caller does about each:

- **`ZeroPeriods`** — "the term is empty." Supply a positive `Period`, or a
  series with at least two flows. Distinct from the existing `NegativePeriods`,
  which rejects a *negative* count.
- **`PaymentDoesNotAmortize`** — "the payment never retires the balance." Raise
  the payment above the first period's interest. This is the single most likely
  real-world failure in the crate (a loan payment sized too small), and it is
  the one condition a user-facing message most wants to name.
- **`NoRealSolution`** — "the closed form has no real answer here." Distinct from
  `SolveDidNotConverge`, where an answer may exist but the iteration did not
  find it; here the arithmetic proves there is none.
- **`NoOutflows`** — "the series has no investment." Sits beside the existing
  `EmptyCashflows` (no flows at all) and is placed next to it in the enum.
- **`DivisionByZero`** — the famous one, and a condition a caller can plausibly
  hit from live data (dividing a total by a count that turned out to be zero)
  rather than from a broken computation.
- **`NonFiniteScalar`** — a `NaN`/infinite `f64` operand supplied by the caller.
  This one is a **reclassification, not just a rename**: ADR-0031 filed it under
  "degenerate result", but a non-finite factor is a *bad input*, of a piece with
  `NonFiniteAmount`, `NonFiniteRate` and `NonFiniteOffset`. It now joins that
  family, which is why it sits with them in the enum rather than with the
  degenerate cases.

### Why six, and not more or fewer

The grouping test applied was: **could a caller plausibly act differently on
these two conditions?** Not "are they arithmetically distinct" — that would give
one variant per `if`, which is noise — and not "are they both degenerate", which
is the status quo being fixed.

Three places where that test produced a *merge* rather than a split are worth
recording, because each is a judgement call:

- **`single_sum::periods` keeps one guard for two conditions** (`rate == 0` and a
  non-positive/non-finite ratio). Both mean "there is no `n`", for different
  arithmetic reasons, and a caller does the same thing about either. Splitting
  would have named the *cause* rather than the *consequence*.
- **`Money::try_mul` and `Money::try_div` share `NonFiniteScalar`.** The two
  operands are called "factor" and "divisor", but the caller always knows which
  method they invoked, so the method disambiguates the operand. What it cannot
  disambiguate is *zero* versus *`NaN`* on a divisor — different faults with
  different fixes — which is why that one **is** split.
- **MIRR's "fewer than two flows" is `ZeroPeriods`, not a series-shaped
  variant.** A one-flow series spans `N = 0` periods; that is the same failure as
  a zero `Period`, expressed through a different input. Its *other* degenerate
  case (no outflows) is genuinely about the series' shape, so it gets
  `NoOutflows`.

And one place where the test produced a **split of arithmetically identical
conditions**: `annuity::periods` and `annuity::periods_from_future` both fail on
a non-positive logarithm argument, but they get different variants. In
`periods`, the argument going non-positive is exactly `PMT ≤ PV·r` — a named,
highly actionable economic condition, and the same one `Schedule::with_payment`
rejects, so the two surfaces agree. In `periods_from_future` nothing is being
amortised, so there is no interest threshold to name; the failure is a sign or
magnitude inconsistency between the payment and the target. Naming them apart
serves the caller; naming them alike would serve the arithmetic.

### No residual `Undefined`

Every site maps to a specific variant, so the catch-all is removed rather than
retained "just in case". Keeping it would leave the exact hazard being fixed: a
future degenerate precondition could be filed under the vague name instead of
earning one, and the enum would drift back. `#[non_exhaustive]` means a genuinely
new kind of degeneracy can be added as a new variant when one appears, which is
the correct mechanism.

This supersedes ADR-0031's standing instruction ("a known degenerate precondition
is guarded and returns `Undefined`"). The rule now reads: **a known degenerate
precondition is guarded and returns a variant that names it** — an existing one
if it fits, a new one if it does not; only a genuine overflow reaches a
`from_operation` funnel and returns `Overflow`.

### Testing (ADR-0045 rule 2)

Every new variant is pinned by a test asserting the *specific* condition that
produces it, and the tests that previously asserted `Err(TvmError::Undefined)`
now assert the precise variant — which is the change made visible. Four branches
that were reachable but untested gained tests: `annuity::periods`' zero-payment
limit branch, both `annuity::periods_from_future` guards, and
`single_sum::periods`' non-positive-ratio guard.

The `CurrencyMismatch` payload earns three: that both currencies are carried, in
the order the operation combined them; that the *rendered* `Display` names both
(`"cannot combine USD with EUR"`), since the payload existing but not reaching
the message would defeat the purpose; and that `Money::convert` reports the
amount's currency and the rate's `from` in that order.

Two `# Errors` sections that named no currency error at all, despite the
functions folding currencies, are corrected and pinned:
`Cashflows::net_present_value` / `net_future_value` (they call `self.currency()`)
and `Schedule::with_payment` (it calls `combine`). Neither had a test; both do
now.

## Consequences

- **This is a breaking change**, twice over: `CurrencyMismatch` gains fields, and
  `Undefined` is gone. Nothing is published (ADR-0038), so no released API moves,
  and doing it now is the entire point — after publication the second half could
  only be done by a silent behaviour change or a major bump.
- A caller can distinguish, and report, the six degenerate cases and the two
  currencies in a mismatch. `TvmError` remains `Debug + Clone + PartialEq + Eq +
  Display + core::error::Error`, and `#[non_exhaustive]`. It did not derive
  `Copy` before and still does not, so nothing relied on that.
- **The wire format is untouched.** `TvmError` does not cross the
  `serde`/`schemars` boundary — it is not `Serialize`, has no `*Wire` struct, and
  appears in neither `serde_impls.rs`, `schemars_impls.rs` nor `wire.rs` — so
  ADR-0042/0044 are unaffected.
- **The binaries' surfaces are unchanged**: the CLI grammar and the MCP tool
  names, input schemas and output schemas are byte-identical. The MCP server maps
  every `TvmError` through one Display-based `invalid_params` mapper, so it
  reports the finer messages with no code change at all.
- The CLI's messages improve at the six sites that could return an ex-`Undefined`
  variant. Those sites previously wrapped the error in a static
  `.context("… is undefined for these inputs")`, and since `main` prints only the
  outermost message (ADR-0028 / issue #30, to avoid doubling text), the library's
  message never reached the user. That was the right call when the context
  *restated* the error; it is the wrong one now that the library says more than
  any static string could. Those sites now build a single message that names the
  operation and interpolates the error — `"amortization schedule: payment does
  not exceed the interest accruing, so the balance is never amortised"` — so
  nothing is doubled and nothing is lost. The rest of the CLI's contexts are
  unchanged.
- Follow-on obligation: a new degenerate precondition earns a variant that names
  it. "It is degenerate" is no longer an available answer.

## Alternatives considered

- **Add the `CurrencyMismatch` payload, but leave `Undefined` alone.** Half the
  work, and the half with the smaller payoff: the currency payload is a genuine
  improvement but affects one variant, while `Undefined` is the variant thirteen
  sites hide behind. It is also the half that *must* be done first, since adding
  fields is the breaking direction — doing only it and deferring the split would
  spend the breaking change and keep the defect.
- **Keep `Undefined` as a residual alongside the new variants.** Superficially
  safer, and it would make the change non-breaking for callers who match it.
  Rejected: nothing maps to it, so it would be a documented lie, and its
  existence invites the next degenerate case to be filed under it rather than
  named. A `#[non_exhaustive]` enum already provides the escape hatch a residual
  variant pretends to be.
- **One variant per guarded condition (thirteen or more).** Faithful to the code
  and useless to a caller: `AnnuityPaymentOverZeroPeriods` and
  `DuePaymentOverZeroPeriods` are one condition reached from two functions, and
  nobody branches on which. It would also freeze implementation detail into the
  public API, so refactoring a guard would become a breaking change.
- **Two variants — `DegenerateInput` and `NoSolution`.** A tidy dichotomy that
  collapses under its own examples: a zero term, a payment below interest and a
  division by zero are all "degenerate input", yet a caller fixes each
  differently, and telling a user "degenerate input" is barely better than the
  `Undefined` being replaced.
- **An error *struct* with a kind plus a context payload** (`TvmError { kind,
  detail }`). More extensible in principle, and it would carry richer data than
  two currency codes. Rejected: it discards the `match`-on-variant ergonomics
  ADR-0004 chose deliberately, and a rich payload wants owned strings or a
  formatter, which the `no_std`, zero-dependency, `alloc`-free-by-default core
  (ADR-0009) will not take. A payload of two `Copy` enum values is what this core
  can afford.
- **Attach payloads to the degenerate variants too** — e.g. `ZeroPeriods` naming
  which argument was zero, `PaymentDoesNotAmortize` carrying the payment and the
  interest it failed to cover. Genuinely useful, and deliberately deferred: the
  variant names already tell the caller what to fix, and every payload is another
  permanent commitment to a field set. Adding a *variant* stays cheap; the time
  to reconsider is when a concrete consumer needs a number it cannot recompute
  from its own inputs.
