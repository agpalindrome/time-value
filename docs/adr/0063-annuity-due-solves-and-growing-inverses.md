# ADR-0063: The annuity-due solves and the growing-annuity inverses

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Follows:** [ADR-0015](0015-annuities.md) (annuities — its `due` submodule gains
  the solves it never had), [ADR-0025](0025-solve-for-periods-and-rate.md) (solve for
  periods and rate — its `_from_future` coinage applied to a second module),
  [ADR-0048](0048-finite-growing-annuity.md) (the finite growing annuity — added the
  values only), [ADR-0049](0049-growing-annuity-in-the-binaries.md) (how the growing
  family is surfaced), [ADR-0028](0028-binary-surface-conventions.md) (§1 coverage,
  §2's anchored solve-for pair), [ADR-0050](0050-role-newtypes-for-ambiguous-arguments.md)
  (role newtypes, and their boundary),
  [ADR-0052](0052-tvmerror-variant-granularity.md) (error granularity),
  [ADR-0054](0054-numeric-robustness-of-the-core-operations.md) (the `expm1`/`ln1p`
  factors, and what counts as a root),
  [ADR-0056](0056-degenerate-rate-solves.md) (where a factor stops depending on `r`),
  [ADR-0062](0062-annuity-sinking-fund-and-perpetuity-due.md) (the previous part of
  #106, whose "out of scope" list this discharges two entries of),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (test the class, not the
  instance)

## Context

Issue #106 enumerated the public surface module by module. ADR-0062 closed two of its
five groups and named the rest so they would not be mistaken for oversights. Two of
those are the annuity family's remaining holes, and this ADR closes them:

- **`annuity::due` had no solves at all.** The parent module solves for the payment,
  the term, and the rate, each from a present *or* a future value — six functions —
  and `due` mirrored only the two payments. A start-of-period stream could be priced
  in both directions but read back in neither.
- **The growing annuity had no inverses.** ADR-0048 added
  `growing_present_value` / `growing_future_value` and their `due` counterparts, and
  stopped there. So the library could price an escalating lease and not tell you what
  the escalating payment would have to be.

The second is where the interesting question is. A due factor is the ordinary factor
scaled by `(1 + r)`, which sounds like it makes the due solves mechanical — and for
the *term* it does. It does not for the *rate*, because scaling by `(1 + r)` moves
where the factor stops depending on `r`, which is precisely what ADR-0056 showed must
be handled before the solver runs. And the growing solves raise a genuine
well-posedness question: the growing factor depends on the **spread** `r − g`, so it
is not obvious that the residual is monotone in `r` alone, and a rate solve over a
non-monotone residual returns an arbitrary member of a set rather than an answer.

ADR-0056's lesson governs the whole change: *an operation that returns a plausible
wrong number is worse than an absent one.*

## Decision

### The core gains seven functions

```rust
annuity::due::periods(rate: Rate<P>, payment: Payment, present: PresentValue)      -> Result<Period<P>>
annuity::due::periods_from_future(rate: Rate<P>, payment: Payment, future: FutureValue) -> Result<Period<P>>
annuity::due::rate(periods: Period<P>, payment: Payment, present: PresentValue)    -> Result<Rate<P>>
annuity::due::rate_from_future(periods: Period<P>, payment: Payment, future: FutureValue) -> Result<Rate<P>>

annuity::growing_payment(rate: Rate<P>, growth: Growth<P>, periods: Period<P>, present: Money) -> Result<Money>
annuity::growing_periods(rate: Rate<P>, growth: Growth<P>, payment: Payment, present: PresentValue) -> Result<Period<P>>
annuity::growing_rate(growth: Growth<P>, periods: Period<P>, payment: Payment, present: PresentValue) -> Result<Rate<P>>
```

**The due period solves *delegate*, they do not restate a formula.** `PMT · a_due(r, n) = PV`
is exactly `(PMT · (1 + r)) · a(r, n) = PV` — the `(1 + r)` can be read as scaling the
factor or as grossing up the payment — so `due::periods` calls `annuity::periods` on a
payment brought forward one period, and `due::periods_from_future` does the same. No
second closed form exists to drift out of step, and the error vocabulary
(`PaymentDoesNotAmortize`, `NoRealSolution`, `NegativePeriods`) is inherited rather
than re-derived. The one new failure the gross-up can introduce is `Overflow`.

**The due rate solves reuse `solve_rate` with a scaled factor**, passing
`|r, n| present_value_factor(r, n) * (1 + r)` (or the future one). No new closed form,
no new acceptance test, no second `Residual`.

**`growing_rate` reuses the same machinery**, passing
`|r, n| growing_present_value_factor(r, g, n)` with `g` captured.

### The degeneracy table, derived rather than guessed

ADR-0056 enumerated where the two *ordinary* factors stop depending on `r`, because a
factor that ignores `r` makes the equation say nothing about `r` and the bracketing
scan then returns its own starting point (`−0.9999`) as though it had solved
something. Adding solves over four more factors means deriving the same table for
them. Two exact identities do it, and they are worth stating because they make the
answers obvious rather than empirical:

```text
a_due(r, n) = (1 + r) · a(r, n)  =  1 + a(r, n − 1)
s_due(r, n) = (1 + r) · s(r, n)  =  s(r, n + 1) − 1
```

(Both are two lines of algebra: `(1+r)(1 − (1+r)⁻ⁿ)/r = (1 + r − (1+r)^−(n−1))/r`, and
`(1+r)((1+r)ⁿ − 1)/r = ((1+r)^(n+1) − 1 − r)/r`.) A due factor is therefore its
ordinary counterpart at a *shifted term*, plus a constant — so it is constant in `r`
exactly where that shifted ordinary factor is. For the growing factor the useful form
is the sum rather than the closed form:

```text
f(r, g, n) = Σ (1 + g)^(k−1) · (1 + r)^(−k)   for k = 1..=n     (present)
F(r, g, n) = Σ (1 + g)^(k−1) · (1 + r)^(n−k)  for k = 1..=n     (future)
```

| factor | constant in `r` at | value there | monotone in `r` |
| --- | --- | --- | --- |
| `a(r, n)` — present, ordinary | `n = 0` | `0` | non-increasing |
| `s(r, n)` — future, ordinary | `n = 0`, `n = 1` | `0`, `1` | non-decreasing |
| `a_due(r, n) = 1 + a(r, n − 1)` | `n = 0`, **`n = 1`** | `0`, `1` | non-increasing |
| `s_due(r, n) = s(r, n + 1) − 1` | `n = 0` **only** | `0` | non-decreasing |
| `f(r, g, n)` — present, growing | `n = 0` **only** | `0` | non-increasing |
| `F(r, g, n)` — future, growing | `n = 0`, `n = 1` | `0`, `1` | non-decreasing |

The first two rows are ADR-0056's. The interesting rows are the middle two, and the
point is that **the `(1 + r)` scaling moves the single-period degeneracy from the
future solve to the present one**:

- `a_due(r, 1) = 1`. The lone payment falls *today* and is never discounted, so the
  present value is the payment at every rate. `due::rate` therefore carries a
  single-period guard that `annuity::rate` does not need.
- `s_due(r, 1) = 1 + r`. The lone payment falls today and compounds for one whole
  period, so the future value *does* depend on the rate — where the ordinary
  `s(r, 1) = 1` does not. `due::rate_from_future` therefore needs **no** guard, and a
  single start-of-period contribution is a perfectly determined solve
  (`r = FV/PMT − 1`) on the very term where `annuity::rate_from_future` reports
  `IndeterminateRate`.

Getting this backwards — copying the ordinary guard into the due module — would have
put a guard where it is wrong and omitted it where it is needed. Both directions are
pinned by tests.

**The growing present factor has no `n = 1` row**, because `f(r, g, 1) = 1/(1 + r)`,
which varies with the rate; growth never enters a one-payment stream at all. So
`growing_rate`'s only degeneracy is `n = 0`, which `solve_rate` already owns.

**Every `n = 1` row is reported through one shared helper.** The two that exist
(`s` and `a_due`) both have the factor identically `1`, so the equation collapses to
`payment = target`: `IndeterminateRate` when that is satisfied, `NoRealSolution` when
it is not. `rate_from_future`'s inline guard was extracted into
`unit_factor_outcome` and `due::rate` calls the same function, so the two cannot
disagree — including about what "satisfied" means, which is `Residual::is_root` and
not `==`, for the reason ADR-0056 records (a target a hair away is still satisfied at
every rate, and an exact-equality guard would let it leak the sentinel).

**Where the factors are zero, which is what the payment solves divide by:**

| factor | zero at | consequence |
| --- | --- | --- |
| `a_due`, `s_due` | `n = 0` only | `due::payment` / `due::payment_from_future` already return `ZeroPeriods` |
| `f(r, g, n)` | `n = 0` only | `growing_payment` returns `ZeroPeriods` |

For every `n ≥ 1`, `f` is a sum of `n` strictly positive discounted payments at every
admissible rate and growth, so it is strictly positive — there is no `r ≤ g` case to
reject as the *perpetuity* must, and no second division by zero. `a_due ≥ 1` and
`s_due > 0` for `n ≥ 1` likewise.

### Monotonicity: the growing rate solve is well posed, and here is why

`solve_rate` rests the uniqueness of its root on the factor being monotone in `r`.
Reading `f(r, g, n) = (1 − ((1+g)/(1+r))ⁿ)/(r − g)` invites doubt, because `r` appears
in the spread as well as in the discount factor, and the closed form even switches to
a limit at `r = g`. **The sum settles it:** with `g` held fixed, every term
`(1 + g)^(k−1)·(1 + r)^(−k)` is strictly decreasing in `r`, so the whole factor is,
for every `n ≥ 1`. There is one root, on both sides of `r = g`, and rates *below* the
growth are recovered as readily as rates above it.

That is an algebraic argument, so it is backed by a floating-point one: the factor is
computed in ADR-0054's `expm1`/`ln1p` form, which keeps it accurate to a few ULP right
through the limit instead of needing a fuzzy band, and a test walks a `1e-11` grid
straddling `r = g` asserting the direction never reverses. The same test exists for the
two due factors, excluding `n = 1` — where `a_due` is *constant*, so it wobbles by an
ULP either way, which is exactly why that term is guarded rather than solved.

### `growing_periods` needs no near-zero band

`RATE_NEAR_ZERO` exists because a *logarithmic* solve divides by `ln(1 + r)`, which is
ill-conditioned near zero; `annuity::periods` switches to its `r → 0` limit inside that
band. The growing term solve is arranged so that no band is needed:

```text
n = ln1p(−PV·(r − g)/PMT) / ln1p(−(r − g)/(1 + r))
```

Both logarithms are `ln1p` of a quantity **proportional to the spread**, so the spread
cancels between them rather than being lost to cancellation inside a `1 − …`
intermediate. Only the exact `0/0` at `r = g` is a special case, and its limit
(`n = PV·(1 + r)/PMT`) is taken there. This is ADR-0054's argument applied to a solve
instead of a factor.

The consequence is visible and is pinned: at `r = 1e-9, n = 12`, checked against a
term-by-term summation, `growing_periods` with `g = 0` answers `12.000000000` where
`annuity::periods` — taking its band's limit — answers `11.999999922`, and the level
*closed form* without a band would answer `11.99999996`. The growing solve is the
accurate one. `annuity::periods` is **not** changed here: rearranging an existing
solve is a behaviour change to a released shape and belongs in its own decision, not
smuggled into an additive PR. The divergence is recorded at the test that pins it.

### Role newtypes (ADR-0050)

- The three solves that take **two** `Money` amounts get roles, matching their level
  counterparts exactly: `Payment` + `PresentValue`, or `Payment` + `FutureValue`.
- `growing_payment` takes **one** `Money` and so keeps a plain `Money`, by ADR-0050
  rule 4 and for the reason ADR-0062 gave for `payment_from_future`: with a single
  monetary argument there is nothing to transpose it with, and a wrapper there would
  imply the plain-`Money` `payment` beside it is less safe.
- `growing_rate` takes only **one** `Rate<P>` — the growth, since the discount rate is
  what is being solved for — and it is still a `Growth<P>`. Rule 4 would exempt a lone
  argument, but the wrapper is doing different work here: it keeps the growing family's
  vocabulary uniform (every other growing function names its second rate `Growth`), and
  it stops a caller passing the *unknown* where the *given* goes in the one function
  where no adjacent rate makes the mistake obvious. It also removes the turbofish —
  `Growth<P>` names the periodicity, so `annuity::growing_rate(…)` infers `P` where
  `annuity::rate::<Monthly>(…)` cannot. That ergonomic difference is pinned by a test
  so it stays deliberate.

### The binaries: both surfaces, ADR-0028 §1

| surface | shape |
| --- | --- |
| CLI | `annuity due nper --rate <r> --payment <PMT> (--present <PV> \| --future <FV>)` |
| CLI | `annuity due rate --periods <n> --payment <PMT> (--present \| --future)` |
| CLI | `annuity growing payment --rate <r> --growth <g> --periods <n> --present <PV>` |
| CLI | `annuity growing nper --rate <r> --growth <g> --payment <PMT> --present <PV>` |
| CLI | `annuity growing rate --growth <g> --periods <n> --payment <PMT> --present <PV>` |
| MCP | `annuity_due_periods`, `annuity_due_rate` (the anchored pair) |
| MCP | `annuity_growing_payment`, `annuity_growing_periods`, `annuity_growing_rate` |

**The due solves reuse ADR-0028 §2's anchored `--present`/`--future` pair**, and the
MCP tools reuse `AnnuityPeriodsInput` / `AnnuityRateInput` unchanged — the same input
shape serving the ordinary and the due tool, exactly as `AnnuityValueInput` already
serves both value tools. That is ADR-0062's precedent: extend the anchor rather than
add a subcommand per anchor.

**The growing solves take `--present` alone, and that asymmetry is deliberate.** The
future-anchored growing inverses do not exist (below), so there is no second anchor to
offer. Should one ever arrive, adding `--future` as an optional partner is the same
additive relaxation ADR-0062 made to `annuity payment`, so nothing here forecloses it.

**Names correspond by flattening the path** (ADR-0049): `annuity growing nper` ↔
`annuity_growing_periods`, `annuity due rate` ↔ `annuity_due_rate`. `growing` stays the
grouping level and `due` stays part of the leaf, unchanged from ADR-0049.

**Dispatcher length: three shared helpers, no `#[allow]`.** ADR-0062 noted that
`run_annuity` was "the last flat arm that fits". Rather than suppress the lint, the
repeated *shapes* were extracted, which shrank the dispatchers instead of growing
them:

| dispatcher | before | after |
| --- | --- | --- |
| `run_annuity` | 97 | **79** |
| `run_annuity_growing` | 61 | **73** |
| `run_annuity_due` | 40 | **67** |

`solved_periods` and `solved_rate` carry the anchored branch for the ordinary *and* the
due group (so `run_annuity` got shorter while gaining nothing, and `run_annuity_due`
absorbed two solves for ~11 lines each); `growing_value` collapses the four
growing-value arms, and one small helper per growing solve keeps those arms to a line.
The MCP server gained the same two `solved_*` helpers beside its existing
`level_payment`. Every one of these serves at least two call sites — they are
deduplication, not lint appeasement.

**One error message changes.** The annuity rate solves' CLI context was the static
string "no rate solves these inputs"; they now interpolate the library error, as
`level_payment` already does. The reason is substantive rather than cosmetic: on a
degenerate term the truth is that *every* rate solves the inputs
(`IndeterminateRate`), which the static string inverts. This is the case `main`'s own
comment describes — the library error is more specific than any fixed context — and it
fixes the same masking for the pre-existing `annuity rate --future` at `n = 1`. No
output shape, exit code, or successful result changes.

### Testing (ADR-0045)

- **Every closed form is checked against an independent reference.** Each new solve is
  fed a value built by **term-by-term summation** of the stream and asked to recover
  the argument that produced it — never against the crate's own value functions. The
  due module's tests carry their own two summations (`Σ PMT/(1+r)^k` for `k = 0..n`,
  and `Σ PMT·(1+r)^k` for `k = 1..=n`); the growing tests reuse ADR-0048's.
- **Each inverse is pinned as a round-trip property, and every tolerance is derived**
  from the residual tolerance divided by the local derivative, as ADR-0054 and
  ADR-0056 state theirs. The derivations are written out at each test; in outline:
  - *Payment* round trips multiply and divide by the **identical** factor, so the error
    is two `f64` roundings (`4.5e-16` relative) with nothing to amplify it →
    `1e-9` relative, six orders of margin.
  - *Period* solves convert a relative error in the value into an absolute error in the
    term through `C = value / (d value / dn)`, which grows without bound as the value
    saturates. Over `r, g ∈ [−0.2, 0.3]` and `n ≤ 20`, `C ≤ 3.2e4`, so a forward value
    carrying a few ULP gives `≲ 3e-11` of term → `1e-6` absolute. **The tolerance fixes
    the ranges, not the other way round:** widening to `r ∈ [−0.5, 0.5], n ≤ 60` pushes
    `C` past `1e10`, and `n ≤ 40` alone puts the growing case within a factor of three
    of the tolerance.
  - *Rate* solves are pinned to `2e-9·PV / |dPV/dr|`, worst at the shortest term where
    the factor barely responds to the rate: `7.4e-9` for `due::rate`, `3.0e-9` for the
    other two, over `r, g ∈ [−0.5, 0.5]` → `1e-7` absolute, 13× on the worst.
- **Every degenerate case is pinned to its specific variant**, on the core and on both
  binaries: `ZeroPeriods` for all six new rate/payment solves at `n = 0`;
  `IndeterminateRate` / `NoRealSolution` for `due::rate` at `n = 1` (including a
  within-tolerance near-miss, which an `==` guard would have let leak the sentinel);
  `PaymentDoesNotAmortize` for a zero payment and for a target at or above the growing
  perpetuity's ceiling; `SolveDidNotConverge` for a due present value below the first
  payment.
- **The rows that are *absent* are pinned too**, side by side with the ordinary
  operation that does have them: `due::rate_from_future` and `growing_rate` succeed at
  `n = 1` in the same test that asserts `annuity::rate_from_future` fails there. An
  absent guard is otherwise indistinguishable from a forgotten one.
- **Monotonicity is asserted in floating point**, not just claimed in prose, for the two
  due factors and the growing one — including a `1e-11` grid across `r = g`.
- **The MCP output-schema conformance test covers all five new tools**, and each
  anchored tool now appears twice, once per anchor, because the anchor selects a
  different core call.

## Consequences

- `annuity::due` mirrors its parent completely: eight values and six solves, every one
  reachable from both binaries. `annuity`'s solve set is now symmetric across
  {payment, periods, rate} × {present, future} × {ordinary, due}.
- The growing annuity can be read back for its payment, its term, and its rate — from
  a present value.
- **Purely additive to every signature, command, tool, flag, output shape, and
  default.** The one behaviour change is the annuity rate solves' CLI error *message*,
  described above.
- ADR-0056's constancy table is now complete over all six annuity factors, and the
  two identities that generate it are recorded, so a future factor can be classified
  the same way instead of being sampled.
- `unit_factor_outcome` is shared by the two solves that need it, so the `n = 1`
  reporting rule has one home.
- `TvmError` is unchanged: `ZeroPeriods`, `IndeterminateRate`, `NoRealSolution`,
  `PaymentDoesNotAmortize`, `NegativePeriods`, and `SolveDidNotConverge` all gain
  callers, none needs a sibling.
- The dispatchers are *shorter* than before this change, so the "next flat arm does not
  fit" warning ADR-0062 left is discharged rather than deferred.

## Alternatives considered

- **Write `due::periods` as its own logarithm** instead of grossing up the payment.
  Rejected: it duplicates a formula that already exists and gives the error conditions
  a second home to drift in. The gross-up is the same algebra read the other way.
- **Copy `rate_from_future`'s `n = 1` guard into `due::rate_from_future`.** The
  symmetric-looking move, and wrong: `s_due(r, 1) = 1 + r` varies with the rate, so the
  guard would reject a well-posed solve. The identities above are what made this
  visible; sampling the factor at two rates would have made it a coin flip.
- **Omit `due::rate`'s `n = 1` guard**, on the reasoning that the ordinary present
  solve has none. Rejected for the mirror-image reason — that is exactly how the
  `−0.9999` sentinel leaked in the first place.
- **A near-zero band on the spread in `growing_periods`**, reusing `RATE_NEAR_ZERO` as
  ADR-0048 did for the factors. Simpler and it matches the house pattern. Rejected:
  the `ln1p / ln1p` arrangement is exact right down to the limit, so the band would
  buy nothing and lose accuracy in a window — the same trade ADR-0054 removed from the
  factors, which there is no reason to reintroduce in a new place.
- **Give `growing_rate` a plain `Rate<P>` for the growth**, per ADR-0050 rule 4's
  single-argument exemption. Rejected: see above — uniform vocabulary, a real (if
  unusual) confusion to prevent, and it is what removes the turbofish.
- **Put the growing solves in a `growing solve …` CLI sub-subgroup.** Rejected: the
  group's leaves are already `pv`/`fv`/`due-pv`/…, so `payment`/`nper`/`rate` read
  correctly beside them, and the depth would break the flatten-the-path correspondence
  with the MCP names.
- **`#[allow(clippy::too_many_lines)]` on `run_annuity_growing`.** Rejected for the
  reason ADR-0049 and ADR-0062 both give: the lint points at a structural answer.
  Here that answer *shortened* three dispatchers.
- **Rearrange `annuity::periods` to the band-free `ln1p / ln1p` form**, since
  `growing_periods` shows it is more accurate. A real improvement, and deliberately not
  taken here: it changes results for existing callers, so it is its own decision with
  its own tests, not a side effect of an additive PR. Recorded at the test that
  measures the gap.

## Out of scope (deliberately, with reasons)

The growing family's solve matrix is **not** completed, and one part of it cannot be:

- **`growing_periods_from_future` has no closed form, so it is left out.** Solving
  `PMT · ((1+r)ⁿ − (1+g)ⁿ)/(r − g) = FV` for `n` means solving
  `(1+r)ⁿ − (1+g)ⁿ = FV·(r − g)/PMT`, a difference of **two** exponentials in `n`.
  Unlike the level case there is no `n = logₐ(…)` rearrangement; it would need a
  numeric solve in `n`, which the crate has no machinery for (`bracket_and_bisect`
  searches the *rate* domain, and its `1 + r > 0` scan geometry does not transfer). An
  operation invented for symmetry, on a solver whose convergence could not be argued
  for, is precisely what ADR-0056 says not to ship.
- **`growing_payment_from_future` and `growing_rate_from_future` are left out too**,
  even though both *are* tractable (a division by `F(r, g, n)`, and a rate solve with
  an `n = 1` guard the table above already derives). The reason is coherence rather
  than difficulty: with the term solve unavailable, a `_from_future` growing family
  cannot be completed, and shipping two of its three members would leave a ragged
  matrix while *implying* the third exists. The growing group is present-anchored, one
  rule with one reason, until the term solve can join it.
- **`due::growing_payment` / `due::growing_periods` / `due::growing_rate` are left
  out.** These are all `(1 + r)` scalings of what this ADR adds — genuinely
  mechanical — and their degeneracies follow from the same table (`f_due(r, g, 1) = 1`,
  so the due growing rate solve would need the `n = 1` guard that `growing_rate` does
  not). They are deferred because the growing × due × solve cell is only worth opening
  once the anchor question above is settled; adding six functions to reach a matrix
  that is still ragged in the other axis buys ordering, not symmetry.
- **`continuous` still has no solve-for-rate or solve-for-years, `DatedCashflows` no
  net future value / MIRR / owned counterpart, and `Currency` no `from_numeric`** —
  #106's remaining three groups, each its own decision. *(The first is discharged by
  [ADR-0064](0064-continuous-solves.md), which also relocated the
  `unit_factor_outcome` helper extracted above into `root.rs` and gave it a parameter
  for the variant to report, so a third call site could share the same rule without
  claiming the rate was the unknown.)*

ADR-0045's boundary applies as it did in ADR-0062: symmetry alone does not earn a
place on the surface. Every function added here answers a named question — what
payment, over how many periods, at what rate — and every one omitted is either
ill-posed or waiting on one that is.
