# ADR-0058: `Money`'s `Display` carries its currency

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0034](0034-money-and-currency.md) (currency is a runtime value on
  `Money` — and now reaches its rendering)
- **Follows:** [ADR-0033](0033-core-domain-model-two-axes-and-an-f64-engine.md)
  (rounding is an explicit presentation step — upheld here, not amended),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (pin every stated
  assumption), [ADR-0039](0039-typed-output-layer-for-the-binaries.md) (the
  binaries present money through their own DTOs)
- **Closes:** issue #102

## Context

ADR-0034 made currency a runtime value carried on every `Money`. `Money`'s
`Display` did not follow: it forwarded the formatter straight to the magnitude, so

```rust
format!("{}", Money::new(100.0, Currency::Usd)?)   // "100"
```

rendered the amount and dropped the denomination — the one thing ADR-0034 had just
added, and the one thing a reader of the output cannot recover.

That made `Money` the only formattable type in the crate to drop its qualifier.
Every sibling includes it, value first: `Rate` renders `0.01 monthly`, `Period`
renders `12 monthly`, `ContinuousRate` renders `0.05 continuous`, and `Currency`
itself renders `USD`. A `Money` rendering as `100` is not a terser member of that
family; it is a different contract.

Two things made this worth settling **now**, ahead of first publication, even
though it is the sort of change the pre-freeze sweep of ADR-0050 through ADR-0056
would not have caught:

- **It is not a compile break, which is exactly the problem.** Nothing about
  changing a `Display` impl trips `cargo`, and no downstream build fails. What
  breaks is the *output*: rendered money gets logged, screenshotted, pasted into
  issues, diffed in golden-file tests and — inevitably — parsed. Changing it after
  publication is a de-facto behaviour break with no compiler to announce it, so it
  is cheapest to decide before there is a published rendering to protect.
- **It was never a decision.** The old impl's own doc comment described the bare
  magnitude as leaving currency-aware formatting "to the caller", which reads as
  deliberate but predates ADR-0034's currency-on-`Money` model. It was true by
  construction, which is the state ADR-0045 exists to end.

## Decision

**`Money` renders its magnitude, then — unless the amount is currency-agnostic — a
space and the ISO 4217 code.**

```
Money::agnostic(100.0)?            ->  "100"           // unchanged
Money::ZERO                        ->  "0"             // unchanged
Money::new(100.0, Currency::Usd)?  ->  "100 USD"
Money::new(1234.5, Currency::Jpy)? ->  "1234.5 JPY"
```

Three sub-decisions carry the weight.

### Value first, qualifier second

The order matches the three siblings (`0.01 monthly`, `12 monthly`,
`0.05 continuous`) rather than the typographic convention of a leading symbol
(`$100`). Consistency within the crate is the more useful property here: a caller
formatting a `Rate` and a `Money` in the same line gets one shape, and the
magnitude stays at a predictable place — the start — for a reader scanning a
column.

It also falls out of the implementation constraint below: the magnitude has to go
through the formatter first, so it has to come first.

Currency *symbols* (`$`, `¥`), thousands separators, and locale-aware placement
are deliberately not in scope. They are locale data, the core is `no_std` and
dependency-free (ADR-0009), and a symbol is ambiguous in a way an ISO code is not —
`$` names a dozen currencies.

### `Currency::Xxx` stays bare

The currency-agnostic path is the default and by far the most-travelled: `Xxx` is
what `Money::agnostic`, `Money::ZERO`, the doctests, the property tests, and the
CLI's default `--currency XXX` all produce. Appending `XXX` there would add noise
to the common case and say nothing — `XXX` *is* "no currency", so printing it
announces an absence.

Keeping it byte-identical also means the change reaches nothing that was already
correct: the CLI's plain-number output, `ScalarOutput`/`MoneyResult` (ADR-0037,
ADR-0039), and the existing assertions across the workspace are untouched. That
was verified rather than assumed — the CLI's rendered output and the MCP server's
`tools/list` schemas and tool results are byte-identical across this change, which
is what one would expect given both present money through their own DTO layer and
never through `Money`'s `Display`.

This is the same reasoning ADR-0034 used to make `Xxx` the identity element on the
currency axis, applied to formatting: the identity element renders as nothing.

### No minor-unit rounding

`Display` does **not** round to the currency's minor unit, even though
`round_to_currency` exists and formatting is the obvious place someone might expect
it to be applied.

ADR-0033 and ADR-0034 make rounding an explicit, opt-in *presentation* step, and
computation never rounds intermediates. Rounding inside `Display` would break that
in the one direction that cannot be undone: it discards digits the caller never
asked to lose, and the rendering is the only thing the reader has, so the full
magnitude is simply gone. `2.348 USD` rendering as `2.35 USD` is a lie about the
value `Money` holds.

The reverse arrangement costs nothing: a caller who wants the rounding asks for it
(`money.round_to_currency()`), and gets a `Money` they can then render. Opt-in
loses no information; opt-out does.

### Format specifiers keep forwarding to the `f64`

The old impl forwarded the whole formatter to the magnitude, so `{:.2}`, `{:+}`,
`{:>10}` and `{:012}` all worked. That is preserved, deliberately and by
construction:

```rust
self.magnitude.fmt(f)?;                    // specifier applies here
if self.currency != Currency::Xxx {
    f.write_str(" ")?;
    f.write_str(self.currency.code())?;    // then the code is appended
}
```

The tempting alternative — build the full rendering into a `String` and write
*that* — is wrong twice. It would hand the specifier the whole string, so `{:.2}`
would truncate `"1234.5678 USD"` to two *characters* rather than two decimal
places, and a width would silently pad the wrong thing. It would also need `alloc`,
which the default core does not have.

**The wart, stated rather than left to be discovered:** because the specifier
reaches the number and the code is appended afterwards, **padding sizes the
magnitude alone**. `format!("{money:>12.1}")` yields `"      1234.6 USD"` — twelve
characters of number, then four more. A caller laying out a column wants
`format!("{:>12}", money.to_string())` instead. This is documented on the impl,
because a user who meets it in a misaligned table should be able to read why rather
than reverse-engineer it.

Only `Display` forwards. `Money` implements neither `LowerExp` nor `UpperExp`, so
`{:e}` does not compile against it; `Money::value()` is the route to the `f64`'s
other formatting traits. (An earlier draft of this ADR claimed `{:e}` forwarded
too. It does not — the compiler said so, which is the argument for ADR-0045 rule 2
in miniature.)

## Consequences

- Rendered money is self-describing. A log line, a panic message, or an
  `assert_eq!` failure now names the denomination, which is the case where the
  information mattered most and was previously lost.
- The currency-agnostic rendering is unchanged, so the binaries, their DTOs, and
  the existing test corpus are unaffected. This was verified against both surfaces
  before and after, not reasoned about.
- Every clause above earns a test (ADR-0045 rule 2): the `Xxx` case is pinned bare
  (including `ZERO` and a negative magnitude); a denominated amount is pinned to
  value-then-code; `Currency::ALL` is iterated exhaustively — a small closed set, so
  iterate rather than sample — asserting the code appears exactly once and only for
  a non-`Xxx` currency; the absence of rounding is pinned on `USD` (two minor
  digits) and `JPY` (none); and the specifier cases `{:.2}`, `{:.0}`, `{:+.1}`,
  `{:>12.1}`, `{:<12.1}`, `{:^12.1}` and `{:012.1}` are each pinned, since they are
  precisely what a `String`-building implementation would silently break.
- The rendering is now a *decided* contract rather than an accident, so it is a
  thing a future change has to argue with.
- `Display` and the binaries' DTOs remain separate presentations of the same value,
  by design (ADR-0039). This ADR does not pull the CLI toward `Display`; the CLI's
  `--json` shape and TSV table are its own contract.

## Alternatives considered

- **Leave it bare.** The status quo, and it has one real argument: the magnitude
  alone composes into a caller's own formatting without anything to strip.
  Rejected — the caller who wants the bare number has `value()`, which is explicit,
  while the caller who wants the whole amount had no way to get it from `Display`
  at all. The default should carry the information, not drop it.
- **Always append the code, `XXX` included.** More uniform, and arguably more
  honest: the rendering would then be total, with no case where the currency is
  implicit. Rejected on cost/benefit — it would change the most common rendering in
  the crate to add a token that means "no currency", and it would ripple into the
  CLI's plain output and a large body of assertions for no information gained.
- **Currency symbol and locale-aware placement (`$100.00`).** What a
  presentation-layer formatter should do, and precisely why it does not belong in a
  `no_std`, dependency-free core: it needs locale data the crate will not carry, and
  symbols are ambiguous where ISO codes are not. A caller wanting `$` has
  `value()` and `currency()`.
- **Round to the minor unit in `Display`.** Discussed above and rejected: it makes
  the rendering lossy in a way the caller cannot opt out of, and contradicts
  ADR-0033's rule that rounding is explicit.
- **Build the rendering into a `String` and write it through the formatter**, so a
  width or precision applies to the whole thing and columns align naturally.
  Genuinely the nicer behaviour for table layout, and rejected twice over: it needs
  `alloc`, which the default `no_std` core does not have, and it silently redefines
  `{:.2}` from "two decimal places" to "two characters" — a change that compiles,
  runs, and produces garbage. The padding wart is the price of keeping every
  numeric specifier meaning what it says, and it is documented rather than hidden.
- **A separate `Money::format_with_currency()` and keep `Display` bare.** Avoids
  changing anything, and gives the explicit-is-better crowd their method. Rejected:
  it leaves the default rendering wrong, and `Display` is what `{}`, `to_string()`,
  panic messages and `assert_eq!` failures all reach for. Making the good behaviour
  opt-in means it is absent exactly where it is most needed — in output nobody wrote
  a format call for.
