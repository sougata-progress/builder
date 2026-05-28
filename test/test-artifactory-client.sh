#!/usr/bin/env bash
# test/test-artifactory-client.sh
#
# Self-contained local test script for components/artifactory-client.
# No Habitat packages, no network access, no Docker required.
#
# Usage:
#   bash test/test-artifactory-client.sh            # normal
#   bash test/test-artifactory-client.sh --verbose  # print test output
#
# Exit codes:
#   0  all checks passed
#   1  one or more checks failed
#
# Requirements:
#   - Rust toolchain matching rust-toolchain (cargo, rustfmt, clippy)
#   - Run from the repository root

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CRATE="artifactory-client"
CRATE_PATH="$REPO_ROOT/components/$CRATE"
VERBOSE=false

if [[ "${1:-}" == "--verbose" ]]; then
    VERBOSE=true
fi

# ── Helpers ──────────────────────────────────────────────────────────────────

pass() { echo "  [PASS] $*"; }
fail() { echo "  [FAIL] $*"; FAILURES+=("$*"); }

FAILURES=()

run_check() {
    local label="$1"; shift
    echo ""
    echo "==> $label"
    if $VERBOSE; then
        "$@" && pass "$label" || fail "$label"
    else
        local out
        if out=$("$@" 2>&1); then
            pass "$label"
        else
            echo "$out"
            fail "$label"
        fi
    fi
}

# ── Pre-flight ────────────────────────────────────────────────────────────────

cd "$REPO_ROOT"

echo "======================================================"
echo " artifactory-client local test suite"
echo " Repo:    $REPO_ROOT"
echo " Crate:   $CRATE_PATH"
echo " Toolchain: $(cat rust-toolchain | grep channel | cut -d'"' -f2 2>/dev/null || cat rust-toolchain)"
echo "======================================================"

# Abort immediately if the toolchain is not installed rather than giving a
# confusing cargo error later.
if ! cargo --version &>/dev/null; then
    echo "[ERROR] cargo not found. Install the Rust toolchain first."
    exit 1
fi

# ── Checks ────────────────────────────────────────────────────────────────────

# 1. Format check (read-only — does not reformat files)
run_check "Format check (cargo fmt)" \
    cargo fmt -p "$CRATE" -- --check

# 2. Build (debug, no tests)
run_check "Build (cargo build)" \
    cargo build -p "$CRATE"

# 3. Unit tests
if $VERBOSE; then
    run_check "Unit tests (cargo test --lib)" \
        cargo test -p "$CRATE" --lib -- --nocapture
else
    run_check "Unit tests (cargo test --lib)" \
        cargo test -p "$CRATE" --lib
fi

# 4. Clippy — deny all warnings (includes pedantic lints declared in lib.rs
#    via #![warn(clippy::pedantic)]; the pedantic attribute applies only to this
#    crate's source, so dependency warnings do not bleed in).
run_check "Lint (cargo clippy)" \
    cargo clippy -p "$CRATE" -- -D warnings

# ── Summary ───────────────────────────────────────────────────────────────────

echo ""
echo "======================================================"
if [[ ${#FAILURES[@]} -eq 0 ]]; then
    echo " All checks passed."
    echo "======================================================"
    exit 0
else
    echo " ${#FAILURES[@]} check(s) FAILED:"
    for f in "${FAILURES[@]}"; do
        echo "   - $f"
    done
    echo "======================================================"
    exit 1
fi
