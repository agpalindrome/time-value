# ADR-0055: Publish readiness — what ships in the tarball, and a README that cannot rot

- **Status:** Accepted
- **Date:** 2026-07-29
- **Deciders:** Project owner
- **Amends:** [ADR-0006](0006-license.md) (the licence texts must travel *inside*
  each published crate, not only at the workspace root)
- **Follows:** [ADR-0045](0045-make-illegal-states-unrepresentable.md) (pin every
  stated assumption with a test that fails when it stops holding),
  [ADR-0038](0038-no-scheduled-release-continuous-development.md) (there is no
  scheduled release; this ADR does not change that),
  [ADR-0009](0009-no_std-and-optional-libm.md) (most of the API is behind
  `std`/`libm`, which is what makes the default-build documentation awkward)

## Context

An adversarial pre-publication review looked past the library at the thing that
would actually be *published* — the `.crate` tarball, the crates.io front page,
and the docs.rs render — and found that none of the three was correct. The
library has been reviewed repeatedly; its packaging never had been.

**The crates.io front page did not compile.** `crates/time_value/README.md` is
the `readme =` target, so it is the first thing a visitor sees. Its example still
called `Money::new(-100.0)?` — a one-argument constructor that stopped existing
when [ADR-0034](0034-money-and-currency.md) made currency a runtime value on
`Money` — and read `.value()` off a `Result`. Three `E0061`s and an `E0599` in the
crate's shop window. The workspace `README.md` had been updated when ADR-0034
landed and the crate's had not, which is the whole diagnosis: two copies of the
same example, one of them never compiled by anything. (The workspace copy was
itself a version behind — `net_present_value` became fallible under
[ADR-0021](0021-fallible-operations-on-non-finite-results.md) and its `?` was
missing.) There was **no `include_str!` anywhere in the workspace**, so no
markdown in the repository was under test.

**Neither licence file shipped.** `cargo package -p time_value` produced 27 files
and neither `LICENSE-MIT` nor `LICENSE-APACHE` was among them: they live at the
workspace root and `cargo package` only walks the crate directory. crates.io
accepts the manifest's `license = "MIT OR Apache-2.0"` on its own, so nothing
complained — but anyone consuming the `.crate` directly (`cargo vendor`, a distro
packager, an internal mirror) would get a dual-licensed crate containing neither
licence text. MIT requires its notice to accompany copies; Apache-2.0 §4 requires
the licence to travel with distributions. ADR-0006 said "the repository carries
`LICENSE-MIT` and `LICENSE-APACHE` at its root", which was true and insufficient.
Compounding it, the crate README linked them as `../../LICENSE-APACHE` — a path
that escapes the package root and resolves nowhere on crates.io.

**docs.rs would render the full API with nothing marking what is gated.**
`all-features = true` was already set, so every item appears — but there was no
`rustdoc-args = ["--cfg", "docsrs"]`, no `#![cfg_attr(docsrs, feature(doc_cfg))]`,
and not one `doc(cfg(...))` attribute, so the generated documentation contained
**zero** "Available on" badges across 53 `#[cfg(feature = ...)]` sites. Several of
those gate whole impl blocks — `Rate::convert` / `Rate::effective_annual`,
`Schedule::for_term`, `Cashflows::modified_internal_rate_of_return`,
`Money::round_to_currency` — so a reader saw gated and ungated methods
interleaved with nothing to tell them apart, wrote `rate.convert::<Annual>()`
against the advertised default build, and got "no method named `convert`".

**And the default-features documentation was noisy.** `cargo doc -p time_value
--no-deps` emitted **38** `rustdoc::broken_intra_doc_links` warnings: the prose
links to `annuity`, `single_sum`, `continuous`, `Period`, `DatedCashflows`,
`OwnedCashflows`, `Rate::convert`, `Schedule::for_term`,
`Money::round_to_currency` and others that exist only behind a feature. docs.rs
is unaffected because it builds `--all-features`, which is exactly why this went
unnoticed — but the warnings are part of the published artefact, emitted into
every downstream `cargo doc` run and attributed to this crate.

Finally, two smaller things. `homepage` pointed at
`https://crates.io/crates/time_value`, so crates.io rendered a "Homepage" link to
the page the reader was already on; and there was no changelog anywhere, for a
crate name whose published history (`0.1.0`–`0.8.0`, last published 2021-02-05,
everything before `0.8.0` since yanked) belongs to a completely different
codebase.

## Decision

Make publishing **possible and correct**. Do not publish, and do not arm the
release machinery — see *What this ADR does not do* below.

### 1. The README is compiled as a doctest

`crates/time_value/src/lib.rs` gains

```rust
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
```

so every fenced `rust` block in the README is built and run by `cargo test
--doc`. This is [ADR-0045](0045-make-illegal-states-unrepresentable.md)'s rule
applied to documentation: the README *asserts* that this code works, so that
assertion earns a test that fails the moment it stops being true. A stale front
page is now a red build, not a discovery made by a reader.

The carrier is `#[cfg(doctest)]` rather than the more common
`#![doc = include_str!("../README.md")]` on the crate root. The README and the
crate documentation are **different documents for different readers** — the
README is a front page that pitches the design and lists features; the crate
docs are an API reference with a model, an operations survey, and a
thread-safety contract. Splicing one into the other would either bloat the
crates.io page with reference material or thin out docs.rs, and including it
under a heading would duplicate the module documentation wholesale. Under
`cfg(doctest)` the item exists only while rustdoc collects doctests: the README
is *tested* without being *rendered*, and neither document has to become the
other.

The README's example is rewritten to compile — `Money::agnostic` for the
currency-agnostic constructor, `?` on the now-fallible `net_present_value` — and
uses an explicit `fn main() -> Result<(), TvmError>` rather than a hidden `#`
line, because hidden doctest lines render as literal text on crates.io. A second
block demonstrates `Money::new(amount, Currency::Usd)` and the
`CurrencyMismatch`, since currency-as-a-runtime-value is a headline design
decision (ADR-0034) that the front page did not show. The stale "an always-finite
monetary amount" description and the "What it computes" table's omission of the
whole [`continuous`](0036-continuous-compounding-force-of-interest.md) module are
fixed in the same pass.

**Residual gap, stated deliberately:** the *workspace* `README.md` is not
doctested. `include_str!` from `crates/time_value/src/lib.rs` may not reach
outside the package directory — a file it pulls in must ship in the tarball, and
`../../README.md` does not — so testing it would require a carrier in a crate
that is never packaged. Its snippet is fixed here and kept deliberately close to
the crate README's, but it remains hand-maintained.

### 2. Both licence texts ship inside the crate

`crates/time_value/LICENSE-MIT` and `crates/time_value/LICENSE-APACHE` are
**symlinks to the workspace-root files**. `cargo package` follows them and writes
the real content into the tarball (verified: the extracted `.crate` contains
1,072 and 11,357 bytes of licence text, not link stubs), so there is one source
of truth in the repository and a complete, self-contained artefact on crates.io.
The tarball goes from 27 files to 29.

The crate README's licence links become plain `LICENSE-APACHE` / `LICENSE-MIT`,
which now resolve both inside the tarball and on crates.io.

This is a standing obligation: **any crate in this workspace that becomes
publishable gets the same two symlinks.**

### 3. docs.rs gets feature badges

`[package.metadata.docs.rs]` gains `rustdoc-args = ["--cfg", "docsrs"]`, lib.rs
gains `#![cfg_attr(docsrs, feature(doc_cfg))]`, and every feature gate on a
public item — modules, re-exports, impl blocks, and the individually-gated
`Money::round_to_currency` — gains a matching
`#[cfg_attr(docsrs, doc(cfg(...)))]`. Badges inherit, so gating a module or an
impl block badges everything inside it; the render goes from 0 "Available on"
occurrences to 69, covering every gated method that previously looked
unconditional.

`doc_cfg` is a nightly rustdoc feature, which is the point of routing it through
the `docsrs` cfg: docs.rs builds on nightly and sets it, and a stable `cargo
doc`, `cargo build`, or the 1.85 MSRV build never sees the attribute at all.

### 4. Feature-gated doc links resolve in every build

Where the default `no_std` build has no local target for a link, a **markdown
link reference definition** supplies the item's docs.rs URL:

```rust
#![cfg_attr(
    not(any(feature = "std", feature = "libm")),
    doc = "
[`annuity`]: https://docs.rs/time_value/latest/time_value/annuity/index.html
…
"
)]
```

A reference definition takes precedence over intra-doc resolution, so the prose
stays *linked in both builds*: to the local item where it exists, and out to the
published documentation where it does not. That is strictly better than dropping
the link to backticks, which would have cost docs.rs — the one render where the
whole API is present — its navigation. The all-features build still resolves the
same paths as intra-doc links, so a rename is still caught by CI; only the URL
fragment, mechanically derived from the path, could drift.

The definitions shared by the `TvmError` variants live in one `docs_rs_links!`
macro (each variant's documentation is its own markdown document, so they must
be attached to each one; unused definitions are inert). Two mechanical details
earned their keep: the definitions must begin with a **blank line**, because a
raw `#[doc]` fragment is joined to the preceding line without one and the
definitions are otherwise absorbed into the last paragraph as text; and inline
links (`[x](path)`) must be rewritten to reference form (`[x][path]`) for a
definition to override them.

Default-features `cargo doc` goes from 38 warnings to **0**, and all six feature
configurations are clean.

### 5. CI checks the documentation

Two steps are added to `ci.yml`, both with `RUSTDOCFLAGS: -D warnings`: `cargo
doc -p time_value --no-deps` (the advertised default, where most of the API is
gated away) and the same with `--all-features` (what docs.rs builds). Rustdoc
warnings are part of the published artefact, so they are now a failing check
rather than something noticed at publication time.

### 6. `homepage` points at the repository

`homepage` becomes `https://github.com/ojhermann-org/time-value`. The crate
README's self-link, which meant the *old* line, points at
`https://crates.io/crates/time_value/0.8.0`.

### 7. A hand-written changelog records the discontinuity

`CHANGELOG.md` at the workspace root, written by hand, states plainly that 1.0 is
a complete rewrite, that 0.x code will not compile against it, and what the crate
now is. A generated changelog would be the wrong artefact here: `release-plz`
builds one from Conventional Commits, and across a from-scratch rebuild that is a
wall of `feat:` lines describing the construction of a library — not the one fact
a reader of this crate's name needs, which is that it is a different library.

It does not name a release date and does not imply a release has happened.

**It does not collide with `release-plz`,** which writes a *per-package*
changelog (`crates/time_value/CHANGELOG.md`); the workspace-root file is outside
its reach and `release-plz.toml` is untouched. The corollary is that the root
`CHANGELOG.md` does **not** ship in the `.crate` tarball, which is acceptable
because the discontinuity is stated on the crates.io front page itself — the
README's opening paragraph — where the reader who needs it will actually be.

## What this ADR does not do

**It does not publish anything, and it does not arm the release machinery.** No
version is bumped, no `publish = false` is flipped, and no tag is created;
`release-plz.yml`, `publish.yml`, and `release-plz.toml` are not modified at all,
so the two `if: ${{ false }}` job guards in `release-plz.yml` still hold. Those
guards are load-bearing: removing them would have `release-plz` tag on the very
next push to `main`, and `publish.yml` — which is triggered by a version tag —
fires on that tag. Touching them *is* publishing, with no second gate.
`cargo package` — a full local build and verification of the tarball, with no
network side effect — is the safe equivalent and is what was used here.

Cutting a release remains solely the owner's decision, whenever they choose it
(ADR-0038). This ADR only makes that decision *safe to act on*.

## Consequences

- **The crates.io front page is under test.** Every fenced `rust` block in
  `crates/time_value/README.md` must compile and pass, in CI, on every change.
  This is a real constraint on how the README is written — illustrative
  pseudo-code needs a ```text or ```ignore fence — and that is the trade.
- **The published tarball is self-contained** with respect to licensing: both
  arms of `MIT OR Apache-2.0` travel with it, and the README's links to them
  resolve. Any future publishable crate in this workspace inherits the
  obligation to carry the same two symlinks.
- **docs.rs distinguishes the default API from the gated API.** A reader can see
  at a glance that `Rate::convert` needs `std` or `libm`, which is the single
  most confusing thing about a crate whose advertised default build is a strict
  subset of its documented surface (ADR-0009).
- **Documentation warnings are a build failure.** Two new CI steps; a broken
  intra-doc link now fails the check that produces `ci`, in both the default and
  the all-features configuration.
- **The docs.rs URLs in the gated link definitions are hand-written.** The
  all-features build validates the *paths* as intra-doc links, so a rename is
  caught; the URL fragments are derived from those paths and are the one part
  that could silently drift. They point at `/latest/`, which is correct once a
  1.0 is published and dangling until then.
- **The workspace README's snippet is still hand-maintained** (see above), and is
  the one piece of markdown in the repository that no test covers.

## Alternatives considered

- **Copy the licence files into `crates/time_value/` instead of symlinking.**
  Two more copies to keep in step, for no benefit — `cargo package` was verified
  to follow the symlinks and write real content into the tarball. Copies remain
  the fallback if a future toolchain stops following them, and the packaging
  check would catch that.
- **Rely on `license = "MIT OR Apache-2.0"` alone.** crates.io is satisfied by
  the SPDX expression, but the obligation is to the *recipient of the copy*, and
  a vendored `.crate` with no licence text does not discharge it.
- **`#![doc = include_str!("../README.md")]` as the crate documentation.** The
  common idiom, and wrong here: the README and the crate docs are different
  documents for different readers, and merging them degrades one or the other.
  `cfg(doctest)` gets the testing without the merge.
- **Keep the example untested and just fix it.** That is precisely what was done
  when ADR-0034 landed, for the workspace README, while the crate README was
  missed. The defect is structural — untested markdown — so the fix has to be.
- **Drop the feature-gated intra-doc links to plain backticks.** The simplest way
  to silence the 38 warnings, and it would cost docs.rs its navigation for the
  half of the API that is gated. The reference-definition override keeps both.
- **`#![allow(rustdoc::broken_intra_doc_links)]` in the default build.** A
  one-line silencer that also hides links that are *genuinely* broken, in the
  configuration where they are hardest to notice.
- **Also `--cfg docsrs` in CI.** It needs a nightly toolchain, and the flake pins
  a stable one (`rust-toolchain.toml`). The badges were verified locally with
  `RUSTC_BOOTSTRAP=1`; adding a nightly toolchain to the flake to check a
  presentation detail is not proportionate.
- **A `CHANGELOG.md` generated by `release-plz` from the commit history.** It
  would describe the *construction* of a library, commit by commit, to an
  audience whose only question is why version `1.0.0` shares a name with a crate
  last touched in 2021.
- **Put the changelog in `crates/time_value/` so it ships.** It would then be in
  `release-plz`'s per-package path and liable to be rewritten by the first
  generated release. The discontinuity is stated on the front page instead,
  which reaches the crates.io reader more directly than a changelog file would.
- **Drop `homepage` entirely.** Valid — crates.io falls back to `repository` —
  but a filled-in field that points somewhere useful is better than an absent
  one.
