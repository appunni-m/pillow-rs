# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State (all references use FreeType 2.14.3)

| Backend | Pass | Total | Rate | Reference generator |
|---------|------|-------|------|---------------------|
| PIL | 1170 | 1910 | 61.3% | PIL 12.0 `getmask`/`getbbox` |
| FreeType | 1087 | 1910 | 56.9% | `/tmp/gen_ft_refs` (FT_LOAD_RENDER from vendored 2.14.3) |

## Edge-Level Diagnostics (2026-06-27)

Traced 'A' (DejaVuSans 10pt, 2 contours, 11 points) through our autohinter:

**VERT edges (horizontal stems):**
```
edge[0]: fpos=0   opos=0   pos=0    flags=DONE   ← baseline
edge[1]: fpos=383 opos=128 pos=134  link=2       ← lower x-height  
edge[2]: fpos=551 opos=185 pos=186  link=1       ← upper x-height
edge[3]: fpos=1493 opos=500 pos=512  flags=DONE   ← top
```

**HORZ edges (vertical stems): 0 segments → 0 edges**
'A' has no horizontal flat segments — purely diagonal strokes.
FreeType also produces 0 HORZ edges for this glyph profile.

**Pixel comparison:**
```
  row0: OUR=00 00 51 ff 28 00 00  PIL=00 00 51 ff 28 00 00  MATCH
  row1: OUR=00 00 aa b7 80 00 00  PIL=00 00 aa c3 80 00 00  DIFF
  row2: OUR=00 0c e3 21 d8 00 00  PIL=00 0c e5 26 d8 00 00  DIFF
```

**Finding:** Edge positions (128→134, 185→186, 500→512) are in the right
range. Row 0 matches perfectly, but intermediate rows differ. This means
the problem is in **subpixel point interpolation** (align_strong_points,
align_weak_points) AFTER edges are fixed. The stem at 134→186 (52 units =
0.8px) produces different coverage because the intermediate point positions
between edges are interpolated differently than FreeType does.

**Root cause location:** `align_strong_points` and `align_weak_points`
functions. These interpolate non-edge points based on edge positions. Our
interpolation produces slightly different subpixel positions, which the
rasterizer (`grays.rs`) renders with different coverage values.

## Version Audit

All three reference sources use FreeType 2.14.3:

| Component | FreeType | How to verify |
|-----------|----------|---------------|
| PIL 12.2.0 (bundled) | 2.14.3 | `python3 -c 'from PIL import _imagingft; print(_imagingft.freetype2_version)'` |
| Local C build | 2.14.3 | Built from `pillow-rs-freetype/freetype/` via cmake, installed to `~/.local` |
| Vendored C source | 2.14.3 | `head -3 pillow-rs-freetype/freetype/README` |
| Our Rust port | 2.14.1 | Algorithm baseline from VER-2-14-1 tag |

## Key Finding: No Algorithm Changes in autofit

Diff between FreeType 2.14.1 and 2.14.3 `src/autofit/aflatin.c`:
- Zero algorithm changes
- Only overflow-safety macros: `SUB_LONG`, `ADD_LONG`, `MUL_LONG`, `FT_PIX_ROUND_LONG`
- These don't affect i32 arithmetic — they're for 16-bit `FT_Pos` compatibility

→ Bugs are in our port's implementation details, not version differences.

## Reference Regeneration

### PIL references (`coverage_matrix.json`)

```bash
python pillow-rs-freetype/scripts/generate_font_refs.py
```

Uses PIL 12.2.0's `getmask()`/`getbbox()` directly. Output: 1910 rows.
Requires: `pip install Pillow>=12.2.0`.

### FreeType raw references (`coverage_matrix_ft.json`)

```bash
# Step 1: Build FreeType 2.14.3 from vendored source
cd pillow-rs-freetype/freetype && mkdir -p build && cd build
cmake .. -DCMAKE_INSTALL_PREFIX="$HOME/.local" -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=ON -DFT_DISABLE_ZLIB=ON -DFT_DISABLE_PNG=ON \
  -DFT_DISABLE_BZIP2=ON -DFT_DISABLE_BROTLI=ON -DFT_DISABLE_HARFBUZZ=ON
cmake --build . -j$(nproc) && cmake --install .

# Step 2: Build the reference generator binary
gcc -o /tmp/gen_ft_refs /path/to/gen_ft_refs.c \
  -I$HOME/.local/include/freetype2 -L$HOME/.local/lib -lfreetype \
  -Wl,-rpath,$HOME/.local/lib

# Step 3: Run the generator (Python wrapper calls /tmp/gen_ft_refs)
python pillow-rs-freetype/scripts/gen_ft_matrix.py
```

`/tmp/gen_ft_refs` is compiled from `pillow-rs-freetype/scripts/gen_ft_refs.c`.
It calls `FT_Load_Glyph(face, idx, FT_LOAD_RENDER)` and outputs per-glyph
bitmap pixels, metrics, and bbox.

## Trace Tools

### C trace tool (`/tmp/trace_edges`)

```bash
gcc -o /tmp/trace_edges pillow-rs-freetype/scripts/trace_edges.c \
  -I pillow-rs-freetype/freetype/include \
  -I pillow-rs-freetype/freetype/src/autofit \
  -L $HOME/.local/lib -lfreetype -Wl,-rpath,$HOME/.local/lib

# Trace a specific glyph
/tmp/trace_edges <font.ttf> <size_pt> <char>
```

Outputs: glyph index, point count, bitmap dimensions, all pixel bytes,
outline coordinates (26.6 format), contour ends.

### Rust trace tools

```bash
# Dump all glyphs (with bitmaps)
cargo run --example dump_all_masks -- <font.ttf> <size> [pil|ft]

# Trace raster coordinates
cargo run --example trace_raster -- <font.ttf> <size> <char>

# Dump outline before autohinter
cargo run --example dump_outline -- <font.ttf> <size> <char>
```

## Task Breakdown

### 1. Trace Outline Coordinates (Pre-Autohinter)

- [ ] Modify `dump_outline` to print 26.6 coordinates in same format as C trace
- [ ] Compare outline coordinates for 'A' at DejaVuSans 10pt (both should match)
- [ ] If outlines differ, fix the scaler (FT_MulFix, y_scale adjustment)

### 2. Trace Edge Positions (Post-Autohinter)

- [ ] Add edge-position dump to C trace tool (requires including `afhints.h` internals)
- [ ] Add edge debug logging to Rust `hint_edges` function
- [ ] Compare edge `fpos`, `opos`, `pos` for Horz dim (vertical edges) of 'A'
- [ ] Find first edge position mismatch

### 3. Fix Edge Computation

- [ ] `compute_segments(Horz)` — check segment direction detection vs C
- [ ] `extract_widths(Horz)` — verify standard width computation
- [ ] `compute_edges(Horz)` — verify edge assembly from segments
- [ ] `hint_edges(Horz)` — verify edge grid-fitting
- [ ] `snap_width` — verify width snapping to standard widths

### 4. Fix Vertical Dimension (Blue Zones)

- [ ] `compute_blue_edges` — verify blue zone assignment
- [ ] `hint_edges(Vert)` — verify Phase 1 blue alignment

### 5. Fix Advance Width

- [ ] Phantom-point advance adjustment (afloader.c:395-490)
- [ ] Fixes ~30 right-edge bbox failures

### 6. Validation

- [ ] All 1910 PIL tests pass
- [ ] All 1910 FreeType tests pass
