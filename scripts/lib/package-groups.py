#!/usr/bin/env python3
"""Group the canisters in icp.yaml by the Rust package they are built from.

Canisters built from one package differ only in init_args, and init args are
install-time arguments -- they are not part of the module. So every canister in a
group MUST report the same module hash. scripts/check-fleet-coherence.sh uses this
grouping to assert exactly that.

Reads:  icp.yaml (for name -> package), and an id-mapping JSON (for which canisters
        actually exist on the target network).
Writes: one line per group on stdout, "<package>\t<name> <name> ...", sorted.

Canisters with no `package:` (the asset canister) are omitted -- there is nothing to
compare them to.

    ./scripts/lib/package-groups.py .icp/data/mappings/ic.ids.json
    ./scripts/lib/package-groups.py --self-test
"""
import json
import re
import sys

# "  - name: X" at list-item indentation, up to the next list item at the same level.
CANISTER_BLOCK = re.compile(r"\n  - name:\s*(\S+)(.*?)(?=\n  - name:|\Z)", re.S)
PACKAGE_LINE = re.compile(r"^\s*package:\s*(\S+)\s*$", re.M)


def group_by_package(manifest_text, known_names):
    """-> {package: [canister_name, ...]}. Pure; takes text, returns a new dict."""
    groups = {}
    for name, body in CANISTER_BLOCK.findall(manifest_text):
        if name not in known_names:
            continue
        match = PACKAGE_LINE.search(body)
        if not match:
            continue
        groups.setdefault(match.group(1), []).append(name)
    return {pkg: sorted(names) for pkg, names in groups.items()}


def self_test():
    """The parser has to survive a manifest it has not seen. Prove it on fixtures."""
    manifest = """
canisters:
  - name: lobby
    recipe:
      configuration:
        package: lobby_canister
  - name: table_1 # a trailing comment must not become part of the name
    recipe:
      configuration:
        package: table_canister
    init_args:
      value: '(record { small_blind = 1000000 : nat64 })'
  - name: table_2
    recipe:
      configuration:
        package: table_canister
  - name: frontend
    recipe:
      configuration:
        dir: src/cleardeck_frontend/dist
environments:
  - name: local
    network: local
"""
    cases = [
        (
            "groups by package, ignores the asset canister and the environments list",
            {"lobby", "table_1", "table_2", "frontend"},
            {"lobby_canister": ["lobby"], "table_canister": ["table_1", "table_2"]},
        ),
        (
            "a canister absent from the id map is not reported as deployed",
            {"lobby", "table_1"},
            {"lobby_canister": ["lobby"], "table_canister": ["table_1"]},
        ),
        ("an empty id map yields no groups", set(), {}),
    ]
    failures = []
    for label, names, expected in cases:
        got = group_by_package(manifest, names)
        if got != expected:
            failures.append("  FAIL %s\n    expected %s\n    got      %s" % (label, expected, got))
        else:
            print("  ok   %s" % label)

    # A parser that finds nothing in the REAL manifest is the failure mode that
    # matters: it makes the gate above it pass while measuring nothing.
    try:
        real = open("icp.yaml").read()
    except OSError as exc:
        failures.append("  FAIL could not read the real icp.yaml: %s" % exc)
    else:
        every_name = set(re.findall(r"\n  - name:\s*(\S+)", real))
        real_groups = group_by_package(real, every_name)
        if not real_groups:
            failures.append("  FAIL the real icp.yaml parsed to zero groups")
        elif len(real_groups.get("table_canister", [])) < 2:
            failures.append(
                "  FAIL the real icp.yaml should group at least 2 tables under "
                "table_canister, got %r" % (real_groups.get("table_canister"),)
            )
        else:
            print("  ok   the real icp.yaml groups %d package(s), %d table(s)"
                  % (len(real_groups), len(real_groups["table_canister"])))

    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("package-groups self-test: all checks passed")
    return 0


def main(argv):
    if len(argv) == 2 and argv[1] == "--self-test":
        return self_test()
    if len(argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2

    with open(argv[1]) as handle:
        known = set(json.load(handle))
    groups = group_by_package(open("icp.yaml").read(), known)
    for package in sorted(groups):
        print("%s\t%s" % (package, " ".join(groups[package])))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
