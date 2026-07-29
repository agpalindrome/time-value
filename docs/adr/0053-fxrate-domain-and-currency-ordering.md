# ADR-0053: `FxRate`'s domain is closed under reciprocal, its `from` accessor is renamed, and `Currency`'s ordering is a stated policy

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0034](0034-money-and-currency.md) (its `FxRate` — the accepted
  rate domain and the accessor name; the FX semantics are unchanged)
- **Follows:** [ADR-0021](0021-fallible-operations-on-non-finite-results.md) (an
  operation is fallible when its result can be non-finite),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (enforce an invariant at
  the chokepoint; test the class, not the instance)

## Context

Three defects in the FX and currency surface, each cheap to fix now and permanent
once the crate is published (nothing is published — ADR-0038).

### 1. `FxRate::inverse` documented a guarantee its constructor did not provide

`FxRate::new` accepted any `rate.is_finite() && rate > 0.0`. `FxRate::inverse` is
infallible (`-> Self`) and its rustdoc asserted:

> Infallible: the rate is finite and strictly positive by construction, so its
> reciprocal is finite and strictly positive too.

That is false. `is_finite() && > 0.0` admits **subnormals**, whose reciprocals
overflow:

```text
FxRate::new(Usd, Eur, 5e-324)?.rate()               // 5e-324 — accepted
    .inverse().rate()                                // +∞
    .inverse().inverse().rate()                      // 0.0 — a value `new` rejects
```

So the type had a reachable state in which its own accessor returns a non-finite
rate, and a double inverse lands outside the constructor's domain entirely. A
`Money::convert` through such a rate then multiplies by `∞` and reports
`Overflow`, blaming the amount for a rate that was never valid.

### 2. `FxRate::from` permanently blocks `From::from` in path form

The source-currency accessor was `pub const fn from(self) -> Currency`. An
inherent method named `from` wins method resolution over a trait method of the
same name, and — the part that matters — path-form resolution does **not** fall
through: with an inherent `from` present, `FxRate::from(x)` names the accessor,
and no future `impl From<T> for FxRate` could ever be called that way. The trait
would still work through `x.into()` and `<FxRate as From<T>>::from(x)`, but the
idiomatic form is lost, silently and forever.

Nothing in the crate implements `From<_> for FxRate` today. That is exactly why
this is worth fixing now: the cost is one rename, and it rises to a breaking
change the moment the name ships.

### 3. `Currency`'s `Ord` had no stated policy, and `#[non_exhaustive]` makes that dangerous

`Currency` derives `Ord` and declares its variants in alphabetical order of ISO
4217 code, so `Ord` *currently* means "alphabetical by code". Nothing said so, and
nothing said what a maintainer adding a newly-issued ISO code must preserve.

`#[non_exhaustive]` promises that adding a variant is non-breaking, which is true
of **compilation** and says nothing about **behaviour**. A derived `Ord` on a
fieldless enum compares declaration position, so where a new variant is declared
is a semantic decision about every `BTreeMap<Currency, _>`, sorted report and
persisted sort key downstream — made, under the current documentation, by whoever
happens to paste the variant in.

The module doc compounded the confusion: it said "The enum is exhaustive" nine
lines before the type doc said "The enum is `#[non_exhaustive]`". Both were true
in their own sense — complete over the ISO 4217 active set; open as a Rust type —
but read together they look like a contradiction.

## Decision

### 1. Narrow `FxRate::new` so the domain is closed under reciprocal

```rust
if rate.is_normal() && rate > 0.0 && (1.0 / rate).is_normal() { … }
```

Fix the **constructor**, not `inverse`. `inverse` stays infallible.

All three conjuncts are load-bearing:

- `is_normal()` rejects zero, `NaN`, the infinities **and** the subnormals — the
  case the old test missed. It does *not* reject negatives, so
- `rate > 0.0` is still required (`(-1.0).is_normal()` is `true`).
- `(1.0 / rate).is_normal()` closes the upper end, and it is **not** implied by
  the first two. `f64::MAX` is normal, but `1.0 / f64::MAX ≈ 5.56e-309` is
  *subnormal*, and inverting that again overflows to `+∞`. Verified directly
  rather than assumed.

The accepted domain is therefore `[f64::MIN_POSITIVE, 1.0 / f64::MIN_POSITIVE]` ≈
`[2.225e-308, 4.494e307]`, which is symmetric under reciprocal: for any accepted
`r`, `1.0 / r` is accepted too, so `inverse()` always yields a rate `new` would
itself accept, and `inverse().inverse()` returns to the domain. The rustdoc's
claim is now true, and is pinned by a test at both boundaries.

The excluded band is not a practical restriction: the extremes of real currency
markets span roughly `1e-7` to `1e7` (a hyperinflated unit against a strong one),
some 300 orders of magnitude inside the boundary. Any input that hits the band is
a bug in the caller's rate source, and `InvalidExchangeRate` is the right answer
for it. The rustdoc says so, so a reader does not mistake it for a real limit.

`Rate` is deliberately **not** given the same treatment: it is an additive
interest rate on `(-1, ∞)`, has no reciprocal operation, and closing under
reciprocal would mean nothing there.

Three **messages** are corrected to match, because `5e-324` *is* finite and
greater than zero, so the old text became a lie for exactly the inputs the change
newly rejects:

- `TvmError::InvalidExchangeRate`'s `Display` now reads "exchange rate must be
  greater than zero and invertible (2.3e-308 to 4.5e307)", and its rustdoc names
  the band.
- The CLI's `convert` interpolates that library message instead of restating it
  with a static `.context(…)` — the ADR-0052 pattern, for the same reason: the
  library names the accepted band and no static string here could keep in step.

The MCP tool description ("`rate` … must be finite and positive") is **left
alone**. It is part of the declared MCP surface, and it states a necessary
condition that remains true; rewording it would change the surface to buy
precision about a band no caller can reach in practice.

### 2. Rename the accessor to `source()`

```rust
pub const fn source(self) -> Currency   // was: from
pub const fn to(self) -> Currency       // unchanged
```

`source` over `base`, though `base` is the FX-domain term, because:

- The existing rustdoc already called it "the source currency (the unit being
  priced)" — `source` names what the field was always documented to be.
- `base` belongs to the **base/quote** pair convention (`EUR/USD`), and its
  counterpart is `quote`, not `to`. `to()` is staying (it is not shadowed, it is
  the natural preposition against `convert`), so `base`/`to` would import half of
  a naming convention and leave it mismatched. `source`/`to` reads as one pair.
- `FxRate` is a directional price, not a quoted market pair; adopting the market
  pair's vocabulary would suggest bid/ask and triangulation semantics that
  ADR-0034 explicitly puts out of scope.

The **serde/schemars wire field stays `from`.** It is spelled in `FxRateWire`,
not derived from the accessor name, so `{"from": "USD", "to": "EUR", "rate": 0.9}`
is byte-identical before and after (ADR-0042/0044 untouched), as are the CLI's
`convert --from/--to` flags and the MCP `convert` tool's input schema. Only the
Rust accessor moves; the crate's one internal caller is `Serialize for FxRate`.

### 3. `Currency`'s ordering is alphabetical by ISO code, and stays that way

The committed guarantee: **`a < b` is exactly `a.code() < b.code()`.** The
obligation that follows for a maintainer adding a newly-issued ISO code:
**insert the variant in its alphabetical position** — in the enum and in
`Currency::ALL` alike — rather than appending it.

The two candidate policies turn out not to conflict, which is why this one is
chosen. Inserting into a sorted sequence transposes nothing already in it: the
new code lands between two existing ones and leaves their relation untouched. So
the single rule "insert alphabetically" delivers *both* the alphabetical order the
derive appears to promise *and* the stability of every existing pair — a
`BTreeMap<Currency, _>` gains the new code in its proper place, and no
already-stored ordering is reshuffled.

Two things are explicitly **not** guaranteed, and the rustdoc says so:

- The **discriminant** (`Currency::Usd as u16`) shifts whenever an
  alphabetically-earlier variant is added. It is an implementation detail; a
  caller persisting an ordering key persists `code()`.
- The ISO **numeric** code is uncorrelated with the ordering — ISO does not assign
  numbers alphabetically.

If a non-ISO variant is ever added (ADR-0034 leaves the door open for `Custom`),
it has no ISO code to sort by and is declared last; the guarantee then reads
"alphabetical among the ISO codes, non-ISO variants after them", which still
transposes no existing pair.

The derive is **not** changed. A hand-written `Ord` delegating to `code()` would
enforce the policy mechanically, but it costs a string comparison on a type whose
whole point is being a trivially-comparable `Copy` enum, and it cannot be `const`.
A test enforces the policy instead (below).

Finally, the module doc's "the enum is exhaustive" is reworded to state the claim
it actually meant — each metadata table is an *exhaustive `match`* over the
variants, so the compiler guarantees every variant carries a code, a numeric code
and a minor-unit exponent — and the type doc opens "As a *Rust type* the enum is
`#[non_exhaustive]`, even though it is complete over the ISO 4217 active set". The
two statements no longer read as a contradiction.

### Testing (ADR-0045 rule 2)

Every assertion above earns a test:

- Both ends of the newly-excluded band are rejected (`5e-324`,
  `f64::MIN_POSITIVE / 2.0`, `f64::MAX`), and the test *shows* why both halves of
  the predicate are needed by asserting `f64::MAX.is_normal()` alongside
  `(1.0 / f64::MAX).is_subnormal()`.
- Both exact boundaries are accepted (`f64::MIN_POSITIVE` and its reciprocal).
- `inverse()` of an accepted rate is itself accepted by `new` — the closure
  property, checked at the extremes and not only in the middle.
- A double inverse recovers the original direction exactly and the magnitude to a
  relative `1e-15` (`1.0 / (1.0 / x)` is not exact for every `x`, per ADR-0033's
  approximate-real contract).
- The narrowed domain is pinned at **both binary surfaces** too, not only in the
  core: the CLI's `convert` and the MCP `convert` tool each reject a subnormal
  rate as an exchange-rate error, and the CLI test also asserts that a
  realistically extreme rate (`1e-7`) still succeeds — the band is not a
  restriction anyone can trip over by accident.
- The ordering policy is pinned **exhaustively over the finite enum** — the
  ADR-0045 preference for exhaustive iteration over sampling on a small closed set
  — asserting both that `ALL` is in alphabetical order and that `Ord` agrees with
  `code()` for every pair, adjacent or not. Appending a new ISO code instead of
  inserting it fails this test.

## Consequences

- **Breaking, twice.** `FxRate::from` is gone (call `source()`), and `FxRate::new`
  rejects a band it previously accepted. Nothing is published (ADR-0038), so no
  released API moves. The rate narrowing can only reject inputs that were already
  producing a lying `inverse()`.
- `FxRate::inverse`'s rustdoc is now true, and `FxRate` has no reachable state in
  which `rate()` is non-finite or `inverse()` escapes the constructor's domain.
- A future `impl From<T> for FxRate` is usable in path form. None exists yet; the
  point is that the door is no longer nailed shut.
- **No wire-format or binary-surface change.** The serde/schemars representation,
  the CLI `convert` grammar and the MCP `convert` tool's name and input/output
  schemas are unchanged. Verified by running both binaries at `main` and at this
  branch and diffing: the CLI's full recursive `--help` dump (34 subcommands) and
  the MCP `tools/list` response (34 tools, with their complete input and output
  schemas) are byte-identical. What does change, by design, is the *outcome* for a
  rate in the newly-excluded band — `InvalidExchangeRate` instead of a subnormal
  result or a misattributed `Overflow` — and the wording of that error.
- Follow-on obligation: **a newly-issued ISO 4217 code is inserted into `Currency`
  in alphabetical position**, in the enum and in `ALL`. The ordering test is the
  enforcement.
- Follow-on obligation: an inherent method must not be named `from` on a public
  type. `to` is fine — there is no `To` trait in the prelude to shadow.

## Alternatives considered

- **Make `inverse` fallible (`-> Result<Self, TvmError>`) and leave `new` alone.**
  Rejected on two counts. It pushes a `?` onto a pure accessor for a failure with
  no caller-actionable meaning — "your rate was subnormal" is not something a
  caller can respond to differently from "your rate was invalid", which `new`
  already says. And it puts the check at the wrong place: the constructor is the
  chokepoint (ADR-0045), and validating there means every `FxRate` in existence is
  invertible, rather than every *call* to `inverse` re-deriving the fact. It also
  ripples: the crate's own doctest writes `eur.convert(usd_to_eur.inverse())?`
  inline, which would need a second `?`.
- **Clamp the rate into the valid band instead of rejecting it.** Silently
  changing a caller's number is precisely the foot-gun this crate exists to avoid
  (ADR-0021).
- **Require `rate > 0.0 && rate.is_finite() && (1.0 / rate).is_finite()`.** Weaker
  and subtly wrong: `1.0 / 5e-324` is `+∞` so that case is caught, but
  `1.0 / f64::MAX` is *finite* (subnormal) and would be accepted, leaving the
  double-inverse-to-infinity hazard in place. Normality is the property that
  actually closes the domain.
- **Name the accessor `base`.** The FX-domain term, and rejected above: its
  counterpart is `quote`, not the `to` that is staying, and it implies market-pair
  semantics ADR-0034 puts out of scope.
- **Name it `from_currency`.** Unshadowed and unambiguous, but noisy beside a bare
  `to()`, and it reads as a *constructor* (`from_code`, `from_effective_annual`)
  in a crate where `from_*` already means exactly that.
- **Append new ISO codes at the end of `Currency` and drop the alphabetical
  claim.** The obvious way to "keep existing order stable", and unnecessary:
  alphabetical insertion keeps existing pairs stable *as well*, so appending would
  give up a real property to buy one already held.
- **Hand-write `Ord` to delegate to `code()`.** Enforces the policy mechanically
  rather than by test, but replaces a discriminant comparison with a string
  comparison on the crate's cheapest type, and cannot be `const`. The exhaustive
  test catches the same mistake at build time.
- **Document the ordering as unspecified** ("do not rely on it; sort by `code()`").
  Honest and cheap, and rejected because it is a worse outcome for the caller: the
  order *is* alphabetical, a `BTreeMap<Currency, _>` *does* iterate usefully, and
  disclaiming a property the type already has just makes callers reimplement it.
