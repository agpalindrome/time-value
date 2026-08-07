# Repo-level rulesets

This repo's **own** GitHub rules, version-controlled in GitHub's native ruleset
format. One file per ruleset. Reconcile with `scripts/settings.sh`:

```sh
./scripts/settings.sh --check   # diff files against live; exit 1 on drift
./scripts/settings.sh --apply   # push files to live
```

The script is deliberately owner-run and not wired into CI, so settings never
change silently. It needs `gh` authenticated with repo admin, and `jq`.

## What is here

| file                 | applies to            | gives                                    |
| -------------------- | --------------------- | ---------------------------------------- |
| `branch-naming.json` | every non-default ref | `^(feat\|fix\|chore\|docs\|refactor)/.*` |
| `pull-request.json`  | the default branch    | merging requires a pull request          |

`pull-request.json` requires **zero** approving reviews. That is not an
oversight: GitHub forbids approving your own pull request, so any higher count
would make a sole maintainer's own work unmergeable. It carries an
`OrganizationAdmin` bypass, so the owner can push directly when they mean to.

There is deliberately **no required status check and no merge queue**. CI runs
on every pull request and reports, but does not gate.

## What is _not_ here

Org-wide rules — `deletion` and `required_linear_history` on the default branch
— come from an organization ruleset managed with OpenTofu in `~/github-settings`
and are invisible to this script. The two layers compose, and GitHub enforces
the more restrictive. Running `--apply` here cannot loosen them.

## Drift in both directions

`--check` reports a file that disagrees with the live ruleset **and** a live
ruleset with no file — the second being the more dangerous case, since it is a
rule enforcing something nobody can review. It reports that case and never
deletes anything.
