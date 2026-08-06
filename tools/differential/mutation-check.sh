#!/usr/bin/env bash
#
# Does this harness actually have teeth?
#
# A differential harness that reports "no disagreements" is only worth something if
# it would have reported a disagreement had there been one. This script proves that
# by mutation: it copies `src/poker_core` and `tools/differential` into a throwaway
# directory, breaks the copied engine in three different ways, and requires the
# harness to convict each time.
#
#   1. FullHouse payload swapped        -> ordering INVERSION + sub-rank detail
#   2. One-pair kickers truncated       -> ordering FALSE TIE (a chop that should be a win)
#   3. HandRank variant order swapped   -> cross-category INVERSION (Flush beating a full house)
#
# The real repository is NEVER written to: everything happens inside a temp dir, and
# the script verifies afterwards that the repo copy is untouched.
#
# Usage:   ./mutation-check.sh
# Exit:    0 = every mutation was caught; 1 = the harness missed one (harness is broken)

set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_POKER_CORE="$HERE/../../src/poker_core"

if [ ! -f "$REPO_POKER_CORE/src/hand.rs" ]; then
  echo "FAIL: cannot find src/poker_core relative to $HERE" >&2
  exit 1
fi

SANDBOX="$(mktemp -d "${TMPDIR:-/tmp}/cleardeck-mutation.XXXXXX")"
cleanup() { rm -rf "$SANDBOX"; }
trap cleanup EXIT

mkdir -p "$SANDBOX/src" "$SANDBOX/tools"
cp -R "$REPO_POKER_CORE" "$SANDBOX/src/"
cp -R "$HERE" "$SANDBOX/tools/differential"
rm -rf "$SANDBOX/tools/differential/target"

PRISTINE="$SANDBOX/hand.rs.pristine"
cp "$REPO_POKER_CORE/src/hand.rs" "$PRISTINE"
TARGET="$SANDBOX/src/poker_core/src/hand.rs"

# Sizes: big enough to hit every check path, small enough to stay under a minute
# per mutation. The exhaustive five-card sweep runs in full every time: it is the
# check whose completeness is the whole point.
ARGS=(--sevens 20000 --pairs 20000)

fail=0

run_mutation() {
  local name="$1" description="$2" python_edit="$3"

  cp "$PRISTINE" "$TARGET"
  if ! MUTANT_TARGET="$TARGET" python3 -c "$python_edit"; then
    echo "FAIL[$name]: could not apply the mutation (did poker_core's source change?)" >&2
    fail=1
    return
  fi

  local out
  out="$(cd "$SANDBOX/tools/differential" \
    && CLEARDECK_DIFF_REPORT="$SANDBOX/$name.json" \
       cargo run --quiet --release -- "${ARGS[@]}" 2>/dev/null || true)"

  if grep -q '^verdict=DISAGREE_FUND_IMPACTING$' <<<"$out"; then
    echo "PASS[$name]: caught: $description"
    grep -m 2 '^DISAGREEMENT id=ordering' <<<"$out" | sed 's/^/        /' || true
  else
    echo "FAIL[$name]: the harness did NOT convict a broken engine: $description" >&2
    grep -m 5 '^verdict=\|^fund_impacting' <<<"$out" | sed 's/^/        /' >&2 || true
    fail=1
  fi
}

run_mutation "M1-fullhouse-payload-swapped" \
  "FullHouse(trips, pair) built as FullHouse(pair, trips)" \
  '
import os, pathlib
p = pathlib.Path(os.environ["MUTANT_TARGET"])
s = p.read_text()
old = "HandRank::FullHouse(trips[0], pairs[0])"
assert old in s, "anchor missing: " + old
p.write_text(s.replace(old, "HandRank::FullHouse(pairs[0], trips[0])"))
'

run_mutation "M2-one-pair-kickers-truncated" \
  "one-pair kickers truncated from three to two" \
  '
import os, pathlib
p = pathlib.Path(os.environ["MUTANT_TARGET"])
s = p.read_text()
old = """        let kickers: Vec<u8> = ranks.iter()
            .filter(|&&r| r != pairs[0])
            .take(3)"""
assert old in s, "anchor missing: one-pair kicker block"
p.write_text(s.replace(old, old.replace(".take(3)", ".take(2)")))
'

run_mutation "M3-variant-order-swapped" \
  "HandRank variant order swapped so a Flush outranks a FullHouse" \
  '
import os, pathlib
p = pathlib.Path(os.environ["MUTANT_TARGET"])
s = p.read_text()
old = """    Straight(u8),
    Flush(Vec<u8>),
    FullHouse(u8, u8),"""
assert old in s, "anchor missing: HandRank variant list"
new = """    Straight(u8),
    FullHouse(u8, u8),
    Flush(Vec<u8>),"""
p.write_text(s.replace(old, new))
'

# The repository engine must be byte-identical to how we found it.
if ! diff -q "$PRISTINE" "$REPO_POKER_CORE/src/hand.rs" >/dev/null; then
  echo "FAIL: the repository copy of poker_core/src/hand.rs CHANGED during this run" >&2
  fail=1
else
  echo "OK: repository poker_core/src/hand.rs is byte-identical to before the run"
fi

if [ "$fail" -ne 0 ]; then
  echo
  echo "MUTATION CHECK FAILED: the harness cannot be trusted to detect engine bugs." >&2
  exit 1
fi

echo
echo "MUTATION CHECK PASSED: all three injected engine bugs were convicted."
