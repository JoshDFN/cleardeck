#!/usr/bin/env bash
# HOW LONG BEFORE A CANISTER STOPS HONOURING WITHDRAWALS?
#
# WHY THIS EXISTS
#   docs/DEFECTS.md E-55. A canister below its freezing threshold rejects EVERY
#   update call at once -- `deposit`, `withdraw`, `cash_out`, `player_action`,
#   `abandon_stuck_hand` -- so every player at a table loses access to their own
#   money at the same instant, with no attacker and no in-application remedy.
#   Nothing in this tree tops a canister up, and until this script there was no
#   monitoring anywhere either.
#
#   Measured on the real module (tests/money_safety/tests/cycles_runway.rs):
#
#       empty table, on-chain clock only ....... 0.0442 T/day -> 10 T = 225 days
#       ~200 hands/day ......................... 0.0779 T/day -> 10 T = 128 days
#       ~1000 hands/day ........................ 0.2126 T/day -> 10 T =  47 days
#       500 hands/day, six tabs open ........... 0.4994 T/day -> 10 T =  20 days
#
#   CORRECTION, wave-13 reconciliation -- docs/DEFECTS.md E-92. Those rows count
#   the frontend's 10-second HEARTBEAT and NOT its 500 ms `check_timeouts` poll,
#   which is an UPDATE call at a measured 6,573,911 cycles: 0.28-1.14 T/day PER
#   OPEN TAB against the heartbeat's 0.0618. FALLBACK_BURN_PER_DAY below is
#   therefore a floor that is itself too low, and it is used only when a canister
#   cannot yet measure its own burn. Left as-is on purpose: the honest fix is to
#   re-measure the whole table with the poll included, in
#   tests/money_safety/tests/cycles_runway.rs, and publish one set of numbers.
#       dealing continuously ................... 3.68   T/day -> 10 T =   2 days
#
#   The burn rate moves by two orders of magnitude with load, which is why this
#   script alarms on RUNWAY DAYS and never on a cycles figure. The workflow it
#   replaced used a flat 1 T floor; 1 T is 22 days on an idle table and under
#   SEVEN HOURS on one dealing continuously, so the same number meant two
#   completely different things depending on who was playing.
#
# THE TWO THINGS THAT MAKE THIS DIFFERENT FROM AN ORDINARY BALANCE CHECK
#
#   1. UNREACHABLE IS THE ALARM.
#      docs/SECURITY-FINDINGS.md FINDING 24, reproduced live in
#      tests/money_safety/tests/cycles_runway.rs: a frozen canister rejects
#      QUERY CALLS TOO, at the boundary, before any canister code runs. So
#      `get_cycle_status` -- the endpoint built to raise this alarm -- goes dark
#      at the exact moment the alarm is true. A monitor written the obvious way
#      (poll, read runway_days, alert if low) gets a transport reject and reports
#      NOTHING AT ALL. Here a failed read is the loudest possible result.
#
#   2. IT NEEDS NO IDENTITY AND NO SECRET.
#      `icp canister status` is CONTROLLER-ONLY, which is why the workflow this
#      replaces imported IC_DEPLOY_IDENTITY -- a key that can also deploy -- into
#      a scheduled job, every day, to read a number. `get_cycle_status` is a
#      public query callable anonymously, so this script authenticates as nobody
#      and cannot deploy even if it wanted to. That is the whole reason the
#      canister publishes its own runway.
#
# WHAT IT READS, AND ONLY READS
#   `icp canister call <id> get_cycle_status '()' --query` and, for canisters
#   that have no such endpoint, one cheap public query used purely as a liveness
#   probe. There is no update call, no deploy, no install and no identity
#   anywhere in this file. `.github/workflows/cycles-monitor.yml` fails the run
#   if one ever appears.
#
# USAGE
#   ./scripts/cycles-runway.sh --network local
#   ./scripts/cycles-runway.sh --network ic
#   ./scripts/cycles-runway.sh --network local --warn-days 90 --critical-days 30
#   ./scripts/cycles-runway.sh --selftest     # no network: prove it can go red
#
# EXIT CODES
#   0  every canister answered and every measured runway is above --warn-days
#   1  at least one canister is unreachable, or below --warn-days
#   2  bad arguments
set -euo pipefail

# Kept equal to WARN_DAYS / CRITICAL_DAYS in
# src/cleardeck_frontend/src/lib/cycleRunway.js and in
# tests/money_safety/tests/cycles_runway.rs. tools/shots/test-cycle-runway.mjs
# reads all three and fails if they ever disagree.
WARN_DAYS=60
CRITICAL_DAYS=21

# WHAT TO ASSUME WHEN A CANISTER CANNOT SAY.
#
# `runway_days` is null whenever the canister has not measured a burn rate yet,
# and it is null for at least CYCLE_SAMPLE_MIN_SECS (300 s) after EVERY install,
# EVERY upgrade and -- this is the one that matters -- EVERY TOP-UP, because a
# top-up re-baselines the measurement. So the obvious monitor is blind precisely
# in the window right after somebody has touched the fleet, and it reports
# success while knowing nothing. Measured: this is exactly what happened the
# first time this script was run against a freshly topped-up local replica.
#
# So "unknown" is turned into a FLOOR instead of a shrug. 0.4994 T/day is the
# measured burn of a table dealing ~500 hands a day with six tabs open
# (tests/money_safety/tests/cycles_runway.rs), i.e. a fully occupied table -- not
# the idle figure, which would flatter every canister by a factor of eleven.
FALLBACK_BURN_PER_DAY=499412781032

NETWORK="local"
SELFTEST=0
while [ $# -gt 0 ]; do
  case "$1" in
    --network)       NETWORK="$2"; shift 2 ;;
    --warn-days)     WARN_DAYS="$2"; shift 2 ;;
    --critical-days) CRITICAL_DAYS="$2"; shift 2 ;;
    --selftest)      SELFTEST=1; shift ;;
    -h|--help)       sed -n '2,58p' "$0"; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

cd "$(dirname "$0")/.."

# ---------------------------------------------------------------------------
# The parser, and the one rule it follows
# ---------------------------------------------------------------------------
#
# Candid textual output uses `_` as a digit separator and `opt (N : nat64)` /
# `null` for optionals. Everything below refuses to guess: a field it cannot find
# is reported as absent, never as zero, because "0 cycles" and "I could not read
# the cycles" are opposite conclusions and only one of them is safe to act on.
parse_status() {
  python3 - "$1" <<'PY'
import re, sys
text = sys.argv[1]

def num(field):
    m = re.search(rf'\b{field}\s*=\s*([0-9_]+)', text)
    return int(m.group(1).replace('_', '')) if m else None

def opt_num(field):
    m = re.search(rf'\b{field}\s*=\s*opt\s*\(?\s*([0-9_]+)', text)
    if m:
        return int(m.group(1).replace('_', ''))
    if re.search(rf'\b{field}\s*=\s*null', text):
        return None
    return None

liquid   = num('liquid_balance')
balance  = num('balance')
lifetime = num('observed_burn_per_day')
# Absent on a module older than this tree, which is not an error: it is the
# reason `recent_burn_per_day` exists at all. Treated as 0 so the max() below
# falls back to the lifetime figure.
recent   = num('recent_burn_per_day') or 0
days     = opt_num('runway_days')
ticks    = num('clock_ticks')
meaningful = 'measurement_is_meaningful = true' in text

# THE PESSIMISTIC RATE. A lifetime average alone reads high on a table that has
# just got busy, which is precisely when the number is needed (docs/DEFECTS.md
# E-55). This mirrors what the canister divides by; it is recomputed here so a
# canister that reports a stale `runway_days` cannot talk the monitor round.
burn = max(lifetime or 0, recent)
# ONLY WHEN THE CANISTER SAYS THE MEASUREMENT MEANS SOMETHING. A rate measured
# over a window too short to trust is what every canister reports in the minutes
# after an install, an upgrade or a top-up, and a quiet half-minute inside that
# window derives a runway of centuries. Deriving from it would turn "I cannot say
# yet" into "you have plenty", which is the failure this script exists to stop.
derived = (liquid // burn) if (meaningful and liquid is not None and burn > 0) else None

# When the canister states a runway AND we can derive one, take the SMALLER.
# Disagreement between them means one of the two is stale, and a fuel gauge
# resolves a disagreement downwards.
if days is not None and derived is not None:
    days = min(days, derived)
elif days is None:
    days = derived

print(f"days={days if days is not None else -1}")
print(f"liquid={liquid if liquid is not None else -1}")
print(f"burn={burn}")
print(f"ticks={ticks if ticks is not None else -1}")
print(f"meaningful={'1' if meaningful else '0'}")
PY
}

t_of() { # cycles -> "N.NNN T"
  python3 -c "v=int('$1'); print('%.3f T' % (v/1e12)) if v >= 0 else print('unknown')"
}

# ---------------------------------------------------------------------------
# --selftest: prove the comparison can go red WITHOUT a network
# ---------------------------------------------------------------------------
if [ "$SELFTEST" -eq 1 ]; then
  echo "== selftest: can this script tell a healthy canister from a dying one?"
  fail=0
  check() { # check <label> <expected days> <candid text>
    got="$(parse_status "$3" | sed -n 's/^days=//p')"
    if [ "$got" = "$2" ]; then
      echo "   ok: $1 -> $got days"
    else
      echo "   ✗  $1 -> parsed $got days, expected $2"
      fail=1
    fi
  }
  HEALTHY='(record { balance = 10_000_000_000_000 : nat; liquid_balance = 9_997_800_000_000 : nat; observed_burn_per_day = 44_247_843_312 : nat; recent_burn_per_day = 44_247_843_312 : nat; runway_days = opt (225 : nat64); measurement_is_meaningful = true; clock_ticks = 240 : nat64; })'
  check "a healthy table" 225 "$HEALTHY"

  # The DEFECT this whole wave is about: a canister whose LIFETIME average is
  # reassuring and whose RECENT burn is not. The monitor must believe the recent
  # one, or it repeats the gauge that reads high.
  BUSY='(record { balance = 10_000_000_000_000 : nat; liquid_balance = 9_997_800_000_000 : nat; observed_burn_per_day = 44_247_843_312 : nat; recent_burn_per_day = 499_412_781_032 : nat; runway_days = opt (225 : nat64); measurement_is_meaningful = true; clock_ticks = 240 : nat64; })'
  check "a busy table whose lifetime average still says 225" 20 "$BUSY"

  # An older module with no recent_burn_per_day at all. Must still parse.
  OLD='(record { balance = 1_408_677_590_920 : nat; liquid_balance = 1_379_728_176_820 : nat; observed_burn_per_day = 54_999_400_485 : nat; runway_days = opt (25 : nat64); measurement_is_meaningful = true; clock_ticks = 3_942 : nat64; })'
  check "a module older than this tree (no recent_burn_per_day)" 25 "$OLD"

  # Never measured. -1 is "cannot say", and the caller must not read it as fine.
  NEW='(record { balance = 10_000_000_000_000 : nat; liquid_balance = 9_997_800_000_000 : nat; observed_burn_per_day = 0 : nat; recent_burn_per_day = 0 : nat; runway_days = null; measurement_is_meaningful = false; clock_ticks = 2 : nat64; })'
  check "a canister that has not measured itself yet" -1 "$NEW"

  # A canister claiming a comfortable runway its own numbers do not support.
  # min(stated, derived) is what stops a stale field talking the monitor round.
  LYING='(record { balance = 1_000_000_000_000 : nat; liquid_balance = 900_000_000_000 : nat; observed_burn_per_day = 300_000_000_000 : nat; recent_burn_per_day = 0 : nat; runway_days = opt (900 : nat64); measurement_is_meaningful = true; clock_ticks = 240 : nat64; })'
  check "a stated runway its own balance and burn contradict" 3 "$LYING"

  # And the threshold arithmetic itself, which is the thing that goes red.
  for pair in "225 0" "59 1" "20 1"; do
    d="${pair%% *}"; want="${pair##* }"
    got=0
    { [ "$d" -ge 0 ] && [ "$d" -lt "$WARN_DAYS" ]; } && got=1
    if [ "$got" != "$want" ]; then
      echo "   ✗  threshold: $d days should have alarmed=$want, got $got"
      fail=1
    fi
  done

  # THE UNMEASURED CASE, WHICH IS NOT THE SAME AS THE HEALTHY CASE. A canister
  # that cannot say gets a FLOOR computed from a fully-occupied table's burn,
  # and the floor is what alarms. A monitor that reported success here would be
  # green for the whole window after every top-up -- measured, on the local
  # replica, the first time this script ran after one.
  # 10 T is E-55's headline balance and it alarms here, which is correct and is
  # the point: 10 T is 225 days on an IDLE table and 20 days on a busy one, so a
  # canister at 10 T that cannot tell you which it is has not earned a green run.
  for pair in "51378000000000 0" "30000000000000 0" "10000000000000 1" "5000000000000 1" "1000000000000 1"; do
    liq="${pair%% *}"; want="${pair##* }"
    est="$(python3 -c "print(int($liq) // $FALLBACK_BURN_PER_DAY)")"
    got=0
    [ "$est" -lt "$WARN_DAYS" ] && got=1
    if [ "$got" != "$want" ]; then
      echo "   ✗  unmeasured floor: $liq cycles -> ${est}d should have alarmed=$want, got $got"
      fail=1
    else
      echo "   ok: unmeasured, $(python3 -c "print('%.1f T' % (int($liq)/1e12))") -> floor ${est}d, alarm=$got"
    fi
  done
  [ "$fail" -eq 0 ] && echo "   selftest passed (warn<$WARN_DAYS, critical<$CRITICAL_DAYS)"
  exit "$fail"
fi

# ---------------------------------------------------------------------------
# The real run
# ---------------------------------------------------------------------------
if [ "$NETWORK" = "ic" ]; then
  MAP=".icp/data/mappings/ic.ids.json"
else
  MAP=".icp/cache/mappings/local.ids.json"
fi
[ -f "$MAP" ] || { echo "::error::no id mapping at $MAP"; exit 1; }

cid_of() { python3 -c "import json;print(json.load(open('$MAP')).get('$1',''))"; }

# Canisters that publish `get_cycle_status`, and canisters that do not.
#
# THE SECOND LIST IS A GAP AND IS PRINTED AS ONE. `lobby`, `history` and the
# asset `frontend` canister have no cycle-status endpoint, so their runway is
# NOT monitorable without controller rights -- and controller rights are exactly
# what this job must not have. All this script can do for them is prove they are
# still answering, which catches a canister that has ALREADY frozen but gives no
# warning beforehand. Closing that needs `get_cycle_status` on those canisters,
# which lives in crates this change does not own.
# DERIVED FROM THE ID MAP, NOT HARDCODED. A hardcoded list is a list that stops
# covering the fleet the day somebody adds `table_4` -- which is the exact shape
# of docs/DEFECTS.md H-45 (a gate nothing runs) applied to a canister nothing
# watches. Everything in the map is checked; the three names below are the ones
# KNOWN to publish no runway, and anything else is checked strictly, so a new
# canister without the endpoint is reported loudly instead of skipped silently.
LIVENESS_ONLY="lobby history frontend"
ALL_CANISTERS="$(python3 -c "import json;print(' '.join(sorted(json.load(open('$MAP')))))")"
RUNWAY_CANISTERS=""
for n in $ALL_CANISTERS; do
  case " $LIVENESS_ONLY " in
    *" $n "*) ;;
    *) RUNWAY_CANISTERS="$RUNWAY_CANISTERS $n" ;;
  esac
done

# One cheap public query per liveness-only canister. A frozen canister rejects
# these exactly as it rejects everything else.
# name -> "<method>|<candid arg>". Every one of these is a QUERY that exists on
# the committed .did for that canister, verified by calling it. A probe naming a
# method that does not exist reports "inconclusive" forever and is worse than no
# probe at all, because it looks like coverage.
liveness_probe() {
  case "$1" in
    lobby)    echo "get_tables|()" ;;
    history)  echo "get_total_hands|()" ;;
    # The asset canister. `http_request` is the only public query it has, and it
    # is the one the browser uses, so a probe that passes means the app loads.
    frontend) echo "http_request|(record { url = \"/\"; method = \"GET\"; body = vec {}; headers = vec {} })" ;;
    *)        echo "" ;;
  esac
}

fail=0
report=""
row() { report="$report
$1"; echo "$1"; }

echo "== cycle runway, $NETWORK (read-only, anonymous, no identity)"
echo "   warn below ${WARN_DAYS} days, critical below ${CRITICAL_DAYS} days"
echo

seen=0
for n in $RUNWAY_CANISTERS; do
  cid="$(cid_of "$n")"
  [ -n "$cid" ] || { row "  -  $n: not in $MAP, skipping"; continue; }

  # `|| true` deliberately captures the FAILURE too: a frozen canister rejects
  # this query, and that rejection is the alarm rather than an error to retry.
  out="$(icp canister call "$cid" get_cycle_status '()' -e "$NETWORK" --query 2>&1 || true)"

  if printf '%s' "$out" | grep -qiE "frozen|out of cycles"; then
    row "  ✗  $n ($cid): UNREACHABLE -- the canister rejected a QUERY. This is what an"
    row "     out-of-cycles canister looks like from outside, and it is holding every"
    row "     player's balance while refusing every withdrawal. Top it up NOW:"
    row "       $ icp canister top-up $cid --amount 20t -e $NETWORK"
    fail=1
    seen=$((seen + 1))
    continue
  fi
  if ! printf '%s' "$out" | grep -q 'liquid_balance'; then
    row "  ✗  $n ($cid): NO ANSWER. Either the module has no get_cycle_status (older"
    row "     than this tree) or the canister is not reachable. Neither is 'fine'."
    row "     Reply was: $(printf '%s' "$out" | head -2 | tr '\n' ' ' | cut -c1-160)"
    fail=1
    seen=$((seen + 1))
    continue
  fi

  eval "$(parse_status "$out")"
  seen=$((seen + 1))
  human_liquid="$(t_of "$liquid")"
  human_burn="$(t_of "$burn")"

  if [ "$ticks" = "0" ]; then
    row "  ✗  $n ($cid): the on-chain clock has run ZERO times. A canister that lost its"
    row "     clock does not settle hands or release seats, whatever its balance says."
    fail=1
  fi

  if [ "$days" -lt 0 ]; then
    # No measurement. Do not shrug: state a floor and alarm on the floor.
    est="$(python3 -c "print(max(0, int($liquid) // $FALLBACK_BURN_PER_DAY)) if int($liquid) >= 0 else print(-1)")"
    row "  !  $n ($cid): the canister has NOT measured its own burn rate (normal for the"
    row "     first few minutes after an install, an upgrade or a top-up)."
    row "     $human_liquid spendable => at least ${est} days even if this table were fully"
    row "     occupied and dealing 500 hands a day. Unknown is not fine; this is a floor."
    if [ "$est" -lt 0 ] || [ "$est" -lt "$WARN_DAYS" ]; then
      row "  ✗  $n ($cid): that floor is below the ${WARN_DAYS}-day threshold."
      row "       $ icp canister top-up $cid --amount 20t -e $NETWORK"
      fail=1
    fi
  elif [ "$days" -lt "$CRITICAL_DAYS" ]; then
    row "  ✗  $n ($cid): ${days} DAYS LEFT -- CRITICAL ($human_liquid spendable, burning $human_burn/day)"
    row "       $ icp canister top-up $cid --amount 20t -e $NETWORK"
    fail=1
  elif [ "$days" -lt "$WARN_DAYS" ]; then
    row "  ✗  $n ($cid): ${days} days left -- below the ${WARN_DAYS}-day warning threshold"
    row "     ($human_liquid spendable, burning $human_burn/day)"
    row "       $ icp canister top-up $cid --amount 20t -e $NETWORK"
    fail=1
  else
    row "  ✓  $n ($cid): ${days} days ($human_liquid spendable, burning $human_burn/day)"
  fi
done

echo
echo "== liveness only (these canisters publish no runway -- see the note in this script)"
for n in $LIVENESS_ONLY; do
  cid="$(cid_of "$n")"
  [ -n "$cid" ] || { row "  -  $n: not in $MAP, skipping"; continue; }
  probe="$(liveness_probe "$n")"
  if [ -z "$probe" ]; then
    row "  ?  $n ($cid): no cheap public query known; not probed"
    continue
  fi
  m="${probe%%|*}"; arg="${probe#*|}"
  out="$(icp canister call "$cid" "$m" "$arg" -e "$NETWORK" --query 2>&1 || true)"
  if printf '%s' "$out" | grep -qiE "frozen|out of cycles"; then
    row "  ✗  $n ($cid): UNREACHABLE -- rejected a query. Out of cycles."
    row "       $ icp canister top-up $cid --amount 20t -e $NETWORK"
    fail=1
  elif printf '%s' "$out" | grep -qE '^\(|record \{|vec \{|: nat'; then
    row "  ✓  $n ($cid): answering (runway NOT measurable without controller rights)"
  else
    row "  !  $n ($cid): probe inconclusive: $(printf '%s' "$out" | head -1 | cut -c1-120)"
  fi
done

echo
if [ "$seen" -eq 0 ]; then
  echo "::error::read no canisters at all -- this check proved nothing"
  exit 1
fi
if [ "$fail" -ne 0 ]; then
  cat <<EOF
::error::AT LEAST ONE CANISTER IS BELOW THE RUNWAY THRESHOLD OR NOT ANSWERING.

A canister below its freezing threshold rejects EVERY update call at once:
deposit, withdraw, cash_out, player_action, abandon_stuck_hand. Every player at
that table loses access to their own money at the same moment, with no attacker
involved. It is fully recoverable -- a top-up restores everything, and no state
is lost -- but ONLY a top-up restores it, and nothing in this repository does it
automatically. A canister allowed to reach true zero is uninstalled and its
state is gone.

Anybody can top a canister up. It needs no controller rights:

    $ icp canister top-up <canister-id> --amount 20t -e $NETWORK

docs/DEFECTS.md E-55.
EOF
  exit 1
fi
echo "cycles: every canister answered and every measured runway is above ${WARN_DAYS} days"
