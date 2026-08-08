#!/usr/bin/env bash
#
# Every check that must pass, defined once. CI runs this; so should you.
#
#   nix develop -c ./scripts/check.sh          # everything
#   nix develop -c ./scripts/check.sh clippy   # one check, by name
#
# It runs *all* checks and reports each, rather than stopping at the first
# failure — fixing one thing only to discover the next on the following push is
# the slower loop.
#
# The list below is the only definition of what CI enforces. Documentation
# points here rather than repeating it: a list in two places is a list that goes
# stale in one of them, which is how the markdown check once ran in CI while the
# docs described six checks and not seven.
#
# It is not quite everything that guards this repo. `flake.nix` also wires
# pre-commit hooks, and four of them — prettier, nixfmt, end-of-file-fixer,
# trim-trailing-whitespace — *fix* files rather than report on them. Running a
# fixer from here would make a script that verifies into one that edits your
# tree, so those stay hooks. The two hooks that only report content problems,
# `typos` and nix formatting, run below in their non-mutating form, because a
# clone without hooks installed otherwise pushes straight past them.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 99

# name|command. Order is the order they run in.
CHECKS=(
  "fmt|cargo fmt --all -- --check"
  # The glob stays quoted so prettier expands it. Unquoted, bash expands it —
  # and bash without globstar reads `**` as `*`, so it matches one directory
  # deep and silently skips every file at the root.
  "markdown|prettier --check '**/*.md'"
  # The hook runs `nixfmt`, which rewrites; `--check` only reports. The file
  # list comes from git rather than a literal `flake.nix` so a second .nix file
  # is covered the day it lands — naming it here would be the same list-in-two-
  # places trap. `-z`/`-0` because a path may contain a space.
  "nixfmt|git ls-files -z '*.nix' | xargs -0 nixfmt --check"
  # Reports by default; only `--write-changes` edits.
  "typos|typos"
  # The knowledge bundle has no step of its own. It had one until 2026-08-08,
  # running okf-graph as a nix-provided binary; okf-graph is now a crate, so the
  # same checker runs inside crates/bundle-check's tests below — one copy of a
  # check rather than two that can drift, and a version pinned in Cargo.lock
  # rather than in flake.lock.
  #
  # One consequence to know about: okf-graph exits 0 on a *report* (a dangling
  # link, an out-of-order log entry) and used to print it here. Five of those are
  # defects in this repo's own bundle and the rest are not — the test is whether
  # the material is ours to fix, and the bundle's producer-not-consumer principle
  # holds the table. bundle-check applies it as a Policy and asserts it.
  "clippy|cargo clippy --workspace --all-targets --locked"
  "test|cargo nextest run --workspace --locked"
  "doctest|cargo test --doc --workspace --locked"
  # --workspace, not -p time_value: a crate the doc step skips has its
  # [lints.rustdoc] silently inert, which is the failure this step exists to
  # catch.
  "doc|cargo doc --workspace --no-deps --locked"
  "deny|cargo deny check all"
)

# `cargo fmt` silently ignores every nightly-only option in rustfmt.toml when it
# resolves a stable rustfmt, then exits 0 — a pass that verified almost nothing.
# The devshell sets RUSTFMT to a pinned nightly; refuse to run without it rather
# than report a green that is not one.
require_nightly_rustfmt() {
  local version
  version="$(cargo fmt --version 2>/dev/null || true)"
  case "$version" in
    *nightly*) return 0 ;;
    *)
      echo "error: rustfmt is '${version:-not found}', not the pinned nightly." >&2
      echo "       Most of rustfmt.toml is nightly-only and stable ignores it" >&2
      echo "       silently, so this would pass without checking." >&2
      echo "       Run inside the devshell: nix develop -c ./scripts/check.sh" >&2
      exit 2
      ;;
  esac
}

run_one() {
  local name=$1 command=$2
  echo "########## $name"
  # eval, so quoting inside a command is honoured rather than stripped.
  eval "$command"
}

require_nightly_rustfmt

declare -a names=() codes=()
wanted="${1:-}"
found=0

for entry in "${CHECKS[@]}"; do
  name="${entry%%|*}"
  command="${entry#*|}"
  [ -n "$wanted" ] && [ "$wanted" != "$name" ] && continue
  found=1
  run_one "$name" "$command"
  # Captured on the very next line. Anything between here and the capture — an
  # echo, an array append — overwrites `$?` with its own success, and every
  # check then records a pass.
  code=$?
  names+=("$name")
  codes+=("$code")
done

if [ -n "$wanted" ] && [ "$found" -eq 0 ]; then
  echo "error: no check named '$wanted'. Known:" >&2
  printf '  %s\n' "${CHECKS[@]%%|*}" >&2
  exit 2
fi

echo
echo "================ SUMMARY ================"
failed=0
for i in "${!names[@]}"; do
  if [ "${codes[$i]}" -eq 0 ]; then
    printf 'PASS  %s\n' "${names[$i]}"
  else
    printf 'FAIL  %s (exit %s)\n' "${names[$i]}" "${codes[$i]}"
    failed=1
  fi
done
echo "========================================="

exit "$failed"
