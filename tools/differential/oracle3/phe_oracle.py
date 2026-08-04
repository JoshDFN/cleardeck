#!/usr/bin/env python3
"""Third reference evaluator, used only as an ADJUDICATOR.

Reads one hand per line on stdin ("Ah Kh Qh Jh Th"), writes one integer per line
on stdout: phevaluator's rank, where 1 is the best possible hand and 7462 the
worst. The Rust side negates it so that "greater is better" holds everywhere.

`phevaluator` is a third independent lineage (Henry Lee's perfect-hash tables,
peer-reviewed and C-backed), deliberately NOT `treys`, because reference B in
the Rust harness is already a port of treys and two ports of the same tables
would not be independent.

Install:
    python3 -m venv .venv && .venv/bin/pip install phevaluator
Then point the harness at it:
    CLEARDECK_PHE_PYTHON=$PWD/.venv/bin/python cargo run --release

Any failure prints a single line beginning with "ERROR " on stdout and exits
non-zero, so the Rust side degrades to "adjudicator unavailable" rather than
silently reporting a wrong verdict.
"""

import sys


def main() -> int:
    try:
        from phevaluator import evaluate_cards
    except Exception as exc:  # pragma: no cover - environment dependent
        sys.stdout.write(f"ERROR phevaluator import failed: {exc}\n")
        return 2

    out = []
    for line in sys.stdin:
        hand = line.split()
        if not hand:
            continue
        if len(hand) not in (5, 6, 7):
            sys.stdout.write(f"ERROR unsupported card count {len(hand)}\n")
            return 3
        try:
            out.append(str(evaluate_cards(*hand)))
        except Exception as exc:
            sys.stdout.write(f"ERROR evaluate_cards({hand}) failed: {exc}\n")
            return 4

    sys.stdout.write("\n".join(out))
    sys.stdout.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
