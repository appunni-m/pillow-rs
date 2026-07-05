# Bytecode Hinter — What We Learned (and Didn't Solve)

## Current Review Notes

### 2026-07-05: TrueType VM opcode fixes must be reviewed across lanes

Two isolated subagent patches touched overlapping interpreter behavior after
main `9116545a`:

- `e5e9c57f` in `/home/appunni/work/pil-wasm-tt-pixel-coverage` changed
  `MUL` plus `SPVFS`/`SFVFS`. It improved
  `native_tt_default_matrix` from `6757/7640` to `6813/7640`, but local
  orchestrator verification showed `outline_cbox_matrix` dropped from
  `7546/11086` to `7543/11086`. Do not merge that patch as-is. The likely
  split point is to test vector setup independently from `MUL`.
- `3a1e5479` in `/home/appunni/work/pil-wasm-noto-thai-vertical` changed
  glyph-program stack clearing, `MUL`, `FLOOR`/`CEILING`, `LOOPCALL`, and
  `JROT`/`JROF`. The subagent reported broad gains:
  `outline_cbox_matrix 7546/11086 -> 10959/11086`,
  `metrics_only_matrix 8571/11086 -> 11065/11086`, and
  `native_tt_default_matrix 6757/7640 -> 7413/7640`.
  This was merged to main as `55999119` after local C-reference review,
  added implementation-site comments, full matrix verification, no-runtime-FFI,
  fmt, and clippy.

Review requirement before merging similar patches:

1. Re-run the full matrix from the candidate worktree and compare every lane,
   not just the owned bucket.
2. Inspect each opcode against FreeType `ttinterp.c`; these handlers share VM
   stack semantics, so a fix can move failures between native rendering,
   metrics, and outline cbox lanes.
3. Keep code comments at every non-obvious fix site with the C function/file
   area and the reason. Do not leave this knowledge only in a subagent report.
4. If `MUL` is correct but still regresses outline in one candidate, the
   regression is probably an upstream VM-state dependency exposed by `MUL`,
   not permission to keep the old arithmetic silently.

### 2026-07-05: Primitive C-oracle checks can narrow integration ambiguity

Added `tests/vector_norm_parity.rs` and `fixed::ft_normalize_2dot14` to compare
Rust directly with FreeType's `Normalize` + `FT_Vector_NormLen` path. This is a
small primitive parity test, not a fixture rewrite and not a runtime C path.

Local result on main `3866f4aa`:

- The Rust fixed-point port matches the C oracle for the tested vectors.
- Replacing the interpreter's current float/raw vector path with the exact
  primitive regressed `native_tt_default_matrix` from `7413/7640` to
  `7408/7640`, with metrics and outline unchanged.

Conclusion: vector normalization is still a real suspect, but it is not a
standalone mergeable integration fix. Use the primitive oracle to debug
specific traced `SPVFS`/`SFVFS` inputs and first-divergence points before
changing interpreter behavior.

## The Goal

Match FreeType `FT_LOAD_RENDER` pixel output. This path uses
`FT_LOAD_DEFAULT`, which activates FreeType's native TrueType bytecode
interpreter. Our code didn't have one. We built one.

---

## 1. TrueType Bytecode Architecture (C Reference)

### 1.1 The Three Programs

A TrueType font with bytecode has three separate bytecode streams:

| Table | When Executed | What It Does |
|---|---|---|
| `fpgm` | Once at face load | Defines functions (FDEF/ENDF). Stack: push params, FDEF pops func#. Function bodies contain actual opcodes CALL'd later. |
| `prep` | Each ppem change | Scales CVT from font units → pixel units. Uses WCVTP to write values. Runs against twilight zone (scratch area). |
| Glyph ins | Each glyph render | Grid-fits the glyph outline. Uses MIRP/MDRP with CVT references, IUP to smooth untouched points. |

### 1.2 C Execution Pipeline (Exact Order)

From `ttobjs.c` and `ttgload.c`:

```
tt_size_init_bytecode()
├── Allocates twilight zone (maxTwilightPoints + 4 phantom points)
├── tt_size_run_fpgm(size)
│   ├── TT_Load_Context(exec, face, size)  ← RESETS everything
│   ├── TT_Set_CodeRange(exec, tt_coderange_font, fpgm, fpgm_len)
│   ├── exec->pts.n_points = 0; exec->pts.n_contours = 0;  ← no points!
│   ├── TT_Run_Context(exec, size)  ← executes on EMPTY zone
│   └── TT_Save_Context(exec, size)  ← saves GS/storage after fpgm
│
├── [CVT loaded from file: FT_GET_SHORT() * 64 for each entry]
│
└── [Prep NOT run here — run later on first glyph load]

tt_size_run_prep(size)  ← called on first glyph, and when ppem changes
├── Splits exec->pts into twilight zone
├── TT_Load_Context(exec, face, size)  ← RESETS GS + storage to POST-FPGM saved state
├── TT_Set_CodeRange(exec, tt_coderange_cvt, prep_bytes, prep_len)
├── TT_Run_Context(exec, size)  ← executes against twilight zone
├── [CVT values are modified in place by WCVTP opcode]
└── [GS changes persist into subsequent glyph execution]

TT_Hint_Glyph(loader)
├── Copies cur → org (for "original" reference)
├── Rounds phantom points (pp1-pp4) to pixel grid
├── If composite: sets scale to 1:1, copies orus → cur
├── TT_Set_CodeRange(exec, tt_coderange_glyph, glyph_ins, ins_len)
├── exec->pts = loader->zone  ← zone with real glyph points + phantoms
├── TT_Run_Context(exec, size)  ← executes glyph program
└── Saves phantom points back to loader->pp1..pp4
```

### 1.3 The Critical `TT_Load_Context` Function

C's `TT_Load_Context` (ttobjs.c:891-957):

```c
exec->GS = tt_default_graphics_state;  // fresh GS each time
exec->GS.auto_flip = TRUE;

// Scale CVT values from FU to pixel units
for (i = 0; i < face->cvt_size; i++)
    exec->cvt[i] = FT_MulFix(face->cvt[i] / 64, size->ttmetrics.scale);
//                                                  ^^^ divide by 64 because
// CVT is stored as FU * 64 from the parser

// Reset twilight points to (0,0)  ← KEY: always starts fresh
for (i = 0; i < size->twilight.n_points; i++) {
    size->twilight.org[i] = (0,0);
    size->twilight.cur[i] = (0,0);
}

// Reset storage to zeros
FT_ARRAY_ZERO(exec->storage, exec->storeSize);

// Set function pointers for proj/freedom/move/round based on GS
exec->func_project = TT_Project;
exec->func_move    = Direct_Move;
// ...
```

**Critical insight:** `TT_Load_Context` is called BEFORE each of fpgm, prep, and
first glyph. It resets the GS to defaults AND re-scales CVT AND zeroes storage
AND zeroes twilight zone. But `TT_Save_Context` after fpgm saves storage and
code range state. So when `TT_Load_Context` is called again for prep, storage
is restored from saved state, but GS is reset.

### 1.4 The `TT_Run_Context` / `TT_RunIns` Main Loop

```c
TT_Run_Context(exec, size) {
    // Set up IP, code range, etc.
    // Then calls TT_RunIns:
    
    while (1) {
        opcode = CUR.opcode = NEXT_Byte();
        
        if (opcode >= 0xF0) {
            // 1-byte opcode: PUSH, MDRP, MIRP, etc.
            execute_one_byte(opcode);
        } else {
            // Look up Pop_Push count
            FT_Byte pop_push = Pop_Push_Count[opcode];
            pops = pop_push >> 4;
            
            // Pop 'pops' arguments into exec->args array
            // exec->top moves up, args[] get values from stack
            // args[0] = value that was at TOP of stack (last pushed)
            // args[1] = value below top
            // etc.
            
            // Execute opcode handler with args pointer
            switch (opcode) {
                case 0x42: Ins_WS(exec, args); break;
                case 0x44: Ins_WCVTP(exec, args); break;
                // ...
            }
            
            // Push results back (if any)
            pushes = pop_push & 0x0F;
            // push 'pushes' values onto exec->stack
        }
    }
}
```

**This is the fundamental difference from our implementation.** C pops all
arguments FIRST into a flat args array, then passes the array to the handler.
The handler reads `args[0]`, `args[1]`, etc. in a well-defined order.
Our implementation pops values one at a time during handler execution,
which made the stack pop order bug possible.

---

## 2. Our Implementation vs C — Line-by-Line Differences

### 2.1 fpgm Execution

**C (ttobjs.c:884-920):**
```
1. TT_Load_Context() — reset GS to default, scale CVT, zero storage, zero twilight
2. TT_Clear_CodeRange(cvt), TT_Clear_CodeRange(glyph)
3. TT_Set_CodeRange(font, fpgm_bytes, fpgm_size)
4. exec->pts.n_points = 0; exec->pts.n_contours = 0  // execute on EMPTY zone
5. TT_Run_Context() — runs the full VM, all opcodes execute normally
6. TT_Save_Context() — saves storage/GS state for later prep execution
```

**Our (hinter/exec.rs:242-320):**
```
1. Save GS (clone)
2. stack.clear()
3. cur_range = 1, ip = 0
4. Execute custom fpgm parser loop:
   - Handles push ops (NPUSHB, NPUSHW, PUSHB, PUSHW)
   - Handles stack ops (DUP, POP, CLEAR, SWAP)
   - Handles math ops (ADD, SUB, DIV, MUL, ABS, NEG, FLOOR, CEILING)
   - Handles storage/CVT ops
   - FDEF: pops func number, scans to ENDF, registers range
   - ENDF outside FDEF → error
   - All other opcodes → skip (empty body)
5. Restore GS
```

**Difference:** C executes fpgm through the FULL VM (TT_RunIns). Our code
has a custom fpgm parser that handles stack operations but DOES NOT execute
function bodies. The function bodies contain opcodes that push/pop values
from the stack — without executing them, the stack depth gets out of sync,
and subsequent FDEF operations pop wrong function numbers.

**Why we can't run the full VM for fpgm:** The VM dispatches point-moving
opcodes (MDRP, MIRP, MIAP) that need a valid GlyphZone. fpgm runs against
an empty zone (exec->pts.n_points = 0). Our run_program() assumes a valid
zone and will panic on out-of-bounds access when MIRP references point 0.

**Fix needed:** Make run_program() handle empty zones gracefully (skip
point-moving ops when n_points == 0), then use it for fpgm execution.

### 2.2 Prep Program Execution

**C (ttobjs.c:941-997):**
```
1. Split twilight zone — sets up exec->pts to point at twilight zone arrays
2. TT_Load_Context() — RESETS GS to default, RE-SCALES CVT from FU to pixel
3. TT_Set_CodeRange(cvt, prep_bytes, prep_size)
4. TT_Run_Context() — executes against twilight zone
5. [CVT modified in-place by WCVTP]
```

**Our (hinter/exec.rs:382-420):**
```
1. Create twilight zone with 16 points, all zero
2. Set glyph_program = prep_bytes
3. Set cur_range = 2 (glyph)
4. Set zp0=zp1=zp2=0 (twilight zone)
5. Set vectors to Y axis
6. run_program(&mut twilight)
7. Restore zp0=zp1=zp2=1 (glyph zone)
```

**Difference:** Our prep execution uses run_program() which can handle
twilight zone ops. But it inherits the GS state from fpgm execution.
C's TT_Load_Context resets GS to defaults before each program execution.
Our GS carries over the auto-flip, proj vectors, and rounding mode from
fpgm into prep, causing prep to compute different CVT values than C.

**Why prep produces garbage (96 GB memory explosion):** When run_program()
hits IUP (opcode 0x30/0x31), it walks all points in the twilight zone and
interpolates. With prep's stack state modified by the prep program itself,
the interpolation produces values that overflow into huge memory allocations
when later used as mask dimensions.

### 2.3 CVT Scaling

**C (ttobjs.c:970-972, inside TT_Load_Context):**
```c
for (i = 0; i < face->cvt_size; i++)
    exec->cvt[i] = FT_MulFix(face->cvt[i] / 64, size->ttmetrics.scale);
// face->cvt[i] is in FU * 64 (from parser: FT_GET_SHORT() * 64)
// / 64 to get FU, then FT_MulFix to scale to 26.6 pixels
```

**Our (hinter/mod.rs:):**
```rust
let fu = *cv / 64;
*cv = crate::fixed::ft_mul_fix(fu, y_scale);
*cv = crate::fixed::ft_round_fix(*cv);
```

**These are equivalent.** The prep program modifies CVT values further by
applying rounding-mode-specific adjustments, but the linear scaling is
correct.

### 2.4 Glyph Zone Setup for Hinting

**C (ttgload.c:777-860, TT_Hint_Glyph):**
```
1. Copies cur → org: preserves original scaled coords
2. If composite: sets scale to 1:1 (subglyphs already hinted)
   If composite: copies orus → cur (use already-hinted coords)
3. Rounds phantom points:
   zone->cur[n_points-4].x = FT_PIX_ROUND(zone->cur[n_points-4].x)  // pp1
   zone->cur[n_points-3].x = FT_PIX_ROUND(zone->cur[n_points-3].x)  // pp2
   zone->cur[n_points-2].y = FT_PIX_ROUND(zone->cur[n_points-2].y)  // pp3
   zone->cur[n_points-1].y = FT_PIX_ROUND(zone->cur[n_points-1].y)  // pp4
4. If glyph_ins_length > 0:
   TT_Set_CodeRange(glyph, ins, len)
   exec->pts = *zone
   TT_Run_Context()
```

**Our (hinter/mod.rs:87-120):**
```
1. Build GlyphZone with scaled 26.6 coords
2. Add phantom points (pp1-pp4) at zero
3. Copy cur → org
4. Create ExecContext with fpgm/cvt/prep
5. Run fpgm
6. Run glyph program via run_program()
```

**Difference:** We don't round phantom points before hinting. C rounds them.
This matters because MIRP/MDRP reference phantom points (indices n_points-4
through n_points-1) for side bearing/advance width calculations. Our
unrounded phantoms can cause a 1px difference in the leftmost x coordinate.

### 2.5 IUP Interpolation

**C (ttinterp.c:6189+, Ins_IUP):**
```
1. For each contour, find touched points
2. Between consecutive touched points, interpolate untouched points
3. Uses ORIGINAL positions (orus/orig) for the interpolation ratio
4. Uses CURRENT positions (cur) for the output values
5. Handles wraparound (last → first touched in the contour)
```

**Our (hinter/exec.rs: IUP handler):**
```
1. Walk all points linearly (0..n_points), not per-contour
2. Find first and last touched
3. Linear interpolation between them using cur deltas
4. Uses cur positions for both ratio AND output
```

**Difference:** C uses ORIGINAL positions (org) for the interpolation
ratio, not current positions. This is the correct approach because the
ratio should reflect the pre-hinting shape. Using current positions
can amplify rounding errors. Also C walks per-contour, not linearly.

### 2.6 MDRP/MIRP Point Movement

**C (ttinterp.c:5399-5673, Ins_MDRP/Ins_MIRP):**
```
1. Get reference point from zp1[GS.rp0]
2. Compute original projected distance between point and ref
3. For MDRP: round the original distance
4. For MIRP: round the CVT value, auto-flip if signs differ
5. Apply minimum distance (cvt_cut_in for MIRP, min_distance for MDRP)
6. Move point along freedom vector by the computed distance relative to ref
```

**Our (hinter/exec.rs: MDRP/MIRP handlers):**
```
Same logic — correct after the stack pop order fix.
```

---

## 3. Root Cause: Why Prep Can't Work Yet

The prep program for LiberationSans-Regular (835 bytes) does:

```
NPUSHW(33)     — push 33 constant values
NPUSHB(170)    — push 170 bytes
PUSHW[1]       — push word
NPUSHB(...)    — push more
...
CALL(83)       — call function 83 from fpgm  ← FAILS: function not found
CALL(84)       — call function 84
MPPEM          — 640
PUSHW[1]       — push word
GT             — compare
...
```

**Our fpgm registers 16 functions (indices 0-11, 13-17). Function 83 does
not exist.** This is because our custom fpgm parser loses track of the stack
after ~10 function bodies. The function bodies themselves contain push/pop
opcodes that shift the stack — without executing them, each FDEF pops the
wrong function number.

**Fix 1: Run fpgm through the full VM.** This worked (commit d02d15b) — the
VM processes all opcodes including function bodies, keeping the stack
accurate. But the VM's glyph zone accessors panic on zero-length zones.

**Fix 2: Make zone ops safe for empty zones.** All GlyphZone accessors
already return (0,0) for out-of-bounds indices. The remaining issue is the
IUP opcode which walks all points and could overflow.

**Fix 3: Save/Restore GS.** After fpgm runs, the GS has been modified by
function body opcodes (vectors changed, auto-flip toggled, rounding mode
set). Prep needs a fresh GS. C handles this via TT_Load_Context which is
called at the start of each program. We need the same.

---

## 4. Stack Pop Order — Detailed Explanation

### C Convention

The `Pop_Push_Count` table in `ttinterp.c:396-650` defines for each opcode:

```c
#define PACK(x, y) ((x << 4) | y)

// WCVTP: pops 2, pushes 0
[0x44] = PACK(2, 0),  // WCVTP
```

When `TT_RunIns` dispatches an opcode:
1. It reads `pops = Pop_Push_Count[opcode] >> 4`
2. It adjusts `exec->top` (stack pointer) by `pops`
3. It sets `args = exec->stack + exec->top` — args points to the pop'd region
4. `args[0]` = the FIRST value popped (topmost on the original stack)
5. `args[1]` = the SECOND value popped (below the top)

For WCVTP (pops 2):
```
Stack before: [..., index, value]  ← value is on TOP (at stack[top-1])
After pop 2:  args[0] = value, args[1] = index
              stack pointer moves up 2 positions
```

### Our Bug

Our code does:
```rust
0x44 => {
    let idx = self.pop()? as usize;  // pops TOP = value, treats as index ← WRONG
    let val = self.pop()?;           // pops BELOW = index, treats as value ← WRONG
    self.set_cvt(idx, val)?;
}
```

`Vec::pop()` removes from the end. The "top" of our stack is the last element.
So the first pop gets the VALUE (which was pushed last), and the second pop
gets the INDEX (which was pushed before the value).

But we use the first pop as `idx` and the second as `val`. That REVERSES them.

**Fix:** Pop value (top) first, then index (below):
```rust
0x44 => {
    let val = self.pop()?;           // top = VALUE ← correct
    let idx = self.pop()? as usize;  // below = INDEX ← correct
    self.set_cvt(idx, val)?;
}
```

### Why Only WCVTP/WS/SCFS Were Affected

Most opcodes pop only one value, so the order doesn't matter. Only two-arg
opcodes where the arguments are semantically different (index vs value,
point vs amount) are affected. MDRP/MIAP pop one point index — fine.
MIRP/MIAP pop two — the first is a point index, the second is a CVT index.
Getting them backwards means we'd look up CVT[point_index] which is garbage.

---

## 5. Attempted Fixes (Failed to Improve FreeType default score)

| Fix Attempt | Why It Failed |
|---|---|
| Enable prep execution | CVT entries zeroed → mask dimensions explode (96 GB allocation) |
| Run fpgm through full VM | GS corrupted → wrong auto-flip in subsequent MIRP |
| Round CVT after scaling | No change (values already pixel-aligned at 10pt) |
| Use autohinter for FreeType | Regressed 697 tests (autohinter ≠ bytecode hinter) |
| Unpad  masks | Regressed 1,089 tests ( uses rasterized bitmap size, not outline bbox) |
| Per-contour IUP | Pixels shifted up → masks had wrong y-offset |

---

## 6. What Actually Worked (Committed to main)

| Commit | Description | Delta |
|---|---|---|
| 09893a3 | VSEP range check: C uses `adj <= 66`, we used `adj <= 128` | 3→0 FT failures |
| 473fc22 | Bytecode VM + linear CVT scaling | -466  failures |
| d02d15b | Stack pop order: value then index, not index then value | -96  failures |
| a82e1fe | Rounding opcodes: 0x18=RTG,0x19=RTHG,0x3D=RTDG | Quality |
| de8f26f | MPPEM: `ppem * 64` not raw `ppem` | Quality |

---

## 7. Remaining Gap: 4,977  Failures

All 1px subpixel differences. Two sources:

### 7.1 Linear CVT vs Prep-Calculated CVT (~4,800 failures)
Fonts with per-glyph bytecode (LiberationSans, DejaVuSerif-Bold,
NotoSans-Bold, DejaVuSans-Oblique, DejaVuSansMono, LiberationSansNarrow-Bold).
Prep scales CVT with rounding-mode adjustments; linear scaling is close but
off by 1-2 FU → 1px pixel difference. Fix requires working fpgm→prep chain.

### 7.2 Rasterizer DDA Precision (~177 failures)
DejaVuSans-ExtraLight and DejaVuSerif-Italic (0 per-glyph bytecode).
Our grays.rs produces slightly different coverage values than FreeType's
ftgrays.c for curved segments. Fix requires DDA line renderer port.

---

## 8. File Inventory

```
pillow-rs-freetype/src/tt/hinter/
├── mod.rs    (210 lines) — Entry point, zone setup, CVT scaling, dispatch
├── tables.rs ( 95 lines) — Parse cvt, fpgm, prep tables (4 unit tests)
├── zone.rs   ( 92 lines) — GlyphZone struct (cur/org/orus, tags, contours)
├── gs.rs     (240 lines) — GraphicsState (vectors, rounding, auto-flip)
├── exec.rs   (1100 lines) — ExecContext, fpgm/prep/glyph execution, 50+ opcodes

pillow-rs-freetype/doc/
├── BYTECODE_HINTER_IMPL.md — Original implementation plan
└── BYTECODE_HINTER_LESSONS.md — This document
```

## 9. C Code References

| What | File:Line |
|---|---|
| CVT parsing | `ttpload.c:312-360` (tt_face_load_cvt) |
| fpgm parsing | `ttpload.c:395-430` (tt_face_load_fpgm) |
| prep parsing | `ttpload.c:442-505` (tt_face_load_prep) |
| Glyph bytecode loading | `ttgload.c:229-360` (parse_simple_glyph) |
| TT_Hint_Glyph | `ttgload.c:777-860` |
| TT_Load_Context | `ttobjs.c:891-957` |
| tt_size_run_fpgm | `ttobjs.c:884-920` |
| tt_size_run_prep | `ttobjs.c:941-997` |
| tt_size_init_bytecode | `ttobjs.c:1030-1120` |
| TT_Run_Context | `ttinterp.c:7435-7530` |
| TT_RunIns (main loop) | `ttinterp.c:6727-7200` |
| Opcode Pop_Push_Count table | `ttinterp.c:396-650` |
| Opcode dispatch (switch) | `ttinterp.c:6830-7200` |
| Ins_MDRP | `ttinterp.c:5399-5520` |
| Ins_MIRP | `ttinterp.c:5520-5673` |
| Ins_WCVTP | `ttinterp.c:2809-2825` |
| Ins_MIAP | `ttinterp.c:5315-5398` |
| Ins_IUP | `ttinterp.c:6189-` |
| Ins_MDAP | `ttinterp.c:5276-5315` |
| Phantom point setup | `ttgload.c:1337-1362` (tt_loader_set_pp) |
