//! VP8 deblocking (loop) filter — RFC 6386 Section 15.
//!
//! Applies a "simple" deblocking filter to reduce blocking artifacts at 4×4
//! block boundaries in the reconstructed luma and chroma planes.  Filter
//! strength depends on `filter_level` (0-63) and `sharpness` (0-7).
//!
//! The simple filter only modifies the two edge pixels (p0, q0) on each side
//! of the boundary.  All code is safe Rust — no `unsafe` blocks.

#![allow(dead_code)]

use core::cmp;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum filter level value.
const MAX_FILTER_LEVEL: u8 = 63;

/// Maximum sharpness value.
const MAX_SHARPNESS: u8 = 7;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Apply the VP8 deblocking filter to all three colour planes.
///
/// The luma plane `frame_y` has dimensions `width` × `height`.
/// The chroma planes `frame_u` and `frame_v` each have dimensions
/// `width/2` × `height/2` (4:2:0 subsampling).
///
/// `filter_level` must be in 0..=63 and `sharpness` in 0..=7.
///
/// The filter is applied to every 4×4 block boundary in both the horizontal
/// and vertical directions.
pub fn loop_filter_frame(
    frame_y: &mut [u8],
    frame_u: &mut [u8],
    frame_v: &mut [u8],
    width: u32,
    height: u32,
    filter_level: u8,
    sharpness: u8,
) {
    let fl = cmp::min(filter_level, MAX_FILTER_LEVEL);
    let sharp = cmp::min(sharpness, MAX_SHARPNESS);

    if fl == 0 {
        return;
    }

    let blimit = compute_blimit(fl);
    let limit = fl;
    let interior_limit = compute_interior_limit(fl, sharp);

    let w = width as usize;
    let h = height as usize;

    // ── Luma: vertical edges ──
    filter_frame_vertical(frame_y, w, h, limit, blimit, interior_limit);

    // ── Luma: horizontal edges ──
    filter_frame_horizontal(frame_y, w, h, limit, blimit, interior_limit);

    // ── Chroma planes ──
    let cw = (width / 2) as usize;
    let ch = (height / 2) as usize;

    filter_frame_vertical(frame_u, cw, ch, limit, blimit, interior_limit);
    filter_frame_horizontal(frame_u, cw, ch, limit, blimit, interior_limit);

    filter_frame_vertical(frame_v, cw, ch, limit, blimit, interior_limit);
    filter_frame_horizontal(frame_v, cw, ch, limit, blimit, interior_limit);
}

// ---------------------------------------------------------------------------
// Frame-level filter helpers
// ---------------------------------------------------------------------------

/// Apply the simple filter on every 4-column vertical boundary in a plane.
fn filter_frame_vertical(
    plane: &mut [u8],
    w: usize,
    h: usize,
    limit: u8,
    blimit: u8,
    interior_limit: u8,
) {
    let mut col = 4;
    while col < w {
        for row in 0..h {
            let base = row * w + col;
            // Need p3..p0 at col-4..col-1 and q0..q3 at col..col+3
            if col >= 4 && col + 3 < w {
                let mut seg = [0u8; 8];
                for k in 0..8 {
                    seg[k] = plane[base - 4 + k]; // base-4 = p3, base+3 = q3
                }
                let filtered = simple_filter(&seg, limit, blimit, interior_limit);
                if let Some(f) = filtered {
                    for k in 0..8 {
                        plane[base - 4 + k] = f[k];
                    }
                }
            }
        }
        col += 4;
    }
}

/// Apply the simple filter on every 4-row horizontal boundary in a plane.
fn filter_frame_horizontal(
    plane: &mut [u8],
    w: usize,
    h: usize,
    limit: u8,
    blimit: u8,
    interior_limit: u8,
) {
    let mut row = 4;
    while row < h {
        for col in 0..w {
            if row >= 4 && row + 3 < h {
                let p3_base = (row - 4) * w + col;
                let mut seg = [0u8; 8];
                for k in 0..8 {
                    seg[k] = plane[p3_base + k * w];
                }
                let filtered = simple_filter(&seg, limit, blimit, interior_limit);
                if let Some(f) = filtered {
                    for k in 0..8 {
                        plane[p3_base + k * w] = f[k];
                    }
                }
            }
        }
        row += 4;
    }
}

// ---------------------------------------------------------------------------
// Simple filter core
// ---------------------------------------------------------------------------

/// Apply the VP8 "simple" deblocking filter on an 8-pixel segment.
///
/// The segment is laid out as `[p3, p2, p1, p0, q0, q1, q2, q3]`.
/// The edge is between `p0` and `q0`.
///
/// Returns `Some(filtered_segment)` if the filter was applied, or `None` if
/// the edge was skipped (no detectable blocking artifact).
fn simple_filter(seg: &[u8; 8], limit: u8, blimit: u8, interior_limit: u8) -> Option<[u8; 8]> {
    let p0 = seg[3];
    let p1 = seg[2];
    let q0 = seg[4];
    let q1 = seg[5];

    // Blockiness mask:
    //   |p0-q0|*2 + |p1-q1|/2  <  blimit*16
    let diff_p0q0 = abs_diff(p0, q0) as u32;
    let diff_p1q1 = abs_diff(p1, q1) as u32;
    let mask = diff_p0q0 * 2 + diff_p1q1 / 2;

    if mask >= (blimit as u32) * 16 {
        return None; // Not a blocky edge — skip.
    }

    // High-edge-variance (flatness) check.
    let hev_p = abs_diff(p1, p0) > interior_limit;
    let hev_q = abs_diff(q1, q0) > interior_limit;

    if hev_p || hev_q {
        return None; // Too much local detail — skip to preserve sharpness.
    }

    // Simple filter: compute delta and apply to p0 and q0.
    let p0_i = p0 as i16;
    let q0_i = q0 as i16;
    let p1_i = p1 as i16;
    let q1_i = q1 as i16;

    // delta = clamp((4*(q0-p0) + (p1-q1)) / 8, -limit, limit)
    let mut delta = (4 * (q0_i - p0_i) + (p1_i - q1_i)) / 8;

    let limit_i = limit as i16;
    if delta < -limit_i {
        delta = -limit_i;
    } else if delta > limit_i {
        delta = limit_i;
    }

    let mut out = *seg;
    out[3] = clamp_u8(p0_i + delta);
    out[4] = clamp_u8(q0_i - delta);

    // p1 and q1 are left untouched (simple filter mode).
    Some(out)
}

// ---------------------------------------------------------------------------
// Filter parameter computation
// ---------------------------------------------------------------------------

/// Compute the blockiness threshold from the filter level.
///
/// `blimit = 2 * filter_level + 60`, clamped to 255.
#[inline]
fn compute_blimit(filter_level: u8) -> u8 {
    let v = (filter_level as u16) * 2 + 60;
    if v > 255 { 255 } else { v as u8 }
}

/// Compute the interior (flatness) threshold from filter level and sharpness.
///
/// `interior_limit = max(0, filter_level - 4 * sharpness)`, clamped to 63.
#[inline]
fn compute_interior_limit(filter_level: u8, sharpness: u8) -> u8 {
    let v = (filter_level as i16) - 4 * (sharpness as i16);
    if v < 0 {
        0
    } else if v > 63 {
        63
    } else {
        v as u8
    }
}

// ---------------------------------------------------------------------------
// Elementary helpers
// ---------------------------------------------------------------------------

/// Absolute difference between two `u8` values.
#[inline]
fn abs_diff(a: u8, b: u8) -> u8 {
    if a > b { a - b } else { b - a }
}

/// Clamp a `i16` pixel value to `[0, 255]`.
#[inline]
fn clamp_u8(v: i16) -> u8 {
    if v < 0 {
        0
    } else if v > 255 {
        255
    } else {
        v as u8
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_params() {
        assert_eq!(compute_blimit(0), 60);
        assert_eq!(compute_blimit(10), 80);
        assert_eq!(compute_blimit(63), 186);
        assert_eq!(compute_blimit(255), 255);

        assert_eq!(compute_interior_limit(20, 0), 20);
        assert_eq!(compute_interior_limit(20, 5), 0);
        assert_eq!(compute_interior_limit(10, 2), 2);
        assert_eq!(compute_interior_limit(63, 7), 63 - 28); // 35
    }

    #[test]
    fn test_simple_filter_strong_artifact() {
        // A strong block edge (p side flat at 100, q side flat at 150).
        let seg = [100u8, 100, 100, 100, 150, 150, 150, 150];
        let result = simple_filter(&seg, 40, 10, 20);
        assert!(result.is_some(), "Should filter strong artifact");
        let out = result.unwrap();
        // p0 should have increased, q0 decreased.
        assert!(out[3] > 100, "p0 should increase, got {}", out[3]);
        assert!(out[4] < 150, "q0 should decrease, got {}", out[4]);
        // p1, q1 should be untouched.
        assert_eq!(out[2], 100, "p1 should be untouched");
        assert_eq!(out[5], 150, "q1 should be untouched");
    }

    #[test]
    fn test_simple_filter_skips_high_variance() {
        // Large interior gradient — should skip.
        let seg = [100u8, 100, 140, 100, 150, 110, 150, 150];
        let result = simple_filter(&seg, 40, 10, 10);
        // interior_limit=10. hev_p = |140-100| = 40 > 10 → skip.
        assert!(result.is_none(), "Should skip high-variance edge");
    }

    #[test]
    fn test_simple_filter_store_small_transition() {
        // Small transition, but low blimit to force filtering.
        let seg = [128u8, 128, 128, 128, 130, 130, 130, 130];
        let result = simple_filter(&seg, 40, 5, 20);
        // mask = |128-130|*2 + |128-130|/2 = 4 + 1 = 5
        // blimit=5 → threshold = 5*16 = 80.  5 < 80, so mask passes.
        // hev both pass. delta = (4*(130-128) + (128-130))/8 = (8-2)/8 = 0.
        // So values stay the same because the transition is too small.
        assert!(result.is_some());
        let out = result.unwrap();
        assert_eq!(out[3], 128);
        assert_eq!(out[4], 130);
    }

    #[test]
    fn test_loop_filter_frame_noop_zero_level() {
        let width = 16u32;
        let height = 16u32;
        let mut y = vec![128u8; (width * height) as usize];
        let mut u = vec![128u8; (width * height / 4) as usize];
        let mut v = vec![128u8; (width * height / 4) as usize];

        loop_filter_frame(&mut y, &mut u, &mut v, width, height, 0, 0);
        assert!(y.iter().all(|&x| x == 128));
        assert!(u.iter().all(|&x| x == 128));
        assert!(v.iter().all(|&x| x == 128));
    }

    #[test]
    fn test_loop_filter_frame_reduces_blockiness() {
        // Build a 32×32 luma plane with a clear block boundary at row 16.
        let width = 32u32;
        let height = 32u32;
        let mut y = vec![100u8; (width * height) as usize];
        let w = width as usize;
        // Upper half at value 150, lower half at 100.
        for row in 0..16 {
            for col in 0..w {
                y[row * w + col] = 150;
            }
        }

        let mut u = vec![128u8; (width * height / 4) as usize];
        let mut v = vec![128u8; (width * height / 4) as usize];

        let diff_before = (y[15 * w] as i16 - y[16 * w] as i16).abs();

        loop_filter_frame(&mut y, &mut u, &mut v, width, height, 20, 2);

        let diff_after = (y[15 * w] as i16 - y[16 * w] as i16).abs();

        assert!(
            diff_after < diff_before,
            "Filter should reduce difference at block boundary: before={diff_before} after={diff_after}"
        );
    }

    #[test]
    fn test_chroma_filtered_too() {
        // Make a small frame where chroma has a block boundary.
        let width = 16u32;
        let height = 16u32;
        let mut y = vec![128u8; (width * height) as usize];
        let mut u = vec![128u8; (width * height / 4) as usize];
        let mut v = vec![128u8; (width * height / 4) as usize];

        // 4:2:0 → U is 8×8.  Set a 4×4 half to 200.
        let uw = (width / 2) as usize;
        for row in 0..4 {
            for col in 0..uw {
                u[row * uw + col] = 200;
            }
        }

        let diff_before = (u[3 * uw] as i16 - u[4 * uw] as i16).abs();
        loop_filter_frame(&mut y, &mut u, &mut v, width, height, 20, 2);
        let diff_after = (u[3 * uw] as i16 - u[4 * uw] as i16).abs();
        assert!(
            diff_after < diff_before,
            "Chroma boundary should be smoothed: before={diff_before} after={diff_after}"
        );
    }

    #[test]
    fn test_frame_too_small_no_crash() {
        // Very small frame — no 4-boundaries inside, should be a no-op.
        let width = 4u32;
        let height = 4u32;
        let mut y = vec![100u8; 16];
        let mut u = vec![100u8; 4];
        let mut v = vec![100u8; 4];

        // Should not panic.
        loop_filter_frame(&mut y, &mut u, &mut v, width, height, 30, 3);
        assert_eq!(y, vec![100u8; 16]);
    }
}
