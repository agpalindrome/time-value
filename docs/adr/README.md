# Architecture Decision Records

This directory records the significant design decisions behind `time_value`, in
the lightweight [Nygard format](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).
The practice itself is [ADR-0001](0001-record-architecture-decisions.md).

## How to add one

1. Copy [`0000-adr-template.md`](0000-adr-template.md) to the next free number,
   `NNNN-kebab-title.md`.
2. Fill in Context → Decision → Consequences → Alternatives considered.
3. Commit it **with the change it describes**.
4. Add a row to the index below.

An Accepted ADR is immutable. To change a decision, write a new ADR that marks
the old one **Superseded** (link both ways) — don't rewrite the old one.

## Index

| # | Title | Status |
|---|-------|--------|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-workspace-layout.md) | Workspace layout & crate boundaries | Accepted (amended by 0018) |
| [0003](0003-synchronous-computation-model.md) | Synchronous computation model | Accepted |
| [0004](0004-error-handling.md) | Error handling | Accepted |
| [0005](0005-domain-modelling-and-strong-typing.md) | Domain modelling & strong typing | Accepted (amended by 0019; extended by 0050, 0059) |
| [0006](0006-license.md) | License | Accepted (amended by 0055) |
| [0007](0007-rust-edition-and-msrv.md) | Rust edition & MSRV | Accepted |
| [0008](0008-nix-flake-dev-environment.md) | Nix flake dev environment | Accepted |
| [0009](0009-no_std-and-optional-libm.md) | `no_std` core & optional `libm` | Accepted (amended by 0019) |
| [0010](0010-cli-surface.md) | CLI surface | Accepted (amended by 0028, 0029) |
| [0011](0011-mcp-server.md) | MCP server | Accepted (amended by 0028, 0029) |
| [0012](0012-ci-and-release-automation.md) | CI and release automation | Accepted |
| [0013](0013-core-api-values-and-discrete-operations.md) | Core API — values, cashflows & discrete operations | Accepted (amended by 0020, 0021, 0026) |
| [0014](0014-transcendental-single-sum-operations.md) | Transcendental operations behind `std`/`libm` — single-sum value | Accepted (amended by 0019, 0021, 0025) |
| [0015](0015-annuities.md) | Annuities — convention, the `r → 0` limit, and a fallible payment | Accepted (amended by 0021, 0025, 0062, 0063; extended 2026-07-10 — annuity-due & perpetuity) |
| [0016](0016-msrv-and-toolchain-bump.md) | Toolchain & MSRV bump to 1.88 for the MCP server | Accepted (amended by 0017) |
| [0017](0017-per-crate-msrv-core-1.85.md) | Per-crate MSRV — the core keeps 1.85, verified separately | Accepted |
| [0018](0018-kebab-case-binary-crate-names.md) | Kebab-case binary crate names | Accepted |
| [0019](0019-1.0-public-api-decisions.md) | 1.0 public API decisions | Accepted (§2 superseded by 0021; §1 serde drop reversed by 0042) |
| [0020](0020-robust-irr-newton-with-bisection-fallback.md) | Robust IRR — Newton with a bisection fallback | Accepted (amended by 0021, 0025, 0054) |
| [0021](0021-fallible-operations-on-non-finite-results.md) | Operations are fallible when their result can be non-finite | Accepted (amended by 0023, 0054) |
| [0022](0022-core-first-sequencing-before-the-first-release.md) | Core-first sequencing before the first release | Accepted |
| [0023](0023-money-arithmetic-surface.md) | The `Money` arithmetic surface — `Neg` and `try_*` | Accepted (amended by 0061) |
| [0024](0024-rate-conversions-effective-and-nominal.md) | Rate conversions — effective between periodicities, nominal as a quote | Accepted |
| [0025](0025-solve-for-periods-and-rate.md) | Solve for periods (NPER) and rate (RATE) | Accepted (amended by 0056, 0062, 0063) |
| [0026](0026-modified-internal-rate-of-return.md) | Modified internal rate of return (MIRR) | Accepted |
| [0027](0027-amortization-schedule.md) | Amortization schedule as a lazy iterator | Accepted (amended by 0051, 0054) |
| [0028](0028-binary-surface-conventions.md) | Binary surface conventions (CLI grammar & MCP tools) | Accepted (its §4/§5 output shape amended by 0039; extended by 0049, 0062) |
| [0029](0029-dated-cashflows-xnpv-xirr.md) | Dated cashflows — XNPV / XIRR | Accepted (amended by 0030) |
| [0030](0030-shared-day-count-support-crate.md) | Shared day-count support crate | Accepted |
| [0031](0031-split-non-finite-result-into-overflow-and-undefined.md) | Split `NonFiniteResult` into `Overflow` and `Undefined` | Accepted |
| [0032](0032-ergonomic-convenience-impls.md) | Ergonomic convenience impls (`ZERO` / `Default` / `TryFrom` / `From`) | Accepted (amended by 0061) |
| [0033](0033-core-domain-model-two-axes-and-an-f64-engine.md) | Core domain model — two axes, and an `f64` computation engine | Accepted |
| [0034](0034-money-and-currency.md) | Money and currency — `f64` magnitude, a runtime ISO-4217 enum, and FX | Accepted (amended by 0053, 0057, 0058) |
| [0035](0035-periodicity-tagged-time.md) | Periodicity-tagged time (`Period<P>`) | Accepted (amended by 0059) |
| [0036](0036-continuous-compounding-force-of-interest.md) | Continuous compounding — a periodicity-free force of interest | Accepted (amended by 0064 — the two solves) |
| [0037](0037-currency-in-the-binaries.md) | Currency in the binaries — an opt-in code that is echoed, not rounded | Accepted |
| [0038](0038-no-scheduled-release-continuous-development.md) | No scheduled release — continuous development | Accepted |
| [0039](0039-typed-output-layer-for-the-binaries.md) | A typed output layer for the binaries — "types in, types out" | Accepted (MCP `CurrencyCode` workaround retired by 0044) |
| [0040](0040-fx-convert-in-the-binaries.md) | FX convert in the binaries — a standalone `convert` surface | Accepted |
| [0041](0041-continuous-compounding-in-the-binaries.md) | Continuous compounding in the binaries — a `continuous` family | Accepted (extended by 0064 — the two solves) |
| [0042](0042-serde-support.md) | `serde` support — an optional, validating wire format | Accepted (amends 0019; amended by 0060) |
| [0043](0043-owned-cashflows.md) | Owned cashflows — `OwnedCashflows` behind an `alloc` feature | Accepted (amended by 0060) |
| [0044](0044-schemars-support.md) | `schemars` support — JsonSchema companion to the serde wire format | Accepted (extended by 0060) |
| [0045](0045-make-illegal-states-unrepresentable.md) | Make illegal states unrepresentable; test the class, not the instance | Accepted |
| [0046](0046-thread-safety-of-the-public-types.md) | The public value types are thread-safe (`Send + Sync`), locked by a test | Accepted |
| [0047](0047-shared-disciplines-across-the-sibling-rust-mcp-repos.md) | Shared disciplines across the sibling Rust MCP repos — a cross-repo index | Accepted |
| [0048](0048-finite-growing-annuity.md) | The finite growing annuity | Accepted (amended by 0063 — the inverses it deferred) |
| [0049](0049-growing-annuity-in-the-binaries.md) | The growing annuity in the binaries | Accepted (extended by 0062, 0063) |
| [0050](0050-role-newtypes-for-ambiguous-arguments.md) | Role newtypes for transposable arguments | Accepted (extends 0005) |
| [0051](0051-installment-private-fields.md) | `Installment`'s fields are private, read through accessors | Accepted (amends 0027) |
| [0052](0052-tvmerror-variant-granularity.md) | `TvmError` variant granularity — a payload on `CurrencyMismatch`, `Undefined` split | Accepted (amends 0004, 0031; extended by 0061) |
| [0053](0053-fxrate-domain-and-currency-ordering.md) | `FxRate`'s domain is closed under reciprocal, `from` → `source`, and `Currency`'s ordering policy | Accepted (amends 0034) |
| [0054](0054-numeric-robustness-of-the-core-operations.md) | Numeric robustness — schedule termination, rounding finiteness, cancellation-free annuity factors, and what counts as a root | Accepted (amends 0020, 0021, 0027) |
| [0055](0055-publish-readiness-of-the-packaged-crate.md) | Publish readiness — what ships in the tarball, and a README that cannot rot | Accepted (amends 0006) |
| [0056](0056-degenerate-rate-solves.md) | Degenerate rate solves report the degeneracy rather than the scan sentinel | Accepted (amends 0025, 0052; amended by 0063 — the table extended to the due and growing factors; extended by 0064 — `IndeterminateSpan`) |
| [0057](0057-currency-is-checked-where-a-result-is-denominated.md) | Currency is checked where a result is denominated — the rate solves do not fold it | Accepted (amends 0034) |
| [0058](0058-money-display-carries-its-currency.md) | `Money`'s `Display` carries its currency — bare for `XXX`, magnitude then ISO code otherwise | Accepted (amends 0034) |
| [0059](0059-the-finite-scalars-are-totally-ordered.md) | The finite-by-construction scalars are totally ordered (`Eq` + `Ord`), `Money` stays partial | Accepted (amends 0035, extends 0005) |
| [0060](0060-owned-cashflows-on-the-wire.md) | `OwnedCashflows` on the wire — a bare array of `Money`, no periodicity | Accepted (amends 0042, 0043; extends 0044) |
| [0061](0061-money-and-currency-ergonomics.md) | `Money`/`Currency` ergonomics — `try_sum` not `Sum`, fallible `min`/`max`, infallible sign, lenient `FromStr` | Accepted (amends 0023, 0032; extends 0052) |
| [0062](0062-annuity-sinking-fund-and-perpetuity-due.md) | The sinking-fund payment and the perpetuity-due | Accepted (amends 0015, 0025; extends 0028, 0049; extended by 0063) |
| [0063](0063-annuity-due-solves-and-growing-inverses.md) | The annuity-due solves and the growing-annuity inverses | Accepted (amends 0015, 0025, 0048, 0056; extends 0028, 0049) |
| [0064](0064-continuous-solves.md) | The continuous solves — force of interest and span (closed forms, a two-sided `ln1p`, and `IndeterminateSpan`) | Accepted (amends 0036, 0041; extends 0025, 0028, 0056) |
