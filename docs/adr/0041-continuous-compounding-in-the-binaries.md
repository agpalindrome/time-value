# ADR-0041: Continuous compounding in the binaries — a `continuous` family

- **Status:** Accepted
- **Date:** 2026-07-14
- **Deciders:** Project owner
- **Follows:** [ADR-0036](0036-continuous-compounding-force-of-interest.md)
  (continuous compounding in the core),
  [ADR-0028](0028-binary-surface-conventions.md) (binary surface conventions),
  [ADR-0037](0037-currency-in-the-binaries.md) (currency in the binaries),
  [ADR-0040](0040-fx-convert-in-the-binaries.md) (FX convert in the binaries)

> **Extended (2026-07-30) by [ADR-0064](0064-continuous-solves.md).** The family
> below covers the four operations that existed when it was written; ADR-0064 adds
> the two solves — CLI `continuous rate` / `continuous years`, MCP `continuous_rate` /
> `continuous_years` — under this ADR's own grammar. Two of its decisions decided
> those names: `--years` rather than `--periods` for a period-free span (so the span
> solve is `years`, giving a rare exact CLI↔MCP↔core name match), and `--rate` for
> the force of interest on the grounds that the family name says what kind of rate it
> is (so the force solve is `rate`, parallel to `single-sum rate`). The solves take
> **both** amounts, so they use no `--present`/`--future` anchor.

## Context

Continuous compounding landed in the **core** in ADR-0036: a periodicity-free
[`ContinuousRate`](../../crates/time_value/src/continuous.rs) (a force of
interest δ), the [`continuous`](../../crates/time_value/src/continuous.rs) module
(`future_value` / `present_value` over a real-number `years` span), and the
`ContinuousRate ⇄ Rate<Annual>` bridge (`from_effective_annual` /
`effective_annual`). It was kept **core-only**, matching the FX precedent
(ADR-0037/0040); this ADR fixes the binary surface (issue #68).

The forces are the same as ADR-0040's: the core API already exists, so this is
pure surface; and continuous `fv`/`pv` are the direct continuous analogue of the
discrete single-sum `fv`/`pv`, so the surface should read as a parallel of them.
Two things differ from the discrete case and shape the grammar: the time span is
a continuous `f64` `years`, **not** a `Period<P>` (ADR-0036), so it is a plain
numeric arg with no periodicity to name; and δ is a force of interest, not a
per-period `Rate` (no `> −100%` floor — any finite δ is valid).

## Decision

**Surface continuous compounding as a `continuous` family on both binaries**
(ADR-0028 §2/§5), covering all four core operations: the two monetary values and
both rate bridges.

- **CLI:** a `continuous` command group mirroring `single-sum` (periods → years):
  - `continuous fv --rate <δ> --years <Y> --present <PV>` → `FV = PV·e^(δ·Y)`
  - `continuous pv --rate <δ> --years <Y> --future <FV>` → `PV = FV·e^(−δ·Y)`
  - `continuous from-effective --rate <r>` → the force of interest `δ = ln(1+r)`
  - `continuous effective --rate <δ>` → the effective annual rate `e^δ − 1`
- **MCP:** family-prefixed tools (ADR-0028 §5): `continuous_future_value`,
  `continuous_present_value` (each `{ rate, years, amount, currency? }`), and the
  bridges `continuous_from_effective`, `continuous_effective` (each `{ rate }`).

**`--rate` is the force of interest δ** (an effective annual rate for
`from-effective`, whose whole job is to take one). Reusing `--rate` keeps the flag
vocabulary uniform across families (as the `rate` family already does); the
command name disambiguates what kind of rate it is.

**`years` is a plain numeric span, not a periodicity.** It may be fractional or
negative (ADR-0036). No `Period<P>` and no `--periodicity` — continuous
compounding has no discrete period.

**`fv`/`pv` honour the global `--currency`; the bridges do not.** The value
operations are monetary — they denominate the amount and echo the code through the
existing `MoneyResult` presentation (ADR-0037), exactly like single-sum `fv`/`pv`.
The two bridges are rate → rate, so they carry no currency, like the `rate`
family.

**Validation and errors are the core's.** `ContinuousRate::new` rejects a
non-finite δ (`NonFiniteRate`); `continuous::{future_value,present_value}` reject a
non-finite `years` (`NonFiniteOffset`) or an overflowing growth factor
(`Overflow`); `effective` can `Overflow` / hit the `−1` floor
(`RateOutOfRange`). Each surfaces as the usual CLI `error:` line / MCP
`invalid_params`.

## Consequences

- With ADR-0040 (FX) this closes the surfacing backlog: every core operation is
  now reachable from both binaries (ADR-0028 §1).
- Purely additive: no core change, and no existing command, tool, output shape, or
  default is altered.
- The continuous values conform to the ADR-0039 typed-output contract —
  `Json<MoneyResult>` / `Json<ScalarResult>` with auto-declared `outputSchema`,
  covered by the output-schema conformance test.
- The `continuous` group takes `--years`, so it is the second family after `rate`
  whose grammar diverges from the shared `--periods` — a deliberate consequence of
  continuous time having no period.

## Alternatives considered

- **Positional `<present>`/`<future>` amounts** (as the issue sketched) — but
  continuous `fv`/`pv` are the continuous single-sum, and `single-sum fv/pv` use
  named `--present`/`--future`; named flags keep the parallel exact. Rejected the
  positional form for consistency.
- **Drop the rate bridge (values only)** — smaller, but leaves a core operation
  unreachable and breaks the "surface every non-trivial op" rule (ADR-0028 §1);
  the bridge is what lets a continuous rate be compared with the discrete
  effective-rate machinery. Rejected — surface all four.
- **A dedicated `--force`/`--delta` flag instead of `--rate`** — more literal, but
  fragments the flag vocabulary for no real gain; `--rate` + the command name is
  clear. Rejected.
- **Model `years` as a `Period<Annual>`** — would drag a periodicity tag onto an
  intrinsically period-free quantity, the very thing ADR-0036 rejected in the
  core. Rejected.
