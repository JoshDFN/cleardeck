#!/usr/bin/env bash
# Fetch the pinned REAL mainnet ICP ledger wasm for the money-safety harness.
#
# The harness does this itself on first use; this script exists so the fetch can
# be done deliberately (offline CI, air-gapped review, or just to inspect the
# module before running anything against it).
#
# download.dfinity.systems/ic/<commit>/... is immutable per commit. The commit is
# the one the OISY wallet pins for its ICP ledger, i.e. a build in real use.
set -euo pipefail

COMMIT="6dcfafb491092704d374317d9a72a7ad2475d7c9"
URL="https://download.dfinity.systems/ic/${COMMIT}/canisters/ledger-canister.wasm.gz"
SHA256="a47a915ea5f62bb74d91259f866111158b8f9b7c04b715942532bc42453866ec"

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
out_dir="${here}/../../target/money-safety"
out="${out_dir}/ledger-canister.wasm.gz"

mkdir -p "${out_dir}"

if [ -f "${out}" ] && [ "$(shasum -a 256 "${out}" | cut -d' ' -f1)" = "${SHA256}" ]; then
  echo "already present and verified: ${out}"
  exit 0
fi

echo "fetching ${URL}"
curl -sSL --fail --max-time 180 -o "${out}.tmp" "${URL}"

got="$(shasum -a 256 "${out}.tmp" | cut -d' ' -f1)"
if [ "${got}" != "${SHA256}" ]; then
  rm -f "${out}.tmp"
  echo "sha256 MISMATCH: expected ${SHA256}, got ${got}" >&2
  echo "Refusing to install an unidentified ledger: it would invalidate every M2 result." >&2
  exit 1
fi

mv "${out}.tmp" "${out}"
echo "verified ${SHA256}"
echo "wrote ${out}"
