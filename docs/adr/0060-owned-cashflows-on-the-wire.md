# ADR-0060: `OwnedCashflows` on the wire — a bare array of `Money`, no periodicity

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Follows:** [ADR-0042](0042-serde-support.md) (the validating `serde` wire
  format), [ADR-0044](0044-schemars-support.md) (the `JsonSchema` companion),
  [ADR-0045](0045-make-illegal-states-unrepresentable.md) (pin every stated
  assumption)
- **Amends:** [ADR-0042](0042-serde-support.md) and
  [ADR-0043](0043-owned-cashflows.md) (both of which explicitly deferred a
  serialized shape for the cashflow aggregates), extends
  [ADR-0044](0044-schemars-support.md) (same set, one more type)

## Context

ADR-0042 gave the owned **value** types a validating wire format and excluded the
**aggregates**: a borrowing type cannot `Deserialize`, and a lazy iterator has no
natural owned wire form. ADR-0043 then added `OwnedCashflows<P>` — a `Vec<Money>`
plus the periodicity tag — and repeated the deferral verbatim ("`OwnedCashflows`
*could* round-trip … but a serialized series shape is a separate, additive
decision").

That leaves the one series type that *can* round-trip without a wire format, so a
consumer deserializes a `Vec<Money>` and wraps it by hand — re-implementing, at
each boundary, a shape the crate should own. Issue #105 (from the adversarial
pre-publication API review) asks for it.

## Decision

**`OwnedCashflows<P>` gets `Serialize` / `Deserialize` (feature `serde` + `alloc`)
and `JsonSchema` (feature `schemars`, which already implies `alloc`). Its wire form
is a bare JSON array of `Money` in period order.**

```json
[{"amount": -100.0, "currency": "USD"}, {"amount": 60.0, "currency": "USD"}]
```

**The periodicity is not on the wire.** `P` is a zero-sized compile-time marker
with no runtime data, so there is nothing to write; this is the same choice
ADR-0019/ADR-0042 already made for `Rate<P>` and `Period<P>`, which serialize as
bare numbers. The consequence is real and accepted: **a serialized series does not
record its own periodicity, so deserializing one into the wrong `P` succeeds
silently.** The periodicity of a document is the *caller's* context — the field
type they deserialize into — not the document's content, and a crate whose
headline feature is making periodicity mismatches compile errors cannot check this
one at runtime without putting a tag on the wire that every existing consumer of
`Rate` would then be inconsistent with. `tests/serde.rs` pins the behaviour
(`the_periodicity_is_not_recorded_on_the_wire`) so it is a documented property
rather than an accident.

**Only `OwnedCashflows`.** `Cashflows<'a, P>` and `DatedCashflows<'a>` borrow, so
they have no storage to deserialize into (a serialize-only impl would be an
asymmetric API, rejected already by ADR-0042). `Schedule<P>` is a lazy iterator
whose meaningful wire form is the `Vec<Installment>` a consumer collects —
`Installment` is already covered — and serializing its internal position would
commit iterator state as wire API. `TvmError` stays excluded on ADR-0042's
reasoning: an error is presented, not round-tripped.

**One declaration, shared by both features.** The shape lives in `src/wire.rs` as
`OwnedCashflowsWire<'a>(Cow<'a, [Money]>)`, the module's only sequence type and its
only newtype — serde serializes a newtype struct as its field, and schemars derives
the field's schema, so both descriptions come from that one line and cannot drift
(ADR-0044's reason for the module). The `Cow` is what lets a single declaration
serve both directions: **borrowed** on the way out, so serializing a series copies
nothing, and **owned** on the way in.

**Validation is element-wise.** Every element is a `Money` rebuilt through
`Money::new`, so a non-finite amount or an unknown currency code anywhere in the
array fails the whole document. The series constructor `OwnedCashflows::new` is
total (it accepts any `Vec<Money>`, including a mixed-currency one — the
*operation* reports the mismatch, ADR-0057), so deserialization is exactly as
permissive as building the value in process: no wire input can produce a series
that could not have been constructed in Rust, and none is rejected that could.

**`JsonSchema` is inlined**, an `array` whose `items` are the `Money` definition —
matching `schemars`' own treatment of sequences, and for the same reason the
bare-number newtypes are inlined: with no tag on the wire, a `$ref` to a definition
named `OwnedCashflows` would imply a per-periodicity schema that does not exist.
The schema for `OwnedCashflows<Monthly>` and `OwnedCashflows<Annual>` is therefore
the same document, which the conformance test asserts.

**`alloc` now carries `serde?/alloc`.** The sequence shape is built from serde's
`Vec`/`Cow` impls, which live behind serde's own `alloc` feature; the *weak*
dependency feature turns it on when — and only when — both features are enabled, so
`serde` alone still pulls nothing extra. **No pre-existing CI configuration would
have caught its absence**, which is why a seventh clippy check was added
(`--no-default-features --features alloc,serde`, deliberately **without**
`--all-targets`): any build that includes the test targets pulls the `serde_json`
dev-dependency, which enables `serde/std` and masks a missing `serde/alloc` — as
`--all-features` does too. Lib-only is the dependency graph a downstream
`features = ["alloc", "serde"]` user actually gets.

**Float exactness belongs to the deserializer, not the format.** Pinning the
round-trip as a property (ADR-0045) surfaced that `serde_json`'s *default* float
parser is best-effort: it can return a value one ULP from the one that was written,
even though the written decimal (`ryu`'s shortest round-trip form) identifies the
`f64` uniquely. `serde_json`'s off-by-default `float_roundtrip` feature makes the
parse exact. This is a property of every `f64` the crate's wire format has ever
carried — `Money`, `Rate`, `FxRate` — not of the series, and it is not the core's
to fix, so it is recorded here and the properties assert recovery to within a few
ULP while the point tests, whose amounts are exactly representable, pin the shape
exactly.

## Consequences

- A consumer serializes and deserializes an owned series directly, with the crate
  owning the shape; the hand-rolled `Vec<Money>` wrap at each boundary goes away.
- The wire format is now settled for **every** public type that can round-trip; the
  remaining gaps (`Cashflows`, `DatedCashflows`, `Schedule`, `TvmError`) are
  deliberate and documented above, not pending work.
- A serialized series is periodicity-agnostic: it can be read as any `P`. Consumers
  that need the periodicity on the wire must carry it in their own envelope, which
  is where a document's context belongs.
- One more CI configuration to keep green (`no_std + alloc + serde`, lib only), and
  a note in `CLAUDE.md`'s verification list.
- `alloc` now has a (weak) effect on the `serde` dependency's own features — visible
  in `Cargo.toml`, and inert unless `serde` is enabled.
- The pinned behaviours: a round-trip property over arbitrary series
  (`tests/properties.rs`), and point tests for the exact array shape, the empty
  series, periodicity-agnosticism, element rejection, a mixed-currency document,
  and schema conformance against what `serde` actually writes.

## Alternatives considered

- **A struct with the periodicity on the wire** (`{ "periodicity": "monthly",
  "flows": [...] }`), rejecting a document whose tag is not `P::NAME` — the only
  alternative that addresses the real cost of this decision, and entirely feasible:
  `Periodicity` is sealed and every marker already carries a `NAME` the binaries use
  as their input vocabulary. Rejected on **consistency**: it contradicts the shape
  already fixed for `Rate<P>` and `Period<P>` (bare numbers, ADR-0042), so the
  format would check the periodicity of a series and not of the rate it is
  discounted at — a partial check, and partial in the direction of false confidence.
  If the crate ever does put the tag on the wire it should be for *every*
  periodicity-carrying type at once, which is a breaking change to a settled format
  and its own ADR; this one does not front-run it.
- **`#[serde(transparent)]` on a named-field wire struct** instead of a newtype —
  equivalent output, but it leans on both derives honouring the attribute, where a
  newtype's pass-through is the derives' plain, documented behaviour. Rejected as
  the more fragile way to say the same thing.
- **A `Vec<Money>` wire struct, cloning on serialize** — simpler than the `Cow`,
  but it allocates and copies the whole series every time one is serialized, for a
  wire form that is a borrow away. Rejected.
- **Hand-written `serialize_seq` / `SeqAccess` impls** — avoids needing serde's
  `alloc` feature at all, but re-states the sequence shape in code rather than
  sharing the one declaration the `serde`/`schemars` pair reads, which is exactly
  the drift `src/wire.rs` exists to prevent. Rejected; the weak feature is the
  smaller cost.
- **Cover `Cashflows` serialize-only, `Schedule` as a collected array, `TvmError`
  as a tagged enum** — each is a separable decision with no consumer asking, and
  the first two would publish an asymmetric or state-bearing shape. Rejected; see
  the reasoning above and in ADR-0042.
- **Enable `serde_json`'s `float_roundtrip` on the dev-dependency** so the
  round-trip property could assert bit equality — tidier-looking, but Cargo unifies
  features across the workspace, so a dev-only flag would change how the shipped
  MCP binary parses floats, and the property would then assert something a default
  consumer does not get. Rejected; assert what is true for everyone and document
  the caveat.
