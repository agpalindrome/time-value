# ADR-0062: The sinking-fund payment and the perpetuity-due

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Follows:** [ADR-0015](0015-annuities.md) (annuities — its 2026-07-10 amendment
  deferred "a due perpetuity", **discharged** here),
  [ADR-0025](0025-solve-for-periods-and-rate.md) (solve for periods and rate — the
  `_from_future` coinage, **extended** to the payment),
  [ADR-0028](0028-binary-surface-conventions.md) (binary surface conventions —
  §1 coverage, §2's anchored solve-for pair),
  [ADR-0048](0048-finite-growing-annuity.md) /
  [ADR-0049](0049-growing-annuity-in-the-binaries.md) (the growing family, and how
  it was surfaced), [ADR-0050](0050-role-newtypes-for-ambiguous-arguments.md)
  (role newtypes, and their boundary),
  [ADR-0054](0054-numeric-robustness-of-the-core-operations.md) (the
  `expm1`/`ln1p` factors), [ADR-0056](0056-degenerate-rate-solves.md) (where the
  factors stop depending on `r`),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (test the class, not the
  instance)

> **Extended (2026-07-30) by [ADR-0063](0063-annuity-due-solves-and-growing-inverses.md).**
> The first two entries of the *Out of scope* list below are discharged there:
> `annuity::due` now has its `periods` and `rate` solves (from either anchor), and the
> growing annuity has its present-anchored inverses. The note about `run_annuity` being
> "the last flat arm that fits" is discharged too — ADR-0063 extracted the repeated
> shapes, leaving that dispatcher shorter than it is here. #106's other three groups
> (`continuous`, `DatedCashflows`, `Currency::from_numeric`) remain open.

## Context

Issue #106 enumerated the public surface module by module and found several
operations whose obvious counterpart was absent. Two of them are textbook gaps in
the annuity family, and this ADR closes those two; the other three groups (the
`annuity::due` solves, the growing-annuity inverses, the `continuous` and
`DatedCashflows` gaps) are separate work.

**The `_from_future` coinage was applied to two of three solves.** `annuity` has
`periods` / `periods_from_future` and `rate` / `rate_from_future`, but `payment`
had no `payment_from_future` — the sinking-fund payment, Excel's
`PMT(rate, nper, 0, fv)`: "how much must I set aside each period to reach this
target". Every other bank-account, savings-goal, or debt-service-reserve question
is that one, and the library could price the stream forward
(`annuity::future_value`) without being able to solve it back.

**There was no perpetuity-due.** ADR-0015's amendment added `perpetuity` and
`growing_perpetuity`, noted they are ordinary (end-of-period), and said in as many
words: "a due perpetuity is again a `(1 + r)` scaling and can be added later if
wanted". `PV = (PMT / r) · (1 + r)` is textbook — the first payment falls today and
is not discounted — and its absence was the only hole left in the `due` module's
mirroring of the parent.

## Decision

### The core gains four functions

```rust
annuity::payment_from_future(rate: Rate<P>, periods: Period<P>, future: Money)      -> Result<Money>
annuity::due::payment_from_future(rate: Rate<P>, periods: Period<P>, future: Money) -> Result<Money>
annuity::due::perpetuity(rate: Rate<P>, payment: Money)                             -> Result<Money>
annuity::due::growing_perpetuity(rate: Rate<P>, growth: Growth<P>, payment: Money)  -> Result<Money>
```

**The perpetuities-due live in `annuity::due`, beside the other due forms.** The
core is organised by *timing*: the top level is ordinary (end-of-period) and `due`
is the submodule that mirrors it name-for-name, which is exactly what ADR-0015's
amendment chose over a `_due` suffix. `perpetuity` and `growing_perpetuity` sit at
the top level not because perpetuities are special but because those two *are* the
ordinary ones. So `annuity::due::perpetuity` reads against `annuity::perpetuity`
the way `annuity::due::present_value` already reads against
`annuity::present_value`, and the `due` module's rustdoc keeps its single
organising sentence ("each factor is scaled by `(1 + r)`") true of every member.

**A growing perpetuity-due is in scope, because it is the two-line delegation the
level one is built from.** At the module top level `perpetuity` already delegates
to `growing_perpetuity` with `Growth(0)`; `due::perpetuity` does the same, and
`due::growing_perpetuity` delegates to `super::growing_perpetuity` and scales the
result by `(1 + r)`. Excluding the growing form would have meant *hand-writing* the
level one instead — more code, not less, and a second place for the divergence rule
to live.

**Delegation, not a second closed form.** `due::growing_perpetuity` calls the
ordinary function and multiplies, so the two cannot disagree about which
rate/growth pairs are admissible. The sinking-fund payments divide by the existing
private `future_value_factor` (ADR-0054's `expm1`/`ln1p` form); no new closed form
is written anywhere in this change, so the cancellation ADR-0054 removed cannot
reappear.

### Role newtypes: none of the four gets one (ADR-0050 rule 4)

`payment_from_future` takes a `Rate<P>`, a `Period<P>`, and **one** `Money` — the
target — and returns the payment. ADR-0050 rule 4 is explicit that "a function with
at most one `Money` argument … keeps taking a plain `Money`", and names
`annuity::payment` itself as an example. There is no transposition to prevent: with
one monetary argument there is no adjacent same-typed argument to swap it with, and
a `FutureValue` wrapper here would catch no failure mode while making the pair
`payment(r, n, Money)` / `payment_from_future(r, n, FutureValue)` read as though one
of them were safer than the other. `rate_from_future` and `periods_from_future` do
take roles, but they take *two* amounts (a `Payment` and a `FutureValue`) — that is
the ambiguity being tagged, not the `_from_future` suffix.

`due::growing_perpetuity` does take two `Rate<P>`s, so its second is a
`Growth<P>`, matching `annuity::growing_perpetuity` exactly.

### Degeneracies, worked out rather than guessed

ADR-0056 enumerated where the annuity factors stop depending on `r`; the same table
settles what these four must reject.

| operation | degenerate at | outcome |
| --- | --- | --- |
| `payment_from_future` | `n = 0` (the factor is `0`) | `ZeroPeriods` |
| `due::payment_from_future` | `n = 0` | `ZeroPeriods` |
| `due::perpetuity` | `r ≤ 0` | `DivergentPerpetuity` |
| `due::growing_perpetuity` | `r ≤ g` | `DivergentPerpetuity` |

- **`n = 0` is `ZeroPeriods`, matching `annuity::payment`.** The future-value factor
  is `0` there, so the payment is absent rather than merely large, and both
  `payment` and the two rate solves already report that variant on the same input
  (ADR-0056). No new variant is needed.
- **`n = 1` needs nothing.** `s(r, 1) = 1`, so the payment *is* the target, at every
  rate. This is the case ADR-0056 had to guard in `rate_from_future`, and the
  contrast is the point: there the rate is the unknown, so a factor that ignores it
  leaves the equation under-determined (`IndeterminateRate`); here the rate is
  *given*, so the same identity is a perfectly determined answer. The check was made,
  not assumed, and it is pinned by a test.
- **No other term divides by zero.** `s(r, n) ≥ 1` for every `n ≥ 1` and every rate
  in `Rate`'s domain — it is exactly `1` at `n = 1` and rises with `n`, for negative
  rates as well as positive — so `n = 0` is the only zero. Nor does the due scaling
  add one: `Rate::new` rejects anything at or below `−100%`, so `1 + r` is strictly
  positive.
- **A perpetuity-due diverges exactly when the ordinary one does.** Bringing every
  payment forward one period rescales a convergent sum by `(1 + r)`; it cannot make
  a divergent one converge. So the due forms reuse `DivergentPerpetuity` by
  delegating, rather than restating the condition.

### The binaries: both surfaces, via the anchor ADR-0028 §2 already established

ADR-0028 §1 requires every new core operation to reach both binaries in the same
PR. The `_from_future` half is surfaced the way the *other* two `_from_future`
functions already are — **not** as a second command or tool. ADR-0028 §2 fixed that
convention ("solve-for variants collapse into one command with a
mutually-exclusive flag pair"), which is why there is no `annuity nper-from-future`
or `annuity_periods_from_future` today, and `payment` now follows it:

| surface | shape |
| --- | --- |
| CLI | `annuity payment --rate <r> --periods <n> (--present <PV> \| --future <FV>)` |
| CLI | `annuity due payment …` — the same anchored pair |
| MCP | `annuity_payment` / `annuity_due_payment`, `present` and `future` both optional, exactly one required |

`--present` becomes optional rather than required, which is additive: every
existing invocation still parses, and omitting both now yields the same "provide
either `--present` or `--future`" message `nper` and `rate` give.

The perpetuities-due are new operations, so they do get names:

| surface | level | growing |
| --- | --- | --- |
| CLI | `annuity due perpetuity --rate <r> --payment <PMT>` | `annuity growing due-perpetuity --rate <r> --growth <g> --payment <PMT>` |
| MCP | `annuity_due_perpetuity` | `annuity_growing_due_perpetuity` |

**The growing one goes in the `growing` group, the level one in `due` — because that
is where their non-perpetual siblings already are.** ADR-0049 decided that the
binaries organise by *variation* rather than by module path: `growing` is the
grouping level and `due` becomes part of the leaf name, giving
`annuity growing due-pv` ↔ `annuity_growing_due_present_value`. Applying the same
rule puts the growing perpetuity-due at `annuity growing due-perpetuity` ↔
`annuity_growing_due_perpetuity`, and leaves the level one at
`annuity due perpetuity` ↔ `annuity_due_perpetuity`. The split between the two
groups mirrors the split that already exists for the values (`annuity due pv` for
the level form, `annuity growing due-pv` for the growing one), and every name
remains derivable from its counterpart on the other binary by flattening the path.

**Dispatcher length: a helper, not an `#[allow]`.** `run_annuity` was already the
longest dispatcher, and the anchored branch written inline in both the ordinary and
the due arm would have pushed it over the workspace's function-length lint. A shared
`level_payment` helper — parameterised by the pair of core functions, whose
signatures are identical — carries the branch for both arms instead, so
`run_annuity` grows by four lines (93 → 97) and stays inside the lint, with the same
helper serving the MCP server's two tools. This is the structural answer ADR-0049
reached for the growing subgroup, at the smaller scale this change needed. It is
also the last flat arm that fits: the *next* addition to `annuity`'s direct
subcommands needs a subgroup or an extracted dispatcher, not another inline arm.

### Testing (ADR-0045)

- **The closed forms are checked against an independent reference**, never against
  the crate's own other functions — the discipline ADR-0048's tests set. The
  sinking-fund payments are compared with the future-value factor **summed term by
  term** (`Σ (1+r)^k`, one payment at a time), and the perpetuities-due with the
  geometric series summed directly, truncated at 1500 terms where the remaining tail
  is provably about `1e-19` relative — eleven orders below the tolerance asserted.
- **The inverse relationship is a property**, not a point test:
  `payment_from_future` inverts `annuity::future_value` (and the due pair likewise)
  across the generated rate/period/amount space. **The tolerance is derived.** The
  round trip multiplies by `s(r, n)` and divides by the identical factor, so the
  error is two `f64` roundings (≈ `4.5e-16` relative) and `s ≥ 1` means nothing
  amplifies it; `1e-9` relative is that bound with six orders of margin.
- **The `n = 1` identity is a property too**, since it is universal in the rate. It
  is asserted to `1e-12` relative rather than exactly, and the reason is recorded at
  the test: `1` is the factor's *algebraic* value, but it is computed as
  `expm1(1·ln1p(r)) / r`, and `expm1 ∘ ln1p` is not bit-exactly the identity — an
  exact `==` assertion was written first and fails (e.g. at `r = 7.18`). The rustdoc
  says "to within a couple of ULP" for the same reason.
- **Every degenerate case is pinned to its specific variant**, on both the core and
  the CLI, and the divergence contrast is asserted on all three surfaces.
- **The MCP output-schema conformance test covers the new tools and the new
  anchor** — `annuity_payment` and `annuity_due_payment` appear twice each, once per
  anchor, because the anchor selects a different core call.

## Consequences

- `annuity`'s solve set is symmetric: `payment`, `periods`, and `rate` each solve
  from a present *or* a future value, and each has a `due` counterpart for the
  payment.
- ADR-0015's deferred perpetuity-due is discharged, so `annuity::due` now mirrors
  every member of its parent module except the two solves (`periods` / `rate`),
  which #106 tracks separately.
- Every core operation is again reachable from both binaries, so ADR-0028 §1 holds
  and the surfacing backlog is empty.
- **Purely additive.** No existing signature, command, tool, flag, output shape, or
  default changes; `AnnuityPaymentInput`'s `present` relaxes from required to
  optional, which no existing caller can notice.
- One shared `level_payment` helper now sits between each binary's dispatcher and
  the core's payment solves. It is the reason `run_annuity` is still inside the
  function-length lint, and the note above records that the next flat arm is not.
- `DivergentPerpetuity` and `ZeroPeriods` gain callers but no new variant is added,
  so `TvmError` is unchanged.

## Alternatives considered

- **`payment_from_future` takes a `FutureValue`.** Symmetrical with
  `rate_from_future` / `periods_from_future` at a glance. Rejected on ADR-0050's own
  rule 4: those two take *two* amounts, which is the ambiguity a role tags; this one
  takes a single amount, where the newtype catches nothing and only implies that the
  plain-`Money` `payment` beside it is less safe.
- **A fourth argument on `payment`** (`payment(rate, periods, present, future)`,
  Excel-style, one of them zero). Rejected: it makes the common case carry the rare
  one's ceremony and encodes "which one is meant" in a magic zero — the reasoning
  ADR-0048 used to reject an optional `growth`, and ADR-0049 to reject a `growing`
  flag.
- **A separate CLI command / MCP tool per anchor** (`annuity payment-from-future`,
  `annuity_payment_from_future`). Rejected: ADR-0028 §2 settled this for the whole
  solve-for family, and there is no `annuity_periods_from_future` tool for the same
  reason. Two tools would also double the surface for one shared input shape.
- **Put the perpetuities-due at the module top level** as `perpetuity_due` /
  `growing_perpetuity_due`. Rejected: the top level is *ordinary*, and a `_due`
  suffix is precisely what ADR-0015's amendment chose the `due` submodule over.
- **Put the growing perpetuity-due in the `due` CLI group** as
  `annuity due growing-perpetuity` (MCP `annuity_due_growing_perpetuity`), pairing it
  with the top-level `annuity growing-perpetuity`. A real alternative — it groups the
  two perpetuities-due together. Rejected because ADR-0049 already decided the
  ordering for a due × growing combination (`growing` first, `due` in the leaf), and
  a user who has seen `annuity_growing_due_present_value` will guess
  `annuity_growing_due_perpetuity`, not the other order. The cost accepted is that
  the ordinary `annuity growing-perpetuity` remains a flat `annuity` subcommand
  rather than moving into the `growing` group — moving it would break the existing
  grammar, which this change does not do.
- **Skip the growing perpetuity-due**, adding only the level one. Rejected: the level
  form is *built* from the growing one by delegation, exactly as at the module top
  level, so omitting it means writing more code and giving the divergence rule a
  second home.
- **A fresh closed form for the sinking fund** (`FV · r / ((1 + r)ⁿ − 1)`). Rejected:
  that is the literal expression ADR-0054 replaced, and writing it here would
  reintroduce the cancellation for small rates in a new place. Dividing by the
  existing private factor inherits the fix.
- **`#[allow(clippy::too_many_lines)]` on `run_annuity`.** Rejected for the reason
  ADR-0049 gives: the lint is pointing at a structural answer, and a shared helper is
  that answer.

## Out of scope (deliberately)

Named here so the remaining parts of #106 are not mistaken for oversights:

- **`annuity::due` has no `periods` or `rate` solve.** The due factors are the
  ordinary ones scaled by `(1 + r)`, so both are tractable, but a solve is a larger
  change than a scaling (the rate solve is iterative, and the `n = 1` degeneracy
  moves) and it is its own decision.
- **The growing annuity has no inverses** — no `growing_payment`,
  `growing_periods`, or `growing_rate` (ADR-0048 added the values only).
- **`continuous` has no solve-for-rate or solve-for-years**, unlike `single_sum`.
- **`DatedCashflows` has no net future value, no MIRR, and no owned counterpart.**
- **`Currency::from_numeric` does not exist**, the inverse of `numeric()`.

Each is additive and each is judged on #106's own terms — "a menu rather than a
checklist": ADR-0045's boundary ("a newtype that catches no real failure mode is
not an improvement") generalises to operations nobody calls, so symmetry alone does
not earn a place on the surface. The two closed here were chosen because they are
textbook operations with a named question behind them, not because they completed a
matrix.
