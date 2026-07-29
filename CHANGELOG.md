# Changelog

Notable changes to the crates in this workspace. `time_value`, the published
core library, follows [semantic versioning](https://semver.org/spec/v2.0.0.html).

**Nothing below has been released.** `1.0.0` is the in-tree version; no crate in
this workspace has been published from this codebase, and cutting a release is a
separate, deliberate decision (see
[ADR-0038](docs/adr/0038-no-scheduled-release-continuous-development.md)). The
CLI (`time-value-cli`) and MCP server (`time-value-mcp`) carry `publish = false`
and are not versioned here.

## `time_value` 1.0.0 — unreleased

**A complete rewrite. `0.x` code will not compile against it.**

`time_value` 1.0 shares its name, its author, its subject, and its
`MIT OR Apache-2.0` licence with the 0.x line — and nothing else. Not one type,
function, or module survives, so this is not an upgrade with a migration path:
callers of 0.x rewrite against the new API, or stay on 0.8.0. Because 1.0.0 is a
major version, nobody is carried across automatically and nothing breaks
silently; a 0.x dependency keeps resolving to 0.x.

The 0.x line is `0.1.0`–`0.8.0`, last published on 2021-02-05. Every version
before `0.8.0` is yanked. Those releases remain available and unchanged; this
codebase does not supersede them in place.

### What the crate is now

A `#![no_std]`, zero-dependency time-value-of-money library whose organising idea
is that the compiler should reject the mistakes the domain actually makes.

- **Periodicity is a compile-time tag.** `Rate<Monthly>` and `Rate<Annual>` are
  distinct types, and so are `Period<P>` and `Cashflows<P>`, so discounting
  monthly cashflows with an annual rate does not compile. Periodicity is the
  crate's only type-level tag.
- **Currency is a runtime value on `Money`,** not a type tag: an `f64` magnitude
  plus an ISO 4217 `Currency`, with `Xxx` as the agnostic identity and a
  mismatch reported as `TvmError::CurrencyMismatch`.
- **Role newtypes** — `Payment`, `PresentValue`, `FutureValue`, `Principal`,
  `Growth<P>` — make transposing two same-typed arguments a compile error.
- **Every fallible operation returns `Result`.** Constructors validate their
  domain; an operation that could produce a non-finite result reports it rather
  than returning one.
- **Operations:** NPV / NFV / IRR / MIRR, single-sum present and future value
  with the `periods` (NPER) and `rate` (RATE) solves, the annuity module
  (ordinary, due, perpetuity, growing, and the finite growing forms) with its
  own solves, amortization schedules as a lazy allocation-free iterator, dated
  cashflows (XNPV / XIRR), continuous compounding at a periodicity-free force of
  interest, FX conversion, and nominal/effective rate conversion.
- **Features, all off by default:** `std` and `libm` (either provides the
  transcendental math the non-elementary operations need), `alloc` (the owned
  `OwnedCashflows` series), `serde` (a validating wire format), and `schemars`
  (its JSON-Schema companion).
- **MSRV 1.85**, verified in CI on a real 1.85 toolchain.

The library is accompanied in this repository by a `time-value` CLI and a
`time-value-mcp` MCP server, both unpublished.
