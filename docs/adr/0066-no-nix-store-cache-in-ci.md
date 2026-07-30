# ADR-0066: No Nix store cache in CI

- **Status:** Accepted
- **Date:** 2026-07-30
- **Deciders:** Project owner
- **Amends:** [ADR-0012](0012-ci-and-release-automation.md) (CI and release
  automation), [ADR-0055](0055-publish-readiness-of-the-packaged-crate.md) (which
  added the two `cargo doc` steps)

## Context

CI ran `DeterminateSystems/magic-nix-cache-action@v13` to save and restore the Nix
store between runs. Issue #93 observed that its teardown step took longer than the
verification it was meant to accelerate, and asked whether it earned its keep.

It did not. Measured across **45 completed `ci` runs** (the #99–#118 arc), reading
step timestamps from the Actions API:

| event | n | median total | median `Post Nix store cache` | mean |
| --- | --- | --- | --- | --- |
| `pull_request` | 15 | 223 s | 57 s | 79.5 s |
| `merge_group` | 15 | 240 s | 91 s | 101.1 s |
| `push` (main) | 15 | 227 s | 46 s | 85.3 s |

Across all runs the save step averaged **88.6 s** (median 57 s, range 2–219 s) and
**33% were full misses**. Against that, the cache bought **26 s**: realising the
devShell took 66 s cold and 40 s warm. A net loss of roughly a minute per run, paid
twice per landed PR because the merge queue re-verifies after rebasing.

**The mechanism is eviction from an over-budget cache, and it is self-inflicted.**
`actions/cache/usage` reported **2091 entries totalling 11.37 GB** against GitHub's
10 GB per-repository limit, so the cache was permanently evicting. By ref:

| scope | entries | size | readable by a PR or queue run? |
| --- | --- | --- | --- |
| `refs/heads/main` | 931 | 5.64 GB | yes — the only one |
| `refs/pull/*/merge` | 580 | 2.87 GB | no |
| `gh-readonly-queue/*` | 580 | 2.87 GB | no |

A branch may read caches from itself and from the default branch, so **5.74 GB —
half the budget — sat in scopes nothing would ever read again**, evicting the `main`
entries that later runs depend on. Each landed PR wrote 2.87 GB into dead scopes
whose only lasting effect was to cause the next run's miss.

Issue #93 originally attributed the cost to the queue ref writing a *larger* entry
than the PR ref. That was wrong, and is recorded here because the error is
instructive: the two scopes hold identical counts and identical bytes (580 entries,
2.87 GB each). The single pair of runs the issue cited was a partial miss against a
full miss — two draws from a bimodal distribution read as a systematic difference.

**Removing the cache cannot turn a download into a build.** No run in the sample
emitted a `building '…'` line; every path was substituted. Of the 127 store paths CI
realises, 111 are on cache.nixos.org and the remaining 16 are rust-overlay toolchain
components fetched from static.rust-lang.org.

## Decision

**Remove the `Nix store cache` step from `ci.yml`.** Realisation becomes a
consistent cold fetch, and the whole Actions cache budget returns to the cargo
cache — one 0.42 GB entry that has always been a primary-key hit with a ~1 s
teardown, and which was previously exposed to eviction it survived only by luck.

**The cargo cache (`actions/cache@v4`) stays.** It is measurably working.

**`publish.yml` keeps its copy of the step, for now.** It is release machinery,
triggered only by a version tag, and has never run; changing it is the owner's call
and has no effect on the wall-clock this ADR is about.

**The `ci` job id and the `merge_group` trigger are untouched**, as CLAUDE.md
requires — this removes a step, not the job.

## Consequences

- Blocking CI per landed PR should fall from about 8.8 to 6.4 minutes: two runs, each
  losing a mean 88.6 s of saving and gaining 26 s of cold realisation.
- Run duration becomes *predictable*. The 180–400 s spread was the bimodal hit/miss
  split; without a cache to miss, every run pays the same cold cost.
- CI now depends on cache.nixos.org and static.rust-lang.org being reachable. It
  already did on the 33% of runs that were full misses, so this converts an
  intermittent dependency into a constant one rather than adding a new one.
- The existing Nix cache entries are left to expire on GitHub's own schedule rather
  than being deleted, so nothing regenerable is discarded by hand.

## Alternatives considered

- **Restore always, save only on `push` to `main`.** The obvious fix, and **not
  expressible with this action**: `use-gha-cache` governs read and write together,
  and the upload hook cannot be gated separately from the restore. An `if:` on the
  step degenerates to full removal for PR and queue runs while still paying the
  save on the push run — strictly worse than removing it.
- **Swap to `nix-community/cache-nix-action`**, which stores one `/nix/store` entry
  and can gate saving explicitly. This would genuinely buy back the 26 s, but at the
  cost of a new action, a multi-gigabyte single entry, and eviction tuning. Not
  worth 26 s.
- **Narrow what is cached.** The only lever is `upstream-cache`, which already
  excludes cache.nixos.org — and the action uploaded 1.43 GB regardless, roughly
  0.9 GB of it duplicating content cache.nixos.org already serves. There is no input
  meaning "toolchain only".
- **Do nothing.** Costs ~2.3 minutes of blocking CI per landed PR and leaves the
  cache permanently over budget, with the working cargo entry exposed to an LRU
  eviction whose symptom — suddenly slow clippy and test steps — would look
  unrelated to its cause.
