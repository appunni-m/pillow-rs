#!/bin/bash
# ============================================================================
# Canonical entry point: make quality owns the checks and their order.
# The Makefile calls this script with --supplemental for repository-specific
# source, binding, and optional supply-chain checks. Direct use delegates back
# to the same Make target.
# ============================================================================
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ "${1:-}" != "--supplemental" ]]; then
    exec make -C "$ROOT" quality "$@"
fi

PYTHON="${PYTHON:-python3}"
CARGO="${CARGO:-cargo}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

failures=0

check() {
    local name="$1"; shift
    echo ""
    echo -e "${YELLOW}━━━ ${name} ━━━${NC}"
    if "$@"; then
        echo -e "${GREEN}✅ ${name} passed${NC}"
    else
        echo -e "${RED}❌ ${name} FAILED${NC}"
        failures=$((failures + 1))
    fi
}

# ── Project-specific source contracts ────────────────────────────────────────
# Compare per-file production findings with the commit supplied by CI (or HEAD
# for a local dirty worktree). This keeps existing migrations visible while
# preventing new raw image constructors, encoded mode flags, and String errors.
check "Rust source-contract ratchets" "$PYTHON" scripts/check_source_contracts.py \
    --baseline-ref "${SOURCE_CONTRACT_BASE_REF:-HEAD}"
check "backend-boundary checker contracts" "$PYTHON" -m unittest discover \
    -s scripts -p 'test_check_backend_boundaries.py' -v
check "backend feature and Rayon boundaries" "$PYTHON" scripts/check_backend_boundaries.py

# ── Rust: supply chain (advisories, duplicate deps, licenses, sources) ──
echo ""
echo "=== cargo-deny ==="
if command -v cargo-deny &>/dev/null; then
    check "advisories (CVEs)" "$CARGO" deny check advisories
    check "bans (duplicate deps)" "$CARGO" deny check bans
    check "licenses" "$CARGO" deny check licenses
    check "sources" "$CARGO" deny check sources
else
    echo -e "${YELLOW}⚠️  cargo-deny not installed — skipping${NC}"
    echo "   Install: cargo install cargo-deny"
fi

# ── Rust: vulnerability advisory DB ──
echo ""
echo "=== cargo-audit ==="
if command -v cargo-audit &>/dev/null; then
    check "RUSTSEC advisories" "$CARGO" audit
else
    echo -e "${YELLOW}⚠️  cargo-audit not installed — skipping${NC}"
    echo "   Install: cargo install cargo-audit"
fi

# ── Python: binding-layer rule enforcement (AST-based, no loops/arithmetic/logic) ──
check "Python binding rules" "$PYTHON" scripts/check_bindings.py

# ── Python: migration parity specification checks ──
echo ""
echo "=== migration parity specification ==="

check "migration parity inputs" "$PYTHON" scripts/check_migration_parity_inputs.py
check "migration parity inventory" "$PYTHON" scripts/migration_parity_inventory.py --format check
check "migration parity contract" "$PYTHON" scripts/validate_migration_parity_contract.py \
	--manifest pillow-rs/tests/fixtures/manifest.yaml

# ── Summary ──
echo ""
if [ "$failures" -eq 0 ]; then
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${GREEN}  All checks passed.${NC}"
    echo -e "${GREEN}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    exit 0
else
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${RED}  ${failures} check(s) FAILED.${NC}"
    echo -e "${RED}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    exit 1
fi
