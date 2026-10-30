#!/usr/bin/env bash
# Regenerates the C++-derived fixtures from the unmodified C++ sources:
#   fixtures/payload_vectors.tsv   <- src/tradelayer/{createpayload,varint}.cpp
#   fixtures/rules_vectors.tsv     <- src/tradelayer/rules.cpp (IsFeatureActivated, ActivateFeature,
#                                     DeactivateFeature, IsTransactionTypeAllowed)
# Run inside WSL/Linux with g++. No node build, no boost, no Bitcoin Core headers needed.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
tmp="${TMPDIR:-/tmp}/tl_core_ref_build"
mkdir -p "$tmp/gen/tradelayer"

g++ -std=c++17 -O0 -Wall -I"$here/shim" -I"$here/../../../src" "$here/vectors.cpp" -o "$tmp/vectors"
"$tmp/vectors" > "$here/../fixtures/payload_vectors.tsv"
echo "wrote fixtures/payload_vectors.tsv ($(wc -l < "$here/../fixtures/payload_vectors.tsv") lines)"

# rules.cpp needs only the TransactionType enum and the MP_TX_PKT_V* constants from tradelayer.h,
# which we cut out of the real header so they cannot drift.
tl_h="$here/../../../src/tradelayer/tradelayer.h"
{
  echo '#pragma once'
  echo '#include <stdint.h>'
  sed -n '/^enum TransactionType/,/^};/p' "$tl_h"
  grep -E '^const uint16_t +MP_TX_PKT_V[01]' "$tl_h"
} > "$tmp/gen/tradelayer/tradelayer.h"

g++ -std=c++17 -O0 -Wall -Wno-unused-function -I"$tmp/gen" -I"$here/shim_rules" -I"$here/../../../src" \
    "$here/rules_vectors.cpp" -o "$tmp/rules_vectors"
"$tmp/rules_vectors" > "$here/../fixtures/rules_vectors.tsv"
echo "wrote fixtures/rules_vectors.tsv ($(wc -l < "$here/../fixtures/rules_vectors.tsv") lines)"
