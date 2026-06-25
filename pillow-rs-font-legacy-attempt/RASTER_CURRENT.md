# pillow-rs-font Rasterizer — Current State

_554 lines. Cell-based, FreeType ftgrays.c compatible._

---

```rust
//! Smooth rasterizer — FreeType ftgrays.c compatible (cell-based, ONE_PIXEL=256).
//!
//! Rewrite #3: Fresh implementation focused on correctness.

use crate::scaler::ScaledGlyph;

// ── Constants matching ftgrays.c ─────────────────────────────────────────────

const PIXEL_BITS: i32 = 8;
const ONE_PIXEL: i32 = 1 << PIXEL_BITS;          // 256
const UPSCALE: i32 = ONE_PIXEL >> 6;              // 4 = 256/64
const MAX_AREA: i32 = ONE_PIXEL * ONE_PIXEL;      // 65536
const BAND_SIZE: i32 = 64;
const NIL: usize = usize::MAX;

// ── Cell ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
struct Cell {
    x: i32,
    cover: i32,
    area: i32,
    next: usize,
}

// ── Raster state ─────────────────────────────────────────────────────────────

struct Raster {
    cx: i32, cy: i32,    // current pen position (sub-pixel)
    cover: i32, area: i32, // accumulated since last x-boundary crossing
    cells: Vec<Cell>,
    free_cell: usize,
    y_cells: Vec<usize>, // linked list heads per scanline within band
    num_ycells: usize,
    band_top: i32,
}

impl Raster {
    fn new() -> Self {
        Raster {
            cx: 0, cy: 0, cover: 0, area: 0,
            cells: Vec::new(), free_cell: NIL,
            y_cells: Vec::new(), num_ycells: 0, band_top: 0,
        }
    }

    fn alloc_cell(&mut self) -> usize {
        if self.free_cell != NIL {
            let idx = self.free_cell;
            self.free_cell = self.cells[idx].next;
            self.cells[idx] = Cell::default();
            idx
        } else {
            self.cells.push(Cell::default());
            self.cells.len() - 1
        }
    }

    fn free_cell_idx(&mut self, idx: usize) {
        self.cells[idx].next = self.free_cell;
        self.free_cell = idx;
    }

    /// Record accumulated cover/area into the cell at current x position.
    /// Following FreeType's gray_set_cell: adds to existing cell at this position.
    fn set_cell(&mut self, ex: i32, _ey: i32) {
        let old_px = self.cx >> PIXEL_BITS;
        let new_px = ex >> PIXEL_BITS;

        // Record at current cell if moving to a new pixel column
        if old_px != new_px {
            self.record_cell();
        }

        // Update pen position
        self.cx = ex;
        self.cy = _ey;
    }

    /// Add current cover/area accumulators to the cell at self.cx.
    fn record_cell(&mut self) {
        let x = self.cx >> PIXEL_BITS;
        if x < 0 { return; }

        if self.cover == 0 && self.area == 0 { return; }

        let y = (self.cy >> PIXEL_BITS) - self.band_top;
        if y < 0 || y >= BAND_SIZE { return; }
        let y_idx = y as usize;

        // Ensure y_cells array is large enough
        if y_idx >= self.num_ycells {
            self.y_cells.resize(y_idx + 1, NIL);
            self.num_ycells = self.y_cells.len();
        }

        // Find existing cell at this x, or insert new one (sorted by x ascending)
        let head = self.y_cells[y_idx];
        if head == NIL || self.cells[head].x > x {
            let idx = self.alloc_cell();
            self.cells[idx].x = x;
            self.cells[idx].cover = self.cover;
            self.cells[idx].area = self.area;
            self.cells[idx].next = head;
            self.y_cells[y_idx] = idx;
        } else {
            let mut cur = head;
            loop {
                if self.cells[cur].x == x {
                    // Accumulate into existing cell
                    self.cells[cur].cover += self.cover;
                    self.cells[cur].area  += self.area;
                    break;
                }
                if self.cells[cur].next == NIL || self.cells[self.cells[cur].next].x > x {
                    let idx = self.alloc_cell();
                    self.cells[idx].x = x;
                    self.cells[idx].cover = self.cover;
                    self.cells[idx].area = self.area;
                    self.cells[idx].next = self.cells[cur].next;
                    self.cells[cur].next = idx;
                    break;
                }
                cur = self.cells[cur].next;
            }
        }

        // Reset accumulators
        self.cover = 0;
        self.area = 0;
    }

    /// Finish current cell before pen moves
    fn finish_cell(&mut self) {
        self.record_cell();
    }
}

// ── DDA line rendering ───────────────────────────────────────────────────────

/// Render a line from current pen to (ex, ey).
/// Uses simple per-pixel DDA: step one pixel column at a time, accumulate cover/area.
fn gray_render_line(ras: &mut Raster, ex: i32, ey: i32) {
    let dx = ex - ras.cx;
    let dy = ey - ras.cy;
    if dx == 0 && dy == 0 { return; }

    if dy == 0 {
        // Horizontal: move pen
        ras.set_cell(ex, ey);
        return;
    }

    // For vertical lines (dx==0): accumulate and record at each y pixel boundary
    if dx == 0 {
        let incr: i32 = if dy > 0 { 1 } else { -1 };
        let fx = ras.cx & (ONE_PIXEL - 1);
        let mut y = ras.cy;
        while y != ey {
            let y_next = if incr > 0 {
                if (y & (ONE_PIXEL - 1)) != 0 {
                    (y + ONE_PIXEL) & !(ONE_PIXEL - 1)
                } else {
                    y + ONE_PIXEL
                }
            } else {
                if (y & (ONE_PIXEL - 1)) != 0 {
                    y & !(ONE_PIXEL - 1)
                } else {
                    y - ONE_PIXEL
                }
            };

            let y_stop = if incr > 0 { y_next.min(ey) } else { y_next.max(ey) };
            let d = (y_stop as i64 - y as i64).unsigned_abs() as i32;

            let cov = incr.saturating_mul(d);
            // Scale cover by ONE_PIXEL to match FreeType's internal units.
            // FreeType accumulates cover per ONE_PIXEL step, but the DDA
            // produces per-sub-pixel values.
            ras.cover = ras.cover.saturating_add(cov);
            ras.area = ras.area.saturating_add(cov.saturating_mul(ONE_PIXEL - fx));

            y = y_stop;
            // Record cell at each y boundary
            ras.cy = y;
            ras.record_cell();
        }
        ras.cx = ex;
        ras.cy = ey;
        return;
    }

    // Step along the major axis to ensure convergence.
    // For steep lines (|dx| < |dy|): step one y-pixel at a time.
    // For shallow lines (|dx| >= |dy|): step one x-pixel at a time.
    let mut cx = ras.cx;
    let mut cy = ras.cy;
    let dx_abs = dx.unsigned_abs();
    let dy_abs = dy.unsigned_abs();

    if dx_abs >= dy_abs {
        // Shallow: step in x
        let step_x: i32 = if dx > 0 { 1 } else { -1 };
        while cx != ex {
            let next_x = if step_x > 0 {
                (cx + ONE_PIXEL) & !(ONE_PIXEL - 1)
            } else {
                (cx - 1) & !(ONE_PIXEL - 1)
            };
            // Clamp to target
            let x_target = if step_x > 0 { next_x.min(ex) } else { next_x.max(ex) };
            let x_step = (x_target - cx).unsigned_abs() as i64;
            let y_frac = (x_step * dy as i64) / dx as i64;
            cx = x_target;
            cy = (cy as i64 + y_frac) as i32;

            let cov = y_frac as i32;
            ras.cover = ras.cover.saturating_add(cov);
            ras.area = ras.area.saturating_add(cov.saturating_mul(ras.cx & (ONE_PIXEL - 1)));

            ras.cx = cx;
            ras.cy = cy;
            ras.set_cell(cx, cy);
        }
    } else {
        // Steep: step in y
        let step_y: i32 = if dy > 0 { 1 } else { -1 };
        while cy != ey {
            let next_y = if step_y > 0 {
                (cy + ONE_PIXEL) & !(ONE_PIXEL - 1)
            } else {
                (cy - 1) & !(ONE_PIXEL - 1)
            };
            // Clamp to target
            let y_target = if step_y > 0 { next_y.min(ey) } else { next_y.max(ey) };
            let y_step = (y_target - cy).unsigned_abs() as i64;
            let x_frac = (y_step * dx as i64) / dy as i64;
            cy = y_target;
            cx = (cx as i64 + x_frac) as i32;

            let cov = step_y.saturating_mul(y_step as i32);
            let fx = ras.cx & (ONE_PIXEL - 1);
            ras.cover = ras.cover.saturating_add(cov.saturating_mul(ONE_PIXEL));
            ras.area = ras.area.saturating_add(cov.saturating_mul(ONE_PIXEL).saturating_mul(fx));

            let old_px = ras.cx >> PIXEL_BITS;
            let new_px = cx >> PIXEL_BITS;
            if old_px != new_px {
                ras.cx = cx;
                ras.cy = cy;
                ras.set_cell(cx, cy);
            } else {
                ras.cx = cx;
                ras.cy = cy;
                ras.record_cell();
            }
        }
    }

    ras.cx = ex;
    ras.cy = ey;
    ras.set_cell(ex, ey);
}

// ── Conic (quadratic bezier) rendering ───────────────────────────────────────

const FLATNESS_THRESHOLD: i32 = ONE_PIXEL / 4; // 64 in FreeType units (0.25 px)

fn conic_flatness(p0: (i32, i32), p1: (i32, i32), p2: (i32, i32)) -> i32 {
    let dx = (p0.0 + p2.0 - 2 * p1.0).abs();
    let dy = (p0.1 + p2.1 - 2 * p1.1).abs();
    dx.max(dy)
}

fn gray_render_conic(ras: &mut Raster, control: (i32, i32), to: (i32, i32)) {
    let p0 = (ras.cx, ras.cy);
    let mut stack: Vec<((i32,i32), (i32,i32), (i32,i32))> = Vec::with_capacity(32);
    stack.push((p0, control, to));

    while let Some((a, b, c)) = stack.pop() {
        if conic_flatness(a, b, c) <= FLATNESS_THRESHOLD {
            gray_render_line(ras, b.0, b.1);
            gray_render_line(ras, c.0, c.1);
        } else {
            let ab = ((a.0 + b.0) / 2, (a.1 + b.1) / 2);
            let bc = ((b.0 + c.0) / 2, (b.1 + c.1) / 2);
            let mid = ((ab.0 + bc.0) / 2, (ab.1 + bc.1) / 2);
            stack.push((mid, bc, c));
            stack.push((a, ab, mid));
        }
    }
}

// ── Sweep ─────────────────────────────────────────────────────────────────────

fn gray_sweep(ras: &mut Raster, pixels: &mut [u8], w: u32, band_top: i32) {
    let w_i32 = w as i32;

    for y in 0..ras.num_ycells as i32 {
        let mut cell_idx = ras.y_cells[y as usize];
        let mut cover: i32 = 0;

        // x tracks the last processed pixel column; start at 0
        let mut x: i32 = 0;

        while cell_idx != NIL {
            let cell = &ras.cells[cell_idx];

            // Fill span [x, cell.x) with current cover
            if cover != 0 && x < cell.x {
                // FreeType span fill: coverage = cover >> (PIXEL_BITS + 1) = cover >> 9
                let span_alpha = (cover.abs() >> (PIXEL_BITS + 1)).min(255);
                if span_alpha > 0 {
                    for px in x..cell.x {
                        if px >= 0 && px < w_i32 {
                            let idx = ((y + band_top) as i64 * w as i64 + px as i64) as usize;
                            if idx < pixels.len() {
                                let a = pixels[idx] as i32 + span_alpha as i32;
                                pixels[idx] = a.min(255) as u8;
                            }
                        }
                    }
                }
            }

            // Accumulate this cell's cover delta (FreeType: cover += cell->cover * ONE_PIXEL * 2)
            cover += cell.cover * ONE_PIXEL * 2;

            // Cell area fill: coverage = area >> (PIXEL_BITS * 2 + 1) = area >> 17
            let area = cover - cell.area;
            if area != 0 {
                let alpha_val = (area.abs() >> (PIXEL_BITS * 2 + 1)) & 255;
                if alpha_val > 0 {
                    let px = cell.x;
                    if px >= 0 && px < w_i32 {
                        let idx = ((y + band_top) as i64 * w as i64 + px as i64) as usize;
                        if idx < pixels.len() {
                            let a = pixels[idx] as i32 + alpha_val as i32;
                            pixels[idx] = a.min(255) as u8;
                        }
                    }
                }
            }

            x = cell.x + 1;
            cell_idx = cell.next;
        }

        // Fill remaining span
        if cover != 0 && x < w_i32 {
            let rem_alpha = (cover.abs() >> (PIXEL_BITS + 1)).min(255);
            if rem_alpha > 0 {
                for px in x..w_i32 {
                    let idx = ((y + band_top) as i64 * w as i64 + px as i64) as usize;
                    if idx < pixels.len() {
                        let a = pixels[idx] as i32 + rem_alpha as i32;
                        pixels[idx] = a.min(255) as u8;
                    }
                }
            }
        }
    }

    // Free cells
    for i in 0..ras.num_ycells {
        let mut idx = ras.y_cells[i];
        while idx != NIL {
            let next = ras.cells[idx].next;
            ras.free_cell_idx(idx);
            idx = next;
        }
    }
    ras.y_cells.fill(NIL);
    ras.num_ycells = 0;
}

// ── Public API ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(crate) struct RasterizedGlyph {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub xmin: i32,
    pub ymin: i32,
}

pub(crate) fn rasterize(glyph: &ScaledGlyph) -> RasterizedGlyph {
    if glyph.points.is_empty() || glyph.num_contours == 0 {
        return RasterizedGlyph { width: 0, height: 0, pixels: vec![], xmin: 0, ymin: 0 };
    }

    let w = (glyph.xmax - glyph.xmin).max(0).min(4096) as u32;
    let h = (glyph.ymax - glyph.ymin).max(0).min(4096) as u32;
    if w == 0 || h == 0 {
        return RasterizedGlyph { width: 0, height: 0, pixels: vec![], xmin: 0, ymin: 0 };
    }

    // Expand implicit on-curve midpoints
    let mut expanded: Vec<(i32, i32)> = Vec::new();
    let mut expanded_oc: Vec<bool> = Vec::new();
    let mut expanded_end: Vec<usize> = Vec::new();
    let mut pt_idx = 0usize;

    for &end_idx in &glyph.end_pts {
        let start = pt_idx;
        let end = end_idx as usize + 1;
        let len = end - start;
        for i in 0..len {
            let p = glyph.points[start + i];
            let ni = if i + 1 < len { start + i + 1 } else { start };
            let np = glyph.points[ni];
            let oc = glyph.on_curve[start + i];
            let noc = glyph.on_curve[ni];
            expanded.push(p);
            expanded_oc.push(oc);
            if !oc && !noc {
                expanded.push(((p.0 + np.0) / 2, (p.1 + np.1) / 2));
                expanded_oc.push(true);
            }
        }
        expanded_end.push(expanded.len() - 1);
        pt_idx = end;
    }

    // Rendering in bands
    let total = (w * h) as usize;
    let mut pixels = vec![0u8; total];

    for band_y in (0..h as i32).step_by(BAND_SIZE as usize) {
        let _band_end = (band_y + BAND_SIZE).min(h as i32);
        let mut ras = Raster::new();
        ras.band_top = band_y;

        // Offset coordinates so points align with pixel buffer.
        // Flip y-axis: TrueType uses y-up, bitmap uses y-down.
        let off_x = glyph.xmin;
        let off_y = glyph.ymin;
        // Total height in sub-pixel units (ONE_PIXEL=256)
        let total_h = (h as i32) * ONE_PIXEL;

        // Convert a 26.6 point to upscaled, offset, y-flipped sub-pixel coordinate
        let to_subpx = |p: (i32, i32)| -> (i32, i32) {
            let sx = (p.0 - off_x * 64) * UPSCALE;
            // Flip y: bitmap y = total_h - 1 - (scaled y - ymin)
            // scaled y = (p.1 * UPSCALE) + y_offset. We want: top of glyph at y=0.
            // topmost y (highest in y-up) maps to bitmap y=0.
            // For TrueType y-up: max_y is the top.
            // After offset: (p.1 - off_y*64) * UPSCALE gives sub-pixel coords where
            // the glyph bottom is at 0. We flip: bitmap_y = total_h - 1 - subpx_y.
            let raw_y = (p.1 - off_y * 64) * UPSCALE;
            let sy = total_h.saturating_sub(raw_y);
            (sx, sy.max(0))
        };

        // Start pen at top-left of band (band-relative y)
        ras.cx = 0;
        ras.cy = band_y * ONE_PIXEL;
        ras.set_cell(ras.cx, ras.cy);

        pt_idx = 0;
        for &end_idx in &expanded_end {
            let start = pt_idx;
            let end = end_idx + 1;
            let len = end - start;

            let mut i = 0;
            while i < len {
                let idx = start + i;
                if !expanded_oc[idx] { i += 1; continue; }

                let p0 = expanded[idx];
                if i == 0 {
                    // Move to first point of contour, offset by the glyph bbox
                    let sp0 = to_subpx(p0);
                    ras.cx = sp0.0;
                    ras.cy = sp0.1;
                    ras.set_cell(sp0.0, sp0.1);
                }

                let ni = if i + 1 < len { i + 1 } else { 0 };
                let nidx = start + ni;
                let p1 = to_subpx(expanded[nidx]);
                let oc1 = expanded_oc[nidx];

                if oc1 {
                    gray_render_line(&mut ras, p1.0, p1.1);
                    i += 1;
                } else {
                    let ei = if i + 2 < len { i + 2 } else { 0 };
                    let eidx = start + ei;
                    let p2 = to_subpx(expanded[eidx]);
                    gray_render_conic(&mut ras, p1, p2);
                    i += 2;
                }
            }
            pt_idx = end;
        }

        // Finish any pending cell
        ras.finish_cell();
        // Sweep to pixels
        gray_sweep(&mut ras, &mut pixels, w, band_y);
    }

    RasterizedGlyph { width: w, height: h, pixels, xmin: glyph.xmin, ymin: glyph.ymin }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_glyph(pts: Vec<(i32, i32)>, oc: Vec<bool>, eps: Vec<u16>) -> ScaledGlyph {
        let nc = eps.len() as u16;
        ScaledGlyph {
            points: pts, on_curve: oc, end_pts: eps,
            num_contours: nc,
            lsb: 0, advance_width: 0, xmin: 0, ymin: 0, xmax: 10, ymax: 10,
        }
    }

    #[test]
    fn empty_glyph_returns_zero_size() {
        let g = ScaledGlyph {
            points: vec![], on_curve: vec![], end_pts: vec![], num_contours: 0,
            lsb: 0, advance_width: 0, xmin: 0, ymin: 0, xmax: 0, ymax: 0,
        };
        assert_eq!(rasterize(&g).width, 0);
    }

    #[test]
    fn square_renders_nonzero() {
        let pts = vec![(0,0), (640,0), (640,640), (0,640)];
        let oc = vec![true, true, true, true];
        let g = make_glyph(pts, oc, vec![3]);
        let r = rasterize(&g);
        let nz = r.pixels.iter().filter(|&&b| b > 0).count();
        assert!(nz > 0, "square should produce non-zero coverage, got {nz}");
    }

    #[test]
    fn vertical_edge_produces_coverage() {
        // A single vertical edge from top to bottom at x=5
        let pts = vec![(320, 0), (320, 640)];
        let oc = vec![true, true];
        let g = make_glyph(pts, oc, vec![1]);
        let r = rasterize(&g);
        let nz = r.pixels.iter().filter(|&&b| b > 0).count();
        assert!(nz > 0, "vertical edge should produce coverage, got {nz}");
    }
}
```
