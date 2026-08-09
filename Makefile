# ClearDeck. One place to run everything.
#
# All logic lives in scripts/dev.sh; this file is the discoverable front door so
# `make help` works and nobody has to remember which harness lives where.
#
# LOCAL REPLICA ONLY. The mainnet canisters custody real ICP and ckBTC; every
# icp invocation in scripts/dev.sh refuses `-e ic` and refuses any argument
# naming a canister listed in .icp/data/mappings/ic.ids.json.

DEV := ./scripts/dev.sh

.DEFAULT_GOAL := help
.PHONY: help doctor local-up local-status wasm test custody archive fuzz \
        fuzz-default diff-full diff-full-sevens settlement settlement-fast \
        shots shots-verdict shots-selftest cycles known-defects hygiene selftest \
        no-peeking phe-venv check declarations deployed-config suite-wiring

help:            ## show this help
	@$(DEV) help

doctor:          ## read-only: toolchain, replica liveness, deployed ids, wasm identity
	@$(DEV) doctor

local-status: doctor

local-up:        ## bring the local stack up end to end (idempotent)
	@$(DEV) local-up $(ARGS)

wasm:            ## build table_canister.wasm and print its sha256
	@$(DEV) wasm

test:            ## FAST gate: workspace + differential fast + money-safety fast. No replica needed.
	@$(DEV) test

custody:         ## THE CONTROLLER SEAT with the transcript (SECURITY-FINDINGS FINDING 23). In `test` too.
	@$(DEV) custody

archive:         ## the offline archive analyser's own 39 gates (DEFECTS H-50). No replica. In `test` too.
	@$(DEV) archive

no-peeking:      ## the sealed-dealer spike's own 36 tests (DEFECTS H-53). A spike; in `test` too.
	@$(DEV) no-peeking

fuzz-default:    ## the fuzzer with NO environment: its own default seeds and steps (DEFECTS H-28)
	@$(DEV) fuzz-default

fuzz:            ## LONG: fuzz-default, then 9 seeds x 600 hostile steps vs the real canister + real ICP ledger
	@$(DEV) fuzz

settlement:      ## independent settlement oracle: what each seat is OWED vs what the canister paid
	@$(DEV) settlement $(ARGS)

settlement-fast: ## settlement oracle without the PocketIC runs (rules + golden reproducers)
	@$(DEV) settlement fast

diff-full:       ## exhaustive evaluator differential (all C(52,5) x 3 evaluators)
	@$(DEV) diff-full $(ARGS)

diff-full-sevens: ## diff-full plus all C(52,7) seven-card hands (~20 min more)
	@$(DEV) diff-full --exhaustive-sevens

shots:           ## screenshot the real UI against the real local canisters (needs local-up)
	@$(DEV) shots $(ARGS)

shots-verdict:   ## the LAST RECORDED sweep's verdict as a gate: no red may be unacknowledged. No replica.
	@$(DEV) shots-verdict

shots-selftest:  ## the screenshot harness's own gates on known-answer fixtures. No replica.
	@$(DEV) shots-selftest

cycles:          ## how long before a canister stops honouring withdrawals (DEFECTS E-55). Read-only, no identity.
	@$(DEV) cycles $(ARGS)

known-defects:   ## run the markers that are RED on purpose; shouts when one gets fixed
	@$(DEV) known-defects

declarations:    ## the frontend's Candid bindings must regenerate to what is committed (DEFECTS D-11)
	@./scripts/check-declarations-js.sh $(ARGS)

deployed-config: ## live TableConfig vs icp.yaml, and every lobby row vs its contract (DEFECTS L-04, H-55)
	@./scripts/check-deployed-config.sh --selftest --network local $(ARGS)

suite-wiring:    ## every cargo test target on disk is run by something (DEFECTS H-45)
	@./scripts/check-suite-wiring.sh

hygiene:         ## no large/binary files added; player-protection notices intact
	@$(DEV) hygiene

selftest:        ## prove the mainnet guard refuses every hostile argument shape
	@$(DEV) selftest

phe-venv:        ## install the third reference evaluator (phevaluator)
	@$(DEV) phe-venv

check: selftest hygiene test ## what CI should run: guard + hygiene + the fast gate
