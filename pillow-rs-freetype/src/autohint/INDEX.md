# Autohinter Port — Complete History & Architecture

Pillow-rs FreeType autohinter (Latin script). Port of FreeType 2.14.1 `src/autofit/`.
Target: PIL 12.2.0 `ImageFont.getmask()` = `FT_LOAD_DEFAULT|RENDER` on bytecode-stripped fonts.

---

## Progress Timeline

| # | Milestone | Passes | Δ | Key Discovery |
|---|-----------|--------|---|---------------|
| 0 | Initial baseline | 405 | — | Bare pipeline: segments→edges→hint→points |
| 1 | Segment filtering (Phase C) | 416 | +11 | Height/delta thresholds reduce edge noise |
| 2 | `ft_mul_div` in strong-IP | 421 | +5 | Bit-exact interpolation |
| 3 | x-height scale adjustment | 489 | +68 | Nudge y_scale so x-height blue aligns to pixel grid |
| 4 | Per-glyph orientation + `abs(major_dir)` | 470 | +23 | Outline area → CW/CCW → major_dir; must ABS(major_dir) for segment matching |
| 5 | **Phase 1 blue alignment actually applied** | 746 | +279 | `blue` variable was never set for non-linked edges — blues never snapped |
| 6 | Link_segments cleanup | 798 | +52 | Disabled broken stem pairs; noise reduction |
| 7 | Edge sorting + width data + SNAP removal | 836 | +38 | Edges must be sorted by fpos; `other_flags=STEM_ADJUST` only for smooth subpixel |
| 8 | **snap_width in smooth branch** | 942 | +106 | FreeType calls `snap_width` in BOTH smooth and strong — we skipped it in smooth |
| 9 | Phase 3 'm' symmetry | 943 | +1 | Equalize outer stems around middle for 3-stem glyphs |
| 10 | Directionless segments 2nd pass | 946 | +3 | Catch segments without direction, attach to existing edges |
| 11 | **Mask positioning at `bbox_x_min`** | 1051 | +105 | Raster was placed at column 0; must offset by `scaled.bbox_x_min` to match PIL's bitmap_left positioning |

**Net gain: 405 → 1051 (+646, +159%)**

---

## Failed experiments & reversions

| Attempt | Effect | Why it failed |
|---------|--------|---------------|
| `link_segments` enabled without width scaling | 405→283(-122) | max_width in font units vs per-glyph distances mismatch |
| `link_segments` with all fixes but early | 746→739(-7) | Minor regression; eventually works with edge sorting |
| LSB delta applied to outline points via += | 946→825(-121) | Wrong sign; should be -= not += |
| LSB delta applied via -= | 825 | same | Fundamental approach wrong — should be mask-level, not outline-level |

---

## Critical Bug: `major_dir` not absolutified

**Symptom:** All glyphs pass at ~470 with hardcoded `major_dir=Right` for VERT, but regression to 251 when computing per-glyph orientation.

**Root cause (aflatin.c:1577):** `major_dir = (AF_Direction)FT_ABS(axis->major_dir)` — the segment direction matching in `compute_segments` uses the **absolute** major_dir (Up/Right), not the raw direction (Left/Down). Our code compared `abs_dir(out_dir)` with raw `major_dir` instead of `abs_dir(major_dir)`.

**Fix:** `let major_dir = abs_dir(raw_major_dir)` in compute_segments.

**Verification:** The raw orientation (CW=TT=Left, CCW=PS=Right) is stored in `axis.major_dir` for `link_segments` and `compute_blue_edges` (which compare raw edge.dir with raw major_dir). The absolutified version is only for segment detection.

---

## Critical Bug: Phase 1 blue never applied

**Symptom:** Blue zones computed correctly (verified against FreeType trace), `compute_blue_edges` assigns correct `blue_edge.fit`, but hint_edges never snaps to it.

**Root cause:** In `hint_edges` Phase 1, the `blue` variable was initialized to `None` and only set inside the `if link != usize::MAX` neutral-dedup block. For edges without stem links (most edges), `blue` stayed `None`, and the condition `if edge1_idx.is_none() { continue }` skipped the blue alignment.

**Fix:** Moved `edge1_idx = Some(i); blue = Some(b)` OUTSIDE the link-dedup block, so non-linked edges also get their blue applied.

---

## Critical Bug: `snap_width` missing in smooth branch

**Symptom:** `|` glyph X stem width = 56 instead of FreeType's 61. The width wasn't being snapped to the standard width (60).

**Root cause:** `compute_stem_width` smooth branch (no SNAP flags) had `dist < 56 → dist = 56` but then skipped `snap_width(std_widths, dist)`. The C code calls `snap_width` in BOTH smooth and strong branches.

**Fix:** Added `if !std_widths.is_empty() { dist = snap_width(std_widths, dist); }` after the smooth-branch quantization checks.

---

## Critical Bug: Mask positioning at wrong offset

**Symptom:** All glyphs shifted 1px left vs PIL reference. `|` coverage values matched FreeType (244) but bar was at column 0 instead of column 1.

**Root cause:** The scaler translates the hinted outline by `off_x = FT_PIX_FLOOR(x_min)`, producing coordinates relative to pixel boundary. The outline's `bbox_x_min` correctly reports the pixel offset (e.g., 1 for `|`). But `getmask` placed the raster at column 0 regardless of `bbox_x_min`.

**Fix:** In `getmask`, offset the raster placement by `scaled.bbox_x_min` pixels: `dst = y * new_width + bbox_x_min`. This matches PIL's convention where glyph content starts at `bitmap_left` rather than at mask column 0.

---

## Key architectural facts discovered

### PIL does NOT use FreeType's bitmap rasterizer
PIL's `getmask` output is significantly different from `FT_LOAD_RENDER` bitmap. PIL renders the outline through `_imagingft.c`'s own scan converter, not FreeType's `ftgrays.c`. The reference fixtures are **PIL getmask output**, not raw FreeType bitmap.

### Our rasterizer IS byte-accurate to FreeType
Verified by comparing pixel-by-pixel output for the `|` glyph with identical outline coordinates. Our `grays.rs` matches `ftgrays.c`. Coverage differences come from subpixel edge positions, not rasterizer bugs.

### The standard width collection was correct
Our `metrics_init_widths` produces `horizontal widths: 194` and `vertical widths: 156` matching FreeType's trace exactly. Earlier confusion (864) was from looking at an old code version.

### PIL adds left padding equal to `bitmap_left`
The 1px shift in our output wasn't an autohinter edge error — it was the mask assembly placing the raster at the wrong horizontal offset. PIL's mask width = `max(advance, ink_right - bitmap_left)` and content is offset by `bitmap_left`.

---

## Current State (1051/1910, 55.0%)

| Dimension | Pass | Total | Rate |
|-----------|------|-------|------|
| Non-glyph (metrics/name/length) | 30 | 30 | 100% |
| getbbox | 778 | 940 | 82.8% |
| getmask | 243 | 940 | 25.9% |

### Remaining failures (859)

| Type | Count | Primary cause |
|------|-------|---------------|
| SHA-only (bbox correct) | ~697 | Stem quantization produces slightly different subpixel widths |
| Bbox + SHA | ~162 | Edge miscounting/collapsing for small glyphs and serif fonts |

### Characters passing all getmask tests (both fonts, all sizes)
`/`, `\`, `I`, `l`, `(` — 5 straight-line glyphs

### Characters failing all getmask tests
37 characters — includes most curved letters (`A`, `G`, `Q`, `R`, `S`, `a`, `g`, `m`, `n`, `s`, etc.) and digits 2-9.

---

## Pipeline Architecture

```
Font::truetype()  [once per font+size]
├─ metrics_init_widths()     — scan 'o' glyph → axis[].widths[] (font units)
├─ metrics_init_blues()      — scan 6 Latin char strings → axis[].blues[]
└─ metrics_scale_dim()       — x-height scale opt + scale widths/blues → 26.6

scale_glyph()  [per glyph]
├─ Scale font units → 26.6 using adjusted y_scale
├─ apply_hints():
│   ├─ reload(outline)      — load into hints, compute directions, orientation
│   ├─ For VERT dim (Y-axis / horizontal edges):
│   │   ├─ compute_segments()  — segment detection with height extension
│   │   ├─ link_segments()     — stem pairing with C-exact scoring
│   │   ├─ compute_edges()     — edge grouping with dynamic thresholds + 2nd pass
│   │   ├─ compute_blue_edges()— assign edges to nearest active blue zone
│   │   ├─ hint_edges()        — Phase 1(blues)+Phase 2(stems)+Phase 3(m)+Phase 4(non-stem)
│   │   ├─ align_edge_points() — snap edge points to edge positions
│   │   ├─ align_strong_points()— IP (interpolate points between edges)
│   │   └─ align_weak_points() — IUP (interpolate untouched points in storage order)
│   ├─ For HORZ dim (X-axis / vertical edges): same minus blue edges
│   └─ save_to_outline()
├─ Compute CBox → pixel bbox
└─ Translate outline to pixel-bbox origin

Font::getmask()  [per glyph text]
├─ scale_glyph() → ScaledGlyph
├─ grays::rasterize(outline) → raster at subpixel positions
└─ Assemble mask: offset raster by bbox_x_min/bbox_y_min, pad to advance width
```

## Key Files

| File | Purpose |
|------|---------|
| `src/autohint/latin.rs` | Main autohinter (~2400 lines, Phases A–F + all fixes) |
| `src/autohint/types.rs` | Data structures (GlyphHints, AFEdge, AfLatinMetrics, etc.) |
| `src/autohint/loader.rs` | `reload()` — outline loading + direction computation + orientation |
| `src/autohint/mod.rs` | Module exports |
| `src/scaler.rs` | `scale_glyph()` + `autohint_glyph()` + CBox/bbox computation |
| `src/font.rs` | `Font::truetype()` (metric init) + `getmask()`/`getbbox()` |
| `src/grays.rs` | Smooth rasterizer (byte-accurate to ftgrays.c) |
| `freetype/src/autofit/aflatin.c` | FreeType 2.14.1 reference (vendored) |
| `freetype/src/autofit/afhints.c` | FreeType 2.14.1 glyph hints reference |
| `freetype/src/autofit/afloader.c` | FreeType 2.14.1 loader reference |
