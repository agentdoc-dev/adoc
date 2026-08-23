#!/usr/bin/env sh
# AgentDoc Validation Runtime harness (E1.7.T2; SEMANTICS §S6).
#
# Reproducible invocation of the SAME AgentDoc-domain validation through a
# checksum-pinned adoc binary in an isolated working directory. The pinned
# binary is the one authoritative runtime (MILESTONES §E1.7): this harness
#   1. locates (or builds) the adoc binary,
#   2. verifies its sha256 against the recorded pin — fail closed, no pin
#      or a mismatch refuses to invoke,
#   3. runs the committed domain fixture through the pinned binary, and
#   4. compares the emitted adoc.validation_receipt.v0 byte-for-byte with
#      the committed golden receipt, then proves determinism by comparing
#      two further pinned invocations byte-for-byte.
#
# The runtime binary digest inside the receipt is an explicit attested
# input supplied by this harness (a binary cannot hash itself
# deterministically): the golden run supplies the documented
# GOLDEN_RUNTIME_DIGEST constant — the same constant the direct
# library-path test uses (crates/adoc-cli/tests/check_cli.rs), so the
# byte-for-byte golden match proves library-path/packaged-binary receipt
# parity; the pinned runs supply the verified pin and assert it is
# recorded end-to-end.
#
# Usage:
#   scripts/validation-runtime/run.sh pin   # build + record the pin
#   scripts/validation-runtime/run.sh run   # verify pin, prove parity
#
# ponytail: the pin is recorded per-build (`pin` at packaging time) until a
# released binary artifact exists to pin at release time; the enforcement
# path — no invocation without a verified pin — is what ships here and is
# tamper-tested in CI.

set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
REPO_ROOT=$(CDPATH= cd -- "$SCRIPT_DIR/../.." && pwd)
BIN=${ADOC_BIN:-"$REPO_ROOT/target/debug/adoc"}
PIN_FILE="$SCRIPT_DIR/adoc.sha256"
FIXTURE_DIR="$SCRIPT_DIR/fixture"
GOLDEN_RECEIPT="$SCRIPT_DIR/golden/validation_receipt.golden.json"

# sha256("adoc-validation-runtime-golden") — keep in sync with
# GOLDEN_RUNTIME_DIGEST in crates/adoc-cli/tests/check_cli.rs.
GOLDEN_RUNTIME_DIGEST="sha256:ca1bf018dc0b72ee1197d9d521d96d227cd3e54cc81528ea5f45776c99d95f4d"
EVALUATION_DATE="2026-01-01"

die() {
    echo "validation-runtime: $*" >&2
    exit 1
}

sha256_of() {
    if command -v sha256sum > /dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

ensure_binary() {
    if [ ! -x "$BIN" ]; then
        [ -n "${ADOC_BIN:-}" ] && die "ADOC_BIN '$BIN' is not an executable"
        (cd "$REPO_ROOT" && cargo build -p adoc-cli --locked)
    fi
    [ -x "$BIN" ] || die "adoc binary not found at '$BIN'"
}

emit_receipt() {
    # $1 = output path, $2 = attested runtime binary digest
    (cd "$FIXTURE_DIR" && "$BIN" check \
        --receipt "$1" \
        --as-of "$EVALUATION_DATE" \
        --runtime-binary-digest "$2") > /dev/null
}

cmd=${1:-run}
case "$cmd" in
    pin)
        ensure_binary
        sha256_of "$BIN" > "$PIN_FILE"
        echo "validation-runtime: recorded pin $(cat "$PIN_FILE") for $BIN"
        ;;
    run)
        ensure_binary
        [ -f "$PIN_FILE" ] || die "no recorded pin at '$PIN_FILE'; refusing to invoke (record one with 'run.sh pin' at packaging time)"
        pin=$(cat "$PIN_FILE")
        actual=$(sha256_of "$BIN")
        [ "$actual" = "$pin" ] || die "pin mismatch for '$BIN': pinned $pin, actual $actual; refusing to invoke"

        workdir=$(mktemp -d)
        trap 'rm -rf "$workdir"' EXIT

        # Golden parity: packaged binary vs the committed golden the direct
        # library-path test also pins — byte-identical or fail.
        emit_receipt "$workdir/golden-run.json" "$GOLDEN_RUNTIME_DIGEST"
        cmp -s "$workdir/golden-run.json" "$GOLDEN_RECEIPT" \
            || die "receipt diverged from committed golden '$GOLDEN_RECEIPT' (diff: $(diff "$GOLDEN_RECEIPT" "$workdir/golden-run.json" || true))"

        # Pinned determinism: two invocations attesting the verified pin
        # are byte-identical and record the pin end-to-end.
        emit_receipt "$workdir/pinned-1.json" "sha256:$pin"
        emit_receipt "$workdir/pinned-2.json" "sha256:$pin"
        cmp -s "$workdir/pinned-1.json" "$workdir/pinned-2.json" \
            || die "pinned receipts are not byte-identical across invocations"
        grep -q "sha256:$pin" "$workdir/pinned-1.json" \
            || die "verified pin is not recorded in the receipt"

        echo "validation-runtime: pin verified, golden parity and determinism proven"
        ;;
    *)
        die "unknown command '$cmd' (expected 'pin' or 'run')"
        ;;
esac
