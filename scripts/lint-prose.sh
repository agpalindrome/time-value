#!/usr/bin/env bash
#
# Authored prose must pass the shared house-style rules.
#
# Run as the `prose` check in scripts/check.sh, which is where the list of what
# CI enforces lives. It is a script rather than a one-line entry there because
# the file list needs building, asserting on, and explaining.
#
# The rules in .vale/styles are the word-level half of ~/.claude/prose.md,
# vendored rather than referenced: a machine-global styles directory is
# invisible to CI, and a `vale sync` package needs a public host. Re-vendor with
# `~/.claude/scripts/sync-vale.sh .`, and report drift with `--check .` — a
# stale copy here is indistinguishable from a live one, so a rule already fixed
# upstream looks like a rule that needs fixing.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 99

# Scope: every tracked markdown file except crates/bundle-check's fixtures,
# which are deliberately malformed OKF documents standing in for a broken
# bundle. Nothing in them is authored prose, and correcting one would destroy
# the defect its test asserts on.
#
# The exclusion lives here rather than in .vale.ini because sync-vale.sh
# regenerates that file from ~/.claude/vale/.vale.ini on every sync, carrying
# over nothing but StylesPath — so a repo-specific scope written there is
# discarded the next time the rules are updated, silently and in the direction
# that lints more.
#
# `git ls-files -z` and a NUL-delimited read, because a path may contain a
# space. A read loop rather than `mapfile -d ''`, which needs bash 4.4 —
# macOS ships 3.2, and this has to behave the same inside the devshell and out.
files=()
while IFS= read -r -d '' file; do
  files+=("$file")
done < <(git ls-files -z '*.md' ':(exclude)crates/bundle-check/tests/fixtures/*')

# Assert the check has inputs. With an empty list, `vale` reads empty stdin,
# reports no errors and exits 0 — a green check over nothing. That hole shipped
# into ~/.claude's CI once. An empty list here always means the pathspec broke,
# never that the repo has no prose.
echo "prose files to lint: ${#files[@]}"
if [ "${#files[@]}" -eq 0 ]; then
  echo "error: no prose files matched — the file list is broken, not the prose" >&2
  exit 1
fi

# Errors block and warnings do not, with no flag needed: vale's exit code counts
# errors alone, whatever MinAlertLevel shows.
#
# --no-global, or vale merges a machine-global styles directory on top of the
# vendored one — which is exactly how a local run comes to disagree with CI.
exec vale --no-global --config .vale.ini "${files[@]}"
