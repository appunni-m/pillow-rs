#!/bin/bash
# ============================================================================
# AS PER DESIGN — DO NOT REMOVE CHECKS:
#   Canonical lint script for pillow-rs. Used by both local dev and CI.
#   Rust correctness:  rustfmt → clippy → tests → cargo-deny → cargo-audit
#   Python bindings:  AST-based CLAUDE.md rule check
#   Migration spec:   fixed manifest/input/result contract checks
# ============================================================================
set -e
cd "$(dirname "$0")/.."

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

# ── Rust: format ──
echo "=== rustfmt ==="
cargo fmt --check

# ── Rust: clippy (workspace lints enforce: no unwrap, no expect, no dead_code,
#     no cast_truncation, missing_docs, arithmetic_side_effects, ...) ──
echo ""
echo "=== clippy (workspace lints) ==="
cargo clippy --all-targets --all-features -- -A deprecated

# ── Rust: tests ──
echo ""
echo "=== core tests ==="
cargo test -p pillow-rs

# ── Project-specific grep enforcements ──────────────────────────────────────
# AS PER DESIGN — DO NOT REMOVE:
#   These patterns cannot be caught by clippy alone. Each prevents an entire
#   class of bugs documented in CODEBASE_AUDIT.md / SYSTEMIC_FIXES.md.
#   Allowed exceptions: checked_dims.rs, image_utils.rs, op_def.rs, tests.
# ============================================================================

grep_banned() {
    # Usage: grep_banned "check name" "pattern" "explanation"
    local name="$1" pattern="$2" explanation="$3"
    local hits
    hits=$(grep -rn "$pattern" pillow-rs/src/ \
        | grep -v 'checked_dims.rs' \
        | grep -v 'image_utils.rs' \
        | grep -v 'op_def.rs' \
        | grep -v 'handler.rs' \
        | grep -v '#\[cfg(test)\]' \
        | grep -v '#\[test\]' \
        | grep -v '//' \
        | grep -v '///' \
        || true)
    if [ -n "$hits" ]; then
        echo -e "${RED}❌ ${name} — violations:${NC}"
        echo "$hits"
        echo -e "   ${explanation}"
        return 1
    fi
    echo -e "${GREEN}✅ ${name}${NC}"
    return 0
}

echo ""
echo "=== project grep enforcements ==="

# Fix 1: No bare (w * h) as usize — must use CheckedDims::new()
grep_banned \
    "CheckedDims — no bare (w*h) as usize" \
    '(w\s*\*\s*h)\s*as\s*usize' \
    "→ Use CheckedDims::new(w, h, channels)? — see pillow-rs/src/checked_dims.rs"

# Fix 7: No duplicate raw_bytes_to_image — must use image_utils::
grep_banned \
    "no duplicate raw_bytes_to_image" \
    'ImageLuma8.*from_raw|ImageRgb8.*from_raw|ImageRgba8.*from_raw|ImageLumaA8.*from_raw' \
    "→ Use image_utils::raw_bytes_to_image() — see pillow-rs/src/image_utils.rs"

# Fix 8: No bare mode integer comparisons — must use named mode helpers
grep_banned \
    "mode helpers — no bare mode ints" \
    'has_gb = mode [>=]|has_a = mode [=|!]=' \
    "→ Use a named mode helper instead of comparing encoded integers directly"

# Fix 9: No Result<_, String> error types
grep_banned \
    "no Result<_, String>" \
    'Result<.*String>' \
    "→ Use PilError variants — see pillow-rs/src/error.rs"

# ── Rust: supply chain (advisories, duplicate deps, licenses, sources) ──
echo ""
echo "=== cargo-deny ==="
if command -v cargo-deny &>/dev/null; then
    check "advisories (CVEs)" cargo deny check advisories
    check "bans (duplicate deps)" cargo deny check bans
    check "licenses" cargo deny check licenses
    check "sources" cargo deny check sources
else
    echo -e "${YELLOW}⚠️  cargo-deny not installed — skipping${NC}"
    echo "   Install: cargo install cargo-deny"
fi

# ── Rust: vulnerability advisory DB ──
echo ""
echo "=== cargo-audit ==="
if command -v cargo-audit &>/dev/null; then
    check "RUSTSEC advisories" cargo audit
else
    echo -e "${YELLOW}⚠️  cargo-audit not installed — skipping${NC}"
    echo "   Install: cargo install cargo-audit"
fi

# ── Python: binding-layer rule enforcement (AST-based, no loops/arithmetic/logic) ──
check "Python binding rules" python scripts/check_bindings.py

# ── Python: migration specification tests ──
echo ""
echo "=== migration parity specification ==="
python scripts/check_migration_parity_inputs.py
python -m unittest \
    tests.test_migration_parity_inventory \
    tests.test_migration_parity_cases \
    tests.test_migration_parity_evidence

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
