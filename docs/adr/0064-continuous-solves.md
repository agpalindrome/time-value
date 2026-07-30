# ADR-0064: The continuous solves — force of interest and span

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Amends:** [ADR-0036](0036-continuous-compounding-force-of-interest.md) (continuous
  compounding — its `continuous` module gains the two solves it never had),
  [ADR-0041](0041-continuous-compounding-in-the-binaries.md) (the `continuous` CLI
  group and MCP family gain two leaves each)
- **Follows:** [ADR-0014](0014-transcendental-single-sum-operations.md) (the
  transcendental gating), [ADR-0021](0021-fallible-operations-on-non-finite-results.md)
  (a non-finite *result* is an overflow),
  [ADR-0025](0025-solve-for-periods-and-rate.md) (the single sum's four-operation
  solve set, which this mirrors),
  [ADR-0028](0028-binary-surface-conventions.md) (§1 coverage, §2's solve-for
  conventions), [ADR-0050](0050-role-newtypes-for-ambiguous-arguments.md) (role
  newtypes), [ADR-0052](0052-tvmerror-variant-granularity.md) (error granularity),
  [ADR-0054](0054-numeric-robustness-of-the-core-operations.md) (`expm1`/`ln1p`, and
  what counts as a root), [ADR-0056](0056-degenerate-rate-solves.md) (a solve whose
  unknown drops out of the equation reports that),
  [ADR-0057](0057-currency-is-checked-where-a-result-is-denominated.md) (a
  non-monetary result folds no currency),
  [ADR-0062](0062-annuity-sinking-fund-and-perpetuity-due.md) /
  [ADR-0063](0063-annuity-due-solves-and-growing-inverses.md) (the previous two parts
  of #106, whose "out of scope" lists this discharges an entry of),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (test the class, not the
  instance)

## Context

Issue #106 enumerated the public surface module by module. ADR-0062 and ADR-0063
closed its two annuity groups; this one closes the third.

`continuous` had **two** of the four operations over `FV = PV·e^(δ·Y)` —
`future_value` and `present_value`, one for each amount — where `single_sum` has all
four over `FV = PV·(1+r)ⁿ`. A model could be priced forward and back and read in
neither direction: "what force of interest turns this into that over three years"
and "how long does this take at that force" both had to be done by hand, in a crate
whose whole point is that the five classic TVM variables are all solvable
(ADR-0025).

The interesting thing about this pair is how much *less* there is to it than the
annuity solves ADR-0063 added, and that is worth stating rather than assuming.

## Decision

### The core gains two functions, and both are closed forms

```rust
continuous::rate(years: f64, present: PresentValue, future: FutureValue) -> Result<ContinuousRate>
continuous::years(rate: ContinuousRate, present: PresentValue, future: FutureValue) -> Result<f64>
```

Taking the logarithm of `FV = PV·e^(δ·Y)` gives `ln(FV/PV) = δ·Y`, which is
**linear** in both unknowns. So each solve is the same logarithm divided by the other
given:

```text
δ = ln(FV/PV) / Y          Y = ln(FV/PV) / δ
```

**Neither reaches for `bracket_and_bisect`, and that is a real simplification, not a
shortcut.** An iterative rate solve owes its caller three separate arguments that
none of this needs: that the residual is *monotone* in the unknown so the root is
unique (ADR-0063 had to prove this for the growing factor by rewriting the closed
form as a sum); that the bracketing scan's geometry actually contains the root; and
that the acceptance tolerance admits real roots while rejecting the `r → ∞`
degenerate ones (ADR-0054). Here the answer is an exact expression, so uniqueness is
immediate, there is no scan and no tolerance, and `SolveDidNotConverge` is
unreachable from `continuous`. The only questions left are the *domain* and the
*conditioning*, which the rest of this ADR is about.

### Naming: `rate` and `years`, matching the arguments they recover

`single_sum` calls its solves `periods` and `rate`, each named for the argument of
the value functions that it recovers. Applying the same rule here is what settles the
names, because `continuous::future_value(rate, years, present)` names its arguments
`rate` and `years`:

- **`years`, not `periods` or `nper`.** Continuous time has no period, which is the
  whole of ADR-0036; ADR-0041 already rejected `--periods` for the CLI on that
  ground and chose `--years`. The core, the CLI leaf and the MCP tool are therefore
  all *`years`* — a rare case where the correspondence is exact, since elsewhere the
  CLI's `nper` flattens to the core's `periods`.
- **`rate`, not `force` or `force_of_interest`.** This one is a genuine trade. δ is a
  force of interest, and `ContinuousRate` is deliberately a *sibling* of `Rate<P>`
  rather than a case of it (ADR-0036), so a function called `rate` returning a
  `ContinuousRate` reads oddly in isolation. But it is never in isolation: it is
  `continuous::rate` / `continuous rate` / `continuous_rate`, and ADR-0041 already
  fixed the convention that **the family name says what kind of rate this family's
  `rate` is** — that is exactly why `--rate` on `continuous fv` is a force of
  interest and not a per-period rate. Introducing `force` would give δ a *third*
  name (after the `--rate` flag and the `ContinuousRate` type) and would break the
  parallel with `single_sum::rate`, which is the symmetry #106 asked for. The cost
  is recorded under *Alternatives considered*.

The argument order mirrors `single_sum` exactly — the given time-or-rate first, then
the two amounts — so the four functions read as one family:

```text
single_sum::periods(rate, present, future)     continuous::years(rate, present, future)
single_sum::rate::<P>(periods, present, future) continuous::rate(years, present, future)
```

### Role newtypes (ADR-0050)

Both take **two** `Money` amounts in adjacent positions, so both take
`PresentValue` + `FutureValue` — the same pair `single_sum::periods` and
`single_sum::rate` take, for the same reason: transposing them compiles and returns a
plausible wrong number (the reciprocal's logarithm, i.e. the answer with its sign
flipped, which is *especially* easy to mistake for a valid answer here because a
negative span is legitimate). A `compile_fail` doctest pins it.

`years` is a plain `f64` and δ a `ContinuousRate`, neither wrapped: they are the only
argument of their type in either signature (rule 4).

`continuous::years` returns a plain `f64`, not a `Period<Annual>`. ADR-0041 rejected
modelling the span as a `Period` because it would drag a periodicity tag onto an
intrinsically period-free quantity; the return type follows the argument type.

### The domain, enumerated

`e^(δ·Y)` is strictly positive for every finite δ and Y, so `FV = PV·e^(δ·Y)` forces
`FV/PV > 0`. Enumerating what that excludes:

| inputs | `FV/PV` | outcome | why |
| --- | --- | --- | --- |
| `PV`, `FV` same sign, non-zero | positive | **solved** | the ordinary case |
| both **negative** | positive | **solved** | the relation is homogeneous: scaling both amounts by `−1` leaves δ and `Y` unchanged, so a liability growing at δ is the same solve as an asset growing at δ |
| opposite signs | negative | `NoRealSolution` | no real logarithm; a positive growth factor cannot flip a sign |
| `FV = 0` | `0` | `NoRealSolution` | `e^(δ·Y)` is never zero, at any finite δ or `Y` |
| `PV = 0`, `FV ≠ 0` | `±∞` | `NoRealSolution` | nothing grows out of nothing |
| `PV = FV = 0` | `NaN` | `NoRealSolution` | see below |
| ratio or its reciprocal overflows | — | `NoRealSolution` | see below |

**Both negative is admissible, and answering it is the point of checking.** The naive
guard "both amounts must be positive" would reject an entirely ordinary debt
calculation. The ratio test admits it automatically, and a test pins that the answer
equals the all-positive one.

**`PV = FV = 0` is `NoRealSolution`, though every δ and every `Y` satisfy it.** This
is the one row where the table is arguably generous to itself, so the reasoning is
recorded: the equation is degenerate in *all four* variables at once, so there is
nothing for `IndeterminateRate` / `IndeterminateSpan` to advise — those say "your
inputs are fine, they just do not pin this unknown down; supply a different one",
and here no other input can be supplied that would help. It is also exactly what
`single_sum::periods` answers for a zero present (ADR-0052), and a solve set whose
two halves disagree about the all-zeros input would be worse than either choice.

**An overflowing ratio is `NoRealSolution` too**, and the guard is deliberately
*two*-sided: both `FV/PV` and `PV/FV` must be finite. That excludes only amounts more
than ~308 decades apart — where an exact answer does exist and is not returned, the
same documented limit `single_sum::periods` has always had — and in exchange it makes
the arithmetic below **provably** finite rather than incidentally so, which is what
lets the two-sided logarithm below be written without a second overflow check.

**One guard, one home.** Both solves call the same private `log_ratio`, so they
cannot drift apart about what they accept. A property asserts exactly that over the
whole same-sign/opposite-sign space, rather than at the handful of points the unit
tests pin.

### `ln1p` earns its place — but only with a two-sided branch

`ln(FV/PV)` written literally is [ADR-0054](0054-numeric-robustness-of-the-core-operations.md)'s
cancellation, in a new place. Forming the ratio rounds it to within an ULP of `1`, so
for a small `δ·Y` every significant digit of the answer is destroyed *before* `ln` is
called. Measured against a 60-digit `decimal` reference, at `δ·Y = 1e-12` the literal
form is wrong by `1.4e-5` **relative** — the fifth digit.

The obvious fix, `ln1p((FV − PV)/PV)`, is right for that case (the subtraction of two
nearby amounts is exact, by Sterbenz) and **wrong for the opposite one**. As
`FV/PV → 0` its argument approaches `−1`, and an `f64` near `−1` cannot carry the
information that `1 + x` is tiny; at `ln(FV/PV) = −30` that form is wrong by `5.5e-6`
relative, where the literal `ln(FV/PV)` is accurate to a few ULP. The two forms fail
at opposite ends.

The resolution is the identity `ln(FV/PV) = −ln(PV/FV)`: evaluate whichever side
keeps the `ln1p` argument **non-negative**.

```text
ln(FV/PV) =  ln1p((FV − PV)/PV)   when |FV| ≥ |PV|
          = −ln1p((PV − FV)/FV)   otherwise
```

Measured relative error against the 60-digit reference:

| `ln(FV/PV)` | this branch | literal `ln(FV/PV)` | one-sided `ln1p` |
| --- | --- | --- | --- |
| `+1e-12` | `1.5e-16` | `1.4e-5` | `1.5e-16` |
| `−1e-12` | `3.6e-17` | `1.2e-5` | `3.6e-17` |
| `−15` | `3.2e-18` | `3.2e-18` | `6.0e-12` |
| `−30` | `4.3e-19` | `4.3e-19` | `5.5e-6` |
| `+15` | `2.2e-18` | `2.2e-18` | `2.2e-18` |

**This branch is not the seam ADR-0054 removed**, and the distinction matters because
that ADR deleted a `|r| < 1e-9` band for good reasons. That band had a tuned
constant, and inside it the code returned a *different* (limit) expression that was
not the answer. Here there is no constant to tune: the switch is at `FV = PV`, the
natural symmetry point of the identity, where both branches evaluate to exactly `0`;
and both branches compute the *same* quantity by the *same* function, each accurate
to a couple of ULP over its whole half. The resulting bound,
`|Δ ln(FV/PV)| ≲ 2u·(1 − e^(−|L|)) + u·|L|`, holds everywhere.

### The degeneracies, and a second `Indeterminate*` variant

A solve says nothing about its unknown when the unknown drops out of the equation
(ADR-0056). For `FV = PV·e^(δ·Y)` the growth factor is `1` — and δ and `Y` both
vanish from it — exactly when the exponent is zero. Each solve therefore has one
degenerate row, and they are mirror images:

| solve | degenerate at | equation becomes | if satisfied | if not |
| --- | --- | --- | --- | --- |
| `rate` | `Y = 0` | `FV = PV`, δ absent | `IndeterminateRate` | `NoRealSolution` |
| `years` | `δ = 0` | `FV = PV`, `Y` absent | **`IndeterminateSpan`** | `NoRealSolution` |

**"Satisfied" is `Residual::is_root`, not `==`**, for exactly the reason ADR-0056
gives: a target a hair from the present amount is *still* satisfied at every value of
the unknown, so an exact-equality guard would report those near-misses as though they
had been solved. Both rows go through **the same shared helper** ADR-0063 extracted —
`unit_factor_outcome`, which has moved from `annuity.rs` to `root.rs` (beside the
`Residual` it is about) and gained a parameter for the variant to report when the
equation *is* satisfied. That is the whole of the change to existing code: no
behaviour of any annuity solve moves, and the rule about what "satisfied" means still
has one home, now shared by three call sites instead of two.

**`ZeroPeriods` is deliberately *not* used for the `Y = 0` row**, despite being the
`n = 0` answer everywhere else in the crate. Three reasons, in increasing order of
weight:

1. There is no `Period<P>` here. `ZeroPeriods`' own rustdoc says "supply a positive
   `Period<P>`", and `continuous`'s span is a plain `f64` (ADR-0036) that a
   constructor never sees.
2. That advice is not even directionally right, because the span may legitimately be
   **negative**. `ZeroPeriods` is defined against `NegativePeriods` as "zero rather
   than negative, both wrong"; here only *zero* is degenerate and negative is an
   ordinary input.
3. Most importantly, `ZeroPeriods` names the *input* and stops. The
   `IndeterminateRate` / `NoRealSolution` pair names the *outcome*, and the two
   outcomes at `Y = 0` are opposite: with `FV = PV` every force of interest works,
   and otherwise none does. Reporting one variant for both would collapse ADR-0056's
   entire distinction on the one row where it is cleanest.

**`IndeterminateSpan` is a new variant, and it is not `IndeterminateRate` reused.**
Applying ADR-0052's granularity test — *could a caller plausibly act differently?* —
the answer is yes, and the difference is the fix: an indeterminate *rate* is fixed by
supplying a longer term, an indeterminate *span* by supplying a non-zero force of
interest. Reuse would also make the rendered message actively wrong, telling a caller
who asked for a span that "every **rate** satisfies these inputs". The variant is
additive on a `#[non_exhaustive]` enum, so no downstream `match` breaks, and it is
the mechanism ADR-0052 says to use when a genuinely new kind of degeneracy appears
rather than filing it under a name that does not fit.

`NoRealSolution` needs no such twin: "no value of the unknown works" is the same
statement whichever unknown it is, and the method the caller invoked says which.

### A negative span is an answer, not an error

ADR-0036 states that the span "may be fractional or negative", and the two value
functions honour it. The solves must too, and this is where they part company with
their `single_sum` counterparts:

- **`continuous::rate` accepts a negative `years`** and simply flips the sign of the
  answer — growing to `future` over `−Y` is decaying to it over `Y`. There is nothing
  ill-defined about it: `δ = ln(FV/PV)/Y` is a well-defined signed quotient for every
  non-zero finite `Y`.
- **`continuous::years` *returns* a negative span** when `future` is on the far side
  of `present` from the direction the force of interest points — discounting to a
  smaller amount at a positive δ puts that amount in the past. `single_sum::periods`
  reports `NegativePeriods` on the same shape of input, and it is right to: its
  return type is `Period<P>`, which is non-negative by construction. `continuous`
  has no such type, and inventing a rejection to match a sibling's *type* constraint
  would contradict ADR-0036's own sentence.

Both are pinned by tests, on the core and on both binaries, precisely because a
future reader comparing the two modules will wonder whether the missing
`NegativePeriods` is an oversight.

### Overflow

`ContinuousRate` gains a `pub(crate) from_operation`, the mirror of
`Rate::from_operation` and `Money::from_operation` minus the domain floor those have
(every *finite* force of interest is valid — ADR-0036). A non-finite quotient is
therefore `Overflow`, per ADR-0021/0031's rule that a non-finite value *produced* by
arithmetic is an overflow where a non-finite value passed *in* is `NonFiniteRate`. It
is reachable: a subnormal-but-non-zero `years` against a large logarithm.

`continuous::years` returns a bare `f64` with no newtype to funnel through, so it
applies the same rule by hand, in one line, with a comment saying that is what it is
doing.

### Currency (ADR-0057)

Neither solve folds the two amounts' currencies, and neither can return
`CurrencyMismatch`. ADR-0057's rule reads off the result type — an operation
returning `Money` folds because it needs one denomination to stamp; an operation
returning a rate or a bare span has nothing to derive — and it says in as many words
that it "extends by itself: a future operation returning `Rate` or `Period` does
not". Both solves carry the `# Currency` rustdoc section that rule requires, and both
binaries' tests assert that no currency is echoed even when the amounts were
denominated.

### The binaries: both surfaces, ADR-0028 §1

| surface | shape |
| --- | --- |
| CLI | `continuous rate --years <Y> --present <PV> --future <FV>` |
| CLI | `continuous years --rate <δ> --present <PV> --future <FV>` |
| MCP | `continuous_rate`, `continuous_years` |

**No anchor, and that is the convention rather than an exception to it.** ADR-0028
§2's mutually-exclusive `--present`/`--future` pair — which ADR-0062 extended to
`annuity payment` and ADR-0063 reused for the due solves — exists where a solve can
be posed from *either* end of the horizon. These take **both** amounts, like
`single-sum nper` and `single-sum rate`, so there is nothing to choose between and
no flag group to add. The grammar is therefore the single-sum one with `--years`
substituted for `--periods`, which is the same substitution ADR-0041 made for the
values.

**Names correspond exactly** by flattening the path: `continuous rate` ↔
`continuous_rate`, `continuous years` ↔ `continuous_years`.

**Two new MCP input schemas**, `ContinuousRateSolveInput` and
`ContinuousYearsSolveInput`, rather than one shared shape: the two differ in their
third scalar (`years` against `rate`), and folding them together would mean each tool
advertising a field it ignores. They are the continuous twins of
`SingleSumPeriodsInput` / `SingleSumRateInput`, and like those they carry the optional
`currency` — it denominates the *inputs*, even though the scalar result never echoes
it.

**Errors interpolate the library's message.** Both CLI arms build
`"force of interest: {e}"` / `"span in years: {e}"`, following `single-sum nper` and
the change ADR-0063 made to the annuity rate solves, and for the same substantive
reason: the degenerate truth here is that *every* value satisfies the inputs, which a
static "no rate solves these inputs" would invert. No new dispatcher structure was
needed — `run_continuous` goes from 29 to 55 lines, well inside the function-length
lint, and unlike ADR-0062/0063's annuity dispatchers there is no repeated *shape* to
extract: the two arms differ in which argument they pass and which core function they
call, so a helper would have taken more lines than it saved.

### Testing (ADR-0045)

- **An independent high-precision reference, not the crate's own functions.** The
  worked case is checked against `ln(FV/PV)/Y` computed in Python's `decimal` at 60
  significant digits for exactly the two `f64` amounts involved:
  `0.04999999999999996936032344491634440436…` and
  `2.99999999999999799508595300120729449320…`. Note these are *not* `0.05` and `3` —
  the reference is the logarithm of the representable amounts, not the inputs they
  were built from, which is what makes the assertion a test of the arithmetic rather
  than of a round trip. Both are asserted to `1e-15` relative.
- **`ln1p` is pinned by measurement, not by assertion.** Two tests reproduce the
  table above: one shows the answer is right to `1e-14` relative near a unit ratio
  *and* that the literal `ln(FV/PV)` is wrong by more than `1e-6` there; the other
  shows the same for a deep discount against the one-sided `ln1p`. Each asserts the
  rejected form's failure, so removing the branch fails a test rather than silently
  losing accuracy.
- **Round-trips are properties with derived tolerances.** Writing `u = 1.1e-16` and
  `L = δ·Y`: `future_value` contributes ~`1.5u` of absolute error in `L`, the
  two-sided `ln1p` ~`2u` plus `u·|L|` of its own.
  - *Force*: `|Δδ| ≲ 3.5u/|Y| + u·|δ|`. Over `|Y| ∈ [0.25, 30]`, `|δ| ≤ 0.5` that is
    `1.6e-15`; a 300k-sample sweep peaks at `7.6e-16`. Asserted at `1e-13`, ~60×.
  - *Span*: `|ΔY| ≲ 3.5u/|δ| + u·|Y|`, so the force must be bounded away from zero.
    Over `|δ| ≥ 0.01` the bound is `4.2e-14`, measured peak `1.8e-14`. Asserted at
    `1e-11`, ~240×. **The tolerance fixes the range:** `|δ| ≥ 1e-4` would give
    `3.9e-12` (still inside), `|δ| ≥ 1e-6` gives `3.9e-10` (outside). That floor is
    the honest shape of the operation rather than a generator convenience — as
    `δ → 0` the span stops being recoverable, and at `δ = 0` the crate says so.
  - *Residual*: over **arbitrary** same-sign amounts — not a pair the crate produced —
    re-exponentiating the solved unknown must reprice the given `future`. This is the
    property a round trip could not catch, because a sign or branch error in
    `log_ratio` cancels in a round trip. `|ΔFV|/FV ≲ (4 + |L|)·u`, and with both
    amounts in `[1, 1e6]`, `|L| ≤ 13.8` gives `2.0e-15` against a measured `2.3e-15`.
    Asserted at `1e-12` relative.
  - Every span range is generated in **both** signs, since the span is signed.
- **The domain is tested as a class.** A property walks all four sign combinations of
  two amounts and asserts the two solves succeed together and fail together, with the
  same variant — the guard is shared, so this is what would catch it being unshared
  again.
- **Every degeneracy is pinned to its specific variant** on the core and on both
  binaries: `IndeterminateRate` at `Y = 0` with equal amounts, `IndeterminateSpan` at
  `δ = 0` with equal amounts, `NoRealSolution` for the unequal cases and for each row
  of the domain table, `NonFiniteOffset` for a non-finite span, `Overflow` for a
  subnormal divisor. The near-miss case (`1000` against `1000 + 1e-7`, inside
  `is_root`'s `1e-9` relative tolerance) is pinned separately, because an `==` guard
  would pass every other degeneracy test and fail only that one.
- **The signed span is pinned on all three surfaces**, and the MCP output-schema
  conformance test covers each new tool **twice** — once with a positive answer and
  once with the negative one a discount produces — since a signed scalar is the shape
  most likely to slip past a schema that assumed otherwise.

## Consequences

- `continuous` mirrors `single_sum`'s solve set: four operations over one relation,
  every one of them a closed form, every one reachable from both binaries.
- #106's third group is discharged. `DatedCashflows`' gaps and `Currency::from_numeric`
  remain, each its own decision.
- **`TvmError` gains one variant, `IndeterminateSpan`.** Additive on a
  `#[non_exhaustive]` enum. `NoRealSolution`, `IndeterminateRate`, `NonFiniteOffset`
  and `Overflow` gain callers; no other variant changes.
- **Purely additive to every signature, command, tool, flag, output shape, and
  default.** The only change to existing code is internal: `unit_factor_outcome` moved
  from `annuity.rs` to `root.rs` and took a parameter, with both annuity call sites
  passing `IndeterminateRate` — the behaviour they already had.
- `ContinuousRate` gains a `pub(crate) from_operation`, completing the set of
  `from_operation` funnels across `Money` / `Rate` / `Period` / `ContinuousRate`.
- The crate now has a worked, tested statement of *when* `ln1p` helps and when it
  hurts, with the two-sided identity as the answer. A future logarithm of a ratio
  should reach for the same shape rather than rediscovering half of it.

## Alternatives considered

- **Name the force solve `force` or `force_of_interest`.** The most literal reading:
  δ *is* a force of interest, `ContinuousRate` is not a `Rate<P>`, and
  `continuous_force` sits unambiguously beside `continuous_from_effective` where
  `continuous_rate` arguably does not. Rejected on balance: it would be δ's third
  name in one family (the `--rate` flag, the `ContinuousRate` type, and now `force`),
  it breaks the `single_sum::rate` parallel that is the point of the addition, and
  ADR-0041 already settled that the *family* disambiguates what kind of rate is
  meant. The ambiguity with the bridges is real and is answered where it bites — in
  the MCP tool descriptions, which say "solve for the force of interest at which…"
  against "the force of interest equivalent to an effective annual rate".
- **Name the span solve `nper`, matching `single-sum nper`.** Rejected for the reason
  ADR-0041 gave when it rejected `--periods`: there is no period to count. The
  by-product is a rare exact CLI↔MCP↔core name correspondence.
- **Return `Period<Annual>` from the span solve.** It would let the answer flow into
  the periodic operations. Rejected: ADR-0041 rejected the same modelling for the
  input, `Period` cannot be negative so it would force a spurious `NegativePeriods`
  on ordinary discounting, and the round trip through `continuous::future_value`
  would need an unwrap the value functions do not ask for.
- **Report `ZeroPeriods` at `Y = 0`.** The variant every other zero-term case uses,
  so it needs nothing new. Rejected on the three grounds above; the decisive one is
  that it would collapse the satisfied/not-satisfied distinction on the row where it
  is clearest.
- **Reuse `IndeterminateRate` for the span degeneracy.** No new variant, and
  defensible under ADR-0052's "the method the caller invoked disambiguates" reasoning
  (the same reasoning that keeps `NonFiniteScalar` shared between `try_mul` and
  `try_div`). Rejected because that reasoning holds for a *generically*-named variant
  and this one names the rate in its identifier and in its message; a caller of
  `continuous::years` reading "every rate satisfies these inputs" is being told
  something false about an argument they did not supply.
- **`ln(FV/PV)`, unadorned.** What `single_sum::periods` does today. Rejected: it is
  the exact cancellation ADR-0054 removed from the annuity factors, measurably wrong
  in the fifth digit at `δ·Y = 1e-12`. `single_sum::periods` is **not** changed here
   — rearranging an existing solve alters results for existing callers and is its own
  decision, exactly as ADR-0063 declined to rearrange `annuity::periods` — but the
  divergence is now recorded in two places.
- **One-sided `ln1p((FV − PV)/PV)`.** The obvious fix, and half right. Rejected on
  measurement: at `ln(FV/PV) = −30` it is wrong by `5.5e-6` relative, worse than the
  literal form it replaces, because an argument near `−1` cannot carry the size of
  `1 + x`.
- **`ln(|FV|) − ln(|PV|)`, one formula for every case.** No branch at all. Rejected:
  it reintroduces cancellation precisely where it matters most — for `PV = 1e6` and a
  ratio `1 + 1e-12` it differences two numbers near `13.8`, losing the answer's
  leading digits — which is the failure being fixed.
- **A third branch using `ln(FV/PV)` for extreme ratios**, to answer the
  308-decades-apart pairs the domain guard rejects. Rejected: it buys an answer only
  for inputs no model produces, and it would need a tuned threshold — the one thing
  the two-sided branch avoids.
- **Add a numeric solve for symmetry with the annuity module.** Rejected before it
  started, and named here because the temptation is real when the neighbouring module
  is full of `solve_rate` calls: `bracket_and_bisect` searches the *rate* domain with
  a `1 + r > 0` scan geometry that does not transfer to a force of interest (which
  has no `−1` floor) at all, and using an iterative method where a closed form exists
  would trade an exact answer for a tolerance.
