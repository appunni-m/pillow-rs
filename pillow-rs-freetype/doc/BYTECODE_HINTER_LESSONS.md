# Bytecode Hinter — What We Learned (and Didn't Solve)

## The Goal

Match Python Pillow's `ImageFont.getmask()` / `getbbox()` pixel output. Pillow
uses `FT_LOAD_DEFAULT` which activates FreeType's native TrueType bytecode
interpreter. Our code didn't have one. We built one.

## TrueType Bytecode 101

A TrueType font with bytecode has three tables:

```
fpgm (Font Program)  — Executed once at face load. Defines functions (FDEF/ENDF).
                       Uses NPUSHB/NPUSHW to push function numbers and parameters
                       onto the stack, then FDEF pops the function number and records
                       the bytecode range. Function bodies contain actual hinting
                       opcodes that get CALL'd by glyph programs.

prep (CVT Program)   — Executed each time ppem changes. Scales the Control Value
                       Table (CVT) from font design units to pixel units for the
                       current size. Uses WCVTP to write scaled values. Must run
                       against a twilight zone (scratch point array).

cvt  (Control Value Table) — Array of i16 FWORD values. Each is a reference
                       distance in font units (stem width, x-height, cap height,
                       etc.). Parsed as `i16 * 64` in 26.6 format. Prep scales
                       them to pixel units; glyph programs reference them via
                       RCVT for MIRP/MIAP.
```

Glyph bytecode lives in each glyph's `glyf` table entry, between the contour
endpoints and the coordinate data. Length is a u16 at `glyf[10 + nc*2]`.

## Stack Convention (Critical Bug We Found and Fixed)

FreeType's bytecode calling convention:
```
Pop_Push_Count[opcode] = (pops << 4) | pushes

For two-argument opcodes like WCVTP:
  Stack before: [..., index, value]   (value is on TOP)
  args[0] = value   (first popped = top of stack)
  args[1] = index  (second popped = below top)
```

Our Rust code uses `Vec::pop()` which removes from the END (top). For two-arg
opcodes, we initially popped index first (deeper) then value (top), which
SWAPPED them. Fixed in commit d02d15b.

Fixed opcodes: WCVTP (0x44), WS (0x42), SCFS (0x48), SHPIX (0x38),
MIRP (0xE0-FF), MIAP (0x3E/3F), LOOPCALL (0x2A), JROT/JROF (0x78/0x79).

## Rounding Opcode Map (Another Bug We Found and Fixed)

The TrueType rounding opcodes were swapped:
- 0x18 = RTG (Grid) — we had mapped it to RTDG (DoubleGrid)
- 0x19 = RTHG (HalfGrid) — we had it missing
- 0x3D = RTDG (DoubleGrid) — we had mapped it to RTG

C's spec is in `ttinterp.h`:
```
#define TT_Round_To_Half_Grid   0
#define TT_Round_To_Grid        1
#define TT_Round_To_Double_Grid 2
#define TT_Round_Down_To_Grid   3
#define TT_Round_Up_To_Grid     4
#define TT_Round_Off            5
```

Opcode dispatch in `ttinterp.c`:
- 0x18 → Ins_RTG (Round To Grid)
- 0x19 → Ins_RTHG (Round To Half Grid)
- 0x3D → Ins_RTDG (Round To Double Grid)
- 0x7C → Ins_RTHG (alternative encoding)
- 0x7D → Ins_RDTG (Round Down To Grid)
- 0x7E → Ins_RUTG (Round Up To Grid)
- 0x7F → Ins_ROFF (Round Off)

Fixed in commit a82e1fe.

## MPPEM Value Format

MPPEM (Measure Pixels Per EM, opcode 0x4B) returns ppem * 64 (26.6 format).
We initially returned raw ppem (10 instead of 640). This caused all CVT
scaling operations to compute values 64x too small. Fixed in commit de8f26f.

## CVT Value Format

The 'cvt ' table contains FWORD (i16) values in font design units.
FreeType parses them in `tt_face_load_cvt` (ttpload.c:346):
```c
*cur = FT_GET_SHORT() * 64;  // Store as 26.6
```

So `cvt[i] = font_unit_value * 64`. When the prep program scales these to
pixel units, it does:
```c
exec->cvt[i] = FT_MulFix(face->cvt[i] / 64, size->ttmetrics.scale);
```

Our linear CVT scaling in `mod.rs`:
```rust
let fu = *cv / 64;
*cv = ft_mul_fix(fu, y_scale);
*cv = ft_round_fix(*cv);
```

## The Prep Program Problem (Not Fully Solved)

The prep program executes against a twilight zone — an array of scratch
points used for temporary computation. C initializes twilight points to all
zeros (`FT_ARRAY_ZERO` in `ttobjs.c:954-955`). Our code does the same.

The prep program modifies CVT values via WCVTP. To compute the right CVT
values, it uses:
1. MIAP — sets twilight point to rounded CVT value
2. MIRP — moves twilight point relative to another point using CVT entry
3. GC — reads projected coordinate of twilight point
4. SCFS — sets twilight point from stack value
5. Math ops — computes scaling formulas using stack values

The problem: MIAP on twilight zone sets the current position from CVT:
```rust
let rnd_cvt = self.gs.round(cvt_val);
let delta = rnd_cvt - org_dist; // org_dist = project(cur_x, cur_y) = 0
zone.set_cur(p, cur_x + dx, cur_y + dy);
```

With twilight starting at (0,0) and projection along Y axis:
- `project(0, 0)` = 0
- `delta = round(cvt_val) - 0` = round(cvt_val)
- `move_along_free(round(cvt_val))` with freedom=(0, 0x4000) = (0, round(cvt_val))
- Twilight point gets set to (0, round(cvt_val))

This should work for Y-axis projection. But our implementation fails because:

1. **fpgm function bodies modify GraphicsState** — The fpgm's function
   definitions contain actual opcodes that change vectors, rounding mode,
   and auto-flip. When we ran fpgm through the full VM (commit `d02d15b`),
   the GS got corrupted for subsequent programs.

2. **Function definitions not registered correctly** — Our FDEF handler
   scans for ENDF but the scanning misses mismatched FDEF/ENDF pairs in
   nested function bodies. LiberationSans has 17 functions but we only
   got 16 registered.

3. **Stack corruption in prep** — Even with stack.clear() at start, the
   fpgm leaves the GS in an unpredictable state. MIRP then computes
   wrong relative distances because auto_flip is wrong.

## Attempted Fixes (All Failed to Improve PIL Score)

1. **Enable prep execution** — CVT entries get zeroed → masks explode
2. **Run fpgm through full VM** — GS corrupted → masks explode
3. **Round CVT values after scaling** — No change (already pixel-aligned)
4. **Use autohinter for PIL backend** — Regressed 697 tests (wrong algorithm)
5. **Unpad PIL masks** — Regressed 1,089 tests (wrong bbox computation)

## What Actually Worked (Committed to main)

Only three changes produced measurable improvements:

| Commit | Description | Delta |
|---|---|---|
| 09893a3 | VSEP range check from 128→66 FU | 3 FT failures → 0 |
| 473fc22 | Bytecode VM + linear CVT scaling | -466 PIL failures |
| d02d15b | Stack pop order fix (8 opcodes) | -96 PIL failures |

The remaining 4,977 failures are all 1px subpixel differences from two sources:

### Source 1: Linear CVT vs Prep-Calculated CVT

The prep program applies rounding-mode-specific adjustments to CVT values.
Our linear scaling (`fu * scale_factor`) is close but not exact. At 10pt
Debian Sans, CVT value differences of 1-2 FU (0.015-0.03 pixels) propagate
through MIRP into 0-1 pixel coordinate differences.

**Why we can't fix this without prep:** The prep program for LiberationSans
uses function calls (CALL) into fpgm-defined functions. These functions were
defined in fpgm while the GS was in a specific state. Re-executing them in
prep's context requires the GS to match. We can't replicate this without
proper fpgm execution.

### Source 2: Rasterizer DDA Precision

For DejaVuSans-ExtraLight (0 per-glyph bytecode instructions), our grays.rs
DDA line renderer produces slightly different coverage values than FreeType's
ftgrays.c. This affects 175 glyphs (ExtraLight + Italic variants).

**Why we can't fix this:** The differences are in subpixel coverage for
curved segments. FreeType's `gray_render_conic` uses a different subdivision
threshold than our port. Fixing this requires line-by-line comparison of
the DDA rendering, which is a separate project.

## The Real Blocker: FT_LOAD_DEFAULT vs Our Pipeline

FreeType's `FT_LOAD_DEFAULT` → `TT_Process_Simple_Glyph` does:

```
1. Set phantom points (pp1-pp4) in the glyph zone
2. Round phantom point coordinates to pixel grid
3. Execute glyph bytecode via TT_Run_Context
4. Execute prep program first if ppem changed
5. Execute fpgm once at face load
```

Our pipeline:
```
scaler::scale_glyph() with metrics=None:
1. Scale coordinates to 26.6
2. Build glyph zone with phantom points
3. If bytecode tables exist + glyph has instructions:
   a. Run fpgm (fails → stack/GS corruption)
   b. Run prep (fails → memory explosion from bad CVT)
   c. Run glyph program (works correctly for simple glyphs)
4. Compute bbox, return scaled glyph
```

The gap: fpgm → prep → glyph program is a chain. If any link breaks,
downstream links produce wrong output. Our fpgm execution is incomplete
because LiberationSans's fpgm contains function bodies with opcodes that
modify the GS. When prep CALLs into these functions, the GS difference
causes wrong CVT scaling.

**What ImageFT does differently:** PIL's `_imagingft.c` calls
`FT_Load_Glyph(face, glyph_index, FT_LOAD_DEFAULT)` which handles the
entire fpgm→prep→glyph_ins chain internally. The only way to match PIL's
output is to match FreeType's bytecode interpreter output exactly — which
requires a complete, bug-for-bug compatible bytecode VM with working
fpgm → prep → glyph program execution.

## File Inventory

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

## References

| What | File:Line |
|---|---|
| CVT parsing | `ttpload.c:312-360` (tt_face_load_cvt) |
| fpgm parsing | `ttpload.c:395-430` (tt_face_load_fpgm) |
| prep parsing | `ttpload.c:442-505` (tt_face_load_prep) |
| Glyph bytecode loading | `ttgload.c:229-360` (parse_simple_glyph) |
| TT_Hint_Glyph | `ttgload.c:777-860` |
| TT_Run_Context | `ttinterp.c:7435-7530` |
| TT_RunIns (main loop) | `ttinterp.c:6727-7200` |
| Opcode Pop_Push_Count table | `ttinterp.c:396-650` |
| Opcode dispatch (switch) | `ttinterp.c:6830-7200` |
| Ins_MDRP | `ttinterp.c:5399-5520` |
| Ins_MIRP | `ttinterp.c:5520-5673` |
| Ins_WCVTP | `ttinterp.c:2809-2825` |
| Ins_MIAP | `ttinterp.c:5315-5398` |
| Ins_IUP | `ttinterp.c:6189-` |
| C VT rounding | `ttobjs.c:891-957` (TT_Load_Context) |
| C VT init (twilight) | `ttobjs.c:949-955` |
| C VT init (storage) | `ttobjs.c:960` |
| Phantom point setup | `ttgload.c:1337-1362` (tt_loader_set_pp) |
| ASC/DESC metrics | `ttgload.c:116-150` (TT_Get_VMetrics) |
