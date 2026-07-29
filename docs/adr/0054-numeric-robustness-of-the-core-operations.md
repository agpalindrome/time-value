# ADR-0054: Numeric robustness of the core operations — termination, finiteness, conditioning, and what counts as a root

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0027](0027-amortization-schedule.md) (the schedule's
  termination contract and the condition `with_payment` rejects),
  [ADR-0020](0020-robust-irr-newton-with-bisection-fallback.md) /
  [ADR-0021](0021-fallible-operations-on-non-finite-results.md) (the solver's
  convergence test is now relative to the terms present at the candidate rate,
  not to a scale fixed in advance)
- **Follows:** [ADR-0033](0033-core-domain-model-two-axes-and-an-f64-engine.md)
  (the crate is an `f64` engine with an approximate-real precision contract),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (enforce the invariant
  at the chokepoint; test the class, not the instance)

## Context

An adversarial pre-publication review reproduced four defects in the core. Each
is a case where the code was correct in exact arithmetic and wrong in `f64`, and
in each case a stated promise — a rustdoc line, a type invariant, or a solver's
own correctness argument — was false. Nothing is published (ADR-0038), so all
four are cheap to fix now and permanent if they ship.

### 1. `Schedule` could be an infinite iterator

`Schedule`'s rustdoc promised installments "until the balance is retired". It did
not always end. Each period computed `principal = payment − interest` and then
`balance − principal`; once that reduction fell below the ULP of the balance,
`balance − principal == balance` and the schedule froze, yielding the same
installment forever.

`with_payment`'s guard, `payment <= principal * rate`, did not catch it. That is a
*first-period* test asking whether the payment strictly exceeds the interest, and
in every reproduction it did — by an amount too small to be **representable**:

```text
with_payment(rate 0.05, payment 1000.000000000001, principal 20_000)
    // interest 1000, reduction 1e-12, ULP of 20_000 ≈ 3.6e-12 → balance unmoved
for_term(rate 0.2, periods 200, principal 100_000)
    // the sized payment exceeds the interest by ~3.6e-12; ULP ≈ 1.5e-11
with_payment(rate 0.0, payment 1e-8, principal 1e9)
    // ULP of 1e9 ≈ 1.2e-7
with_payment(rate -0.10, payment -50, principal 1000)
    // healthy for ~330 periods, then converges on 500 asymptotically and stalls
```

The consequences were not academic. The CLI died with `memory allocation of
2684354560 bytes failed`; a debug build panicked at `self.period += 1` with
"attempt to add with overflow" once the `u32` wrapped at `2³²`; and the **MCP
server takes these three `f64`s straight from tool arguments**, so a
non-terminating loop was reachable from untrusted input.

The last reproduction is the one that shapes the fix: with a negative rate the
balance falls *every* period, genuinely, and converges on a positive limit. No
up-front check can reject it — it is only at period ~332 that the reductions
finally round to nothing.

### 2. `Money::round_to_currency` could return a non-finite `Money`

money.rs and lib.rs both state "Every `Money` is finite", and ADR-0021 makes an
operation fallible exactly when its result can be non-finite. `round_to_currency`
returns `Self`, and computed:

```rust
magnitude: crate::math::round(self.magnitude * scale) / scale
```

writing the result straight into the struct, past `from_operation`'s finiteness
funnel. `Money::new(f64::MAX, Currency::Usd)?.round_to_currency().value()` was
`inf`. The threshold is `f64::MAX / scale` — about `1.8e306` for USD, lower for
the 3- and 4-decimal currencies, unreachable for JPY (scale `1`). The existing
rounding properties bound their magnitudes to `±1e9`, three hundred decades below
the boundary, which is why they never saw it.

On the same line, `[1.0, 10.0, 100.0, 1000.0, 10_000.0][exponent as usize]`
indexed a five-slot array by the minor-unit exponent. The exponents in use are
exactly `{0, 2, 3, 4}` — zero headroom. A future ISO code with five decimals would
be an out-of-bounds panic inside a `#[must_use] -> Self` method with no way to
report it.

### 3. `present_value_factor` cancelled, and `annuity::rate` returned the wrong sign

The present-value annuity factor was the literal closed form,
`(1 - (1 + r)⁻ⁿ) / r`. For small `r`, `(1 + r)⁻ⁿ` is a number just below `1`, and
subtracting it from `1` discards every significant digit of the answer. The factor
stopped being **monotone in `r`**:

```text
annuity::present_value(r = 0,    n = 12, pmt = 1000) = 12000.0000000000
annuity::present_value(r = 1e-9, n = 12, pmt = 1000) = 12000.0008818621  ← larger, at a positive rate
annuity::present_value(r = 2e-9, n = 12, pmt = 1000) = 11999.9994940834
true value at r = 1e-9 (60-digit decimal)            = 11999.9999220000
```

Monotonicity is not a nicety here: `solve_rate`'s own rustdoc cites it as the
reason its residual has a single root. With it broken, round-tripping the
library's own present value back through `annuity::rate` returned the **wrong
sign** — a true `r = 1e-9` solved to `−1.12e-8`, a true `r = 5e-10` to `−2.06e-9`.

`future_value_factor` has the mirror-image cancellation (`(1 + r)ⁿ` is just
*above* one), and both growing factors have the same shape in the spread `r − g`.
The growing factors additionally papered over it with a `|r − g| < 1e-9` band that
returned the limit `n/(1+r)`, which is not the answer — at a spread of `1e-9` the
band was wrong in the eighth digit against a term-by-term summation.

### 4. The IRR accepted absurd roots

Acceptance was `|NPV(r)| < 1e-9 · Σ|CFₜ|`, a tolerance fixed in advance from the
raw inputs (ADR-0021). But `NPV(r) → CF₀` as `r → ∞`: every term except the first
is discounted away. When `|CF₀|` is below that fixed tolerance, **every
sufficiently large rate passes**.

`[0, 0, -100, 110]` is an ordinary construction-phase shape — nothing for two
periods, then an outlay and a repayment. Its IRR is exactly `0.1` and it is
unique: `−100(1+r)⁻² + 110(1+r)⁻³ = 0` gives `1 + r = 1.1`. It answered:

```text
internal_rate_of_return_from(0.9) → Ok(28114.440732477335)   // 2,811,444% per period
internal_rate_of_return_from(5.0) → Ok(22157.828871701382)
```

Giving the first flow a magnitude above the tolerance (`[-1e-6, 0, -100, 110]`)
made the spurious roots vanish, which confirms the mechanism. The default guess of
`0.1` finds the true root, so this is reachable through
`internal_rate_of_return_from` — a public entry point whose documented purpose is
steering the solver. XIRR shares the code path and has the same `XNPV(r) → CF₀`
limit.

## Decision

### 1. A schedule terminates, and an unamortizable loan is rejected loudly

Both. They are not alternatives — each catches cases the other cannot.

- **In the iterator.** `Schedule::next` ends the schedule whenever a period does
  not **strictly** reduce the balance. This is the only thing that can catch the
  asymptotic case, which is healthy for hundreds of periods before it stalls. A
  non-reducing period is fatal rather than merely unproductive: nothing about the
  schedule's state changes across it, so every subsequent period is identical.
  Termination then follows structurally — the balance is a strictly decreasing
  sequence of `f64`s, and there is no infinite one.
- **In the constructor.** `with_payment` runs that same test once, on the first
  period, and returns `TvmError::PaymentDoesNotAmortize` if it fails. An
  unamortizable loan is a caller error, not an empty schedule handed back without
  comment. `for_term` inherits this, and can now reject a term it used to hang on.

The two checks are the *same code*: `with_payment` calls the private
`Schedule::step` that `next` calls. The constructor and the iterator cannot drift
apart about what "amortises" means (ADR-0045: enforce the invariant at the
chokepoint).

`next` also advances the period with `checked_add`, ending the schedule rather
than wrapping or panicking at `2³²`.

`PaymentDoesNotAmortize` widens accordingly: it now covers "the payment does not
reduce the balance", of which "`PMT ≤ PV·r`" is the arithmetic case. Its rustdoc
and `Display` say so.

### 2. Rounding guards the multiply, and the scale lookup is total

`round_to_currency` computes `magnitude · scale`, and if that is not finite
returns the amount **unchanged**.

This is **exact, not a fallback**. Overflow needs `|magnitude| > f64::MAX / scale`,
which is at least `≈1.8e304`. Every `f64` above `2⁵³ ≈ 9.0e15` is already an
integer, so a magnitude that large is already an exact multiple of any minor unit
and rounding it is the identity — returning it unchanged *is* the right answer,
not an approximation of it. And only the multiplication can overflow: if
`magnitude · scale` is finite then `round(magnitude · scale) / scale` is finite
too, because `scale ≥ 1`.

The signature stays `-> Self`. Nothing about this needs to become fallible.

The array lookup becomes a `match` returning `Option<f64>`, and an exponent past
the table leaves the amount alone instead of panicking. A test walks
`Currency::ALL` and asserts every minor-unit exponent maps to a scale, so the
`None` arm stays unreachable: a currency that outgrows the table fails a test
rather than shipping a panic.

### 3. The annuity factors are computed with `expm1` / `log1p`

`crate::math` gains `exp_m1` and `ln_1p` (`f64::exp_m1`/`f64::ln_1p` under `std`,
`libm::expm1`/`libm::log1p` under `libm`), so the fix holds across the whole
feature matrix. The four factors become:

```text
present_value_factor(r, n)             = −expm1(−n·ln1p(r)) / r
future_value_factor(r, n)              =  expm1( n·ln1p(r)) / r
growing_present_value_factor(r, g, n)  = −expm1(n·ln1p(−spread/(1+r))) / spread
growing_future_value_factor(r, g, n)   = (1+r)ⁿ · growing_present_value_factor(r, g, n)
```

None of these ever forms the near-`1` intermediate whose subtraction was the
cancellation. Two details earn their keep:

- The growing factor keeps everything in terms of the **spread**. The ratio
  `(1+g)/(1+r)` is never formed; `ln((1+g)/(1+r))` is rewritten as
  `ln1p(−spread/(1+r))`, which is accurate right down to the limit.
- The growing *future*-value factor is computed as exactly what its rustdoc has
  always said it is — the present-value factor compounded forward by `(1+r)ⁿ` —
  rather than as a second closed form differencing two powers. The identity also
  carries the limit (`(1+r)ⁿ · n/(1+r)` *is* `n·(1+r)ⁿ⁻¹`), so there is one limit
  branch across the two factors instead of two that must be kept in step.

The fuzzy `|r| < 1e-9` and `|r − g| < 1e-9` bands go. The reformulated factors are
accurate at every rate but `0` (and every spread but `0`), where the closed form
is genuinely `0/0`, so the guard is now the exact comparison it should always have
been. `RATE_NEAR_ZERO` survives for `annuity::periods` /
`periods_from_future`, which divide by `ln(1 + r)` and do need a band.

### 4. A residual is judged against the terms that produced it

`root` gains a `Residual { value, scale }`: every sample of the function being
driven to zero carries, alongside its value, the sum of the magnitudes of the
terms combined to produce it **at that same rate** — `Σ|CFₜ|(1+r)⁻ᵗ` for an NPV,
`|PMT·a(r,n)| + |target|` for a solve-for-rate. A sample is a root when
`|value| < 1e-9 · scale`.

That closes the loophole without bounding anything. As `r → ∞` the NPV tends to
`CF₀` and the scale tends to `|CF₀|`, so the ratio tends to `±1` — never to `0`.
The spurious roots are rejected because the residual is, relative to what is left
at that rate, as large as it can possibly be. At the true root the NPV is
genuinely zero while the scale is the full discounted magnitude of the series, so
legitimate roots are unaffected: `[0, 0, -100, 110]` now returns `0.1` from every
guess tried, and the non-conventional `[-100, 230, -132]` still steers to `0.10`
or `0.20` according to the guess, which is what guess-steering is for.

The scale must not be floored at `1` the way `relative_tolerance` floored it —
that floor is precisely what re-admits an absolute tolerance at large `r`. It is
therefore removed, and `relative_tolerance` with it. A non-finite scale would make
the tolerance infinite and accept everything, so it is rejected outright.

The change is applied uniformly to IRR, XIRR, and `annuity::solve_rate`. XIRR has
the identical `XNPV(r) → CF₀` limit and needed it. `solve_rate` is the same shape
one level of indirection away — a target near zero would otherwise admit any rate
large enough to price the stream down to nothing — and uniformity means there is
one rule about what a root is, not two.

## Consequences

- **The library terminates on every input it accepts.** `Schedule` was the only
  unbounded loop reachable from public API, and it was reachable from MCP tool
  arguments. Two proptest properties now assert termination and strict balance
  reduction over the *whole* `with_payment` domain, not just the well-behaved
  slice a generator would otherwise pick.
- **A few inputs that used to hang now return an error.** `for_term(0.2, 200,
  100_000)` is the honest example: the level payment exceeds the first period's
  interest by `3.6e-12` against a `1.5e-11` ULP, so no `f64` schedule retires that
  balance. This is a real behaviour change and the right one — the alternative on
  offer was an infinite iterator.
- **`Money`'s finiteness invariant holds without a signature change.** The
  exactness argument is what buys that: were the unchanged return merely a
  plausible fallback, the honest fix would have been `-> Result` and a breaking
  change.
- **The annuity factors are a few ULP different everywhere**, in both directions:
  measurably *better* near a zero rate or a vanishing spread (where they were
  catastrophically wrong), and up to ~20 ULP worse in the far field, where
  `exp∘log` composes two roundings that `powf` does in one. That trade is
  deliberate and consistent with ADR-0033's approximate-real contract: the crate's
  correctness arguments depend on monotonicity, not on the last two bits.
- **Solver tolerances are tighter in the far field and unchanged near a root.** A
  rate is now resolvable only to the point where the residual is negligible against
  the terms still being discounted; for `annuity::rate` at a near-zero rate that
  floor is about `3e-10`, and the round-trip tests state it rather than assuming
  something finer.
- **`TvmError::PaymentDoesNotAmortize`'s message changed** from "payment does not
  exceed the interest accruing…" to "payment does not reduce the balance it is
  meant to retire…", because the first is false for the floating-point case. No
  variant, signature, CLI command, flag, or MCP tool changed; the CLI's assertion
  on the phrase "never amortised" still holds.
- **Standing obligation.** A new currency whose minor-unit exponent exceeds `4`
  must extend `minor_unit_scale`; the exhaustive test over `Currency::ALL` will
  say so.

## Alternatives considered

- **Cap the schedule at some maximum number of periods.** Arbitrary — any cap is
  either too low for a real 40-year monthly loan or too high to prevent the CLI
  running out of memory. Strict reduction is the actual invariant and needs no
  number.
- **Yield the stalled installment before stopping.** It would report a period that
  repaid nothing and left the balance where it was — noise, and it invites a caller
  to treat the schedule as complete.
- **Only guard the constructor, or only the iterator.** Neither alone is enough:
  the asymptotic negative-rate case passes any first-period check, and an
  unamortizable loan that returns `Ok` and then yields nothing is a silent failure.
- **Make `round_to_currency` fallible (`-> Result<Self, TvmError>`).** A breaking
  change to buy nothing: the overflow case has an exact answer, so there is no
  error to report.
- **A compile-time assertion tying the scale array to the maximum exponent.**
  Appealing, but `minor_unit_exponent` is a runtime match over ~180 enum variants
  and const-evaluating it over `Currency::ALL` is far more machinery than a test
  that reads in one line and fails just as loudly.
- **A series expansion for small `r·n`, keeping `powf` elsewhere.** Two code paths,
  a threshold to justify, and a seam between them where monotonicity would have to
  be re-argued. `expm1`/`log1p` are the same idea done once, in libm, correctly
  rounded.
- **Keep `powf` for the future-value factors, where it is a shade more accurate.**
  It would leave the same cancellation in the FV factor that broke the PV one, for
  a benefit measured in ULP.
- **Bound the IRR search domain** (reject roots above, say, 1000%). Blunt: short-
  period venture and bridge cashflows have legitimately enormous per-period rates,
  and a bound would reject them while still admitting a spurious root just inside
  it. The residual test rejects meaningless roots because they are meaningless, at
  any magnitude.
- **Reject series whose first cashflow is zero.** It treats a symptom — the same
  collapse happens with a small-but-nonzero `CF₀` — and rules out a perfectly
  ordinary construction-phase shape.
- **Tighten the relative tolerance from `1e-9`.** No tolerance fixed in advance
  survives `r → ∞`; the scale has to move with the rate.
