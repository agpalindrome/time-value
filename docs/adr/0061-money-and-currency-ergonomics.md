# ADR-0061: `Money` and `Currency` ergonomics — a fallible total, fallible `min`/`max`, infallible sign, and a lenient `FromStr`

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Amends:** [ADR-0023](0023-money-arithmetic-surface.md) (its deferred "a total
  over an iterator needs its own bespoke method and its own decision" — decided
  here), [ADR-0032](0032-ergonomic-convenience-impls.md) (the convenience surface
  it batched, extended with four more items and one trait impl)
- **Follows:** [ADR-0021](0021-fallible-operations-on-non-finite-results.md) (an
  operation is fallible **iff** its result can be non-finite),
  [ADR-0034](0034-money-and-currency.md) (currency is a runtime value, `Xxx` is
  the identity), [ADR-0052](0052-tvmerror-variant-granularity.md) (one variant
  per distinguishable failure; payloads cost more than variants),
  [ADR-0057](0057-currency-is-checked-where-a-result-is-denominated.md) (a
  `Money`-producing operation folds the currencies),
  [ADR-0059](0059-the-finite-scalars-are-totally-ordered.md) (`Money`'s ordering
  stays **partial**, so it has no `Ord`),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (pin every stated
  assumption; iterate a finite domain rather than sample it)
- **Closes:** issue #107

## Context

An adversarial pre-publication API review (issue #107) found the two value types
missing gestures a caller reaches for immediately:

- `Money` has no `abs` (`E0599`), no `signum`, no `min`/`max`, and no total over
  an iterator — `flows.iter().copied().sum::<Money>()` is `E0277`. Folding
  `try_add` by hand was the only route to the most obvious thing anyone wants
  from a slice of cashflows.
- `Currency` has no `FromStr`, so `"USD".parse::<Currency>()` does not compile.
  `from_code` exists but returns `Option`, so the crate had no error to report
  for an unrecognised code.

All of it is additive. What makes it worth an ADR is that three of the five items
cannot be given the shape a caller would first guess, and the reasons are the
crate's existing rules rather than taste:

1. **`std::iter::Sum` must be infallible.** `Sum::sum` returns `Self`. Summing
   `Money` can fail two ways — a currency mismatch (ADR-0034/0057) and overflow
   to non-finite (ADR-0021) — so the trait could only panic or hand back a
   non-finite `Money`, the exact foot-gun ADR-0021 and ADR-0023 exist to remove.
2. **`Money`'s ordering is partial.** ADR-0059 gave `Eq`/`Ord` to the three
   finite scalars and deliberately left `Money` at `PartialEq`/`PartialOrd`,
   because two distinct non-`Xxx` currencies do not compare. `Money` therefore
   has no `Ord`, and so none of `Ord`'s infallible `min`/`max`/`clamp`.
3. **`FromStr::Err` is permanent public API.** Choosing it means choosing between
   the crate's one error enum and a new dedicated type, and — separately —
   deciding whether parsing is as case-sensitive as `from_code`.

[ADR-0032](0032-ergonomic-convenience-impls.md) is the prior decision on this
class of addition, and it was checked first: it covers `ZERO`, `Default`,
`TryFrom<f64>`, `From<Money> for f64`, and the *dropped* `Money::is_finite()`.
**None of the five items here was considered or declined there**, so nothing is
being overturned. ADR-0023 is the ADR with a claim on `Sum`, and it *deferred*
rather than declined: "`Sum` cannot be fallible, so a total over an iterator
needs its own bespoke method and its own decision." This is that decision.

## Decision

`Money` gains four methods and one associated function; `Currency` gains one
trait impl; `TvmError` gains one variant.

```rust
impl Money {
    pub fn abs(self) -> Self;                                   // infallible
    pub fn signum(self) -> f64;                                 // infallible
    pub fn try_min(self, other: Self) -> Result<Self, TvmError>;
    pub fn try_max(self, other: Self) -> Result<Self, TvmError>;
    pub fn try_sum<I: IntoIterator<Item = Money>>(amounts: I) -> Result<Self, TvmError>;
}

impl FromStr for Currency { type Err = TvmError; }              // case-insensitive

enum TvmError { /* … */ UnknownCurrencyCode }
```

ADR-0023's test still decides the operator/method split: **provably finite ⇒
infallible; otherwise ⇒ `try_*`.** Every choice below is that test applied.

### `try_sum`, not `Sum` — and an associated function, not a trait

The total is fallible, so it is a `try_*` name returning `Result`, for the two
reasons above. The remaining question was its *shape*. Three were weighed:

- **An inherent associated function** — `Money::try_sum(flows.iter().copied())?`
- **A free function** — `time_value::try_sum(…)?`
- **An extension trait** — `flows.iter().copied().try_sum()?`

**The associated function wins.** It is a one-word edit away from the gesture
issue #107 reports as broken (`.sum::<Money>()` becomes `Money::try_sum(…)`); it
accepts any `IntoIterator` — a slice's `.iter().copied()`, an array, a `Vec`, the
output of a `map` — so it composes with the iterator chain a caller already has;
it needs no import and no trait in scope; and it lands on `Money`'s own rustdoc
page beside `try_add`, which is where someone looking for it will be.

The extension trait reads best at the call site and was rejected on cost: it is a
permanent public trait (a name to bikeshed, an import to remember, a blanket impl
that constrains future ones) bought for a reordering of the same words. ADR-0059
quotes ADR-0032's test for an impl — that it "removes ceremony from a real call
site" — and the ceremony being removed here is the hand-written fold, which the
associated function already removes. A free function was rejected for
discoverability: `Money`'s own page is where the method belongs.

The item type is `Item = Money` rather than the more permissive
`I::Item: Borrow<Money>`. The `Borrow` bound would additionally accept a bare
`&[Money]`, which is tempting — but it costs a less legible signature, worse
inference, and in particular it breaks `Money::try_sum([])`, the *documented*
empty case, whose element type can then no longer be inferred. `Money` is `Copy`,
so `.iter().copied()` is the idiom callers already write (issue #107 writes it
itself).

Semantics:

- **The empty iterator sums to `Money::ZERO`** — `0 XXX`, the additive identity
  (ADR-0032), stated in the rustdoc and pinned by a test. There is no `Err` for
  "nothing to add": zero *is* the sum of no amounts, and `EmptyCashflows` exists
  for operations that genuinely need a flow (an IRR has no answer for an empty
  series; a total does).
- **It is exactly the left-to-right `try_add` fold from that identity**
  (`try_fold(Money::ZERO, Money::try_add)`), so the currency folds by ADR-0034's
  `Xxx` identity rule and the error variants are `try_add`'s, unchanged:
  `CurrencyMismatch { left, right }` with `left` the currency accumulated so far
  and `right` the offending amount's — the same reading as every other fold in
  the crate (ADR-0052) — and `Overflow` for a running total that leaves the
  finite range. Being the fold *by construction* rather than by a reimplementation
  is why no new error path exists to reason about.

### `try_min` / `try_max` — fallible, because the partiality is real

Two distinct non-`Xxx` currencies are unordered, so an infallible `min`/`max`
would have to invent an answer for `100 USD` against `100 EUR`. They return
`Result<Money, TvmError>` — matching `try_sum` and the rest of the arithmetic
rather than `Option`, because there *is* something to say about the failure and
`CurrencyMismatch { left, right }` says it. The `try_` prefix carries the
fallibility at the call site, as ADR-0023 established for `try_add`/`try_mul`.
`CurrencyMismatch` is the only error: the magnitude returned is one of the two
given, so there is no arithmetic to overflow.

**The currency is folded, not carried over from the selected side.** The smaller
of `0 XXX` and `100 USD` is `0 USD`, not `0 XXX`. Three reasons, in ascending
order of force: it is what ADR-0057's rule says (an operation producing a `Money`
folds its inputs' currencies); it is what `try_add` already does, so the agnostic
identity behaves identically across the arithmetic; and — the decisive one — it
is what makes the operation **commutative**. Without the fold, a tie between an
agnostic and a denominated amount would answer `0 XXX` or `0 USD` depending on
the argument order, and `min` that depends on argument order is not `min`.

`clamp` is not added. It is `try_max(floor).and_then(|m| m.try_min(cap))` at the
call site, and a three-way currency fold with two failure points earns its own
decision if anyone wants it.

### `abs` / `signum` — infallible, and `no_std`-clean

Negation is closed over finite `f64` and so is taking an absolute value, so by
ADR-0021's rule neither can fail. Two details are load-bearing:

- **`Money::abs` needs neither `std` nor `libm`.** `f64::abs` is a `std`-only
  intrinsic, and `crate::math` (which wraps the transcendentals) is itself gated
  behind `std`/`libm` — so routing through it would have put `abs` behind a
  feature while the rest of `Money`'s arithmetic stayed in the default build,
  which is precisely backwards. A sign flip suffices, so `abs` is available in the
  default `no_std`, zero-dependency build. It is written with `is_sign_negative()`
  (a `core` method) rather than the crate-internal `root::abs`, so that `-0.0`
  normalises to `0.0` exactly as `f64::abs` does: `Money::new(-0.0, c)` equals
  `Money::new(0.0, c)`, so leaving the sign bit on would let two *equal* amounts
  render differently (`-0` against `0`). `root::abs` is left alone — it is a
  tolerance helper where signed zero is immaterial.
- **`Money::signum` deliberately diverges from `f64::signum` at zero**, returning
  `0.0` where `f64::signum` returns `1.0` for `+0.0` and `-1.0` for `-0.0`. This
  is not a rounding of an inconvenient edge case, it is a consistency
  requirement: `-0.0 == 0.0`, so `Money::new(0.0, c) == Money::new(-0.0, c)`, and
  delegating would let two amounts that compare **equal** report **opposite**
  signs. Zero is also neither an inflow nor an outflow under the crate's
  signed-cashflow convention, so `0.0` is the honest third answer. It returns
  `f64` (not an `i8` or a three-valued enum) so that it composes with
  `try_mul`: `amount.abs().try_mul(amount.signum())` recovers the amount exactly.

`abs` preserves the currency (a magnitude is still denominated); `signum` does
not report one (a sign has no denomination) and does not consult one either, so
an amount's sign is the same in every currency.

### `Currency: FromStr` — `Err = TvmError`, and case-insensitive

**The error type is `TvmError`, with one new variant `UnknownCurrencyCode`.** A
dedicated `ParseCurrencyError` would be more precise in isolation and was
rejected on composition: every fallible operation in this crate returns
`Result<_, TvmError>`, so a dedicated type would be the one error that does not
thread with `?` through a caller's function until they wrote a `From` impl.
ADR-0004's one-enum error story is the whole reason `?` works everywhere here.

The variant carries **no payload**, following ADR-0052's cost rule: a payload
naming the offending string needs either a lifetime or an owned `String`, and the
core is `no_std` and `alloc`-free by default (ADR-0009). A caller reporting the
failure still holds the input it passed in — the CLI already interpolates it
(`unknown ISO 4217 currency code \`{code}\``). ADR-0052's asymmetry is the reason
this must be right now rather than later: a *variant* can be added
post-publication, a *payload* cannot.

**The parse is case-insensitive; `from_code` stays case-sensitive.** This is the
one genuinely contested call, and the divergence is deliberate and
one-directional:

- Every string `from_code` accepts, `FromStr` accepts, with the same result. The
  two never disagree about a *result* — only about which inputs they reject. So
  this is a widening, not a contradiction, and there is no case where a caller
  gets two different currencies from the same text.
- The two doors have different callers. `from_code` is the strict, canonical
  lookup, and it is what the `serde` wire format validates through (ADR-0042):
  a machine writing a wire format has no excuse for the wrong case. `FromStr` is
  where *human* input arrives — a CLI argument, an environment variable, a query
  parameter — and `"usd".parse()` failing there is a papercut on every
  human-facing surface.
- `FromStr` is also the round trip of `Display`, which prints `"USD"`; that round
  trip holds either way, so case-insensitivity only *adds* accepted inputs.

Leniency stops at case: no trimming, no numeric-code form (`"840"`), and no other
length — exactly three ASCII letters naming a known code. The implementation
uppercases the three bytes into a stack buffer and hands them to `from_code`, so
that generated table remains the single source of truth for which strings name a
currency, rather than a second scan of `Currency::ALL` that could drift from it.

`Currency` gets no `TryFrom<&str>` alongside this: `.parse()` is the idiomatic
spelling and a second door onto the same lookup is not an ergonomic gain.

### Testing (ADR-0045 rule 2)

Every clause above earns a test, and the finite domains are iterated rather than
sampled:

- **`FromStr` round-trips `code()` across all of `Currency::ALL`**, in three
  casings each (as ISO writes it, all-lower, title), and agrees with `from_code`
  wherever that one answers. `from_code`'s case-*sensitivity* is re-pinned in the
  same breath, so the divergence stays deliberate rather than becoming a drift.
- The rejected shapes are enumerated (empty, one/two/four letters, leading and
  trailing space, an interior space, an unknown code in both cases, a numeric
  code, three NUL bytes, and a three-*byte* non-ASCII character — the input shape
  the byte-wise uppercasing has to survive), and the variant's rendered `Display`
  is pinned, since a variant that exists to be reported must reach the message.
- **`abs`** preserves every currency in the closed set (iterated), is idempotent,
  is a fixed point of negation, and normalises `-0.0`; **`signum`** answers `0.0`
  for both zeros — with `(-0.0f64).signum() == -1.0` asserted alongside, so the
  test states the divergence it defends rather than assuming it.
- **`try_min`/`try_max`** are pinned on unordered currencies (both directions,
  payload included), on the currency fold, and on the agnostic tie whose answer
  commutativity depends on.
- **`try_sum`** is pinned on the empty case, the currency fold, the *first* clash
  in a series with two, `Overflow` in a running total whose mathematical sum is
  representable, and equality with a hand-written fold.
- Three **proptest properties** carry the universals: `try_sum` is the `try_add`
  fold (and is linear under negation), `abs`/`signum` decompose an amount exactly
  and `abs` is idempotent and non-negative, and `try_min`/`try_max` select one of
  their arguments, bracket both, account for the pair, and do not depend on
  argument order.

## Consequences

- The gestures issue #107 names all work: `flow.abs()`, `flow.signum()`,
  `Money::try_sum(flows.iter().copied())?`, `fee.try_min(cap)?`, and
  `"usd".parse::<Currency>()?`. None of them drops to bare `f64` or discards a
  currency on the way.
- **Purely additive.** No existing signature, behaviour, or rendering changes;
  `from_code` is untouched (only its rustdoc gains a pointer to `FromStr`), and
  the CLI and MCP surfaces are byte-identical — neither uses the new items, and
  the MCP server maps every `TvmError` through one `Display`-based mapper, so the
  new variant needs no code there.
- `TvmError` gains a variant. That is non-breaking on a `#[non_exhaustive]` enum
  (callers already carry a wildcard arm), and it keeps `Debug + Clone + PartialEq
  + Eq + Display + core::error::Error`.
- **`FromStr::Err = TvmError` is now permanent.** It cannot be narrowed to a
  dedicated type later without a breaking change. That is the accepted cost of
  `?`-composition, and it is the choice the rest of the crate's error surface
  already made.
- The `no_std` boundary is unchanged: all five items are in the default,
  zero-dependency build. `abs` in particular is *not* behind `std`/`libm`, and a
  future sign-adjacent addition should reach for a sign flip before reaching for
  `crate::math`.
- Follow-on obligation: ADR-0023's operator/method test now has a worked
  precedent for n-ary and comparison operations too — **fallibility follows the
  result, and a fallible operation is named `try_*`.** A future `clamp`, or a
  weighted total, follows this ADR rather than re-deciding it.
- The CLI's `parse_currency` still calls `from_code`, so the *binary* remains
  case-sensitive. Adopting `FromStr` there would widen what the CLI accepts,
  which is a behaviour change to a user-facing surface and therefore the owner's
  call, not a side effect of this PR.

## Alternatives considered

- **`impl Sum for Money`, panicking or saturating on failure.** The gesture
  issue #107 actually wrote, and impossible honestly: `Sum::sum` returns `Self`,
  so a currency mismatch could not be reported at all and an overflow could only
  panic or produce a non-finite `Money` — the invariant the type exists to hold.
- **`impl Sum for Money` returning `Result` via a wrapper item type**
  (`Sum<Result<Money, TvmError>>`, as `Result` itself implements). It would make
  `iter.map(Ok).sum::<Result<Money, _>>()` work, but the *fold* still has to be
  fallible, so the impl gains nothing over `try_sum` while adding a shape nobody
  guesses. Rejected as cleverness.
- **An extension trait for `try_sum`** (`flows.iter().copied().try_sum()?`). The
  best call site of the three shapes, and the reason it lost is above: a
  permanent public trait for a reordering of words the associated function
  already says.
- **`I::Item: Borrow<Money>` on `try_sum`.** Accepts a bare slice, at the cost of
  signature legibility, inference, and the documented `Money::try_sum([])` case.
- **Infallible `min`/`max` that fall back to `self` (or to the magnitude
  comparison) when the currencies clash.** Papers over the partiality ADR-0059
  states, and would silently return a `USD` answer to a question asked about
  `USD` and `EUR`.
- **`Option`-returning `min`/`max`.** Consistent with `partial_cmp`, and less
  informative than the `Result` the rest of the arithmetic returns: the caller
  loses which two currencies clashed, for no gain.
- **Giving `Money` `Ord` so that `Ord::min`/`max` just work.** Rejected by
  ADR-0059's own rule: the ordering is genuinely partial, and a total order would
  have to order `100 USD` against `100 EUR`.
- **`signum` delegating to `f64::signum`.** Faithful to the name and inconsistent
  with `PartialEq`: two equal amounts (`0.0` and `-0.0`) would report opposite
  signs.
- **`signum` returning a three-valued enum** (`Sign::{Negative, Zero,
  Positive}`). Arguably the ADR-0045 rule-1 answer, and rejected at that rule's
  own boundary: the `f64` composes with `try_mul` and with a caller's own
  arithmetic, an enum needs a conversion at every use, and no failure mode is
  being prevented — `signum`'s codomain is already only ever three values.
- **A dedicated `ParseCurrencyError` for `FromStr::Err`.** Precise, and it breaks
  `?`-composition with the crate's single error enum.
- **Reusing an existing `TvmError` variant** for an unknown code (there is no
  close fit — `CurrencyMismatch` is about two amounts, not about text). Rejected
  by ADR-0052's rule: a new distinguishable failure earns a variant that names
  it.
- **A case-*sensitive* `FromStr`, mirroring `from_code` exactly.** One rule
  instead of two, and it makes `"usd".parse()` fail on every human-facing
  surface. The divergence chosen instead is a widening that cannot produce a
  disagreement about a result, and it is documented on both doors.
- **Making `from_code` case-insensitive too**, so the two agree. That changes
  documented, tested behaviour of an existing function — and the wire format
  validates through it, where strictness is correct.
- **Trimming whitespace in `FromStr`.** The next step down the leniency slope,
  and one with no natural stopping point (`"$ USD"`? `"usd\n"`?). Case is a
  property of the same three letters; surrounding text is a different string.
