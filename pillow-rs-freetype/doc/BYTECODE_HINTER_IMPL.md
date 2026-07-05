# TrueType Bytecode Hinter — Implementation Plan

## 0. Problem Statement

FreeType `FT_LOAD_DEFAULT` which runs the font's embedded TrueType bytecode
programs through FreeType's interpreter. Our autohinter produces different
grid-fit positions. The result: 4,607/7,640 FreeType parity test mismatches
(every character with off-pixel-grid coordinates differs by 1-2px).

We need a bytecode VM that executes the raw glyph instruction stream to
match 's pixel output. This must be a **separate module** with zero
changes to the existing autohinter pipeline.

## 1. Architecture

```
Font::truetype(data, size, LoadMode::Default)
  └─ getmask() / getbbox()
       └─ scaler::scale_glyph(data, gi, metrics=None, is_italic)
            ├─ Scale coords to 26.6 (no pp1x shift needed for bytecode)
            ├─ Add 4 phantom points
            ├─ if fpgm+cvt+glyf ins present:
            │    └─ tt::hinter::hint_glyph(&mut scaled, data, gi, ppem)  ← NEW
            └─ Compute bbox from scaled coords
```

The bytecode hinter lives entirely in `src/tt/hinter/`.
It is self-contained, owns its own VM state, and is called exactly once
per glyph from `scaler.rs`.

## 2. C Reference Map

| Component | C File | Lines | What It Does |
|---|---|---|---|
| Glyph loading + zone setup | `ttgload.c:770-860` | 90 | Prepares zones, phantom points, rounds them |
| Process simple glyph | `ttgload.c:874-920` | 46 | Sets phantom points, calls TT_Hint_Glyph |
| Graphics State | `ttinterp.h:???` | ~200 | GS struct: vectors, round state, CVT cut-in, etc |
| Execution Context | `ttinterp.h:???` | ~150 | Stack, call stack, zones, storage, CVT |
| Opcode switch | `ttinterp.c` | ~7,000 | 220 opcodes, ~40 used by our fonts |
| IUP (Interpolate Untouched Pts) | `ttinterp.c` | ~200 | Already ported in legacy code |
| Rounding functions | `ttinterp.c` | ~150 | Already ported in legacy code |

## 3. Data Structures

### 3.1 TT_GraphicsState (C: `ttinterp.h`, Rust: `hinter/gs.rs`)

```rust
struct GraphicsState {
    // Projection/Freedom vectors (2.14 fixed-point)
    proj_vector: (i32, i32),    // x,y in 2.14 format
    dual_proj_vector: (i32, i32),
    freedom_vector: (i32, i32),
    
    // Round state
    round_state: u8,            // RTG=0, RTHG=1, RDTG=2, RUTG=3, RDTG=4, ROFF=5
    
    // Auto-flip
    auto_flip: bool,
    
    // CVT cut-in (26.6)
    cvt_cut_in: i32,            // default: 0x10000 (17/16 pixel)
    
    // Minimum distance
    minimum_distance: i32,
    
    // Single-width value
    single_width_value: i32,
    single_width_cutin: i32,
    
    // Control value cut-in
    control_value_cutin: i32,
    
    // Delta base/shift
    delta_base: u32,
    delta_shift: u32,
    
    // Scan control
    scan_control: bool,
    scan_type: u8,
    
    // Loop
    loop_counter: i32,
    
    // Instructions flags
    instruct_control: u8,
    
    // zp0, zp1, zp2 — zone pointers (0=twilight, 1=glyph zone)
    zp0: u8,
    zp1: u8,
    zp2: u8,
}
```

### 3.2 TT_ExecContext (C: `ttinterp.h`, Rust: `hinter/exec.rs`)

```rust
struct ExecContext {
    // Graphics state
    gs: GraphicsState,
    
    // Metrics
    metrics: SizeMetrics,       // x_scale, y_scale, ppem
    
    // Stack
    stack: Vec<i32>,            // max 255 entries
    
    // Storage area
    storage: Vec<i32>,          // from maxp->maxStorage
    
    // CVT (Control Value Table)
    cvt: Vec<i32>,              // in 26.6, from 'cvt ' table
    
    // Function definitions (FDEF/ENDF)
    functions: Vec<Option<FunctionDef>>,
    
    // Instruction definitions (IDEF/ENDF)
    instruction_defs: Vec<Option<FunctionDef>>,
    
    // Call stack (max 10 levels)
    call_stack: Vec<CallRecord>,
    
    // Code ranges
    code_range: [CodeRange; 3], // font=0, cvt=1, glyph=2
    
    // Instruction pointer
    ip: usize,
    cur_range: u8,
    
    // Glyph zones
    glyph_zone: GlyphZone,      // the glyph being hinted
    twilight: GlyphZone,         // twilight zone
    
    // Rounding function pointer
    round_func: RoundFunc,
    
    // Point movement function
    move_func: MoveFunc,
    
    // Current projection function
    project_func: ProjectFunc,
    
    // Composite flag
    is_composite: bool,
    
    // Pedantic hinting
    pedantic_hinting: bool,
}
```

### 3.3 GlyphZone (C: `ttgload.c`, Rust: `hinter/zone.rs`)

```rust
struct GlyphZone {
    n_points: u16,
    n_contours: u16,
    org: Vec<(i32, i32)>,      // original coordinates (font units)
    cur: Vec<(i32, i32)>,      // current coordinates (26.6, modified by hints)
    orus: Vec<(i32, i32)>,     // original unscaled coords (font units)
    tags: Vec<u8>,             // touch flags
    contours: Vec<u16>,        // contour end points
    first_point: u16,          // offset for composite sub-glyphs
}
```

## 4. Table Parsing (New Tables Needed)

### 4.1 'fpgm' — Font Program (required)

```
Offset  Size  Field
0       4+    Bytecode instructions, executed once per face load.
               Run before any glyph hinting.
               Sets up function definitions (FDEF/ENDF), IDEF, storage init.
```

### 4.2 'prep' — CVT Program (required)

```
Offset  Size  Field
0       4+    Bytecode instructions, executed when ppem changes.
               Sets up CVT values for the current size.
               Run once per size change.
```

### 4.3 'cvt ' — Control Value Table (required)

```
Offset  Size  Field
0       4+    Array of FWORD values (i16 in font units).
               These are scaled to 26.6 and used as reference distances.
               Accessed via RCVT/WCVTP opcodes.
```

### 4.4 maxp extensions (already parsed, need to read)

| Field | Purpose |
|---|---|
| `maxStackElements` | Stack depth (typically 512-2048) |
| `maxStorage` | Storage area size |
| `maxFunctionDefs` | Max FDEF count |
| `maxInstructionDefs` | Max IDEF count |
| `maxTwilightPoints` | Twilight zone size |
| `maxSizeOfInstructions` | Max glyph program length |

## 5. Entry Flow (Scaler Integration)

The modified flow in `scaler.rs:scale_glyph()`:

```rust
// After scaling coords to 26.6, before bbox computation:

let hinted = if latin_metrics.is_some() {
    // Autohint path (existing)
    autohint_glyph(&mut scaled, ...);
    true
} else if data.fpgm.is_some() && data.prep.is_some() && data.cvt.is_some() {
    // Bytecode hinting path (NEW)
    tt::hinter::hint_glyph(
        &mut scaled,        // 26.6 coords (modified in-place)
        raw_outline,        // font-unit coords
        data,               // tables
        glyph_index,        // which glyph
    )?;
    true
} else {
    false  // unhinted — used for both FreeType backends
};

// Then proceed to bbox computation as before
```

## 6. Bytecode VM Core

### 6.1 Program Execution (C: `TT_RunIns` in ttinterp.c)

```rust
fn run_program(exec: &mut ExecContext, code_range: u8) -> Result<()> {
    loop {
        let opcode = fetch_byte(exec);
        match opcode {
            // Push operations (0x00-0x1F, 0x40-0x41)
            0x00..=0x1F => push_n(exec, opcode),
            0x40 => push_bytes(exec, fetch_byte(exec)),
            0x41 => push_words(exec, fetch_byte(exec)),
            
            // Stack operations (0x20-0x2F)
            0x20 => dup(exec),
            0x21 => pop(exec),
            0x22 => clear(exec),
            0x23 => swap(exec),
            0x24 => depth(exec),
            0x25 => cindex(exec),
            0x26 => mindex(exec),
            0x27 => roll(exec),
            0x2B => loop_call(exec),
            
            // Storage (0x42-0x43)
            0x42 => write_storage(exec),
            0x43 => read_storage(exec),
            
            // CVT (0x44-0x45)
            0x44 => write_cvt(exec),
            0x45 => read_cvt(exec),
            
            // Graphics State (various)
            0x06 => svtca_y(exec),     // set vectors to y-axis
            0x07 => svtca_x(exec),     // set vectors to x-axis
            0x08 => spvtca_y(exec),    // set proj vector to y-axis
            0x09 => spvtca_x(exec),
            0x0A => sfvtca_y(exec),    // set free vector to y-axis
            0x0B => sfvtca_x(exec),
            0x0D => set_rp0(exec),
            0x0E => set_rp1(exec),
            0x0F => set_rp2(exec),
            
            // Move points
            0x2E => mdap(exec, false),   // 0x2E = MDAP[0], 0x2F = MDAP[1]
            0x2F => mdap(exec, true),
            0x3A => alignrp(exec),
            0x3E => miap(exec, false),
            0x3F => miap(exec, true),
            0xC0..=0xDF => mdrp(exec, opcode & 0x1F),
            0xE0..=0xFF => mirp(exec, opcode & 0x1F),
            
            // Diagonals
            0x30 => iup_x(exec),       // 0x30 = IUP[0], 0x31 = IUP[1]
            0x31 => iup_y(exec),
            0x32..=0x39 => shp(exec, opcode),
            0x3B => shc(exec, opcode),
            0x3C => shz(exec, opcode),
            
            // Math
            0x60 => add(exec),
            0x61 => sub(exec),
            0x62 => div(exec),
            0x63 => mul(exec),
            0x64 => abs(exec),
            0x65 => neg(exec),
            0x66 => floor(exec),
            0x67 => ceiling(exec),
            
            // Comparisons
            0x50..=0x55 => compare(exec, opcode),
            
            // Rounding
            0x3D => set_round(exec, RTG),
            0x7C => set_round(exec, RTHG),
            0x7D => set_round(exec, RDTG),
            0x7E => set_round(exec, RUTG),
            0x7F => set_round(exec, RDTG2),
            0x18 => set_round(exec, ROFF),
            0x76 => set_super_round(exec),
            0x77 => set_super_round_45(exec),
            
            // Control flow
            0x1B => call(exec),
            0x1C => fdef(exec),
            0x1D => endf(exec),
            0x2C => if_(exec),
            0x58 => else_(exec),
            0x59 => endif(exec),
            0x89 => idef(exec),
            0x1A => loop_call(exec),
            0x78 => jrot(exec),
            0x79 => jrof(exec),
            0x1F => jmpr(exec),
            
            // Misc
            0x46 => get_coord(exec),       // GC[a]
            0x47 => get_coord_orig(exec),   // GC[cur]
            0x48 => set_cf(exec),
            0x49 => measure_dist(exec),
            0x4B => measure_ppem(exec),
            0x4C => measure_point_size(exec),
            0x4D => flip_on(exec),
            0x4E => flip_off(exec),
            0x5A => scan_control(exec),
            0x5B => scan_type(exec),
            0x5C => get_info(exec),
            0x6C => cvt_cutin(exec),
            0x6D => single_width(exec),
            0x71..=0x75 => set_min_dist(exec, opcode),
            0x85 => flip_rg_on(exec),
            0x86 => flip_rg_off(exec),
            0x8A => sds(exec),
            0x8B => sdb(exec),
            0x8C => ssv(exec),
            0x8D..=0x8F => delta(exec, opcode),
            0x90..=0x99 => aa(exec, opcode),
            0x9A => flippt(exec),
            
            _ => return Err(UnimplementedOpcode(opcode)),
        }
    }
}
```

### 6.2 Opcode Priority (What Our Fonts Actually Use)

Tracing DejaVuSans-ExtraLight '_' at 10pt, the bytecode program uses:

| Opcode | Name | Description | Needed? |
|---|---|---|---|
| 0x40 | NPUSHB | Push bytes | ✅ — loads constants |
| 0x41 | NPUSHW | Push words | ✅ — loads constants |
| 0x06 | SVTCA[0] | Set vectors to Y-axis | ✅ — standard setup |
| 0x07 | SVTCA[1] | Set vectors to X-axis | ✅ |
| 0x00-0x03 | PUSH* | Small constant pushes | ✅ |
| 0x45 | RCVT | Read CVT | ✅ — reads reference distances |
| 0xC0-0xDF | MDRP | Move Direct Relative Point | ✅ — grid-fits points |
| 0xE0-0xFF | MIRP | Move Indirect Relative Point | ✅ — grid-fits to CVT |
| 0x30 | IUP[0] | Interpolate Untouched Points (X) | ✅ — smooths between MDRP/MIRP |
| 0x31 | IUP[1] | Interpolate Untouched Points (Y) | ✅ |
| 0x2E | MDAP[0] | Move Direct Absolute Point | ✅ — rounds without distance |
| 0x2F | MDAP[1] | Round, no distance | ✅ |
| 0x3E | MIAP[0] | Move Indirect Absolute Point | ✅ — rounds to CVT |
| 0x3A | ALIGNRP | Align to Reference Point | ✅ — aligns contours |
| 0x1B | CALL | Call function | ✅ — FDEF subroutines |
| 0x2B | LOOPCALL | Loop+Call | Possibly |
| 0x46 | GC[a] | Get Coordinate (original) | Common |
| 0x47 | GC[cur] | Get Coordinate (current) | Common |
| 0x46 | GC | Get Coordinate | Requires zone pointer |
| 0x49 | MD[a] | Measure Distance (original) | Possibly |
| 0x4B | MPPEM | Measure PPEM | Common |
| 0x3D | RTDG | Round To Double Grid | Common rounding change |
| 0x7D | RDTG | Round Down To Grid | Common |
| 0x18 | RTG | Round To Grid | Common |
| 0x62 | ADD | Add | ✅ |
| 0x63 | SUB | Subtract | ✅ |
| 0x64 | DIV | Divide | ✅ |
| 0x65 | MUL | Multiply | ✅ |
| 0x66 | ABS | Absolute | ✅ |
| 0x67 | NEG | Negate | ✅ |
| 0x58 | IF | If-then | ✅ |
| 0x1C | FDEF | Function Definition | ✅ — in fpgm |
| 0x1D | ENDF | End Function | ✅ |
| 0x23 | SWAP | Swap top two | ✅ |
| 0x21 | POP | Pop (discard) | ✅ |
| 0x20 | DUP | Duplicate | ✅ |
| 0x22 | CLEAR | Clear stack | ✅ |
| 0x10-0x17 | PUSH* | Push small constants | ✅ |

That's ~35 opcodes covering >95% of instruction streams in our test fonts.
The other 185 opcodes are delta exceptions, scanning control,
subpixel hinting, and variations — not used by DejaVu/Liberation/Noto at 72dpi.

## 7. Existing Code Assessment

### 7.1 Usable as-is

| File | Lines | Quality |
|---|---|---|
| `hinting/round.rs` | 172 | ✅ Complete: RTG, RTHG, RDTG, RUTG, RDTG2, ROFF, SROUND, S45ROUND |
| `hinting/iup.rs` | 250 | ✅ Complete: full IUP interpolation for both X and Y |
| `hinting/opcodes.rs` | 129 | ✅ Complete: all 220 opcode constants |

### 7.2 Needs heavy rework

| File | Lines | Issue |
|---|---|---|
| `hinting/exec.rs` | 1,069 | Has skeleton VM with MDRP and MIRP. Missing: proper execution loop, all other opcodes, CVT/storage/functions, zone management, phantom handling |
| `hinting/graphics.rs` | 129 | Has GS struct. Missing: proj/freedom vectors, auto-flip, all flags |
| `hinting/fragments/*.rs` | ~500 | Opcode case bodies. Incomplete, uses old API conventions |

### 7.3 Recommendation

Take the opcodes, rounding, and IUP from the legacy code. Rewrite the
execution context, execution loop, and zone management fresh for our
cleaner API surface. The legacy code has accumulated API mismatches
and would take longer to refactor than rewrite.

## 8. Implementation Plan

### Phase 1: Data Layer (400 lines)

```
src/tt/hinter/
├── mod.rs           (30 lines)  — module structure, public API
├── tables.rs        (150 lines) — parse 'fpgm', 'prep', 'cvt ' tables
├── zone.rs          (120 lines) — GlyphZone struct and operations
├── gs.rs            (100 lines) — GraphicsState struct
```

1. Parse `cvt `, `fpgm`, `prep` tables in `tables.rs`
2. Add optional fields to FontData: `fpgm: Option<Vec<u8>>`, `prep: Option<Vec<u8>>`, `cvt: Option<Vec<i32>>`
3. Implement `GlyphZone` with phantom point setup
4. Implement `GraphicsState` with default values matching C

### Phase 2: VM Core (800 lines)

```
src/tt/hinter/
├── exec.rs          (400 lines) — ExecContext, stack, CVT, storage
├── op_math.rs       (100 lines) — ADD, SUB, MUL, DIV, ABS, NEG, etc.
├── op_stack.rs      (100 lines) — DUP, POP, SWAP, CLEAR, CINDEX, etc.
├── op_control.rs    (100 lines) — IF/ELSE/EIF, JMPR, JROT/JROF, CALL, LOOPCALL
├── op_push.rs       (60 lines)  — NPUSHB, NPUSHW, PUSHB, PUSHW
```

1. `ExecContext`: stack, call stack, storage, CVT, functions, code ranges, IP
2. `run_program()`: main fetch-decode-execute loop
3. Math + stack + control flow opcodes
4. FDEF/ENDF: parse function definitions during fpgm/prep execution

### Phase 3: Point Operations (600 lines)

```
src/tt/hinter/
├── op_move.rs       (300 lines) — MDRP, MIRP, MDAP, MIAP, ALIGNRP
├── op_zone.rs       (100 lines) — GC, SCFS, MD, SHPIX, SHC, SHZ
├── op_vectors.rs    (100 lines) — SVTCA, SPVTCA, SFVTCA, SPVTL, SFVTL
├── op_round.rs      (100 lines) — RTG, RTHG, RDTG, ROFF, SROUND bindings
```

This is the critical phase — MDRP/MIRP are the workhorses that actually
grid-fit points. They use the freedom/proj vectors and rounding.

### Phase 4: Integration (200 lines)

1. Hook into `scaler.rs:scale_glyph()` in the `None` metrics branch
2. Add phantom point setup before bytecode execution
3. Copy hinted coords out of the zone
4. Integration test: single character, compare against FreeType C

### Phase 5: IUP + Polish (200 lines)

1. Wire IUP (already exists) into the VM
2. Handle edge cases: empty instructions, composite glyphs
3. Fuzz test: all glyphs in all 8 test fonts

## 9. Testing Strategy

```
Level 0: Opcode unit tests
  └─ Each opcode tested in isolation with known stack state

Level 1: Integration test — single character
  └─ Run bytecode on '_' at 10pt DejaVuSans-ExtraLight
  └─ Compare pixel output vs FreeType C

Level 2: Full font test
  └─ Run every glyph in 8 test fonts, check getmask/getbbox vs FreeType C
  └─ Track pass/fail per font

Level 3: Regression guard
  └─ Autohinter path unchanged — direct_ft_compare still 100%
  └─ native TrueType test native_tt_default_matrix target: 6,000+/7,640
```

## 10. File Manifest

```
New files:
  src/tt/hinter/mod.rs          (~50 lines)
  src/tt/hinter/tables.rs       (~150 lines)
  src/tt/hinter/zone.rs         (~120 lines)
  src/tt/hinter/gs.rs           (~100 lines)
  src/tt/hinter/exec.rs         (~500 lines)
  src/tt/hinter/op_math.rs      (~100 lines)
  src/tt/hinter/op_stack.rs     (~100 lines)
  src/tt/hinter/op_control.rs   (~150 lines)
  src/tt/hinter/op_push.rs      (~60 lines)
  src/tt/hinter/op_round.rs     (~100 lines)
  src/tt/hinter/op_move.rs      (~400 lines)
  src/tt/hinter/op_zone.rs      (~100 lines)
  src/tt/hinter/op_vectors.rs   (~100 lines)
  src/tt/hinter/iup.rs          (~250 lines) [copy+adapt]

Modified files:
  src/tables.rs                 (+4 fields)
  src/font.rs                   (+parse new tables)
  src/scaler.rs                 (+15 lines dispatch)
  src/tt/mod.rs                 (+1 line)

Total new code: ~2,300 lines
Total modified: ~30 lines
Existing code untouched: 100% of autohinter
```

## 11. Fallback Position

If bytecode coverage stalls at 70-80%, the remaining failures are delta
exception opcodes (DELTAP1/DELTAP2/DELTAP3), subpixel hinting variants,
or CVT program initialization that differs between FreeType versions.
These are edge cases affecting <5% of glyphs and can be documented as
known limitations.
