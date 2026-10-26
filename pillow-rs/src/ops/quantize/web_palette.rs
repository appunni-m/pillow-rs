//! Pillow-compatible fixed WEB palette conversion.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

// ── WEB palette (PIL's default fixed palette for convert("P")) ──
//
// PIL's convert("P") with default palette=Palette.WEB uses a fixed 226-color palette:
// - Indices 0-10:   reserved (black: 0,0,0)
// - Indices 11-225: web-safe color cube at values {0,51,102,153,204,255}
//   PIL ordering: for b in [0,51,102,153,204,255], for g in [0,51,102,153,204,255],
//   for r in [0,51,102,153,204,255], skipping (0,0,0) at the first position.
//   216 cube entries minus the (0,0,0) duplicate = 215 + 11 reserved = 226 total.

const WEB_PALETTE: [u8; 678] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 51, 0, 0, 102, 0, 0, 153, 0, 0, 204, 0, 0, 255, 0, 0, 0, 51, 0, 51, 51, 0, 102, 51, 0, 153,
    51, 0, 204, 51, 0, 255, 51, 0, 0, 102, 0, 51, 102, 0, 102, 102, 0, 153, 102, 0, 204, 102, 0,
    255, 102, 0, 0, 153, 0, 51, 153, 0, 102, 153, 0, 153, 153, 0, 204, 153, 0, 255, 153, 0, 0, 204,
    0, 51, 204, 0, 102, 204, 0, 153, 204, 0, 204, 204, 0, 255, 204, 0, 0, 255, 0, 51, 255, 0, 102,
    255, 0, 153, 255, 0, 204, 255, 0, 255, 255, 0, 0, 0, 51, 51, 0, 51, 102, 0, 51, 153, 0, 51,
    204, 0, 51, 255, 0, 51, 0, 51, 51, 51, 51, 51, 102, 51, 51, 153, 51, 51, 204, 51, 51, 255, 51,
    51, 0, 102, 51, 51, 102, 51, 102, 102, 51, 153, 102, 51, 204, 102, 51, 255, 102, 51, 0, 153,
    51, 51, 153, 51, 102, 153, 51, 153, 153, 51, 204, 153, 51, 255, 153, 51, 0, 204, 51, 51, 204,
    51, 102, 204, 51, 153, 204, 51, 204, 204, 51, 255, 204, 51, 0, 255, 51, 51, 255, 51, 102, 255,
    51, 153, 255, 51, 204, 255, 51, 255, 255, 51, 0, 0, 102, 51, 0, 102, 102, 0, 102, 153, 0, 102,
    204, 0, 102, 255, 0, 102, 0, 51, 102, 51, 51, 102, 102, 51, 102, 153, 51, 102, 204, 51, 102,
    255, 51, 102, 0, 102, 102, 51, 102, 102, 102, 102, 102, 153, 102, 102, 204, 102, 102, 255, 102,
    102, 0, 153, 102, 51, 153, 102, 102, 153, 102, 153, 153, 102, 204, 153, 102, 255, 153, 102, 0,
    204, 102, 51, 204, 102, 102, 204, 102, 153, 204, 102, 204, 204, 102, 255, 204, 102, 0, 255,
    102, 51, 255, 102, 102, 255, 102, 153, 255, 102, 204, 255, 102, 255, 255, 102, 0, 0, 153, 51,
    0, 153, 102, 0, 153, 153, 0, 153, 204, 0, 153, 255, 0, 153, 0, 51, 153, 51, 51, 153, 102, 51,
    153, 153, 51, 153, 204, 51, 153, 255, 51, 153, 0, 102, 153, 51, 102, 153, 102, 102, 153, 153,
    102, 153, 204, 102, 153, 255, 102, 153, 0, 153, 153, 51, 153, 153, 102, 153, 153, 153, 153,
    153, 204, 153, 153, 255, 153, 153, 0, 204, 153, 51, 204, 153, 102, 204, 153, 153, 204, 153,
    204, 204, 153, 255, 204, 153, 0, 255, 153, 51, 255, 153, 102, 255, 153, 153, 255, 153, 204,
    255, 153, 255, 255, 153, 0, 0, 204, 51, 0, 204, 102, 0, 204, 153, 0, 204, 204, 0, 204, 255, 0,
    204, 0, 51, 204, 51, 51, 204, 102, 51, 204, 153, 51, 204, 204, 51, 204, 255, 51, 204, 0, 102,
    204, 51, 102, 204, 102, 102, 204, 153, 102, 204, 204, 102, 204, 255, 102, 204, 0, 153, 204, 51,
    153, 204, 102, 153, 204, 153, 153, 204, 204, 153, 204, 255, 153, 204, 0, 204, 204, 51, 204,
    204, 102, 204, 204, 153, 204, 204, 204, 204, 204, 255, 204, 204, 0, 255, 204, 51, 255, 204,
    102, 255, 204, 153, 255, 204, 204, 255, 204, 255, 255, 204, 0, 0, 255, 51, 0, 255, 102, 0, 255,
    153, 0, 255, 204, 0, 255, 255, 0, 255, 0, 51, 255, 51, 51, 255, 102, 51, 255, 153, 51, 255,
    204, 51, 255, 255, 51, 255, 0, 102, 255, 51, 102, 255, 102, 102, 255, 153, 102, 255, 204, 102,
    255, 255, 102, 255, 0, 153, 255, 51, 153, 255, 102, 153, 255, 153, 153, 255, 204, 153, 255,
    255, 153, 255, 0, 204, 255, 51, 204, 255, 102, 204, 255, 153, 204, 255, 204, 204, 255, 255,
    204, 255, 0, 255, 255, 51, 255, 255, 102, 255, 255, 153, 255, 255, 204, 255, 255, 255, 255,
    255,
];

/// Quantizes RGB pixels to Pillow's default web palette.
///
/// `pixels` is tightly packed RGB data for a `w` by `h` image. When `dither` is
/// true, Pillow-compatible Floyd-Steinberg error diffusion is applied.
///
/// # Returns
///
/// A tuple of palette index bytes and flat RGB palette bytes.
pub(crate) fn web_palette_quantize(
    pixels: &[u8],
    w: u32,
    h: u32,
    dither: bool,
) -> Result<(Vec<u8>, Vec<u8>), PilError> {
    let dims_pixels = CheckedDims::new_allow_empty(w, h, 1)?;
    let mut out = dims_pixels.alloc_buffer();
    if w == 0 || h == 0 {
        // Empty conversion still installs the normal WEB palette but has
        // neither pixel work nor a row of diffusion errors to allocate.
        return Ok((out, WEB_PALETTE.to_vec()));
    }

    if dither {
        web_palette_dither(pixels, &mut out, w, h);
    } else {
        // No dither: nearest-neighbor mapping
        for (pi, out_pixel) in out.iter_mut().enumerate() {
            let src_i = pi * 3;
            let (best_idx, _, _, _) =
                find_nearest_web(pixels[src_i], pixels[src_i + 1], pixels[src_i + 2]);
            *out_pixel = best_idx;
        }
    }

    Ok((out, WEB_PALETTE.to_vec()))
}

#[inline(always)]
fn web_palette_dither(pixels: &[u8], out: &mut [u8], w: u32, h: u32) {
    // PIL-identical Floyd-Steinberg dither with WEB palette.
    // Uses PIL's 3x/5x/7x accumulator pattern (single division by 16 at read time)
    // instead of dividing each propagated fraction separately.
    let wu = w as usize;
    // Error array: one row + 1 column, 3 channels interleaved
    let mut errors = vec![0i32; (wu + 1) * 3];

    #[inline]
    fn clip8(v: i32) -> u8 {
        if v < 0 {
            0
        } else if v > 255 {
            255
        } else {
            v as u8
        }
    }

    for y in 0..h as usize {
        // Per-channel accumulators (carry 7x, delayed 5x, delayed 1x)
        let mut l = [0i32; 3];
        let mut l0 = [0i32; 3];
        let mut l1 = [0i32; 3];

        for x in 0..wu {
            let src_i = (y * wu + x) * 3;
            let e_ptr = x * 3; // base of errors for this column

            // Read pixel + accumulated error (divided by 16 once)
            for ch in 0..3 {
                let val = pixels[src_i + ch] as i32;
                let acc = l[ch] + errors[e_ptr + 3 + ch]; // errors[x+1] = next col
                l[ch] = clip8(val + acc / 16) as i32;
            }
            let r = l[0] as u8;
            let g = l[1] as u8;
            let b = l[2] as u8;

            let (best_idx, pr, pg, pb) = find_nearest_web(r, g, b);
            out[y * wu + x] = best_idx;

            // Compute error = corrected_input - palette_output
            for ch in 0..3 {
                l[ch] -= [pr as i32, pg as i32, pb as i32][ch];
            }

            // PIL's 3x/5x/7x accumulation pattern
            for ch in 0..3 {
                let err = l[ch];
                let r2 = err;
                let d2 = err + err;
                l[ch] = err + d2; // 3x
                errors[e_ptr + ch] = l[ch] + l0[ch]; // store 3x + l0
                l[ch] += d2; // 5x
                l0[ch] = l[ch] + l1[ch]; // 5x + l1
                l1[ch] = r2; // 1x (delayed)
                l[ch] += d2; // 7x (carry to next pixel)
            }
        }

        // Post-loop: PIL's topalette writes ONLY B-channel accumulators
        // at errors[w*3 + 0..2] — b0, b1, b2 (NOT per-channel l0 values).
        // This effectively loses R and G error at the right edge, but we
        // must match PIL's exact behavior for pixel-level parity.
        // b0 = 5*e_B_last + e_B_second_last, b1 = b2 = e_B_last
        let e_end = wu * 3;
        errors[e_end] = l0[2]; // b0: B channel's 5x delay
        errors[e_end + 1] = l1[2]; // b1: B channel's 1x delay (original error)
        errors[e_end + 2] = l1[2]; // b2: B channel's original error (same as b1)
    }
}

/// Number of colors in the WEB palette (11 reserved + 215 web-safe cube).
const WEB_PALETTE_COLORS: usize = 226;

/// PIL-identical palette lookup cache.
/// PIL divides color space into 8×8×8 cells (size 32×32×32 each).
/// Within each cell, 8×8×8 sub-positions (step 4) are pre-cached.
/// Each pixel maps to its cell+sub-position for the cached result.
struct WebPaletteCache {
    /// Cache per cell: 512 cells × 512 entries per cell
    cells: Vec<Option<[u8; 512]>>,
}

impl WebPaletteCache {
    fn new() -> Self {
        let mut cells = Vec::with_capacity(512);
        for _ in 0..512 {
            cells.push(None);
        }
        WebPaletteCache { cells }
    }

    fn cell_index(r: u8, g: u8, b: u8) -> usize {
        ((r as usize) >> 5) | (((g as usize) >> 5) << 3) | (((b as usize) >> 5) << 6)
    }

    fn sub_index(r: u8, g: u8, b: u8, r0: u8, g0: u8, b0: u8) -> usize {
        let ri = ((r.saturating_sub(r0)) >> 2) as usize;
        let gi = ((g.saturating_sub(g0)) >> 2) as usize;
        let bi = ((b.saturating_sub(b0)) >> 2) as usize;
        ri | (gi << 3) | (bi << 6)
    }

    fn get_or_build(&mut self, r: u8, g: u8, b: u8) -> u8 {
        let ci = Self::cell_index(r, g, b);
        let cell_cache = self.cells[ci].get_or_insert_with(|| build_cell_cache(ci));
        let r0 = ((ci & 7) as u8) << 5;
        let g0 = (((ci >> 3) & 7) as u8) << 5;
        let b0 = ((ci >> 6) as u8) << 5;
        let si = Self::sub_index(r, g, b, r0, g0, b0);
        cell_cache[si]
    }
}

/// Build PIL-identical palette cache for one cell.
/// Each cell is a 32×32×32 color box. We compute the closest palette entry
/// for 8×8×8 sub-positions within the box (step 4 per channel).
fn build_cell_cache(cell_idx: usize) -> [u8; 512] {
    let r0 = ((cell_idx & 7) as u8) << 5;
    let g0 = (((cell_idx >> 3) & 7) as u8) << 5;
    let b0 = ((cell_idx >> 6) as u8) << 5;
    let r1 = r0 + 31;
    let g1 = g0 + 31;
    let b1 = b0 + 31;

    // Compute dmin[i] and tmax[i] for each palette entry
    let mut dmin = [0i32; WEB_PALETTE_COLORS];
    let mut tmax = [0i32; WEB_PALETTE_COLORS];
    let mut dmax = i32::MAX;

    for i in 0..WEB_PALETTE_COLORS {
        let base = i * 3;
        let pr = WEB_PALETTE[base] as i32;
        let pg = WEB_PALETTE[base + 1] as i32;
        let pb = WEB_PALETTE[base + 2] as i32;

        let dr_min = if pr < r0 as i32 {
            r0 as i32 - pr
        } else if pr > r1 as i32 {
            pr - r1 as i32
        } else {
            0
        };
        let dg_min = if pg < g0 as i32 {
            g0 as i32 - pg
        } else if pg > g1 as i32 {
            pg - g1 as i32
        } else {
            0
        };
        let db_min = if pb < b0 as i32 {
            b0 as i32 - pb
        } else if pb > b1 as i32 {
            pb - b1 as i32
        } else {
            0
        };
        dmin[i] = dr_min * dr_min + dg_min * dg_min + db_min * db_min;

        let dr_max = (r1 as i32 - pr).abs().max((pr - r0 as i32).abs());
        let dg_max = (g1 as i32 - pg).abs().max((pg - g0 as i32).abs());
        let db_max = (b1 as i32 - pb).abs().max((pb - b0 as i32).abs());
        tmax[i] = dr_max * dr_max + dg_max * dg_max + db_max * db_max;

        if tmax[i] < dmax {
            dmax = tmax[i];
        }
    }

    // Build 8×8×8 sub-position cache
    let mut cache = [0u8; 512];
    for ri in 0..8u8 {
        for gi in 0..8u8 {
            for bi in 0..8u8 {
                let r = r0 + ri * 4;
                let g = g0 + gi * 4;
                let b = b0 + bi * 4;
                let ci = (ri as usize) | ((gi as usize) << 3) | ((bi as usize) << 6);

                let mut best_dist = i32::MAX;
                let mut best_idx = 0u8;
                for pi in 0..WEB_PALETTE_COLORS {
                    if dmin[pi] <= dmax {
                        let base = pi * 3;
                        let dr = r as i32 - WEB_PALETTE[base] as i32;
                        let dg = g as i32 - WEB_PALETTE[base + 1] as i32;
                        let db = b as i32 - WEB_PALETTE[base + 2] as i32;
                        let dist = dr * dr + dg * dg + db * db;
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = pi as u8;
                        }
                    }
                }
                cache[ci] = best_idx;
            }
        }
    }
    cache
}

/// PIL-style cached nearest-palette lookup.
/// Uses thread-local cache to avoid Mutex overhead.
fn find_nearest_web(r: u8, g: u8, b: u8) -> (u8, u8, u8, u8) {
    use std::cell::RefCell;
    thread_local! {
        static CACHE: RefCell<WebPaletteCache> = RefCell::new(WebPaletteCache::new());
    }
    let idx = CACHE.with(|cache| cache.borrow_mut().get_or_build(r, g, b));
    let base = idx as usize * 3;
    (
        idx,
        WEB_PALETTE[base],
        WEB_PALETTE[base + 1],
        WEB_PALETTE[base + 2],
    )
}
