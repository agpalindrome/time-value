# ADR-0056: Degenerate rate solves report the degeneracy

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Follows:** [ADR-0020](0020-robust-irr-newton-with-bisection-fallback.md) (the
  bracketing solver), [ADR-0025](0025-solve-for-periods-and-rate.md) (solve for
  periods and rate),
  [ADR-0052](0052-tvmerror-variant-granularity.md) (error granularity),
  [ADR-0054](0054-numeric-robustness-of-the-core-operations.md) (the residual
  scale), [ADR-0045](0045-make-illegal-states-unrepresentable.md)

> **Amended (2026-07-30) by [ADR-0063](0063-annuity-due-solves-and-growing-inverses.md).**
> The constancy table below covers the two *ordinary* annuity factors, which was the
> whole surface when this was written. ADR-0063 adds solves over the annuity-due and
> growing factors and extends the table to all six, deriving each row from two exact
> identities (`a_due(r,n) = 1 + a(r,n−1)`, `s_due(r,n) = s(r,n+1) − 1`) rather than by
> inspection. The headline result is that the `(1 + r)` scaling **moves** the
> single-period degeneracy: `annuity::due::rate` needs the guard `annuity::rate` does
> not, and `annuity::due::rate_from_future` needs none where
> `annuity::rate_from_future` does. The reporting rule below is unchanged and is now
> shared by both guards.

> **Extended (2026-07-30) by [ADR-0064](0064-continuous-solves.md).** The rule below
> is applied outside `annuity` for the first time: the continuous growth factor
> `e^(δ·Y)` is `1` whenever its exponent is zero, so `continuous::rate` at `Y = 0`
> and `continuous::years` at `δ = 0` are both of the unit-factor shape. The shared
> helper moved to `root.rs` and took a parameter for the "satisfied" variant, because
> the second of those leaves the **span** under-determined rather than the rate —
> which is why `TvmError::IndeterminateSpan` now sits beside `IndeterminateRate`. The
> satisfied/not-satisfied distinction, and `Residual::is_root` as the test of it, are
> unchanged.

## Context

`bracket_and_bisect` scans outward from `1 + r = 1e-4`, i.e. `r = −0.9999`, and
accepts the first sample that satisfies its root test. That is correct when a root
is genuinely there. It is wrong when the residual is a root at *every* rate,
because the scan then returns its own arbitrary starting point as though it had
solved something.

An adversarial pre-publication review found this. ADR-0054's scale-relative
residual (`|value| < 1e-9 · scale`, where `scale = |priced| + |target|`) removed
most of it incidentally: where the degenerate inputs also drive the scale to zero,
`is_root` became false and the solve now fails honestly. One case survived,
because it is the one where the residual is genuinely zero against a genuinely
non-zero scale:

```text
annuity::rate_from_future(periods = 1, payment = 100, future = 100)  ->  Ok(-0.9999)
```

The reason is exact rather than numerical. `annuity`'s two factors are

```text
present_value_factor(r, n) = (1 − (1+r)⁻ⁿ) / r
future_value_factor(r, n)  = ((1+r)ⁿ − 1) / r
```

and a factor that does not depend on `r` makes the equation say nothing about
`r`. Enumerating where that happens closes the problem completely:

| factor | constant in `r` at |
| --- | --- |
| `present_value_factor` | `n = 0` only (the factor is `0`) |
| `future_value_factor` | `n = 0` (the factor is `0`) and `n = 1` (the factor is `1`) |

At `n = 1` the single payment falls at the end of the term and is never
compounded, so the future value *is* the payment whatever the rate. At `n ≥ 2`
both factors vary with `r`. The degenerate set is therefore small, closed, and
provable — it is not a numerical-conditioning band that has to be guessed at.

A second, related inconsistency showed up alongside it. `annuity::payment`
rejects a zero term with `ZeroPeriods`, but the two solves reached
`SolveDidNotConverge` for the same input — blaming the iteration for a degenerate
input.

## Decision

**Report the degeneracy at the point it is provable, before the solver runs.**

- **`solve_rate` rejects a zero term with [`TvmError::ZeroPeriods`].** Both
  factors are identically `0` there, so the equation constrains nothing. This also
  makes the solves agree with `annuity::payment` on the same input.
- **`annuity::rate_from_future` handles `periods == 1` directly.** The equation
  reduces to `PMT = FV` with the rate absent, so the answer turns on that
  comparison alone: `IndeterminateRate` when they are satisfied (every rate works),
  `NoRealSolution` when they are not (none does).

  **"Satisfied" is the solver's own root test, not `==`.** A target a hair away
  from the payment leaves a residual that is still inside the accepted tolerance at
  *every* rate, so an exact-equality guard would let those near-misses go on
  leaking the sentinel. The guard therefore builds the same `Residual` the solver
  would and asks `is_root`, which is why that method is `pub(crate)` rather than
  private: the guard and the solver cannot then disagree about what counts as
  solved.
- **A new variant, `TvmError::IndeterminateRate`**, meaning the inputs are
  satisfied by every rate so no single one is the answer.

`IndeterminateRate` earns its place by ADR-0052's own test — a caller acts
differently on each of the three neighbouring outcomes. `NoRealSolution` says no
rate works, so the inputs are wrong. `SolveDidNotConverge` says an answer may
exist but the iteration did not find it, so a different guess might help.
`IndeterminateRate` says the inputs are *under-determined*: nothing is wrong with
them, they simply do not pin down a rate, and the fix is to supply a longer term
or solve for something else.

**The guards are placed where the degeneracy is provable, not in the solver.**
`bracket_and_bisect` could have been changed to distrust a root found at its first
probe, but any such rule is a heuristic — it cannot distinguish a flat residual
from a legitimate root that happens to sit at the scan's starting point. The
factor identities above are exact, so the call sites can be, too.

## Consequences

- No rate solve can return the scan's `−0.9999` sentinel as an answer.
- `annuity::rate` and `annuity::rate_from_future` now agree with
  `annuity::payment` about what a zero term means.
- Three previously-`SolveDidNotConverge` inputs and one previously-`Ok` input now
  return a variant that describes what actually happened. This is a behaviour
  change to error *reporting*; no successful solve changes.
- One more public variant on a `#[non_exhaustive]` enum, so downstream `match`
  arms keep compiling.
- The guards are exact identities, so they cannot drift: a future factor with a
  different constancy profile would need its own analysis, and the table above
  records how to do it.

## Alternatives considered

- **Reject a root found at the scan's first probe.** General, and it needs no
  per-call-site knowledge — but it is a heuristic that would also reject a
  legitimate root at `−0.9999`, and it would silently change IRR and XIRR, which
  share the solver and are not affected by this defect. Rejected.
- **Detect constancy by sampling the factor at two rates.** Self-maintaining, but
  agreement at two points is not a proof of constancy, so it trades an exact
  argument for a probabilistic one. Rejected.
- **Reuse `NoRealSolution`.** Wrong in substance: there are infinitely many real
  solutions, not none, and collapsing the two would repeat the mistake ADR-0052
  undid when it deleted `Undefined`. Rejected.
- **Reuse `SolveDidNotConverge`.** What the neighbouring cases already returned,
  so it needs no new variant — but the solver did not fail to converge, and it
  hides a well-posedness problem behind what reads as a numerical one. Rejected.
- **Return `Ok(-0.9999)` and document it.** Any member of the solution set is
  arguably "correct". Rejected: it is indistinguishable from a real answer at the
  call site, which is precisely how this survived until an adversarial review.
