# ADR-0034: Money and currency — `f64` magnitude, a runtime ISO-4217 enum, and FX

- **Status:** Accepted (amended by [0053](0053-fxrate-domain-and-currency-ordering.md),
  [0057](0057-currency-is-checked-where-a-result-is-denominated.md), and
  [0058](0058-money-display-carries-its-currency.md) — `Money`'s `Display` now
  renders the currency it carries; the model below stands)
- **Date:** 2026-07-14
- **Deciders:** Project owner
- **Supersedes:** [ADR-0005](0005-domain-modelling-and-strong-typing.md) §"`Money`
  is a plain untagged newtype; currency is not modelled"
- **Amends:** [ADR-0021](0021-fallible-operations-on-non-finite-results.md) (adds a
  `CurrencyMismatch` error), [ADR-0023](0023-money-arithmetic-surface.md) (arithmetic
  is now currency-checked)
- **Follows:** [ADR-0033](0033-core-domain-model-two-axes-and-an-f64-engine.md)
- **Closes conceptually:** issue #25 (currency tag) and folds in issue #60 (FX)

## Context

Per ADR-0033, currency is dynamic data and belongs on `Money` as a **runtime value**,
not a type tag. This ADR fixes the representation of a monetary amount, the currency
type, the arithmetic semantics, and cross-currency conversion (FX). A monetary
amount is a *magnitude* together with the *currency* it is denominated in.

## Decision

### `Money` is a magnitude plus a currency

```rust
pub struct Money { magnitude: f64, currency: Currency }   // Copy, no_std, ~16 bytes
```

- The magnitude is `f64` (ADR-0033); `Money` stays signed (an outflow is negative),
  `Copy`, and allocation-free.
- The primary constructor takes both: `Money::new(amount, currency) -> Result<Money>`
  (rejecting a non-finite `amount`, as today).
- `Money::value()` returns the magnitude; `Money::currency()` returns the currency.

### `Currency` is a closed, `#[non_exhaustive]` ISO-4217 enum

```rust
#[non_exhaustive]
pub enum Currency { Xxx, Usd, Eur, Jpy, /* … full ISO-4217 active set … */ Xau }
```

- It ships the **full ISO-4217 active set** — fiat, the precious-metal codes
  (`XAU`/`XAG`/`XPT`/`XPD`), `XDR`, and the reserved `XXX`/`XTS`. Metadata (ISO
  alphabetic code, numeric code, **minor-unit exponent**) is exposed by `const`
  methods backed by exhaustive `match` tables, so it is curated and correct.
- It is a plain `Copy` enum: trivially `Eq`/`Hash`/`Ord`, matches are exhaustive,
  parsing (`Currency::from_code("USD") -> Option<Currency>`) and serialization are
  total and canonical.
- **No user-defined currencies in 1.0.** In a `no_std`, runtime-currency, serde-bound
  world a custom-currency door delivers little: a runtime custom currency needs
  either a `&'static` (compile-time only — useless to the CLI/MCP, which resolve a
  currency *string*) or `alloc` (breaking the zero-dependency core), and it makes
  deserialization of an unknown code ill-defined. The enum is `#[non_exhaustive]`,
  so a `Custom(…)` variant can be **added later without a breaking change** if a
  concrete non-ISO need (crypto, points) arises. Until then, the curated set is the
  whole currency universe.

### `XXX` is the currency-agnostic sentinel and the currency identity

ISO-4217 already defines `XXX` = "no currency", which we adopt as the
**currency-agnostic** amount. It behaves as the identity element on the currency
axis:

- an operation between `XXX` and a currency `C` yields `C` (an agnostic amount
  adopts the denomination it is combined with);
- an operation between two *distinct* non-`XXX` currencies is a
  `TvmError::CurrencyMismatch`;
- `XXX` with `XXX` stays `XXX`.

So pure-number TVM (everything `XXX`) computes exactly as the untagged core does
today, denominated amounts stay checked, and `Money::ZERO` is `0 XXX` — a neutral
element that adds cleanly into any currency. There is no separate "no-currency"
concept to invent: `Currency::Xxx` is it.

### Arithmetic and rounding

- `try_add`/`try_sub` combine currencies by the identity rule above (`Overflow` on
  non-finite result, `CurrencyMismatch` on distinct currencies). `try_mul`/`try_div`
  by a scalar preserve the currency. Ordering is defined only within one currency.
- **Rounding to the currency's minor unit is a presentation step**, not part of
  computation: a `Money::round_to_currency()` / formatting helper uses the currency's
  minor-unit exponent (2 for `USD`, 0 for `JPY`, 3 for `BHD`). Computation never
  rounds intermediate values.

### FX is a first-class, caller-supplied conversion

```rust
pub struct FxRate { from: Currency, to: Currency, rate: f64 }   // a directional price
impl Money { pub fn convert(self, fx: FxRate) -> Result<Money, TvmError>; }
```

`convert` requires `self.currency == fx.from` (else `CurrencyMismatch`), multiplies
the magnitude by `rate`, and tags the result `fx.to`. Rates are **caller-supplied**
(the core stays data-free and `no_std`). Inversion is supported (a rate can be used
in either direction). **Triangulation** (via a base currency) and **bid/ask
spreads** are out of scope — they are rate-*sourcing* concerns for a caller, not core
arithmetic.

## Consequences

- Every `Money` construction site gains a currency argument; the currency-agnostic
  path uses `Currency::Xxx` explicitly (or `Money::ZERO`). This touches tests,
  doctests, the CLI, and the MCP.
- Because currency is a value, `Cashflows`, `Schedule`, `DatedCashflows`, and the
  operations **stay non-generic** — they hold `Money` and enforce one currency per
  computation at their boundaries. The type-parameter explosion of the abandoned
  type-tag approach does not occur.
- `TvmError` gains `CurrencyMismatch`; serde (issue #21) serializes `Money` as
  `{ amount, currency-code }` with total round-tripping; the binaries resolve a
  currency string against the closed set and reject unknown codes.
- The FX follow-up (#60) is subsumed here as core; if it later grows triangulation
  or spreads, those are additive.

## Alternatives considered

- **Compile-time currency type tag** — see ADR-0033 Alternatives; rejected.
- **Open `Currency` (trait or `&'static` registry, user-defined markers)** — pays
  for a narrow benefit (compile-time-defined custom currencies for library authors)
  with worse representation, non-total parsing, ill-defined deserialization of
  unknown codes, and no help at all to the runtime binaries. Deferred behind
  `#[non_exhaustive]` instead.
- **Decimal / fixed-point magnitude** — a false exactness for a transcendental
  engine (ADR-0033).
- **A bespoke `NoCurrency` sentinel** — unnecessary; ISO-4217 `XXX` is the
  standards-blessed "no currency" and doubles as the currency identity element.
