#!/usr/bin/env bash
#
# scripts/check-suite-wiring.sh -- A SUITE THAT NOTHING RUNS FAILS THE BUILD.
#
# docs/DEFECTS.md H-45, H-17, H-50, H-53, and H-23 which is what wired this into
# CI. The same defect has now been filed four times under four numbers:
#
#   H-17  `deposit_replay` -- the only proven fund-theft reproducer in the
#         project -- was cargo-auto-discovered and named by no target. One wave.
#   H-45  six money-safety suites, 46 tests, named by nothing. One of them,
#         `fund_reachability`, was RED the whole time, and the register cited it
#         as what held M9 closed.
#   H-50  `tools/archive`, 39 self-tests, shipped by a wave whose own document
#         said they held everything above it.
#   H-53  `tests/no_peeking`, five targets named explicitly in their own
#         Cargo.toml *so that nothing would be auto-discovered* -- and then named
#         by no target at all, in the wave that closed H-45.
#
# Every one of those was found by a human reading a directory listing. This is
# that reading, mechanised, and it is a REQUIRED CI check so the reading cannot be
# skipped.
#
# WHAT IT ASSERTS
#
#   1. Every cargo test target on disk, in every crate in the repo, has a row in
#      scripts/test-suites.list.                      (the file nobody wired)
#      "Every target" includes `--lib`: unit tests compiled into a crate are a
#      DIFFERENT cargo target from `--test X`, and every runner in this repo names
#      the latter. See `discover()`.
#   2. Every row in that file points at a target that is on disk.
#                                                     (the gate somebody deleted)
#   3. Every row's tier is one that something actually runs, and the runner for
#      that tier still exists and still names it.     (the tier that went hollow)
#   4. Every target `scripts/dev.sh` names with `--test` has a row, so the local
#      gate cannot run something CI has never heard of.
#
# It needs no replica, no wasm and no network, and takes about a second.
#
# `--selftest` plants each of those failures in turn and requires this script to
# go red on it. A checker that cannot fail is the thing this project keeps
# finding, so it proves it can fail before it is allowed to say anything is fine
# -- and the CI job runs `--selftest` first.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INVENTORY="${CLEARDECK_SUITE_INVENTORY:-$REPO_ROOT/scripts/test-suites.list}"
DEV_SH="$REPO_ROOT/scripts/dev.sh"
# Overridable only so `--selftest` can point the check at a workflow file with
# the fast tier removed and require it to go red. Nothing else sets these.
CI_YML="${CLEARDECK_CI_YML:-$REPO_ROOT/.github/workflows/ci.yml}"
DEEP_YML="${CLEARDECK_DEEP_YML:-$REPO_ROOT/.github/workflows/fund-safety-deep.yml}"

G=$'\033[32m'; Y=$'\033[33m'; E=$'\033[31m'; R=$'\033[0m'
ok()   { printf '    %sok%s    %s\n'   "$G" "$R" "$*"; }
bad()  { printf '    %sFAIL%s  %s\n'   "$E" "$R" "$*" >&2; }
note() { printf '          %s\n' "$*"; }
step() { printf '\n  %s\n' "$*"; }

# ---------------------------------------------------------------------------
# discovery: every cargo integration-test target in the repo
# ---------------------------------------------------------------------------
#
# NOT driven by the inventory. If discovery only looked in the crates the
# inventory already knows about, a brand new crate full of tests would be
# invisible to the check whose entire job is seeing brand new tests. So it starts
# from every Cargo.toml in the tree.
#
# Cargo's own auto-discovery rules, which this mirrors: a file `tests/X.rs` is the
# target `X`; a directory `tests/X/` containing `main.rs` is the target `X`; a
# directory without `main.rs` (`tests/common/`) is not a target; and a nested
# crate under tests/ (`src/poker_core/tests/wasm32_harness/`, which has its own
# Cargo.toml and no main.rs) is not a target of its parent.
# AND `#[cfg(test)]` INSIDE src/, WHICH IS WHERE THIS CHECK WAS BLIND.
#
# The first version of this script looked only at integration-test targets, and
# that is exactly the shape of blindness this project keeps producing. Hunting
# for it found TWELVE tests that no gate ran:
#
#   tests/money_safety/src/invariants/outcome.rs  1 test on `capped()`, the
#       arithmetic that decides how much of a bet is CONTESTABLE. `cargo test
#       --test invariants` -- the command dev.sh types -- does not run the lib's
#       own unit tests, so it was outside the harness that carries it.
#   src/no_peeking/dealer_types + dealer_canister  11 tests, 0.4 s, in the spike
#       whose five integration targets were H-53.
#
# So a crate whose own `src/` contains `#[cfg(test)]` gets the pseudo-target
# `(lib)`, and it must be listed like any other. `--lib` and `--test X` are
# different targets and a runner that names one does not run the other.
discover() {
  local manifest dir f sub
  while IFS= read -r manifest; do
    dir="$(dirname "$manifest")"
    dir="${dir#"$REPO_ROOT"/}"

    # Integration-test targets.
    if [ -d "$REPO_ROOT/$dir/tests" ]; then
      for f in "$REPO_ROOT/$dir"/tests/*.rs; do
        [ -e "$f" ] || continue
        printf '%s|%s\n' "$dir" "$(basename "$f" .rs)"
      done
      for sub in "$REPO_ROOT/$dir"/tests/*/; do
        [ -f "$sub/main.rs" ] || continue
        printf '%s|%s\n' "$dir" "$(basename "$sub")"
      done
    fi

    # Unit tests compiled into the crate's own lib or bin. Only for a real crate
    # (a virtual workspace manifest such as ./Cargo.toml or src/no_peeking has no
    # src/lib.rs and no unit tests of its own), and only counting files that
    # belong to THIS crate rather than to a crate nested inside it.
    if [ -f "$REPO_ROOT/$dir/src/lib.rs" ] || [ -f "$REPO_ROOT/$dir/src/main.rs" ]; then
      if crate_own_src_has_unit_tests "$dir"; then
        printf '%s|(lib)\n' "$dir"
      fi
    fi
  done < <(all_manifests)
}

all_manifests() {
  find "$REPO_ROOT" -name Cargo.toml \
    -not -path '*/target/*' -not -path '*/node_modules/*' \
    -not -path "$REPO_ROOT/deps/*" -not -path "$REPO_ROOT/.dfx/*" \
    -not -path "$REPO_ROOT/.icp/*" | sort
}

# True when `<dir>/src` contains `#[cfg(test)]` in a file that is not inside a
# NESTED crate. Without the exclusion a parent would be blamed for its children's
# tests and the child's own row would look redundant.
crate_own_src_has_unit_tests() {
  local dir="$1" hit nested skip
  while IFS= read -r hit; do
    skip=0
    while IFS= read -r nested; do
      nested="$(dirname "$nested")"
      nested="${nested#"$REPO_ROOT"/}"
      # Only a crate NESTED INSIDE this one hides files from it. An ANCESTOR
      # manifest (src/no_peeking's virtual workspace above dealer_types) must not,
      # or every member of a nested workspace looks test-free.
      case "$nested" in "$dir"/*) ;; *) continue ;; esac
      case "$hit" in "$REPO_ROOT/$nested"/*) skip=1; break ;; esac
    done < <(all_manifests)
    [ "$skip" = 0 ] && return 0
  done < <(grep -rl '#\[cfg(test)\]' "$REPO_ROOT/$dir/src" 2>/dev/null)
  return 1
}

# ---------------------------------------------------------------------------
# the inventory, parsed
# ---------------------------------------------------------------------------

# Emits `crate|target|tier|threads|filter|env|note` for each non-comment row.
rows() {
  grep -v '^[[:space:]]*#' "$INVENTORY" | grep -v '^[[:space:]]*$'
}

VALID_TIERS='fast deep workspace sweep'

# IT DISCOVERS, AND IT ALSO REQUIRES.
#
# Discovery answers "did anything get missed". It does NOT answer "did anything
# get quietly demoted", and that is the next way this gate dies: the fast tier is
# the one that costs every pull request, so the pressure on it is always downward,
# and moving the custody gate or the settlement oracle to a nightly is a one-word
# edit that leaves every check in this script green.
#
# These targets are not allowed out of the REQUIRED tier. Each one is here because
# it has convicted a real fund defect in this repository, not because it is
# expensive:
REQUIRED_FAST=(
  invariants          # M1..M14, the named fund invariants
  regressions         # the pinned reproducers of every payout defect found here
  deposit_replay      # the E-02 FUND-THEFT reproducer (H-17: named by nothing for a wave)
  admin_custody       # FINDING 07: one controller call destroyed a funded table's chips
  controller_custody  # FINDING 23: the controller seat, and the guardian
  solvency            # FINDING 35: can this canister pay everyone, and can it say so
  fuzz                # the hostile-sequence smoke run
  settlement          # THE ONLY GATE THAT CONVICTS A WRONG-SEAT PAYOUT
  disagreements       # its pinned convictions (E-01, E-05, odd chips)
)

main() {
  local fail=0

  printf '  suite wiring: every test target on disk is named by something that runs it\n'
  printf '  inventory: %s\n' "${INVENTORY#"$REPO_ROOT"/}"

  [ -f "$INVENTORY" ] || { bad "the inventory $INVENTORY does not exist"; return 1; }

  # -- 0. the inventory is well formed -------------------------------------
  step "the inventory itself parses"
  local line n=0 crate target tier threads filter env notetext
  while IFS= read -r line; do
    n=$((n + 1))
    IFS='|' read -r crate target tier threads filter env notetext <<<"$line"
    if [ -z "${notetext:-}" ]; then
      bad "row '$line' has fewer than 7 fields, or an empty note"
      note "every row must say WHY it is in the tier it is in"
      fail=1
      continue
    fi
    case " $VALID_TIERS " in
      *" $tier "*) ;;
      *) bad "row '$crate|$target' has tier '$tier', which is not one of: $VALID_TIERS"; fail=1 ;;
    esac
  done < <(rows)
  [ "$n" -gt 0 ] || { bad "the inventory has no rows at all"; return 1; }
  [ "$fail" = 0 ] && ok "$n row(s), every one with a tier and a note"

  # -- 1. every target on disk has a row ------------------------------------
  step "every cargo test target on disk is in the inventory"
  local disk found orphans=0
  disk="$(discover)"
  [ -n "$disk" ] || { bad "discovery found no test targets at all; it is broken, not the repo"; return 1; }
  while IFS= read -r found; do
    if ! rows | cut -d'|' -f1,2 | grep -qxF "$found"; then
      bad "$found is a cargo test target that NOTHING NAMES"
      note "it is auto-discovered, so 'cargo test' in its crate runs it and no gate does."
      note "add a row to ${INVENTORY#"$REPO_ROOT"/} choosing the tier that will run it."
      orphans=$((orphans + 1))
      fail=1
    fi
  done <<<"$disk"
  [ "$orphans" = 0 ] && ok "$(printf '%s\n' "$disk" | wc -l | tr -d ' ') target(s) discovered, all listed"

  # -- 2. every row points at a real file ------------------------------------
  step "every inventory row points at a target that exists"
  local missing=0 path
  while IFS= read -r line; do
    IFS='|' read -r crate target tier threads filter env notetext <<<"$line"
    if ! printf '%s\n' "$disk" | grep -qxF "$crate|$target"; then
      bad "$crate|$target is in the inventory and is not on disk"
      note "a gate that has been deleted cannot be discovered, which is why rows are required."
      missing=$((missing + 1))
      fail=1
    fi
  done < <(rows)
  [ "$missing" = 0 ] && ok "no row names a target that has been renamed or deleted"

  # -- 3. the runner for each tier exists and still names the tier -----------
  step "each tier has a runner, and the runner still reads this file"
  local runner="$REPO_ROOT/scripts/ci-fund-safety.sh"
  if [ -x "$runner" ]; then
    ok "scripts/ci-fund-safety.sh is present and executable"
  else
    bad "scripts/ci-fund-safety.sh is missing or not executable; the fast and deep rows run nowhere"
    fail=1
  fi
  # The runner must read THIS file, or the rows are a description of CI rather
  # than CI itself -- which is the two-list failure (H-47) all over again.
  if [ -f "$runner" ] && grep -q 'test-suites.list' "$runner"; then
    ok "the runner reads scripts/test-suites.list rather than a copy of it"
  else
    bad "scripts/ci-fund-safety.sh does not read scripts/test-suites.list"
    fail=1
  fi
  local t
  for t in fast deep; do
    local yml desc
    case "$t" in
      fast) yml="$CI_YML";   desc="the required pull-request workflow" ;;
      deep) yml="$DEEP_YML"; desc="the scheduled deep workflow" ;;
    esac
    if [ -f "$yml" ] && grep -q "ci-fund-safety.sh $t" "$yml"; then
      ok "tier '$t' is invoked by ${yml#"$REPO_ROOT"/} ($desc)"
    else
      bad "no job in ${yml#"$REPO_ROOT"/} runs 'ci-fund-safety.sh $t'"
      note "every row in tier '$t' therefore runs nowhere."
      fail=1
    fi
  done
  # `workspace` rows claim `cargo test --locked --workspace` covers them.
  if [ -f "$CI_YML" ] && grep -q 'cargo test --locked --workspace' "$CI_YML"; then
    ok "tier 'workspace' is covered by 'cargo test --locked --workspace' in ci.yml"
  else
    bad "ci.yml no longer runs 'cargo test --locked --workspace'; every workspace row runs nowhere"
    fail=1
  fi
  # `sweep` rows claim a crate-wide `cargo test` runs them. Check per crate.
  local swept_crates
  swept_crates="$(rows | awk -F'|' '$3 == "sweep" { print $1 }' | sort -u)"
  local c
  for c in $swept_crates; do
    # Something must run a bare `cargo test` in the crate's directory OR in a
    # directory ABOVE it -- `cd src/no_peeking && cargo test` sweeps all three of
    # that workspace's members, and refusing to see that would force three
    # redundant lines into dev.sh.
    local d="$c" found=''
    while [ -n "$d" ] && [ "$d" != '.' ]; do
      if grep -q "cd $d && cargo test" "$DEV_SH" 2>/dev/null; then found="$d"; break; fi
      d="$(dirname "$d")"
    done
    if [ -n "$found" ]; then
      # SAY WHERE. `sweep` rows are run by the LOCAL gate, not by a CI job, and a
      # line that reads "ok" without saying so would let a reader conclude these
      # are gated on every pull request. They are not.
      ok "tier 'sweep' for $c: 'cd $found && cargo test' in scripts/dev.sh — LOCAL GATE ONLY, no CI job"
    else
      bad "no 'cd <dir> && cargo test' in scripts/dev.sh covers $c; its sweep rows run nowhere"
      fail=1
    fi
  done

  # -- 4. dev.sh cannot name a target CI has never heard of ------------------
  #
  # TWO SPELLINGS, BECAUSE THIS CHECK WENT BLIND ONCE (docs/DEFECTS.md H-59).
  # Wave 14 replaced sixteen literal `cargo test --test X` lines in cmd_test with
  # the `run_ms X` helper. Those strings stopped existing, this discovery went
  # from 23 subjects to 7, and it printed `ok` both times -- only the count in the
  # message changed, and nothing reads the count. A check that silently narrows
  # its own subject list is the exact instrument-side failure this file exists to
  # catch, so it now recognises the helper as well as the raw flag, and REFUSES to
  # pass on an empty subject list.
  step "every target scripts/dev.sh names has a row"
  local named unknown=0 named_count
  named="$( { grep -oE -- '--test [a-z_0-9]+' "$DEV_SH" | awk '{print $2}';
              grep -oE '^[[:space:]]*run_ms [a-z_0-9]+' "$DEV_SH" | awk '{print $2}'; } \
            | sort -u )"
  named_count="$(printf '%s' "$named" | grep -c . || true)"
  if [ "$named_count" -eq 0 ]; then
    bad "this check found NO test targets named in scripts/dev.sh, which cannot be true"
    note "the discovery pattern has stopped matching how cmd_test invokes cargo. It went"
    note "from 23 subjects to 7 that way once and still said 'ok'. See docs/DEFECTS.md H-59."
    fail=1
  fi
  for t in $named; do
    if ! rows | awk -F'|' '{print $2}' | grep -qxF "$t"; then
      bad "scripts/dev.sh runs '--test $t' and there is no inventory row for it"
      note "the local gate runs something CI does not. Add the row."
      unknown=$((unknown + 1))
      fail=1
    fi
  done
  [ "$unknown" = 0 ] && [ "$named_count" -gt 0 ] && ok "$named_count target(s) named in dev.sh, all listed"

  # -- 5. the gates that have caught fund defects stay in the REQUIRED tier --
  step "no gate that has convicted a fund defect has been demoted out of 'fast'"
  local demoted=0 req
  for req in "${REQUIRED_FAST[@]}"; do
    if rows | awk -F'|' -v t="$req" '$2 == t && $3 == "fast"' | grep -q .; then
      ok "'$req' is in the fast tier"
    else
      bad "'$req' is NOT in the fast tier"
      note "it is on the REQUIRED_FAST list in this script because it has convicted a real"
      note "fund defect here. Moving it to a nightly makes every pull request blind to that"
      note "defect class. If the demotion is genuinely right, argue it in this script's list."
      demoted=$((demoted + 1))
      fail=1
    fi
  done
  [ "$demoted" = 0 ] && ok "${#REQUIRED_FAST[@]} required gates all still required"

  # -- the tier census, printed so a reader can see the split ---------------
  #
  # WITH WHERE EACH TIER RUNS, not just how many rows it has. Three of the four
  # tiers are CI jobs and one is not, and a census that hid that difference would
  # read as "everything is gated on every pull request", which is false.
  step "the split"
  local where
  for t in fast deep workspace sweep; do
    case "$t" in
      # NOT "REQUIRED". Verified against the API on 2026-08-09: branch protection on
      # `main` has NO required status checks (404) and NO rulesets, so a red job here
      # blocks nothing. Saying REQUIRED in a gate's own output is the overclaim this
      # project keeps finding. See docs/DEFECTS.md H-23.
      fast)      where='CI ci.yml -> fund-safety-fast (declared on every PR and on the deploy path; NOT a required status check yet)' ;;
      deep)      where='CI fund-safety-deep.yml (nightly, dispatch, and every push to main)' ;;
      workspace) where='CI ci.yml -> cargo test --locked --workspace' ;;
      sweep)     where='scripts/dev.sh ONLY -- run locally, by NO CI job. See docs/DEFECTS.md H-23.' ;;
    esac
    printf '    %-10s %2s row(s)   %s\n' "$t" \
      "$(rows | awk -F'|' -v t="$t" '$3 == t' | wc -l | tr -d ' ')" "$where"
  done
  printf '    %s\n' "----"
  printf '    %2s of %s row(s) are gated by a CI job; %s are local-only.\n' \
    "$(rows | awk -F'|' '$3 != "sweep"' | wc -l | tr -d ' ')" \
    "$(rows | wc -l | tr -d ' ')" \
    "$(rows | awk -F'|' '$3 == "sweep"' | wc -l | tr -d ' ')"

  step "result"
  if [ "$fail" = 0 ]; then
    printf '    %severy test target in this repository is run by something%s\n' "$G" "$R"
    printf '    %s\n' "(that is the claim this gate makes. It is NOT 'everything runs in CI':"
    printf '    %s\n' " the sweep rows above run only when a human runs scripts/dev.sh.)"
    return 0
  fi
  printf '    %sSUITE WIRING FAILED%s\n' "$E" "$R" >&2
  return 1
}

# ---------------------------------------------------------------------------
# --selftest: prove this script can go red, four ways
# ---------------------------------------------------------------------------
#
# THE STANDING LESSON. A gate that cannot fail is worse than no gate, and this
# repository has shipped one: the Candid drift job compared the wrong thing and
# ended in `exit 0` behind a TODO while the committed .did was missing seven
# fields and a whole method. So this script plants each failure it claims to
# catch, and requires itself to go red on it.
selftest() {
  local tmpdir plantfile rc pass=0 fails=0
  tmpdir="$(mktemp -d)"
  # Expanded now: `tmpdir` is local and is out of scope when the trap fires.
  # shellcheck disable=SC2064
  trap "rm -rf '$tmpdir' '$REPO_ROOT/tools/zz_wiring_selftest_crate'; rm -f '$REPO_ROOT/tests/money_safety/tests/zz_wiring_selftest_probe.rs'" EXIT

  expect_red() {
    local what="$1"; shift
    if ( "$@" ) >"$tmpdir/out" 2>&1; then
      printf '    %sSELFTEST FAILED%s  the check stayed GREEN with %s\n' "$E" "$R" "$what" >&2
      sed 's/^/        /' "$tmpdir/out" >&2
      fails=$((fails + 1))
    else
      printf '    %sok%s    goes red: %s\n' "$G" "$R" "$what"
      pass=$((pass + 1))
    fi
  }
  expect_green() {
    local what="$1"; shift
    if ( "$@" ) >"$tmpdir/out" 2>&1; then
      printf '    %sok%s    stays green: %s\n' "$G" "$R" "$what"
      pass=$((pass + 1))
    else
      printf '    %sSELFTEST FAILED%s  the check went RED on %s\n' "$E" "$R" "$what" >&2
      sed 's/^/        /' "$tmpdir/out" >&2
      fails=$((fails + 1))
    fi
  }

  printf '  check-suite-wiring.sh selftest: every failure this gate claims to catch, planted\n\n'

  # (a) BASELINE. If the tree is already red, every "goes red" below would be
  #     vacuous, so this has to pass first and it is reported as a real case.
  expect_green "the repository as checked out" main

  # (b) A NEW SUITE THAT NOTHING NAMES -- the H-45 shape, in the crate it
  #     happened in. A real .rs file, in a real crate, auto-discovered by cargo.
  plantfile="$REPO_ROOT/tests/money_safety/tests/zz_wiring_selftest_probe.rs"
  cat >"$plantfile" <<'PLANT'
// Planted by scripts/check-suite-wiring.sh --selftest and deleted immediately.
// If you are reading this in a commit, the selftest crashed between the two.
#[test]
fn a_suite_nobody_wired() {}
PLANT
  expect_red "a new test file that no target names" main
  rm -f "$plantfile"
  expect_green "the same tree with the planted file removed" main

  # (b2) A WHOLE NEW CRATE, whose tests are UNIT tests rather than an
  #      integration target. This is the case the first version of this script was
  #      blind to, and being blind to it hid twelve real tests, so it is executed
  #      rather than argued: a crate that does not exist in the inventory, with a
  #      `#[cfg(test)]` module and no `tests/` directory at all.
  mkdir -p "$REPO_ROOT/tools/zz_wiring_selftest_crate/src"
  cat >"$REPO_ROOT/tools/zz_wiring_selftest_crate/Cargo.toml" <<'PLANT'
# Planted by scripts/check-suite-wiring.sh --selftest and deleted immediately.
[package]
name = "zz_wiring_selftest_crate"
version = "0.0.0"
edition = "2021"
publish = false
[workspace]
PLANT
  cat >"$REPO_ROOT/tools/zz_wiring_selftest_crate/src/lib.rs" <<'PLANT'
#[cfg(test)]
mod tests {
    #[test]
    fn a_unit_test_nobody_wired() {}
}
PLANT
  expect_red "a new crate whose only tests are unit tests in src/" main
  rm -rf "$REPO_ROOT/tools/zz_wiring_selftest_crate"
  expect_green "the same tree with the planted crate removed" main

  # (c) A ROW WHOSE FILE IS GONE -- the deleted gate. Discovery cannot see a
  #     file that is not there, which is exactly why rows are required.
  { cat "$INVENTORY"; echo 'tests/money_safety|a_gate_that_was_deleted|fast|2|-|-|planted by the selftest'; } \
    >"$tmpdir/inv-missing.list"
  expect_red "an inventory row naming a target that is not on disk" \
    env CLEARDECK_SUITE_INVENTORY="$tmpdir/inv-missing.list" bash "${BASH_SOURCE[0]}"

  # (d) A TIER WITH NO RUNNER -- the hollow tier. A row can be perfectly formed
  #     and still be run by nothing if the workflow that names its tier is gone.
  grep -v 'ci-fund-safety.sh fast' "$CI_YML" >"$tmpdir/ci-no-fast.yml"
  expect_red "a fast row when no CI job invokes the fast tier" \
    env CLEARDECK_CI_YML="$tmpdir/ci-no-fast.yml" bash "${BASH_SOURCE[0]}"

  # (d2) A GATE DEMOTED OUT OF THE REQUIRED TIER. The settlement oracle is the
  #      only harness that convicts a wrong-seat payout, and moving it to the
  #      nightly is a one-word edit that leaves every other check in this script
  #      green. So the one-word edit is executed here and must go red.
  sed 's/^tests\/settlement|settlement|fast|/tests\/settlement|settlement|deep|/' "$INVENTORY" \
    >"$tmpdir/inv-demoted.list"
  expect_red "the settlement oracle demoted from the required tier to the nightly" \
    env CLEARDECK_SUITE_INVENTORY="$tmpdir/inv-demoted.list" bash "${BASH_SOURCE[0]}"

  # (e) AN INVALID TIER -- a row that looks wired and names a tier nothing runs.
  { cat "$INVENTORY"; echo 'tests/money_safety|invariants|someday|2|-|-|planted by the selftest'; } \
    >"$tmpdir/inv-badtier.list"
  expect_red "a row whose tier is not one anything runs" \
    env CLEARDECK_SUITE_INVENTORY="$tmpdir/inv-badtier.list" bash "${BASH_SOURCE[0]}"

  printf '\n  result\n'
  if [ "$fails" = 0 ]; then
    printf '    %s%s selftest case(s) passed: this gate can fail%s\n' "$G" "$pass" "$R"
    return 0
  fi
  printf '    %s%s selftest case(s) FAILED%s\n' "$E" "$fails" "$R" >&2
  return 1
}

case "${1:-}" in
  --selftest) selftest ;;
  '') main ;;
  *) printf 'usage: %s [--selftest]\n' "$0" >&2; exit 2 ;;
esac
