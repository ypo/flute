#!/usr/bin/env bash
#
# Check that the Reed-Solomon codec of this crate (src/fec/rsgf2m.rs, FEC Encoding IDs 2, 5 and 129/0)
# is compatible with two reference implementations:
#
#  * zfec    <https://github.com/tahoe-lafs/zfec>, derived from the codec of Luigi Rizzo,
#            that RFC 5510 declares to be compatible with
#  * OpenFEC <https://github.com/roc-streaming/openfec>, from INRIA (V. Roca, co-author of RFC 5510),
#            codecs "Reed-Solomon GF(2^8)" and "Reed-Solomon GF(2^m)" (m = 4 and 8)
#
# zfec and OpenFEC are downloaded and built locally, under target/rs-interop by default
# (override with RS_INTEROP_DIR). Their source code is not part of this repository nor of the crate.
#
# For each test case, rs_vectors.c writes the repair symbols computed by the reference implementations,
# then the test fec::rsgf2m::tests::test_interop_vectors checks that this crate produces the same
# repair symbols, and decodes the source block from them.
#
# Requirements: git, a C compiler, cmake, make, cargo
#
# Usage: interop/rs/run.sh

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
SCRIPT_DIR="$ROOT_DIR/interop/rs"
WORK_DIR="${RS_INTEROP_DIR:-$ROOT_DIR/target/rs-interop}"
CC="${CC:-cc}"

ZFEC_URL=https://github.com/tahoe-lafs/zfec.git
ZFEC_REV=1fede6c166592c98da7f102539f94daf99db4756
OPENFEC_URL=https://github.com/roc-streaming/openfec.git
OPENFEC_REV=e549f3b8ba54a7d6e2aaefdd1cdd4c73ef32000c

# Test cases: "m k n E"
CASES_GF_2_8=(
    "8 1 2 4"
    "8 2 255 1"
    "8 4 7 2"
    "8 10 15 16"
    "8 32 48 8"
    "8 64 128 3"
    "8 100 255 2"
    "8 200 255 4"
    "8 254 255 5"
)
CASES_GF_2_4=(
    "4 1 3 1"
    "4 2 15 1"
    "4 4 7 2"
    "4 5 15 4"
    "4 10 15 8"
    "4 14 15 3"
)

# Download a pinned revision of a git repository
fetch() {
    local dir=$1 url=$2 rev=$3
    if [[ ! -d "$dir/.git" || "$(git -C "$dir" rev-parse HEAD)" != "$rev" ]]; then
        echo "Downloading $url ($rev)"
        rm -rf "$dir"
        git init -q "$dir"
        git -C "$dir" fetch -q --depth 1 "$url" "$rev"
        git -C "$dir" -c advice.detachedHead=false checkout -q FETCH_HEAD
    fi
}

mkdir -p "$WORK_DIR"
fetch "$WORK_DIR/zfec" "$ZFEC_URL" "$ZFEC_REV"
fetch "$WORK_DIR/openfec" "$OPENFEC_URL" "$OPENFEC_REV"

echo "Building OpenFEC"
cmake -S "$WORK_DIR/openfec" -B "$WORK_DIR/openfec/build" -DCMAKE_BUILD_TYPE=Release > "$WORK_DIR/openfec-build.log"
cmake --build "$WORK_DIR/openfec/build" --target openfec --parallel >> "$WORK_DIR/openfec-build.log"
OPENFEC_LIB="$WORK_DIR/openfec/bin/Release"

echo "Building the vector generators"
"$CC" -O2 -std=c99 -Wall -DWITH_ZFEC -I"$WORK_DIR/zfec/zfec" \
    "$SCRIPT_DIR/rs_vectors.c" "$WORK_DIR/zfec/zfec/fec.c" -o "$WORK_DIR/zfec_vectors"
"$CC" -O2 -std=c99 -Wall -DWITH_OPENFEC -I"$WORK_DIR/openfec/src/lib_common" \
    "$SCRIPT_DIR/rs_vectors.c" -L"$OPENFEC_LIB" -lopenfec -Wl,-rpath,"$OPENFEC_LIB" \
    -o "$WORK_DIR/openfec_vectors"

VECTORS="$WORK_DIR/vectors.txt"
echo "Generating $VECTORS"
{
    echo "# codec m k n E esi repair_symbol"
    for case in "${CASES_GF_2_8[@]}"; do
        # shellcheck disable=SC2086
        "$WORK_DIR/zfec_vectors" zfec $case
        # shellcheck disable=SC2086
        "$WORK_DIR/openfec_vectors" openfec_rs_2_8 $case
        # shellcheck disable=SC2086
        "$WORK_DIR/openfec_vectors" openfec_rs_2_m $case
    done
    for case in "${CASES_GF_2_4[@]}"; do
        # shellcheck disable=SC2086
        "$WORK_DIR/openfec_vectors" openfec_rs_2_m $case
    done
} > "$VECTORS"

echo "Checking the vectors with this crate"
cd "$ROOT_DIR"
FLUTE_RS_INTEROP_VECTORS="$VECTORS" cargo test --lib fec::rsgf2m::tests::test_interop_vectors \
    -- --ignored --exact --nocapture
