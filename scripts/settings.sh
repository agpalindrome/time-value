#!/usr/bin/env bash
#
# Reconcile this repo's own (non-org-level) GitHub settings from version
# control. Each desired repo-level ruleset lives as one file under
# .github/rulesets/ in GitHub's native ruleset format; this script diffs them
# against the live rulesets (--check) or applies them (--apply).
#
# Org-wide rules live in ~/github-settings and are managed with tofu — this
# script never touches them, and never deletes a live ruleset it has no file
# for. The two layers compose; GitHub enforces the most restrictive.
#
# Run it yourself: it uses your `gh` auth (repo admin required). It is
# deliberately owner-run, not wired into CI, so settings never change silently.
#
# Requires: gh (authenticated), jq.
set -euo pipefail

need() { command -v "$1" >/dev/null 2>&1 || { echo "error: '$1' not found on PATH" >&2; exit 1; }; }
need gh
need jq

ROOT="$(git rev-parse --show-toplevel)"
RULESET_DIR="$ROOT/.github/rulesets"
REPO="$(gh repo view --json nameWithOwner --jq .nameWithOwner)"

# The fields that define a ruleset; server-added metadata (id, timestamps,
# _links, source, current_user_can_bypass) is dropped so the diff shows only
# meaningful drift.
normalize() { jq -S '{name, target, enforcement, bypass_actors, conditions, rules}'; }

existing_id() {
  gh api "repos/$REPO/rulesets" --jq ".[] | select(.name==\"$1\") | .id" 2>/dev/null | head -n1
}

files() {
  # Fails loudly rather than silently reporting success over an empty set.
  local -a f=("$RULESET_DIR"/*.json)
  [ -e "${f[0]}" ] || { echo "error: no ruleset files in $RULESET_DIR" >&2; exit 1; }
  printf '%s\n' "${f[@]}"
}

case "${1:-}" in
  --check)
    drift=0
    while IFS= read -r file; do
      name="$(jq -r .name "$file")"
      id="$(existing_id "$name")"
      if [ -z "$id" ]; then
        echo "drift: no live ruleset named '$name' on $REPO — run --apply to create it"
        drift=1
        continue
      fi
      want="$(normalize <"$file")"
      live="$(gh api "repos/$REPO/rulesets/$id" | normalize)"
      if diff <(printf '%s\n' "$want") <(printf '%s\n' "$live") >/dev/null; then
        echo "in sync: '$name' (id $id)"
      else
        echo "drift between $(basename "$file") (<) and live ruleset '$name' (>):"
        diff <(printf '%s\n' "$want") <(printf '%s\n' "$live") || true
        drift=1
      fi
    done < <(files)

    # A live ruleset with no file is drift too: it is enforcing something no
    # one can review. Reported, never deleted.
    while IFS= read -r live_name; do
      if ! jq -re --arg n "$live_name" 'select(.name==$n) | .name' "$RULESET_DIR"/*.json >/dev/null 2>&1; then
        echo "drift: live ruleset '$live_name' on $REPO has no file in $RULESET_DIR"
        drift=1
      fi
    done < <(gh api "repos/$REPO/rulesets" --jq '.[] | select(.source_type != "Organization") | .name')

    exit "$drift"
    ;;
  --apply)
    while IFS= read -r file; do
      name="$(jq -r .name "$file")"
      id="$(existing_id "$name")"
      if [ -n "$id" ]; then
        echo "updating '$name' (id $id) on $REPO"
        gh api -X PUT "repos/$REPO/rulesets/$id" --input "$file" >/dev/null
      else
        echo "creating '$name' on $REPO"
        gh api -X POST "repos/$REPO/rulesets" --input "$file" >/dev/null
      fi
    done < <(files)
    echo "applied."
    ;;
  *)
    echo "usage: $0 [--check|--apply]" >&2
    exit 2
    ;;
esac
