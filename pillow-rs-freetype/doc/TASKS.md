# Task List — pillow-rs-freetype PIL / FreeType 2.14.3 Parity

## Current State

| Backend | Pass | Total | Rate | Reference source |
|---------|------|-------|------|-----------------|
| PIL | 1482 | 1910 | 77.6% | PIL 12.2.0 |
| FreeType raw | 1517 | 1910 | 79.4% | `/tmp/gen_ft_refs` |

**Session total: 9 commits. PIL +312, FT +430 from baseline (1170/1087).**

## Fixed (2026-06-27)

| Commit | Bug | Impact |
|--------|-----|--------|
| `10147a7` | compute_stem_width used snap_width (strong-only) | +94 PIL, +151 FT |
| `69d93c1` | align_strong_points wrong algorithm | +15 PIL, +20 FT |
| `d44fa19` | Post-hinting pp1.x translation missing | +32 PIL, +42 FT |
| `503c268` | Missing DONE on edge[i] in Phase 2 | +27 PIL, +27 FT |
| `4558626` | BOUND overwrites pos (edge init pos=opos→0) | +3 PIL, +5 FT |
| `913d1e0` | edge2 DONE set too early | +1 PIL, +3 FT |
| **`a41d3fa`** | **Operator precedence: & !63 - cur_len2** | **+97 PIL, +137 FT** |

## Remaining: 428 PIL / 393 FT

| Type | PIL | FT | Nature |
|------|-----|-----|--------|
| getmask SHA | 403 | 366 | Subpixel coverage; mostly LiberationSerif |
| getbbox | 25 | 17 | Small Y-axis offsets + pp2.x advance |
| getlength | 0 | 10 | FT fixture values wrong |

## Remaining getbbox failures (FT: 17)

| Pattern | Count | Probable cause |
|---------|-------|---------------|
| DejaVuSans: ymin:-1 xmax:0 ymax:-1 | 5 | VERT stem edge rounding |
| LiberationSerif: xmax:-1 | 4 | pp2.x phantom point |
| LiberationSerif: ymin:-1 | 3 | VERT blue zone alignment |
| LiberationSerif: ymax:-1 | 3 | VERT stem edge |
| Other | 2 | — |

## Next Steps

| Priority | Item | Est. Impact | Effort |
|----------|------|-------------|--------|
| P1 | pp2.x phantom point (advance width) | ~10 getlength + ~4 bbox | Low |
| P2 | VERT stem edge rounding for y-axis bbox | ~5 bbox | Medium |
| P3 | Phase 4 serif overlap check | ~3 bbox | Medium |
| P4 | LiberationSerif SHA mismatches | ~250 mask | High |

## Verified working

- ✅ Edge links, segments, edges: compute_segments + compute_edges
- ✅ Blue zones: metrics_init_blues + metrics_scale_dim
- ✅ Edge hinting: hint_edges Phases 1-4
- ✅ Point alignment: align_edge_points, align_strong_points, align_weak_points
- ✅ Fixed-point math: ft_mul_div, ft_mul_fix, ft_div_fix
- ✅ Post-hinting LSB: pp1.x translation
- ✅ BOUND check: pos=0 init
- ✅ Phase 2 DONE flags: edge[i] always, edge2 only in rel-to-anchor
- ✅ cur_pos2: correct precedence `((org+len+32) & !63) - cur_len`
