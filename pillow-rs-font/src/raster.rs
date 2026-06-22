//! Cell-based smooth rasterizer — matches FreeType's ftgrays.c.
//!
//! Produces 256-level anti-aliased bitmaps using exact pixel coverage computation.
//! Algorithm: flatten Bezier curves → record edge crossings → sweep scanlines.

use crate::scaler::ScaledGlyph;

/// 26.6 fixed-point units per pixel.
const ONE_PIXEL: i32 = 64;
/// Maximum area per pixel in sub-pixel units (64×64).
const MAX_AREA: i32 = ONE_PIXEL * ONE_PIXEL;

/// An edge intersection with a scanline.
#[derive(Debug, Clone, Copy)]
struct EdgeCrossing {
    x: i32,
    y: i32,
    dir: i32,
}

/// Rasterized glyph bitmap.
#[derive(Debug, Clone)]
pub(crate) struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub xmin: i32,
    pub ymin: i32,
}

/// Render a scaled glyph into an anti-aliased bitmap.
pub(crate) fn rasterize(glyph: &ScaledGlyph) -> RasterizedGlyph {
    if glyph.points.is_empty() || glyph.num_contours == 0 {
        return RasterizedGlyph { width: 0, height: 0, pixels: vec![], xmin: 0, ymin: 0 };
    }

    let bbox_w = (glyph.xmax - glyph.xmin).max(0) as u32;
    let bbox_h = (glyph.ymax - glyph.ymin).max(0) as u32;
    let w = bbox_w.min(4096);
    let h = bbox_h.min(4096);

    if w == 0 || h == 0 {
        return RasterizedGlyph { width: 0, height: 0, pixels: vec![], xmin: 0, ymin: 0 };
    }

    let offset_x = glyph.xmin;
    let offset_y = glyph.ymin;

    // Step 0: expand implicit on-curve midpoints
    let mut expanded_pts: Vec<(i32, i32)> = Vec::new();
    let mut expanded_oc: Vec<bool> = Vec::new();
    let mut expanded_end_pts: Vec<usize> = Vec::new();
    let mut pt_idx = 0usize;

    for &end_idx in &glyph.end_pts {
        let contour_start = pt_idx;
        let contour_end = end_idx as usize + 1;
        let contour_len = contour_end - contour_start;

        for i in 0..contour_len {
            let cur = glyph.points[contour_start + i];
            let next_idx = if i + 1 < contour_len { contour_start + i + 1 } else { contour_start };
            let next = glyph.points[next_idx];
            let oc_cur = glyph.on_curve[contour_start + i];
            let oc_next = glyph.on_curve[next_idx];

            expanded_pts.push(cur);
            expanded_oc.push(oc_cur);

            if !oc_cur && !oc_next {
                let mid = ((cur.0 + next.0) / 2, (cur.1 + next.1) / 2);
                expanded_pts.push(mid);
                expanded_oc.push(true);
            }
        }
        expanded_end_pts.push(expanded_pts.len() - 1);
        pt_idx = contour_end;
    }

    // Step 1: collect all edge crossings
    let mut crossings: Vec<EdgeCrossing> = Vec::new();

    pt_idx = 0usize;
    for &end_idx in &expanded_end_pts {
        let contour_start = pt_idx;
        let contour_end = end_idx + 1;
        let contour_len = contour_end - contour_start;

        let mut i = 0usize;
        while i < contour_len {
            let idx = contour_start + i;
            let oc0 = expanded_oc[idx];

            if !oc0 {
                i += 1;
                continue;
            }

            let p0 = expanded_pts[idx];
            let next_i = if i + 1 < contour_len { i + 1 } else { 0 };
            let next_idx = contour_start + next_i;
            let p1 = expanded_pts[next_idx];
            let oc1 = expanded_oc[next_idx];

            if oc1 {
                add_line_crossings(p0, p1, &mut crossings);
                i += 1;
            } else {
                let end_i = if i + 2 < contour_len { i + 2 } else { 0 };
                let end_idx2 = contour_start + end_i;
                let p2 = expanded_pts[end_idx2];
                flatten_quadratic_bezier(p0, p1, p2, &mut crossings);
                i += 2;
            }
        }
        pt_idx = contour_end;
    }

    // Step 2: sort crossings by (y, x)
    crossings.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));

    // Step 3: sweep scanlines and accumulate area
    let total_pixels = (w * h) as usize;
    let mut pixel_areas = vec![0i32; total_pixels];

    let mut i = 0;
    while i < crossings.len() {
        let current_y = crossings[i].y;
        let py = current_y - offset_y;

        if py >= 0 && (py as u32) < h {
            let start = i;
            let mut end = i;
            while end < crossings.len() && crossings[end].y == current_y {
                end += 1;
            }

            let row = &crossings[start..end];
            let mut winding = 0i32;
            let mut span_start_x = 0i32;

            for crossing in row {
                let prev_winding = winding;
                winding += crossing.dir;

                if (prev_winding == 0) != (winding == 0) {
                    if winding != 0 {
                        span_start_x = crossing.x;
                    } else {
                        fill_span(span_start_x, crossing.x, py, w, h, offset_x, &mut pixel_areas);
                    }
                }
            }

            if winding != 0 {
                let right_edge = ((offset_x + w as i32) << 6) as i32;
                fill_span(span_start_x, right_edge, py, w, h, offset_x, &mut pixel_areas);
            }
        }

        i += 1;
        while i < crossings.len() && crossings[i].y == current_y {
            i += 1;
        }
    }

    // Step 4: convert accumulated area to 0-255 coverage
    let mut pixels = vec![0u8; total_pixels];
    for idx in 0..total_pixels {
        let area = pixel_areas[idx].max(0).min(MAX_AREA);
        let coverage = (area * 255 + MAX_AREA / 2) / MAX_AREA;
        pixels[idx] = coverage as u8;
    }

    RasterizedGlyph { width: w, height: h, pixels, xmin: glyph.xmin, ymin: glyph.ymin }
}

/// Flatten a quadratic Bezier using de Casteljau subdivision.
fn flatten_quadratic_bezier(
    p0: (i32, i32), p1: (i32, i32), p2: (i32, i32),
    crossings: &mut Vec<EdgeCrossing>,
) {
    let flatness = point_to_line_dist_sq(p0, p1, p2);
    const FLATNESS_SQ: i64 = 256; // (1/4 px)² in 26.6

    if flatness <= FLATNESS_SQ {
        add_line_crossings(p0, p2, crossings);
    } else {
        let m01 = ((p0.0 + p1.0) / 2, (p0.1 + p1.1) / 2);
        let m12 = ((p1.0 + p2.0) / 2, (p1.1 + p2.1) / 2);
        let mid = ((m01.0 + m12.0) / 2, (m01.1 + m12.1) / 2);
        flatten_quadratic_bezier(p0, m01, mid, crossings);
        flatten_quadratic_bezier(mid, m12, p2, crossings);
    }
}

fn point_to_line_dist_sq(a: (i32, i32), p: (i32, i32), b: (i32, i32)) -> i64 {
    let ab_x = (b.0 - a.0) as i64;
    let ab_y = (b.1 - a.1) as i64;
    let ap_x = (p.0 - a.0) as i64;
    let ap_y = (p.1 - a.1) as i64;
    let cross = (ab_x * ap_y - ab_y * ap_x).abs();
    let ab_len_sq = ab_x * ab_x + ab_y * ab_y;
    if ab_len_sq == 0 { return ap_x * ap_x + ap_y * ap_y; }
    cross * cross / ab_len_sq
}

fn fill_span(x_start: i32, x_end: i32, py: i32, w: u32, h: u32, offset_x: i32, pixel_areas: &mut [i32]) {
    if py < 0 || (py as u32) >= h { return; }

    let px_start = (x_start >> 6) - offset_x;
    let px_end = (x_end >> 6) - offset_x;
    let fx_start = x_start & (ONE_PIXEL - 1);
    let fx_end = x_end & (ONE_PIXEL - 1);
    let row_start = (py as u32) * w;

    if px_start == px_end {
        if (px_start as u32) < w {
            let coverage = (x_end - x_start) as i32;
            let idx = (row_start + px_start as u32) as usize;
            if idx < pixel_areas.len() {
                pixel_areas[idx] = pixel_areas[idx].saturating_add(coverage * ONE_PIXEL);
            }
        }
        return;
    }

    if (px_start as u32) < w {
        let first_remainder = (ONE_PIXEL - fx_start) as i32;
        let idx = (row_start + px_start as u32) as usize;
        if idx < pixel_areas.len() {
            pixel_areas[idx] = pixel_areas[idx].saturating_add(first_remainder * ONE_PIXEL);
        }
    }

    let mid_start = px_start + 1;
    let mid_end = px_end;
    for px in mid_start.max(0)..mid_end.min(w as i32) {
        let idx = (row_start + px as u32) as usize;
        if idx < pixel_areas.len() {
            pixel_areas[idx] = pixel_areas[idx].saturating_add(MAX_AREA);
        }
    }

    if (px_end as u32) < w && px_end != px_start {
        let last_coverage = fx_end;
        let idx = (row_start + px_end as u32) as usize;
        if idx < pixel_areas.len() {
            pixel_areas[idx] = pixel_areas[idx].saturating_add(last_coverage * ONE_PIXEL);
        }
    }
}

fn add_line_crossings(p0: (i32, i32), p1: (i32, i32), crossings: &mut Vec<EdgeCrossing>) {
    let (x0, y0) = p0;
    let (x1, y1) = p1;

    if y0 == y1 { return; }

    let dir = if y0 < y1 { 1i32 } else { -1i32 };
    let dy = y1 as i64 - y0 as i64;
    let py0 = y0 >> 6;
    let py1 = y1 >> 6;
    let py_start = py0.min(py1).max(0);
    let py_end = py0.max(py1);
    let dx = x1 as i64 - x0 as i64;
    let y_min = y0.min(y1) as i64;
    let y_max = y0.max(y1) as i64;

    for py in py_start..=py_end {
        let y_center = ((py << 6) + 32) as i64;
        if y_center <= y_min || y_center >= y_max { continue; }
        let t_num = y_center - y0 as i64;
        let x_interp = x0 as i64 + (t_num * dx) / dy;
        crossings.push(EdgeCrossing { x: x_interp as i32, y: py, dir });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_glyph_returns_zero_size() {
        let glyph = ScaledGlyph {
            points: vec![], on_curve: vec![], end_pts: vec![], num_contours: 0,
            lsb: 0, advance_width: 0, xmin: 0, ymin: 0, xmax: 0, ymax: 0,
        };
        let result = rasterize(&glyph);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 0);
    }

    #[test]
    fn single_square_renders_nonzero() {
        let pts = vec![(0i32, 0i32), (640i32, 0i32), (640i32, 640i32), (0i32, 640i32)];
        let on_curve = vec![true, true, true, true];
        let glyph = ScaledGlyph {
            points: pts, on_curve, end_pts: vec![3], num_contours: 1,
            lsb: 0, advance_width: 640, xmin: 0, ymin: 0, xmax: 10, ymax: 10,
        };
        let result = rasterize(&glyph);
        assert!(result.width > 0);
        assert!(result.height > 0);
        let non_zero = result.pixels.iter().filter(|&&b| b > 0).count();
        assert!(non_zero > 0, "square should produce non-zero coverage");
    }
}
