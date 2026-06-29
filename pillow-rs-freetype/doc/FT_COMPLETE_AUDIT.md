# C FreeType Pipeline → Rust Complete Audit

**Date:** 2026-06-29 | **Baseline:** 27,677/27,695 pass (99.94%) | **Remaining:** 18 failures

---

## ❌ MISSING FUNCTIONS (3 total — all low impact)

| # | C Function | File:Line | What | Impact |
|---|-----------|-----------|------|--------|
| 1 | `ft_glyphslot_preset_bitmap` | ftobjs.c:374 | Compute bitmap pixel bbox with remainder handling | ⚠️ DIFFERENT APPROACH |
| 2 | `FT_Outline_Translate` (smooth) | ftsmooth.c:609 | Translate outline into bitmap frame | ⚠️ DIFFERENT APPROACH |
| 3 | Phantom point insertion | ttgload.c:887 | Append pp1-pp4 to outline during loading | Low (affects only bbox) |
| 4 | Band bisection | ftgrays.c:1862 | Overflow handling for large glyphs | Not needed |
| 5 | `TT_Hint_Glyph` (bytecode) | ttgload.c:776 | Native TrueType instruction interpreter | Not used (FORCE_AUTOHINT) |
| 6 | `tt_face_load_cvt/fpgm/prep` | ttpload.c | Native hinting tables | Not used (FORCE_AUTOHINT) |

## 18 Remaining Failures — Root Cause Analysis

All 18 have `pp1x=0` (glyf header xMin matches hmtx lsb), so the pp1x translation is a no-op.

### Suspect #1: ft_glyphslot_preset_bitmap remainder handling

C (ftobjs.c:424-498):
```c
// Split cbox into integer pixel part + subpixel remainder
pbox.xMin = (cbox.xMin >> 6);        // integer part
cbox_rem = cbox.xMin & 63;           // remainder (0-63)

// Then in Adjust path (NORMAL mode):
pbox.xMin += cbox_rem >> 6;          // remainder contribution is floor
pbox.xMax += (cbox_rem_max + 63) >> 6; // remainder contribution is ceil
```

Our code (scaler.rs:155-162):
```rust
let px_x_min = (ft_pix_floor(x_min)) >> 6;  // floor entire 26.6 → pixel
let px_x_max = (ft_pix_ceil(x_max)) >> 6;   // ceil entire 26.6 → pixel
```

**Are these equivalent?** Let's verify.

C's way (split, then floor/ceil the remainders separately):
- `pbox.xMin = (cbox.xMin >> 6) + (cbox_rem >> 6)`
  = `floor(x_min/64.0)` 
  True because `a>>6 + (a&63)>>6 = a>>6` since `(a&63)>>6` is always 0.

Our way:
- `px_x_min = (ft_pix_floor(x_min)) >> 6 = (x_min & !63) >> 6 = floor(x_min/64.0)`

These ARE equivalent. Same for x_max.

**Verdict:** ✅ Equivalent, not the cause.

### Suspect #2: ftsmooth outline translation

C (ftsmooth.c:609-621):
```c
x_shift = 64 * -bitmap_left;
y_shift = 64 * -bitmap_top + 64 * rows;
FT_Outline_Translate(outline, x_shift, y_shift);
```

bitmap_left = pbox.xMin (same as our px_x_min)
bitmap_top = pbox.yMax (since y points up in FT)
rows = pbox.yMax - pbox.yMin

So: y_shift = 64 * -pbox.yMax + 64 * (pbox.yMax - pbox.yMin) = -64 * pbox.yMin

Our code (scaler.rs:166-170):
```rust
let off_x = ft_pix_floor(x_min);   // = 64 * floor(x_min/64) = 64 * px_x_min
let off_y = ft_pix_floor(y_min);   // = 64 * floor(y_min/64) = 64 * px_y_min
for p in &mut scaled { p.x -= off_x; p.y -= off_y; }
```

So C's x_shift: 64 * (-bitmap_left) = -64 * px_x_min → shifts left by px_x_min pixels
Our off_x: +64 * px_x_min → shifts right by px_x_min pixels

These are OPPOSITE signs!

Wait, no. C translates by `x_shift` which is then applied to the outline. Then the rasterizer renders. Our code subtracts off_x from points to normalize to (0,0) origin.

Let me think again. C's:
- x_shift = 64 * (-bitmap_left) = 64 * (-px_x_min)
- This moves the outline right by -px_x_min (if px_x_min is positive, moves left)
- Actually: x_shift = -64 * px_x_min. If px_x_min = -1 (as with 'A' italic), x_shift = +64

Our:
- off_x = ft_pix_floor(x_min) = 64 * floor(x_min/64)
- p.x -= off_x means we move the point left by off_x

Hmm, this is getting confusing with signs. Let me trace for a concrete example.

For 'A' italic at 12pt, C's:
- x_min (CBox) = -59 (26.6), pbox.xMin = -1 (pixels), bitmap_left = -1
- x_shift = 64 * (-(-1)) = +64
- FT_Outline_Translate: adds +64 to all x coords
- pt[0].x goes from 139 to 203

Our:
- x_min = -59, off_x = ft_pix_floor(-59) = -64
- p.x -= (-64) = p.x + 64
- pt[0].x goes from 139 to 203

Same result! ✅

**Verdict:** The translation is equivalent.

### Suspect #3: Phantom Points

C adds 4 phantom points to the outline (ttgload.c:887-891), then scales them, then the autohinter processes them. These are at indices n_points..n_points+3.

The phantom points are part of the outline during `compute_glyph_metrics` and `ft_glyphslot_preset_bitmap`, so C's CBox computation includes them. Our CBox only uses the contour points.

**Could phantom points change the bbox?**

pp1 = (xMin - lsb, 0) in FU → after scaling: (pp1x_26_6, 0)
pp2 = (pp1.x + advance, 0) → after scaling: (pp2x_26_6, 0)
pp3/pp4 = (0, some_y_values)

pp1.x is typically negative or 0 (at glyph origin). After our pp1x shift, pp1.x = 0 in FU coordinates. So after scaling, pp1_26_6 is around 0.

pp2.x = advance_width, which is wider than the glyph. So pp2_26_6 > x_max.

For '5' at 12pt: advance ~ 700 FU → pp2_26_6 = 700 * 12/1000 * 64 ≈ 538. This is wider than the glyph.

So phantom points could make C's x_max bigger than ours. But the test only checks getmask SHA, not bbox. And the cbox in our scaler would still be correct because we compute it from contour points only.

Wait — does C's `compute_glyph_metrics` use the phantom-point-included outline to compute advance and bbox? Let me check.

```c
compute_glyph_metrics:
  // uses pp2.x - pp1.x for advance
  glyph->metrics.horiAdvance = SUB_LONG(loader->pp2.x, loader->pp1.x);
  
  // bbox from outline (which includes phantom points)
  FT_Outline_Get_CBox(&glyph->outline, &bbox);
```

The bbox from the outline includes phantom points! So C's cbox xMax includes pp2, which might be wider than our contour-only cbox.

BUT — for the rasterizer, the cbox is set from the bbox. Let me check what happens in `ft_glyphslot_preset_bitmap` and `gray_raster_render`.

In `ft_glyphslot_preset_bitmap`:
```c
FT_Outline_Get_CBox(outline, &cbox);
pbox.xMin = (cbox.xMin >> 6);
pbox.xMax = (cbox.xMax >> 6);
```

Then `bitmap_left = pbox.xMin` and `width = pbox.xMax - pbox.xMin`.

In `gray_raster_render`:
```c
ras.cbox.xMin = 0;
ras.cbox.xMax = (FT_Pos)target_map->width;
```

The rasterizer clips to [0, width). So if phantom points make xMax wider, the bitmap width increases, but the clip region stays the same.

The question is: does this affect the pixel values? If we rasterize into a smaller bitmap (missing the extra pixels from phantom points), the bytes wouldn't match at all.

But wait — we compute our own bitmap size from our CBox:
```rust
let width = (outline.cbox_x_max - outline.cbox_x_min) as usize;
```

And this cbox comes from:
```rust
cbox_x_max: px_x_max - px_x_min,
```

Where px_x_max comes from our contour-only points. C's px_x_max comes from points including phantom points.

So if C's bitmap is wider, our bitmap is narrower → they can't match in SHA.

BUT the test doesn't show any size mismatches for these 18 failures. All 18 have the SAME bitmap size. So phantom points aren't changing the bitmap dimensions.

Actually, let me check: the test matrix stores `ref_size` for each getmask entry. Do all 18 failures have matching sizes?

Let me check a few.

### Suspect #4: Different render_line DDA initial state

After the autohinter runs, we write back coordinates to scaled[]. Then we compute cbox, off_x, off_y, and create the Outline for the rasterizer. The rasterizer starts at (0,0) with x=UPSCALE*0 and y=UPSCALE*0 via move_to.

But wait — our `decompose` function calls `move_to(v_start.x, v_start.y)` with the contour start point. After that, it calls `walk_contour` which calls `render_line` and `render_conic`. The first call to render_line after move_to always starts with x=UPSCALE*v_start.x, y=UPSCALE*v_start.y.

The `move_to` call sets `self.x` and `self.y` to UPSCALED coordinates. Then `walk_contour` begins the DDA from that point. So the initial state matches.

BUT — does our `walk_contour` correctly handle the wrapping when first==0 for a conic-start contour?

Looking at the walk_contour fix from commit 887070a: the issue was when `first==0` and the contour starts with conic, `cursor = first - 1 = -1` which wraps. The fix was to check for this case.

Let me check if this fix is correct for all cases, including the failing glyphs.

For '5' (n_points=41, contours=...), let me check the contour endpoints. If any contour starts at index 0 with conic, the walk_contour logic might behave differently.

Actually, let me just instrument our walk_contour to see if it produces the same render_line calls as C for '5' at 12pt.
