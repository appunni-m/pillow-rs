# Bytecode VM — C vs Rust Parity Audit

## Approach

For each function, we compare the C code line-by-line with our Rust
implementation. ✅ = verified match, ❌ = divergence found, 🚧 = not yet compared.

---

## 1. `tt_size_run_fpgm` (ttobjs.c:884-920) — ❌ DIVERGENT

**C does:** Runs full VM (`TT_Run_Context`) on empty zone. All function body
opcodes execute, keeping stack state accurate.

**We do:** Custom parser that skips function body opcodes. Stack desyncs
after ~10 FDEFs → wrong function numbers registered.

**Fix:** Use `run_program()` with empty zone. Zone accessors already safe for
OOB (return 0,0).

---

## 2. `tt_size_run_prep` (ttobjs.c:941-997) — ❌ DIVERGENT (4 missing steps)

**C does:** (1) Reset GS to default (2) Call TT_Load_Context to scale CVT
(3) Zero storage (4) Execute prep against twilight zone.

**We do:** Skip all four steps. GS carries over fpgm state. CVT stays in
FU*64 format. Storage has stale values.

**Fix:** Add GS reset + CVT scaling + storage.clear() before `run_program`.

---

## 3. `Ins_MDRP` (ttinterp.c:5399-5519) — ❌ TWO CRITICAL BUGS

### Bug 1: Uses `org` instead of `orus` for distance

C checks `gep0/gep1`: if twilight zone → use `org` (scaled 26.6 positions);
if glyph zone → use `orus` (unscaled font-unit distances, then scale to 26.6).

Our code always uses `org`. This means:
- Distances are in 26.6 format instead of font units
- Distances include previous hinting adjustments (wrong baseline for IUP)

### Bug 2: Rounding flag not checked

C checks `opcode & 4` to decide whether to round. MDRP[0] (no rounding)
keeps the exact original projected distance. Our code always rounds.

---

## 4. `Ins_MIRP` (ttinterp.c:5520-5673) — ❌ Same orus bug + 2 more

Same `org` vs `orus` bug as MDRP. Also:
- CVT cut-in: C compares `|org_dist - cvt_val|`, we compare `|org_dist - rnd_cvt|`
- Auto-flip: C uses `org_dist` sign, we use rounded sign

---

## 5. `Ins_IUP` (ttinterp.c:6189+) — ❌ COMPLETELY DIFFERENT ALGORITHM

**C does:** Per-contour walk, finds each pair of consecutive touched points,
interpolates using `orus` (original unscaled) for ratio computation.

**We do:** Linear array walk (no contours), single interpolation from last
to first touched, uses `cur` (current hinted) for ratio → amplifies errors.

---

## Summary

| Priority | Bug | Lines to fix |
|---|---|---|
| 🔴 P0 | MDRP/MIRP: `org`→`orus` for glyph zone | ~30 |
| 🔴 P0 | fpgm: run full VM, not custom parser | ~5 |
| 🔴 P0 | prep: add GS reset + CVT scale + storage clear | ~20 |
| 🟡 P1 | IUP: per-contour walk with org ratios | ~200 |
| 🟡 P1 | MDRP rounding flag check | ~10 |

**Total fix: ~265 lines to close PIL gap from 4,977 → 0.**
