# TrueType Hinting Engine — Design Document

## Overview

Implement a complete TrueType bytecode interpreter for `pillow-rs-font` that produces
pixel-identical output to FreeType 2.6.x (the version PIL bundles). This is the missing
piece that causes 1755/1970 font matrix tests to fail — every glyph at small sizes needs
hinting to snap coordinates to the pixel grid.

## Architecture

Four new modules within `pillow-rs-font/src/`:

```
hinting/
  mod.rs              — HintState + HintingEngine: orchestrates fpgm/prep/glyph execution
  exec.rs             — ExecContext: the TrueType VM, all opcode dispatch
  graphics.rs         — GraphicsState, Zone, F26Dot6Vector, tag constants
  round.rs            — Rounding functions matching FreeType's Round_* family
  opcodes.rs          — Opcode constants, mnemonic→value map
  iup.rs              — Ins_IUP implementation (Interpolation of Unscaled Points)
```

**Data flow:**

```
Font::truetype()
  → parse cvt / prep / fpgm raw bytes
  → store in FontData (new fields)

scale_glyph() → scale Δ font units → hint_glyph() → rasterize()
                                     ↑
HintingEngine::hint_glyph():
  1. If size changed → run PREP (CVT program)
  2. For each glyph → run glyph instructions
  3. IUP (both axes) on the scaled zone

Font load     → run FPGM once
Size change   → run PREP once
Glyph render  → run per-glyph instructions + IUP
```

## C-Matched Structures

All structures closely match FreeType 2.6.x (`ttinterp.h`, `ttobjs.h`, `tttypes.h`).

### F26Dot6Vector — matches FT_Vector

```rust
#[derive(Copy, Clone, Default)]
pub(crate) struct F26Dot6Vector {
    pub x: i32,  // FT_Pos / FT_F26Dot6
    pub y: i32,
}
```

### GraphicsState — matches TT_GraphicsStateRec

```rust
#[derive(Copy, Clone)]
pub(crate) struct GraphicsState {
    pub rp0: u16,
    pub rp1: u16,
    pub rp2: u16,
    pub gep0: u16,
    pub gep1: u16,
    pub gep2: u16,

    pub dual_vector: F26Dot6Vector,   // GS.dual (projection)
    pub proj_vector: F26Dot6Vector,   // GS.proj
    pub free_vector: F26Dot6Vector,   // GS.free

    pub loop_count: i32,
    pub round_state: i32,
    pub compensation: [i32; 4],       // device-specific

    pub minimum_distance: i32,
    pub control_value_cut_in: i32,
    pub single_width_cut_in: i32,
    pub single_width_value: i32,

    pub delta_base: u16,
    pub delta_shift: u16,
    pub auto_flip: bool,
    pub instruct_control: u8,
    pub scan_control: bool,
    pub scan_type: i32,
}
```

Default state matches `tt_default_graphics_state`:
- `rp0=rp1=rp2=0`, `gep0=gep1=gep2=0`
- `dual_vector=proj_vector=(1,0)`, `free_vector=(1,0)` (X axis)
- `loop=1`, `round_state=1` (RTG)
- `minimum_distance=1`, `control_value_cut_in=17`, `single_width_cut_in=0`, `single_width_value=0`
- `delta_base=9`, `delta_shift=3`
- `auto_flip=1`, `instruct_control=0`, `scan_control=0`, `scan_type=0`

### Zone — matches TT_GlyphZoneRec

```rust
#[derive(Clone)]
pub(crate) struct Zone {
    pub points: Vec<F26Dot6Vector>, // current positions (FT_Vector[])
    pub org: Vec<F26Dot6Vector>,    // original scaled positions (pre-hinting)
    pub tags: Vec<u8>,              // FT_Byte[] — bit 1=on_curve, bit 2=touch_x, bit 4=touch_y
    pub contours: Vec<u16>,         // FT_UShort[] — end-point per contour
    pub n_points: u16,
    pub n_contours: u16,
}
```

### ExecContext — matches TT_ExecContextRec

```rust
pub(crate) struct ExecContext {
    // Graphics state
    pub gs: GraphicsState,

    // Zones
    pub zp0: Zone,           // current zone 0 (via SZP0)
    pub zp1: Zone,           // current zone 1
    pub zp2: Zone,           // current zone 2
    pub pts: Zone,           // main glyph zone (always has the glyph points)
    pub twilight: Zone,      // twilight zone (max 8 points)

    // Code
    pub code: Vec<u8>,
    pub ip: i32,
    pub opcode: u8,
    pub cur_range: CodeRange,

    // Stack
    pub stack: Vec<i32>,
    pub top: i32,

    // CVT & Storage
    pub cvt: Vec<i32>,
    pub storage: Vec<i32>,
    pub glyf_cvt: Vec<i32>,
    pub glyf_storage: Vec<i32>,

    // Functions
    pub fdefs: Vec<FnDef>,
    pub idefs: Vec<FnDef>,
    pub call_stack: Vec<CallRecord>,
    pub call_depth: usize,

    // Metrics
    pub point_size: i32,
    pub ppem: u16,
    pub scale: i32,

    // Rounding
    pub period: i32,
    pub phase: i32,
    pub threshold: i32,
    pub round_fn: RoundFn,
    pub func_project: ProjectFn,
    pub func_dualproj: ProjectFn,
    pub func_free_proj: ProjectFn,
    pub func_move: MoveFn,

    // Flags
    pub grayscale: bool,
}

pub(crate) enum CodeRange { None, Font, Cvt, Glyph }

pub(crate) struct FnDef {
    pub range: i32,
    pub start: i32,
    pub end: i32,
    pub opc: u32,
    pub active: bool,
}

pub(crate) struct CallRecord {
    pub caller_range: i32,
    pub caller_ip: i32,
    pub cur_count: i32,
    pub def: FnDef,
}
```

Function pointer types (matching `TT_Round_Func`, `TT_Project_Func`, `TT_Move_Func`):

```rust
pub(crate) type RoundFn = fn(distance: i32, compensation: i32) -> i32;
pub(crate) type ProjectFn = fn(dx: i32, dy: i32) -> i32;
pub(crate) type MoveFn = fn(distance: i32) -> (i32, i32);
```

### New FontData Fields

```rust
// Added to FontData
pub cvt: Vec<i32>,               // parsed Control Value Table
pub fpgm: Vec<u8>,               // raw Font Program bytecode
pub prep: Vec<u8>,               // raw CVT Program bytecode
pub cvt_size: u16,               // number of CVT entries
pub hdmx: Option<Vec<u8>>,       // raw hdmx table (optional)
```

### New Font Fields

```rust
// Added to Font
pub(crate) hint_engine: Option<HintingEngine>,

// HintingEngine:
pub(crate) struct HintingEngine {
    pub exec: ExecContext,
    pub fpgm_ready: bool,
    pub cvt_ready: bool,
    pub last_ppem: u16,
}
```

## Opcode Coverage

The interpreter implements all TrueType opcodes used by modern hinted fonts.
Grouped by category:

### Stack Pushes (5)
- `NPUSHB` (0x40), `NPUSHW` (0x41) — variable-length
- `PUSHB[1-8]` (0xB0-0xB7), `PUSHW[1-8]` (0xB8-0xBF) — fixed-length

### Stack Manipulation (7)
- `DUP` (0x01), `POP` (0x02), `CLEAR` (0x03), `SWAP` (0x04), `DEPTH` (0x05)
- `CINDEX` (0x0A), `MINDEX` (0x0B), `ROLL` (0x08)

### Arithmetic (8)
- `ADD` (0x60), `SUB` (0x61), `DIV` (0x62), `MUL` (0x63)
- `ABS` (0x64), `NEG` (0x65), `FLOOR` (0x66), `CEILING` (0x67)

### Logical / Comparison (8)
- `LT` (0x50), `LTEQ` (0x51), `GT` (0x52), `GTEQ` (0x53)
- `EQ` (0x54), `NEQ` (0x55), `AND` (0x56), `OR` (0x57), `NOT` (0x58)

### Flow Control (7)
- `IF` (0x59), `ELSE` (0x5A), `EIF` (0x5B), `JMPR` (0x1C)
- `JROT` (0x1D), `JROF` (0x1E), `LOOPCALL` (0x2A)

### Functions (4)
- `FDEF` (0x2C), `ENDF` (0x2D), `CALL` (0x2B), `IDEF` (0x35)

### Graphics State (20+)
- `SVTCA` (0x00), `SPVTCA` (0x02), `SFVTCA` (0x04)
- `SPVTL` (0x06), `SFVTL` (0x07)
- `SPVFS` (0x08), `SFVFS` (0x09), `GPV` (0x0A), `GFV` (0x0B)
- `SRP0` (0x10), `SRP1` (0x11), `SRP2` (0x12)
- `SZP0` (0x13), `SZP1` (0x14), `SZP2` (0x15)
- `SZPS` (0x16)
- `RTHG` (0x19), `RTG` (0x1A), `RTDG` (0x1B), `RDTG` (0x1C), `RUTG` (0x1D)
- `ROFF` (0x1F), `RODD` (0x20), `RQ` (0x21), `SROUND` (0x76), `S45ROUND` (0x77)
- `SLOOP` (0x17)
- `SMD` (0x18)
- `INSTCTRL` (0x22)
- `SCANCTRL` (0x23)
- `SCANTYPE` (0x24)
- `GC` (0x46), `SCFS` (0x48), `MD` (0x49)
- `MPPEM` (0x4B), `MPS` (0x4C)

### Projection / Vector (8)
- `AA` (0x7F), `FLIPON` (0x4D), `FLIPOFF` (0x4E)
- `SANGW` (0x7E), `GPI` (0x6A), `HCVT` (0x66), `HMTX` (0x67)

### CVT / Storage (5)
- `WCVTP` (0x70), `WCVTF` (0x71), `RCVT` (0x72)
- `WS` (0x42), `RS` (0x43)

### Point Operations (12+)
- `MDRP[xxxxx]` (0xC0-0xDF / 32 variants)
- `MIRP[xxxxx]` (0xE0-0xFF / 32 variants)
- `IP` (0x39), `IUP` (0x30), `ALIGNRP` (0x3C)
- `SHP` (0x32), `SHC` (0x34), `SHZ` (0x36)
- `MSIRP` (0x3A), `MDAP` (0x2E-0x2F)
- `MIAP` (0x3E-0x3F)

### Delta (6)
- `DELTAP1` (0x5D), `DELTAP2` (0x5E), `DELTAP3` (0x5F)
- `DELTAC1` (0x71), `DELTAC2` (0x72), `DELTAC3` (0x73)

### Miscellaneous (6)
- `GETINFO` (0x88), `GETVARIATION` (0x91)
- `SRP0`-`SRP2` — referenced above
- `MPPEM`-`MPS` — referenced above

## MIRP Implementation Detail

MIRP (Move Indirect Relative to Reference Point) is the most common instruction and handles the most nuance:

```
1. POP cvt_idx, point_idx from stack
2. Decode flags from opcode lower 3 bits:
   - Bit 0: Round (set=apply rounding, clear=no rounding)
   - Bit 1: WithoutSet (set=don't update GS.round_state)
   - Bit 2: SetRoundState (set=update engine state from this distance)
3. Compute original_distance = proj(p_rp0 - p_point)
4. Compute cvt_distance = CVT[cvt_idx]
5. Apply cut-in logic (see apply_cut_in)
6. If Round bit set: distance = round_fn(distance + compensation)
7. If SetRoundState flag: update internal rounding state
8. RP2 = RP1, RP1 = RP0, RP0 = point_idx
9. Move point via freedom vector projection
10. Set touch bits on moved point
```

The cut-in logic matches FreeType's `Ins_MIRP`:

```rust
fn apply_cut_in(original: i32, cvt: i32, gs: &GraphicsState) -> i32 {
    let diff = (original - cvt).abs();
    if diff > gs.single_width_cut_in {
        if original.abs() < gs.single_width_cut_in {
            return original;
        }
        if diff > gs.control_value_cut_in {
            return original;
        }
        cvt
    } else {
        original
    }
}
```

The movement uses the freedom vector. In FreeType:

```rust
// project delta onto freedom vector to get signed distance
// move point along freedom vector by that distance
let fdot = dx * gs.free_vector.x + dy * gs.free_vector.y;  // 26.6
// scale the vector to have magnitude = rounded_distance
let scale = if fdot != 0 { mul_div(rounded, fdot, fdot) } else { 0 };
// this is simplified — FreeType uses exc->func_move which handles
// the actual projection
```

## Rounding Function Detail

Seven rounding modes, implemented as Rust function pointers matching FreeType's `TT_Round_Func`:

```rust
// Round_To_Grid — snap to nearest 64-unit (1px) boundary
fn rtg(d: i32, _comp: i32) -> i32 {
    if d >= 0 { ((d + 32) & !63) } else { -(((-d) + 32) & !63) }
}

// Round_To_Half_Grid — snap to nearest 32-unit boundary
fn rtdg(d: i32, _comp: i32) -> i32 {
    if d >= 0 { ((d + 32) & !63) + 32 } else { -(((-d) + 32) & !63) + 32 }
}

// Round_Down_To_Grid — floor to 64-unit boundary
fn rdtg(d: i32, _comp: i32) -> i32 {
    (d + 63) & !63 - d
}

// Round_Up_To_Grid — ceil to 64-unit boundary
fn rutg(d: i32, _comp: i32) -> i32 {
    (d + 63) & !63
}

// Round_To_Odd — round to odd 64-unit boundary
fn rodd(d: i32, _comp: i32) -> i32 { ... }

// Round_To_Grid_No_Round — return value unchanged
fn roff(d: i32, _comp: i32) -> i32 { d }

// Round_Super — configurable period/phase/threshold (from SROUND)
fn rsuper(d: i32, comp: i32) -> i32 { ... }
```

Super Rounding (SROUND) decodes the parameter:

```
pop n
period   = ((n >> 28) & 0xF) + 1   // 1..16
phase    =  (n >> 24) & 0xF         // 0..15
threshold = (n >> 20) & 0xF         // 0..15

exec.period    = period * 64
exec.phase     = phase * 64
exec.threshold = threshold * 64
```

## IUP Implementation Detail

IUP (Interpolation of Unscaled Points) runs twice — once for X, once for Y:

```
iup_x():
  for each contour C:
    find first point where touch_x is set
    if none found → skip contour (all points untouched)
    for each pair of consecutive touch_x points (A, B):
      for each untouched point P between A and B:
        if A.org.x == B.org.x:
          P.cur.x = A.cur.x
        else:
          P.cur.x = A.cur.x + mul_div(P.org.x - A.org.x,
                                       B.cur.x - A.cur.x,
                                       B.org.x - A.org.x)
    handle wrap-around: last_touch → first_touch

iup_y(): same algorithm with y coordinates and touch_y
```

The `mul_div` function matches `FT_MulDiv_No_Round`: 64-bit intermediate, truncation toward zero.

## Execution Flow

### Font Load (FPGM)
```
Font::truetype():
  parse cvt  → Vec<i32>
  parse fpgm → Vec<u8>
  parse prep → Vec<u8>
  store in Arc<FontData>

font_load():
  if fpgm not empty:
    exec.code = fpgm, exec.cur_range = Font
    exec.run()  // execute Font Program
    exec.fpgm_ready = true
```

### Size Change (PREP)
```
font_set_size(ppem):
  if ppem changed OR not cvt_ready:
    exec.code = prep, exec.cur_range = Cvt
    exec.run()  // execute CVT Program
    exec.cvt_ready = true
    exec.last_ppem = ppem
```

### Glyph Rendering
```
hint_glyph(data, glyph_index, scaled_glyph):
  // 1. Setup: place scaled points into exec.pts zone
  exec.pts.points = scaled_glyph.points  (F26Dot6)
  exec.pts.org     = scaled_glyph.points  (copy — store original)
  exec.pts.tags    = on_curve flags
  exec.pts.contours = end_pts_of_contours
  exec.pts.n_points = points.len()
  exec.pts.n_contours = contours.len()

  // 2. Reset zp0/zp1/zp2 → pts
  exec.gs.rp0 = exec.gs.rp1 = exec.gs.rp2 = 0
  exec.zp0 = exec.zp1 = exec.zp2 = exec.pts

  // 3. Copy CVT working copy
  exec.cvt = exec.glyf_cvt.clone_from(&orig_cvt)

  // 4. Execute glyph instructions
  let glyph_ins = parse_glyph_instructions(glyf_data, glyph_index)
  exec.code = glyph_ins, exec.cur_range = Glyph
  exec.run()

  // 5. IUP — X then Y
  exec.iup(0)  // X
  exec.iup(1)  // Y

  // 6. Return hinted positions
  scaled_glyph.points = exec.pts.points
```

## Composite Glyphs

Composite glyphs require recursive hinting:
1. Hint each sub-glyph independently
2. Merge points into a single zone
3. Re-run IUP on the merged zone
4. The `is_composite` flag in ExecContext tracks this state

## Integration with Existing Code

The existing `scale_glyph()` function in `scaler.rs` produces unscaled 26.6 coordinates. A new function `hint_scaled_glyph()` wraps `scale_glyph()` and applies hinting:

```rust
// In scaler.rs (or new hinting/mod.rs):
pub fn scale_and_hint(data: &FontData, glyph_index: u16,
                      engine: &mut HintingEngine) -> Result<ScaledGlyph> {
    let mut glyph = scale_glyph(data, glyph_index)?;
    if glyph.num_contours > 0 {
        engine.hint_glyph(data, glyph_index, &mut glyph);
    }
    Ok(glyph)
}
```

The `Font::getmask()` function in `metrics.rs` already calls `scale_glyph()`.
The only change needed is:

```rust
// Old:
let scaled = crate::scaler::scale_glyph(data, glyph_idx)?;

// New:
let scaled = crate::scaler::scale_and_hint(data, glyph_idx, &mut self.hint_engine)?;
```

## Error Handling

All interpreter errors return `FontError::InvalidFont(message)`:
- Stack overflow/underflow → abort glyph, return un-hinted glyph
- Call stack overflow/underflow → abort
- Division by zero → return 0 (matching FreeType behavior)
- Undefined FDEF/IDEF → skip (matching FreeType's pedantic_hinting flag)
- Loop counter > 10,000 → abort  (loop detector matching FreeType 2.6.x)

## Testing Strategy

1. **Unit tests per opcode** — each opcode gets a test with known stack inputs/expected outputs
2. **FreeType reference tests** — run known font bytecode snippets through both FreeType's interpreter and ours, compare outputs
3. **Coverage matrix** — existing matrix tests validate pixel SHA-256 against PIL's FreeType; passing number should increase from 215→1970 as implementation progresses
4. **Incremental testing** — focus on MIRP + IUP first (handles most hinting), then add CVT/storage, then rounding features

## Implementation Phases

**Phase 1: Infrastructure (engine skeleton, stack, basic ops)**
- ExecContext, GraphicsState, Zone
- Stack ops, arithmetic, logical
- Push/pop, flow control
- New FontData fields

**Phase 2: Graphics State + Point Operations**
- Vector setting (SVTCA, SPVTCA, SFVTCA)
- Zone switching (SZP0, SZP1, SZP2)
- RP0/RP1/RP2 (SRP0, SRP1, SRP2)
- MDRP, MDAP
- Touch bits, IUP

**Phase 3: CVT + MIRP**
- CVT loading, WCVTP, WCVTF, RCVT
- Cut-in logic, single-width check
- MIRP (all 32 variants)
- Storage (WS, RS)

**Phase 4: Rounding + Delta**
- All 7 rounding functions
- Super rounding (SROUND, S45ROUND)
- Delta P1/P2/P3, Delta C1/C2/C3
- AA, FLIPON/OFF

**Phase 5: Remaining Opcodes + FPGM/PREP**
- Function definitions (FDEF, CALL, LOOPCALL, IDEF)
- GETINFO, misc instructions
- PREP execution at size change
- FPGM execution at font load

**Phase 6: Polish + Edge Cases**
- Composite glyph hinting
- Twilight zone correct allocation
- Loop detectors, timeout guards
- Performance optimization
- Full 1970/1970 test pass
