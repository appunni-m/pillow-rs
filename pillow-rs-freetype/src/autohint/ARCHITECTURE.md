# FreeType Autohinter Architecture (2.14.1)

## Top-level pipeline (`af_loader_load_glyph`, afloader.c:208)

Invoked per-glyph by the autofit module when `FT_LOAD_DEFAULT` is used on a bytecode-stripped font. The full pipeline:

```
┌─────────────────────────────────────────────────────────────────┐
│  1. scaler setup: x_scale, y_scale, x_delta=0, y_delta=0      │
│     render_mode = FT_LOAD_TARGET_MODE(load_flags)              │
│     scaler.flags = 0                                            │
│                                                                 │
│  2. style_metrics_scale(metrics, &scaler)                       │
│     = af_latin_metrics_scale_dim  (aflatin.c:1178)             │
│     → x-height scale optimization + scales widths + blues      │
│                                                                 │
│  3. style_hints_init(hints, metrics)                            │
│     = af_latin_hints_init  (aflatin.c:2646)                    │
│     → sets hints->other_flags, scale, scaler                   │
│                                                                 │
│  4. FT_Load_Glyph(face, gid, LOAD_NO_SCALE)                    │
│     → loads raw outline in font units                           │
│                                                                 │
│  5. style_hints_apply(gid, hints, outline, metrics)             │
│     = af_latin_hints_apply  (aflatin.c:4837)                   │
│     → the main work (see below)                                 │
│                                                                 │
│  6. Adjust advance metrics from hinted edge positions           │
└─────────────────────────────────────────────────────────────────┘
```

## Main work: `af_latin_hints_apply` (aflatin.c:4837)

```
af_glyph_hints_reload(hints, outline)
│   af_glyph_hints_init: sets major_dir per orientation
│   scale + copy coords, link contours, compute directions
│
├─ HORZ dim (horizontal edges / vertical stems — X-axis):
│   af_latin_hints_detect_features(hints, width_count, widths, HORZ)
│   │   af_latin_hints_compute_segments(hints, HORZ)
│   │   af_latin_hints_link_segments(hints, width_count, widths, HORZ)
│   │   af_latin_hints_compute_edges(hints, HORZ)
│   │
│   └─ (NO blue edges for HORZ — blue zones are only for VERT)
│
├─ VERT dim (horizontal edges / horizontal stems — Y-axis):
│   │
│   │  [tilde/top/bottom contour adjustments — skipped for normal glyphs]
│   │
│   │  af_latin_hints_detect_features(hints, width_count, widths, VERT)
│   │  │   af_latin_hints_compute_segments(hints, VERT)
│   │  │   af_latin_hints_link_segments(hints, width_count, widths, VERT)
│   │  │   af_latin_hints_compute_edges(hints, VERT)
│   │
│   └── af_latin_hints_compute_blue_edges(hints, metrics)
│       (only for base glyphs, non-composites)
│
├─ For EACH dim [HORZ, VERT]:
│   1. af_latin_hint_edges(hints, dim)
│   2. af_glyph_hints_align_edge_points(hints, dim)
│   3. af_glyph_hints_align_strong_points(hints, dim)
│   4. af_glyph_hints_align_weak_points(hints, dim)
│   5. af_glyph_hints_apply_vertical_separation_adjustments(hints, dim)
│      (splits vertical stacking at ~, accent glyphs)
│
└── af_glyph_hints_save(hints, outline)
```

## Flag control (aflatin.c:2646 `af_latin_hints_init`)

```c
hints->other_flags = AF_LATIN_HINTS_HORZ_SNAP
                   | AF_LATIN_HINTS_VERT_SNAP
                   | AF_LATIN_HINTS_STEM_ADJUST;

// If anti-aliased (not mono) and not light mode:
if (render_mode != FT_RENDER_MODE_MONO) {
    // Disable SNAP for smooth anti-aliased hinting
    // (aflatin.c ~line 342-349 via hints_init with scaler.render_mode)
    hints->other_flags &= ~(AF_LATIN_HINTS_HORZ_SNAP | AF_LATIN_HINTS_VERT_SNAP);
}
```

**KEY**: For smooth (anti-aliased) rendering, HORZ_SNAP and VERT_SNAP are CLEARED. Only STEM_ADJUST remains. This switches `compute_stem_width` from strong hinting (snap to pixel) to smooth hinting (subpixel quantization).

## detect_features (aflatin.c:2506)

```
af_latin_hints_detect_features(hints, width_count, widths, dim):
    compute_segments(hints, dim)     // segment detection
    link_segments(hints, width_count, widths, dim)  // stem pairing
    compute_edges(hints, dim)        // edge grouping + link/serif propagation
```

## hint_edges (aflatin.c:4214)

Four phases executed sequentially for all edges:

```
Phase 1 — Blue-zone alignment (VERT only, if AF_HINTS_DO_BLUES):
  For each edge with blue_edge:
    - Neutral-blue dedup between linked edge pairs
    - Align edge to blue.fit (pixel-rounded blue zone position)
    - Child edge gets align_linked_edge if present

Phase 2 — Stem alignment:
  For each edge with a LINK (mutual pair):
    - First stem → anchor: compute anchor position via FT_PIX_ROUND or center-based
    - Subsequent stems → anchor-relative: use compute_stem_width for fitted width
    - align_linked_edge positions the paired edge

Phase 3 — lowercase 'm' symmetry (aflatin.c:4582-4627):
  If 6 or 12 edges and edge[0,2,4] are linked to [1,3,5]:
    - Adjust middle stem to be centered between outer stems

Phase 4 — Non-stem edges:
  For edges without DONE flag (no stem link, no blue):
    - Serif handling: if close serif edge exists, use serif alignment
    - First non-stem edge → anchor via FT_PIX_ROUND
    - Subsequent edges → anchor-relative with half-pixel rounding
      pos = anchor_pos + ((opos - anchor_opos + 16) & ~31)
```

## compute_stem_width (aflatin.c:3960)

```
Smooth path (no SNAP flags):
  dist = |width|
  if (stem edge has SERIF flag) && vertical && dist < 192: pass through
  elif base edge ROUND && dist < 80: dist = 64
  elif dist < 56: dist = 56

  // Width histogram quantization (ONLY if width_count > 0):
  // dist = snap_width(axis->widths[i].cur, dist)
  // Our implementation has this!

Strong path (SNAP enabled):
  dist = snap_width(widths, dist)  // find nearest standard width
  if vertical: dist = (dist+16)&~63  // round to 0.25px
  else (horizontal): depends on mono flag
```

## Blue zone computation lifecycle

```
Font construction (once per size):

1. metrics_init_widths(metrics, face)
   → loads 'o' glyph, link_segments at identity scale
   → axis.widths[] populated with stem widths in font units

2. metrics_init_blues(metrics, face)
   → scans 6 Latin blue character strings
   → per char: load glyph, find Y extremum per contour
   → round/flat classification via on_curve + segment endpoint distance
   → median of flats → ref, median of rounds → shoot
   → axis.blues[] populated (blue_count entries)

3. metrics_scale_dim(metrics, scaler)
   → x-height scale optimization (nudge y_scale to pixel-align x-height)
   → scale widths (org → cur) and blues (org → cur, compute fit)
   → set ACTIVE flag if blue height ≤ 3/4px

Per-glyph (inside af_latin_hints_apply):

4. detect_features(metrics->axis[dim])
   → uses scaled widths (widths[i].cur) but link_segments reads .org

5. compute_blue_edges(hints, metrics)
   → for each VERT edge, find nearest active blue
   → distance threshold: min(upem/40, 0.5px), scaled

6. hint_edges Phase 1:
   → snap blue_edge edges to blue.fit
```

## Our port vs FreeType — remaining gaps

| Feature | FreeType | Our port | Impact |
|---------|----------|----------|--------|
| Pipeline structure | Full flow above | Matches | ✅ |
| `compute_segments` | Faithful | Faithful | ✅ |
| `link_segments` | max_width from widths[].org | Same scoring, widths passed | ✅ |
| `compute_edges` | Edge sorted by fpos (insertion) | Sorted post-creation | ✅ |
| `compute_blue_edges` | edge.dir vs major_dir comparison | Matches | ✅ |
| `hint_edges` Phase 1 | blue.fit alignment | Works | ✅ |
| `hint_edges` Phase 2 | Stem anchor + compute_stem_width | Ported, subpixel values off | ⚠️ |
| `hint_edges` Phase 3 | 'm' symmetry | SKIPPED | ⚠️ |
| `hint_edges` Phase 4 | Anchor-relative rounding | Ported | ✅ |
| `align_strong_points` | FT_MulDiv interpolation | Ported | ✅ |
| `align_weak_points` | TrueType IUP (storage order) | Ported | ✅ |
| x-height scale adj | nudge y_scale for ADJUSTMENT blue | Ported | ✅ |
| Tilde glyph handling | af_adjustment_database | SKIPPED | ⚠️ |
| Advance width adjustment | Post-hint metrics fixup | SKIPPED | ⚠️ |
| Stem darkening | Embolden for LIGHT mode | SKIPPED | N/A (we use NORMAL) |

## What we need to fix

1. **Stem-width quantization precision**: `compute_stem_width` in smooth mode should produce subpixel widths matching FreeType's. Currently our width=56, FreeType=61 for `|`. The `snap_width` is called with standard widths from the histogram but may not be picking the right one, or the smooth-branch logic differs.

2. **Phase 2 stem anchoring**: `FT_PIX_ROUND` for the first stem's anchor should use `(opos + 32) & ~63`. But when stem width ≤ 64 (≤1px), FreeType also tests `(org_center + 32) & ~63` with ±32 offsets to pick the best centered position. Our code has this but edge cases may differ.

3. **AF_HINTS_DO_HORIZONTAL / AF_HINTS_DO_VERTICAL**: These check `hints->other_flags & (LATIN_HINTS_HORZ_SNAP|VERT_SNAP|STEM_ADJUST)`. Since we now clear SNAP, `AF_HINTS_DO_VERTICAL` may become false IF the flag check only looks at SNAP bits. Need to verify: the DO_VERTICAL macro is `(other_flags & (VERT_SNAP|STEM_ADJUST))`. With VERT_SNAP=0 and STEM_ADJUST=1, this should still be true. But need to double-check the exact bit definitions.
