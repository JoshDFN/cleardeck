#!/usr/bin/env bash
# CAN THIS FILE DEPLOY, OR AUTHENTICATE AS SOMEBODY WHO CAN?
#
# WHY THIS EXISTS
#   `.github/workflows/cycles-monitor.yml` and `scripts/cycles-runway.sh` claim
#   to be read-only and credential-free. A claim like that written in a comment
#   is worth nothing: the version of cycles-monitor.yml that this replaced ALSO
#   described itself as a read-only check, in its second line, while
#   base64-decoding `secrets.IC_DEPLOY_IDENTITY` into /tmp and making it the
#   default identity -- every day, on a schedule, to read one number. A key that
#   can `install_code` on a canister custodying real ICP was sitting in a
#   scheduled job for the sake of a status line
#   (docs/SECURITY-FINDINGS.md FINDING 23).
#
#   So the claim is enforced instead of asserted.
#
# WHY IT IS A SEPARATE FILE
#   The guard has to name the patterns it forbids, so a guard that scans itself
#   always fails. The previous attempt at this lived inline in the workflow and
#   flagged (a) its own regex and (b) the ADVICE text telling a human how to
#   remedy the alarm. Both are real problems and both were solved by hand with
#   escaping tricks that the next reader would have to re-derive. This file is
#   never in its own argument list, so neither problem exists.
#
# WHAT COUNTS AS ADVICE RATHER THAN A COMMAND
#   A line whose command is prefixed with `$ ` is a command SHOWN TO A PERSON, not
#   one this repository runs -- `$ icp canister top-up ...` is not executable as
#   written. Those are stripped. Everything else is treated as executable.
#
# USAGE
#   ./scripts/assert-read-only.sh <file>...
#   ./scripts/assert-read-only.sh --selftest
set -euo pipefail

# Mutating `icp` subcommands. Every one of these changes a canister or the fleet.
#
# WIDENED, wave-13 reconciliation. The first version listed the verbs that had
# actually been abused, which is a list of yesterday's incidents rather than a
# rule: `icp canister create`, `migrate-id`, `uninstall` and `update-settings`
# all passed it, and a review demonstrated each one going green through this
# guard. The rule is "anything that is not a read", so the deny list is now the
# whole write half of the CLI surface this repo touches.
MUTATING='icp +(deploy|install|build)|icp +canister +(install|reinstall|uninstall|create|delete|stop|start|top-up|snapshot|settings|update-settings|migrate-id|import|sign|send)|icp +identity +(new|import|use|export)|icp +cycles'
# Anything that carries or installs a credential. Assembled from adjacent string
# literals so this line does not match itself.
#
# WIDENED for the same reason: it matched `secrets.IC_` only, so a workflow
# carrying `secrets.DEPLOY_KEY_PEM_BLOB` was reported "read-only, anonymous".
# A scheduled job that reads a PUBLIC query needs NO secret of any name, so the
# rule is now every `secrets.` reference, every identity flag and every identity
# subcommand. If a future reader genuinely needs one, they have to change this
# line, in a file whose whole subject is that they should not.
CREDENTIAL="secre""ts\.|identity imp""ort|identity us""e|--iden""tity|\.pe""m|IC_DEPLOY_IDEN""TITY|priv""ate.?key"

strip() {
  # 1. comments; 2. advice lines (`$ cmd`, shown to a human, not run).
  sed 's/#.*//' "$1" | sed 's/\$ [a-z].*//'
}

selftest() {
  echo "== selftest: can this guard see the things it forbids?"
  local fail=0 probe
  probe="$(mktemp)"
  # Assembled from parts so the literals never appear in this file.
  {
    printf '%s %s -e ic lobby\n' 'icp' 'deploy'
    printf 'icp canister %s table_1\n' 'install'
    printf 'icp canister call x foo "()"\n'
    printf '%s%s\n' '--iden' 'tity monitor'
    printf '%s%s\n' 'secrets.IC_' 'DEPLOY_IDENTITY'
    printf 'cat /tmp/monitor%s%s\n' '.pe' 'm'
    # The four shapes a wave-13 review drove straight through this guard.
    printf 'icp canister %s new_table\n' 'create'
    printf 'icp canister %s table_1 --to abc\n' 'migrate-id'
    printf 'icp identity %s monitor\n' 'use'
    printf '%s%s\n' 'secrets.' 'DEPLOY_KEY_PEM_BLOB'
  } > "$probe"
  local hits
  hits="$(grep -cE "$MUTATING" "$probe" || true)"
  [ "$hits" -ge 5 ] || { echo "   ✗ the mutating pattern found $hits of 5"; fail=1; }
  hits="$(grep -cE "$CREDENTIAL" "$probe" || true)"
  [ "$hits" -ge 5 ] || { echo "   ✗ the credential pattern found $hits of 5"; fail=1; }
  grep -q 'icp canister call' "$probe" || { echo "   ✗ cannot see a canister call"; fail=1; }
  # And the advice stripper must NOT hide a real command.
  local adv; adv="$(mktemp)"
  printf '    $ icp canister top-up abc --amount 20t\n    icp canister install abc\n' > "$adv"
  local left; left="$(strip "$adv")"
  printf '%s' "$left" | grep -q 'top-up' && { echo "   ✗ advice was not stripped"; fail=1; }
  printf '%s' "$left" | grep -q 'install' || { echo "   ✗ a REAL command was stripped as advice"; fail=1; }
  rm -f "$probe" "$adv"
  [ "$fail" -eq 0 ] && echo "   selftest passed"
  return "$fail"
}

if [ "${1:-}" = "--selftest" ]; then
  selftest
  exit $?
fi
[ $# -gt 0 ] || { echo "usage: $0 <file>... | --selftest" >&2; exit 2; }

cd "$(dirname "$0")/.."
selftest || exit 1
echo
echo "== read-only check"
fail=0
for f in "$@"; do
  [ -f "$f" ] || { echo "::error::$f does not exist"; fail=1; continue; }
  s="$(strip "$f")"
  if printf '%s' "$s" | grep -nE "$MUTATING"; then
    echo "::error::$f contains a mutating icp command"
    fail=1
  fi
  if printf '%s' "$s" | grep -n 'icp canister call' | grep -v -- '--query'; then
    echo "::error::$f makes a canister call that is not a query"
    fail=1
  fi
  if printf '%s' "$s" | grep -nE "$CREDENTIAL"; then
    echo "::error::$f references an identity or a secret. This job reads a PUBLIC query;"
    echo "::error::the point is that it cannot deploy even if the runner were compromised."
    fail=1
  fi
  [ "$fail" -eq 0 ] && echo "   ok  $f: read-only, anonymous"
done
[ "$fail" -eq 0 ] || exit 1
echo
echo "read-only: nothing here can deploy, and nothing here holds a credential"
