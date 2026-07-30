# ADR-0065: The dated counterparts — a future value at the horizon, a MIRR over the span, and an owned series

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Amends:** [ADR-0029](0029-dated-cashflows-xnpv-xirr.md) (dated cashflows — its
  "Alternatives considered" **rejected** a dated net future value; this reverses that,
  and says why), [ADR-0026](0026-modified-internal-rate-of-return.md) (MIRR — its
  period-count span generalises to a real number of years),
  [ADR-0043](0043-owned-cashflows.md) (the borrowed/owned pair, which the dated
  module never had), [ADR-0060](0060-owned-cashflows-on-the-wire.md) (the owned
  series' wire shape, extended to a second sequence type)
- **Follows:** [ADR-0013](0013-core-api-values-and-discrete-operations.md) (the
  borrowed series and its four operations),
  [ADR-0020](0020-robust-irr-newton-with-bisection-fallback.md) (multiple roots),
  [ADR-0028](0028-binary-surface-conventions.md) (§1 coverage, the `series` family),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (test the class, not the
  instance), [ADR-0050](0050-role-newtypes-for-ambiguous-arguments.md) (role newtypes,
  and the recorded decision to leave MIRR's two rates untagged),
  [ADR-0052](0052-tvmerror-variant-granularity.md) (error granularity),
  [ADR-0054](0054-numeric-robustness-of-the-core-operations.md) (what counts as a
  root), [ADR-0056](0056-degenerate-rate-solves.md) (a solve whose unknown drops out
  reports that), [ADR-0057](0057-currency-is-checked-where-a-result-is-denominated.md)
  (a rate folds no currency), [ADR-0064](0064-continuous-solves.md) (the previous part
  of #106, whose `ZeroPeriods` reasoning this reuses)

## Context

Issue #106 enumerated the public surface module by module. ADR-0062, ADR-0063 and
ADR-0064 closed its three annuity and continuous groups; this one closes the last,
and with it the issue.

`Cashflows` has four operations plus an owned companion: `net_present_value`,
`net_future_value`, `internal_rate_of_return`, `modified_internal_rate_of_return`,
and `OwnedCashflows` (ADR-0043). `DatedCashflows` had **two** of the four and no
owned form — XNPV and XIRR (ADR-0029) — so a model priced from real dates could not
be valued forward, had no unique assumption-explicit return measure, and could not be
built from an iterator or crossed over a wire.

The interesting part is not the arithmetic, which is a line each. It is that the
dated series has **two candidate anchor dates** where the periodic series has one, and
until now only one operation had to care.

## Decision

### The three anchors, named

For evenly spaced flows at periods `0 … n−1`, "the first flow", "the earliest flow"
and "index 0" are the same thing, and "the last flow", "the latest flow" and
"index n−1" likewise. Dated offsets are arbitrary `f64` years and **need not be
sorted** — ADR-0029 rebases to the *first entry* (matching Excel) and its module
explicitly handles unsorted input and negative offsets — so those readings come
apart, and each operation has to say which it means. Writing `t₀` for the
first-listed offset, `t₋` for the earliest and `T` for the latest:

| operation | anchored at | order-dependent? |
| --- | --- | --- |
| `net_present_value` (XNPV) | `t₀` — the first entry | **yes** (unchanged, ADR-0029) |
| `net_future_value` (new) | `T` — the horizon | no |
| `internal_rate_of_return` (XIRR) | — a rate has no date | no |
| `modified_internal_rate_of_return` (new) | `t₋` → `T` — the whole life | no |

**The rule that generates this table: a value is quoted at a date, a rate is not.**
XNPV's reference is a *presentation* choice — which date the answer is expressed at —
and Excel's answer is the first date given. A rate has no denomination in time any
more than in currency (which is exactly ADR-0057's shape), so a rate-returning
operation has no reason to inherit a presentation choice, and inheriting it would make
the answer depend on the order of a slice.

### 1. `net_future_value` compounds to the **latest** offset

```rust
DatedCashflows::net_future_value(rate: Rate<Annual>) -> Result<Money>
// Σᵢ CFᵢ (1 + r)^(T − tᵢ),  T = max tᵢ;  empty series → 0
```

`Cashflows::net_future_value` compounds "to the final period", which for its flows is
both the last index and the latest date. Three arguments pick the latest date as the
generalisation, in increasing order of weight:

1. **Every exponent `T − tᵢ` is then `≥ 0`, so every flow is *compounded*.** That is
   what makes the answer a future value. Compounding to the *last entry* of an
   unsorted slice would discount the flows dated after it — the result would be a
   value at an arbitrary interior date, neither a present nor a future value.
2. **The answer does not depend on the order of the slice**, and the reference drops
   out of the algebra entirely: `Σᵢ CFᵢ (1+r)^(T − tᵢ)` never mentions `t₀`. So unlike
   the present value, this operation has *no reference to choose* — only a horizon.
3. Where the latest flow happens to be listed **first**, `t₀ = T` and the XNPV and the
   XNFV are the same number, because both then value the series at the same date. That
   identity is a test, and any other horizon breaks it.

It follows that `XNFV = XNPV · (1 + r)^(T − t₀)` with a span that is **never
negative** (`T ≥ t₀` by construction), mirroring the periodic `NFV = NPV · (1 + r)ⁿ⁻¹`.
Both the identity and the sign are pinned as properties.

**ADR-0029 rejected this operation; that judgement is reversed, not overlooked.** Its
reason was "XNFV is not a standard function and has no clear reference date to compound
to". The first half is still true — there is no `XNFV` in Excel — and is not decisive:
the crate is not an Excel port, `Cashflows` carries an NFV that Excel also lacks, and
the asymmetry was visible enough that #106's reviewer listed it. The second half is the
substantive claim, and it is answered above: the horizon is the latest date, the
argument for it is that every exponent is then non-negative, and it needs no reference
at all. What ADR-0029 could not see in 2026-07 is that the *other* new operation here
needs the same date, so choosing it once serves both.

**No caller-supplied horizon.** A "value this series at date X" operation is a
different, genuinely useful thing — and additive later — but it is not the counterpart
of `Cashflows::net_future_value`, which takes no horizon either. Adding the parameter
here would make the dated signature diverge from the periodic one for no gain, and
would leave the natural no-argument case undecided anyway.

### 2. `modified_internal_rate_of_return` annualises over the span in **years**

```rust
DatedCashflows::modified_internal_rate_of_return(
    finance_rate: Rate<Annual>,
    reinvestment_rate: Rate<Annual>,
) -> Result<Rate<Annual>>
```

```text
PVₒᵤₜ = Σ_{CFᵢ<0} CFᵢ (1 + f)^(t₋ − tᵢ)     (≤ 0, every exponent ≤ 0 — discounting)
TVᵢₙ  = Σ_{CFᵢ>0} CFᵢ (1 + i)^(T  − tᵢ)     (≥ 0, every exponent ≥ 0 — compounding)
MIRR  = (TVᵢₙ / −PVₒᵤₜ)^(1 / (T − t₋)) − 1
```

ADR-0026's three steps are unchanged; only the annualising exponent generalises, from
`N = len − 1` **periods** to `T − t₋` **years** — in both cases the distance between
the two dates the two amounts are quoted at. On whole-year offsets `0, 1, 2, …` the two
are the same number, which is the cross-engine property below.

**The span is the series' whole life, earliest to latest — not first-entry to last.**
This is the one place the dated MIRR deliberately parts company with XNPV's reference,
and there are two reasons:

- **Order-independence, matching XIRR.** XIRR is already order-independent, though
  XNPV is not: rebasing multiplies `XNPV(r)` by `(1+r)^{t₀}`, a factor that is never
  zero, so it cannot move a root. A rate-returning operation that *did* depend on the
  order would be the odd one out.
- **A first-entry reference would collapse the span for ordinary input.** A series
  merely listed newest-first has `t₀ = T`, so `T − t₀ = 0` and the operation would
  report a degeneracy for a series XIRR handles without comment. That is not a
  defensible answer to give a caller whose only sin was reverse-chronological order.

For a **sorted** series — the common case — `t₋ = t₀` and the two readings agree, so
this decision is invisible except where it matters.

**Both rates stay untagged `Rate<Annual>`.** ADR-0050 records the decision that MIRR's
two adjacent same-typed rate arguments are *not* role-tagged; the dated version matches
the periodic signature rather than diverging from it. (Its argument order is also the
periodic one, so a caller moving between them cannot be surprised.)

### The degeneracies, and why not `ZeroPeriods`

| condition | outcome | reason |
| --- | --- | --- |
| empty series | `EmptyCashflows` | as the periodic MIRR |
| no outflows (`PVₒᵤₜ = 0`) | `NoOutflows` | no present value to grow from; the ratio does not exist |
| `T = t₋` **and** `−PVₒᵤₜ` matches `TVᵢₙ` | **`IndeterminateRate`** | the growth factor is `1` for every rate, and they all satisfy it |
| `T = t₋` and they do not match | `NoRealSolution` | the factor is `1` and no rate satisfies it |
| no inflows (`TVᵢₙ = 0`) | `RateOutOfRange` | the implied rate is `−100%`, via `Rate::from_operation` |
| non-finite ratio or root | `Overflow` | ADR-0021, via `Rate::from_operation` |

**The zero-span row is `unit_factor_outcome`, not `ZeroPeriods`.** ADR-0064 faced the
same choice for `continuous::rate` at `Y = 0` and its three reasons transfer intact:
there is no `Period<P>` here for `ZeroPeriods`' "supply a positive `Period<P>`" advice
to refer to (the dated span is a *computed* number of years); the variant names the
input and stops; and the two outcomes at zero span are **opposite** — with the flows
matching every rate works, with them mismatched none does — so reporting one variant
for both would collapse ADR-0056's distinction. The row therefore goes through the
shared `root::unit_factor_outcome` helper, which is now used by three families
(annuity, continuous, dated) and keeps one definition of "satisfied":
`Residual::is_root`, **not** `==`. A near-miss (`1000` against `1000 + 1e-7`, inside
the `1e-9` relative tolerance) is still satisfied at every rate, and is pinned
separately because an `==` guard would pass every other degeneracy test and fail only
that one.

Note what this replaces: the periodic MIRR's `len < 2 → ZeroPeriods`. The dated
operation needs **no count guard at all** — a single flow has `T = t₋` and falls into
the zero-span row automatically, which also covers the case a count cannot see, several
flows all dated on one day. The periodic operation is **not** changed; its `N` really
is a count, and altering an existing error is not this ADR's business (the same line
ADR-0064 drew around `single_sum::periods`).

**`NoOutflows` is checked *before* the span**, which is the opposite order to the
periodic operation (whose `len < 2` guard fires first, so a lone *inflow* there is
`ZeroPeriods`). The reason is that the ratio has to exist before there is anything to
annualise: `NoOutflows` reports a zero denominator, the span question is about the
exponent applied to the quotient. A lone dated inflow is therefore `NoOutflows`, and a
test asserts exactly that so the order is a decision rather than an accident.

**No solver is involved.** MIRR is a closed form; the only `Residual` in it is the
zero-span comparison. Nothing here introduces an absolute tolerance or a
`relative_tolerance`-style floor — that floor is what let the spurious IRR roots
through (ADR-0054), and the acceptance rule stays the shared scale-relative one.

### 3. `OwnedDatedCashflows` — the missing half of the pair

```rust
pub struct OwnedDatedCashflows { flows: Vec<DatedCashflow> }
```

An exact mirror of `OwnedCashflows` (ADR-0043), behind the same `alloc` feature —
plus `std`/`libm`, since the dated types are: `new` / `From<Vec<_>>` /
`From<DatedCashflows<'_>>` / `FromIterator<DatedCashflow>` in, `as_slice` / `into_vec`
out, a borrowed view via `as_dated_cashflows`, and **one-line forwards** for all six
operations so the borrowed type stays the single source of truth for the math. There is
no periodicity parameter to carry: the dated discount is intrinsically annual
(ADR-0029), which makes this type strictly simpler than its periodic sibling.

**On the wire (ADR-0060): a bare array of `DatedCashflow`.**

```json
[{"offset_years": 0.0, "amount": {"amount": -100.0, "currency": "USD"}}]
```

Everything ADR-0060 decided applies unchanged, and two things are worth stating
because they *differ*:

- **Order is meaningful here.** The periodic series' array order is the period index;
  this one's is the slice order, which is the XNPV's valuation reference. A consumer
  reordering the array changes the XNPV (and nothing else, per the table above).
- **There is no periodicity to omit**, so ADR-0060's central trade — a serialized
  series that does not record its own periodicity — simply does not arise. The
  `JsonSchema` is still **inlined**, but for the plainer reason: it is a sequence, and
  `schemars` inlines sequences. `DatedCashflow` remains the named definition the
  `items` point at.

The shape lives in `src/wire.rs` as `OwnedDatedCashflowsWire<'a>(Cow<'a,
[DatedCashflow]>)`, the same newtype + `Cow` construction and for the same reasons —
one declaration both derives read, borrowed on the way out, owned on the way in.
Validation is element-wise through `DatedCashflow::new`, so a non-finite **offset** —
the invariant that type adds over a plain `Money` — rejects the whole document.

**This needs a ninth CI clippy configuration, and the gap was real.** The dated wire
impls require `alloc` **and** a transcendental-math feature, and no existing
configuration builds that combination without `std`: the `alloc,serde` and `schemars`
lines stop short of `libm`, so the dated types are compiled out of them, and
`--all-features` brings `std` in. So
`--no-default-features --features alloc,libm,serde,schemars` is added, **lib-only** for
ADR-0060's reason (the test targets' `serde_json` enables `serde/std` and would mask a
`std`-only path). It is the dependency graph a downstream `no_std` consumer of the
dated wire format actually gets.

### Currency (ADR-0057)

The rule reads off the result type and needs no new thought — which is the point of
having it:

- `net_future_value` returns `Money`, so it **folds** the flows' currencies by
  ADR-0034's identity rule and can return `CurrencyMismatch`.
- `modified_internal_rate_of_return` returns `Rate<Annual>`, so it does **not**, and
  never can. It carries the `# Currency` rustdoc section ADR-0057 requires, pointing a
  caller who wants the strict reading at `DatedCashflows::currency`.

No third behaviour is invented. Both are tested, on the core and on both binaries, and
the exhaustive `Currency::ALL` fold test the XNPV already had now covers the XNFV too.

### The binaries: both surfaces, ADR-0028 §1

| surface | shape |
| --- | --- |
| CLI | `series xnfv --rate <annual> DATE:AMOUNT…` |
| CLI | `series xmirr --finance <f> --reinvest <r> DATE:AMOUNT…` |
| MCP | `xnfv`, `xmirr` |

**The names follow the family's own grammar.** ADR-0029 fixed the dated leaves as the
bare acronyms `xnpv` / `xirr` — the `x` prefix *is* how this crate says "dated" — so
the counterparts are `xnfv` and `xmirr`. Neither is an Excel function (Excel has no
`XNFV` or `XMIRR`), and that is not a reason to coin something else: the alternative,
spelling these two out while their siblings stay acronyms, would make the family
inconsistent in exchange for familiarity the operations do not have anyway. CLI and MCP
names correspond by flattening the path, as always.

**`xnfv` reuses `DatedSeriesInput`**, exactly as `npv`/`nfv` share `SeriesInput`: the
two differ only in the date they value the series at, not in their arguments. `xmirr`
gets a new `DatedMirrInput` — the dated twin of `MirrInput` — because it takes two
rates instead of one. Both carry the optional `currency`; only `xnfv` echoes it.

**The CLI's `xmirr` interpolates the library's message** (`"modified internal rate of
return: {e}"`), following `series mirr` and the change ADR-0063 made to the annuity
rate solves, for the substantive reason those did: the degenerate truth here is that
*every* rate satisfies the inputs, which a static "no rate solves these inputs" would
invert. `run_series` goes from 55 to 73 lines, inside the function-length lint, and the
two new arms share no shape worth extracting.

### Testing (ADR-0045)

- **Independent high-precision references, not the crate's own functions.** Each new
  operation has a worked case checked against Python `decimal` at 60 significant
  digits, computed for exactly the `f64` inputs involved:
  - XNFV of `(0, −100), (0.5, 40), (1.25, 80)` at 10% → `10.311474146468699192831…`
    (and its XNPV `9.153346455373549884078…`, asserted separately so the identity test
    is not comparing two unknowns).
  - Dated MIRR of `(0, −1000), (0.5, −500), (1.25, 800), (2, 900)` at 10%/12% →
    `0.095102924143168126085…`, from `PVₒᵤₜ = −1476.731294622796156520…` and
    `TVᵢₙ = 1770.970617132655864657…` over `Y = 2`.
  - The CLI/MCP worked case, through the ACT/365 day-count: XNFV `−27.8267360312988…`
    and XMIRR `0.0950481595731959795…` for four ISO dates.
- **Cross-engine properties on whole-year offsets** — the strongest check available,
  because the dated and periodic engines share no code (one raises `(1+r)` to a
  per-flow power, the other folds a running factor by Horner):
  `net_future_value` against `Cashflows::<Annual>::net_future_value`, and the dated
  MIRR against the periodic one. The MIRR generator forces a leading outflow and a
  trailing inflow so both operations are inside their domain, and the comparison is of
  growth factors relative to their own size, since the `1/N` root only shrinks the
  ratio's relative error. The dated MIRR reproduces the periodic test's long-standing
  `0.0728187246` exactly as a point test too.
- **Order-independence is a property, and stating it precisely mattered.** The first
  draft asserted that reversing a series leaves the XNFV *and the XIRR* unchanged, and
  the XIRR half **failed**: a series with several sign changes has several roots, and
  which one the solver returns is not order-invariant (rebasing rescales the residual,
  so Newton starts in a different basin and the fallback's "lowest bracketed root" is
  measured against a different scale). The *root set* is order-independent; the
  *returned* root need not be. That is ADR-0020's documented multiple-root ambiguity —
  the very thing MIRR exists to resolve — not an order-dependence in the dated engine.
  The property was therefore split: the future value over arbitrary unsorted series,
  and the XIRR over **conventional** ones, where the root is unique. The dated MIRR,
  being a closed form, is order-independent unconditionally and has its own property.
- **Unsorted and negative-offset series throughout**, including the two cases that pin
  the anchor choices from the other side: the XNPV equalling the XNFV when the latest
  flow is listed first, and a flow dated before the first entry setting the MIRR's
  reference.
- **Every degeneracy pinned to its specific variant** on the core and on both
  binaries, plus the near-miss that tells `is_root` from `==`, and the lone dated
  inflow that pins `NoOutflows` ahead of the span check.
- **The owned type**: every forward asserted equal to the borrowed view (all six
  operations), the three constructors, `as_slice`/`into_vec`/the bridge, the empty
  series, the inherited currency split, `Send + Sync + 'static`, `serde` point tests
  for the array shape / empty / element rejection / mixed currencies / a working
  deserialized series, `schemars` shape + conformance against what `serde` writes, and
  round-trip properties over arbitrary dated series (values *and* behaviour).
- **MCP output-schema conformance** covers `xnfv` twice — with and without a currency,
  since the optional field's absence is what a schema most easily gets wrong — and
  `xmirr` twice, once with a negative answer.

## Consequences

- `DatedCashflows` mirrors `Cashflows`: four operations and an owned companion, every
  one of them reachable from both binaries. The three anchor dates are named,
  documented and tested rather than implicit.
- **#106 is closed.** Its four groups are discharged by ADR-0062 (sinking fund,
  perpetuity-due), ADR-0063 (due solves, growing inverses), ADR-0064 (continuous
  solves) and this one. `Currency::from_numeric` was the issue's remaining item and is
  **deliberately not** done here — see below.
- **No new `TvmError` variant.** `NoOutflows`, `IndeterminateRate`, `NoRealSolution`,
  `RateOutOfRange`, `EmptyCashflows`, `CurrencyMismatch` and `Overflow` all gain a
  caller; nothing changes meaning. Four variants' rustdoc names the new operation.
- **Purely additive to every signature, command, tool, flag, output shape and
  default.** No existing behaviour moves — in particular the periodic MIRR keeps its
  `ZeroPeriods` and its check order, and XNPV keeps its first-entry reference.
- One more CI configuration to keep green (`no_std + alloc + libm + serde + schemars`,
  lib only), and a line in `CLAUDE.md`'s verification list. It is the ninth clippy
  check, and it closes a genuine hole rather than adding belt to braces.
- `root::unit_factor_outcome` now serves three families. Its contract — "satisfied"
  means `Residual::is_root` — has one home and three call sites.
- The crate now has a worked statement of *which date* a dated operation is anchored
  at, and a rule that generates it (a value is quoted at a date, a rate is not). A
  future dated operation should read that table rather than rediscover it.

## Alternatives considered

- **Compound to the offset of the **last entry** of the slice.** The most literal
  reading of "the final period" and the one that keeps the XNPV's first/last symmetry.
  Rejected on the substance: for an unsorted series it discounts the flows dated after
  that entry, so the answer is a value at an arbitrary interior date rather than a
  future value, and it would make the operation depend on slice order for no benefit.
  It also breaks the `XNFV = XNPV · (1+r)^span` identity's non-negative span.
- **Require a caller-supplied horizon** (`net_future_value(rate, horizon_years)`).
  Honest about the ambiguity, and strictly more general. Rejected: the periodic
  counterpart takes no horizon, so the two signatures would diverge, and the natural
  no-argument case — which is what #106 asked for — would still need a default. A
  `value_at(date)` operation is a separable, additive decision.
- **Sort the flows (or require sorted input) in the constructor.** It would collapse
  the three anchors back into two and make every operation order-independent. Rejected
  twice over: `DatedCashflows::new` is `const` and allocation-free by design
  (ADR-0013), so it cannot sort; and rejecting unsorted input would break ADR-0029's
  explicit contract and Excel compatibility, where the first entry's date is the
  reference precisely *because* the caller chose to list it first.
- **Anchor the dated MIRR at the first entry, like XNPV.** Consistent within the
  module on a surface reading, and it needs no new concept. Rejected: it would make a
  *rate* order-dependent where XIRR is not, and would report a degeneracy for any
  series listed newest-first. Consistency with the module's other *rate* is the
  consistency that matters.
- **Report `ZeroPeriods` for a zero dated span**, matching the periodic MIRR exactly.
  No new reasoning needed, and it keeps the two operations' error sets identical.
  Rejected on ADR-0064's three grounds, the decisive one being that the two outcomes at
  zero span are opposite and one variant cannot say both.
- **Keep a `len < 2` guard as well**, for a cheap early exit. Rejected: it is
  redundant (a single flow has zero span) and *weaker* than the span check, which also
  catches several flows dated on one day — a case no count can see.
- **Check the span before `NoOutflows`**, matching the periodic order. Defensible, and
  it would make a lone inflow `NoRealSolution` on both surfaces. Rejected because
  `NoOutflows` is the more actionable of two simultaneously-true statements, and
  because the ratio must exist before the exponent applied to it is meaningful.
- **Name the new leaves `series dated-nfv` / `series dated-mirr`, or `xnfv`/`dmirr`.**
  Rejected: ADR-0029 already made `x…` this family's prefix for "dated", and a
  half-acronym family reads worse than two coined acronyms. The MCP tool descriptions
  carry the explanation, which is where an unfamiliar name is actually resolved.
- **A shared `DatedMirrInput`-style input for `xnpv`/`xnfv` too**, or conversely a
  separate input struct for each of the four dated tools. Rejected in both directions:
  `xnpv`/`xnfv` take identical arguments, so sharing is right (and matches
  `npv`/`nfv`); `xmirr` takes two rates, so folding it in would advertise fields the
  other tools ignore — ADR-0064's reason for two continuous input schemas.
- **Give `OwnedDatedCashflows` a periodicity parameter** for symmetry with
  `OwnedCashflows<P>`. Rejected as meaningless: the dated discount is annual by
  construction (ADR-0029), and a phantom tag no operation consults would be ceremony
  ADR-0045's own boundary warns against.
- **Put the reference date on the wire** (`{"reference": …, "flows": […]}`) so a
  serialized dated series records its own anchor rather than relying on array order.
  Rejected: it would change what the type *is* — `DatedCashflows` derives the reference
  from the flows, so a wire field could contradict the value it deserializes into — and
  ADR-0060 already settled that the wire form mirrors the in-process shape exactly.
- **Add `Currency::from_numeric`, #106's last item, here.** It is a one-line lookup and
  the issue lists it. Rejected as unrelated: it belongs to the currency table, not the
  dated series, and #106 itself says to treat the list as a menu — the numeric-code
  inverse has no caller asking, where these three closed a visible asymmetry in the
  series API. Closing the issue does not oblige building every line of it, and this
  ADR says so rather than leaving the omission to be noticed later.
