//! Smooth rasterizer — faithful port of FreeType ftgrays.c.

use crate::scaler::ScaledGlyph;

const PIXEL_BITS: i32 = 8;
const ONE_PIXEL: i32 = 1 << PIXEL_BITS; // 256
const UPSCALE: i32 = ONE_PIXEL >> 6;    // 4
const NIL: usize = usize::MAX;

#[derive(Debug, Clone, Default)]
struct Cell { x: i64, cover: i64, area: i64, next: usize }

pub(crate) struct RasterizedGlyph {
    pub width: u32, pub height: u32, pub pixels: Vec<u8>,
    pub xmin: i32, pub ymin: i32,
}

// ── Free functions for cell operations ───────────────────────────────────

fn set_cell(cells: &mut Vec<Cell>, ycells: &mut [usize], free: &mut usize,
            cur_cell: &mut usize, cell_null: usize, ex: i64, ey: i64,
            band_y: i64, hh: usize, w: i64) {
    let ey_idx = (ey - band_y) as i32;
    if ey_idx < 0 || ey_idx as usize >= hh || ex >= w {
        *cur_cell = cell_null; return;
    }
    let ey_u = ey_idx as usize;
    let ec = if ex < -1 { -1i64 } else { ex };
    let mut prev = NIL;
    let mut cur = ycells[ey_u];
    loop {
        if cur == NIL || cells[cur].x > ec {
            let i = if *free != NIL {
                let i = *free; *free = cells[i].next; cells[i] = Cell::default(); i
            } else {
                let i = cells.len(); cells.push(Cell::default()); i
            };
            cells[i].x = ec; cells[i].next = cur;
            if prev == NIL { ycells[ey_u] = i; } else { cells[prev].next = i; }
            *cur_cell = i; return;
        }
        if cells[cur].x == ec { *cur_cell = cur; return; }
        prev = cur;
        cur = cells[cur].next;
    }
}

fn integ(cells: &mut [Cell], cur_cell: usize, cell_null: usize, a: i64, b: i64) {
    if cur_cell != cell_null { cells[cur_cell].cover += a; cells[cur_cell].area += a * b; }
}

/// Render a line. Returns the new (rcx, rcy) on success, None if clipped.
fn render_line(
    cells: &mut Vec<Cell>, ycells: &mut [usize], free_cell: &mut usize,
    cur_cell: &mut usize, cell_null: usize,
    band_y: i64, be: i64, hh: usize, w: i64,
    rcx: i64, rcy: i64, tox: i64, toy: i64,
) -> Option<(i64,i64)> {
    let dx = tox - rcx; let dy = toy - rcy;
    if dx == 0 && dy == 0 { return None; }
    let ey1 = rcy >> PIXEL_BITS;
    let ey2 = toy >> PIXEL_BITS;
    if (ey1 >= be && ey2 >= be) || (ey1 < band_y && ey2 < band_y) {
        return None;
    }
    let ex1 = rcx >> PIXEL_BITS;
    let ex2 = tox >> PIXEL_BITS;
    let fx1 = rcx & (ONE_PIXEL-1) as i64;
    let fy1 = rcy & (ONE_PIXEL-1) as i64;

    // Compute final integration values based on the branch.
    let final_da: i64;
    let final_db: i64;

    if dy == 0 {
        set_cell(cells, ycells, free_cell, cur_cell, cell_null, ex2, ey2, band_y, hh, w);
        final_da = (toy & (ONE_PIXEL-1) as i64) - fy1;
        final_db = fx1 + (tox & (ONE_PIXEL-1) as i64);
    } else if dx == 0 {
        let t2 = fx1 * 2;
        if dy > 0 {
            let mut fy = fy1; let mut ey = ey1; let mut sc = 0;
            loop { sc += 1; if sc > 0x100000 { break; }
                integ(cells, *cur_cell, cell_null, ONE_PIXEL as i64 - fy, t2);
                ey += 1;
                if ey == ey2 { break; }
                set_cell(cells, ycells, free_cell, cur_cell, cell_null, ex1, ey, band_y, hh, w);
                fy = 0;
            }
            final_da = (toy & (ONE_PIXEL-1) as i64) - 0i64;
            final_db = fx1 + (tox & (ONE_PIXEL-1) as i64);
        } else {
            let mut fy = fy1; let mut ey = ey1; let mut sc = 0;
            loop { sc += 1; if sc > 0x100000 { break; }
                integ(cells, *cur_cell, cell_null, -fy, t2);
                ey -= 1;
                if ey == ey2 { break; }
                set_cell(cells, ycells, free_cell, cur_cell, cell_null, ex1, ey, band_y, hh, w);
                fy = ONE_PIXEL as i64;
            }
            final_da = (toy & (ONE_PIXEL-1) as i64) - ONE_PIXEL as i64;
            final_db = fx1 + (tox & (ONE_PIXEL-1) as i64);
        }
    } else {
        let op = ONE_PIXEL as i64;
        let mut _s = 0;
        let mut fx = fx1; let mut fy = fy1;
        let mut ex1 = ex1; let mut ey1 = ey1;
        let mut prod = dx * fy - dy * fx;
        let mut sc = 0;
        loop { sc += 1; if sc > 0x100000 { break; }
            if prod - dx * op > 0 && prod <= 0 {
                let f2 = (-prod) / (-dx);
                integ(cells, *cur_cell, cell_null, f2 - fy, fx + 0);
                prod -= dy * op; fx = op; fy = f2; ex1 -= 1;
            } else if prod - dx * op + dy * op > 0 && prod - dx * op <= 0 {
                prod -= dx * op;
                let f2 = (-prod) / dy;
                integ(cells, *cur_cell, cell_null, op - fy, fx + f2);
                fx = f2; fy = 0; ey1 += 1;
            } else if prod + dy * op >= 0 && prod - dx * op + dy * op <= 0 {
                prod += dy * op;
                let f2 = prod / dx;
                integ(cells, *cur_cell, cell_null, f2 - fy, fx + op);
                fx = 0; fy = f2; ex1 += 1;
            } else {
                let f2 = prod / (-dy);
                integ(cells, *cur_cell, cell_null, 0 - fy, fx + f2);
                prod += dx * op; fx = f2; fy = op; ey1 -= 1;
            }
            set_cell(cells, ycells, free_cell, cur_cell, cell_null, ex1, ey1, band_y, hh, w);
            if ex1 == ex2 && ey1 == ey2 { break; }
        }
        // After the loop, fx,fy are the last fractional positions from the boundary crossing.
        // The final integration covers the remaining fraction to (tox, toy).
        final_da = (toy & (ONE_PIXEL-1) as i64) - fy;
        final_db = fx + (tox & (ONE_PIXEL-1) as i64);
    }

    integ(cells, *cur_cell, cell_null, final_da, final_db);
    Some((tox, toy))
}

fn render_conic(
    cells: &mut Vec<Cell>, ycells: &mut [usize], free_cell: &mut usize,
    cur_cell: &mut usize, cell_null: usize,
    band_y: i64, be: i64, hh: usize, w: i64,
    rcx: &mut i64, rcy: &mut i64,
    control: (i64,i64), to: (i64,i64),
) {
    let mut stack: Vec<((i64,i64),(i64,i64),(i64,i64))> = Vec::with_capacity(32);
    stack.push(((*rcx,*rcy), control, to));
    let mut _conic_safety = 0;
    while let Some((ax, bx, cx)) = stack.pop() {
        _conic_safety += 1; if _conic_safety > 0x10000 { break; }
        if (ax.0 + cx.0 - 2*bx.0).abs().max((ax.1 + cx.1 - 2*bx.1).abs()) <= (ONE_PIXEL/4) as i64 {
            for &(tox, toy) in &[bx, cx] {
                if let Some(p) = render_line(cells, ycells, free_cell, cur_cell, cell_null,
                    band_y, be, hh, w, *rcx, *rcy, tox, toy) {
                    *rcx = p.0; *rcy = p.1;
                }
            }
        } else {
            let ab = ((ax.0+bx.0)/2, (ax.1+bx.1)/2);
            let bc = ((bx.0+cx.0)/2, (bx.1+cx.1)/2);
            let mid = ((ab.0+bc.0)/2, (ab.1+bc.1)/2);
            stack.push((mid, bc, cx));
            stack.push((ax, ab, mid));
        }
    }
}

// ── Main rasterize ─────────────────────────────────────────────────────────

pub(crate) fn rasterize(glyph: &ScaledGlyph) -> RasterizedGlyph {
    if glyph.points.is_empty() || glyph.num_contours == 0 {
        return RasterizedGlyph { width: 0, height: 0, pixels: vec![], xmin: 0, ymin: 0 };
    }
    // w needs +1 to fit cells at the rightmost pixel boundary
    let w = (glyph.xmax - glyph.xmin + 1).max(0).min(4096) as u32;
    let h = (glyph.ymax - glyph.ymin).max(0).min(4096) as u32;
    if w == 0 || h == 0 {
        return RasterizedGlyph { width: 0, height: 0, pixels: vec![], xmin: 0, ymin: 0 };
    }

    // Expand implicit on-curve midpoints
    let mut ex: Vec<(i32,i32)> = Vec::new();
    let mut eoc: Vec<bool> = Vec::new();
    let mut eend: Vec<usize> = Vec::new();
    let mut pi = 0usize;
    for &ei in &glyph.end_pts {
        let s = pi; let e = ei as usize + 1; let l = e - s;
        for i in 0..l {
            let p = glyph.points[s + i];
            let ni = if i + 1 < l { s + i + 1 } else { s };
            let np = glyph.points[ni];
            let oc = glyph.on_curve[s + i]; let noc = glyph.on_curve[ni];
            ex.push(p); eoc.push(oc);
            if !oc && !noc { ex.push(((p.0+np.0)/2,(p.1+np.1)/2)); eoc.push(true); }
        }
        eend.push(ex.len() - 1); pi = e;
    }

    let total = (w * h) as usize;
    let mut pixels = vec![0u8; total];
    let off_x = glyph.xmin as i64;
    let off_y = glyph.ymin as i64;
    // total_h removed — no y-flip needed
    let band_size = 64i64;
    let w_i64 = w as i64;

    for band_y in (0..h as i64).step_by(band_size as usize) {
        let be = (band_y + band_size).min(h as i64);
        let hh = (be - band_y) as usize;

        let mut ycells: Vec<usize> = vec![NIL; hh];
        let mut cells: Vec<Cell> = vec![Cell::default()];
        let cell_null = 0usize;
        let mut free_cell = NIL;
        let mut cur_cell = cell_null;

        let to_sp = |p: (i32,i32)| -> (i64,i64) {
            let sx = (p.0 as i64 - off_x * 64) * UPSCALE as i64;
            let raw_y = (p.1 as i64 - off_y * 64) * UPSCALE as i64;
            (sx, raw_y.max(0)) // no y-flip — TrueType y-up, bitmap y-down
        };

        let mut rcx = 0i64;
        let mut rcy = band_y * ONE_PIXEL as i64;

        // Set initial cell at (0, band_y)
        set_cell(&mut cells, &mut ycells, &mut free_cell, &mut cur_cell, cell_null,
                 0, band_y, band_y, hh, w_i64);

        pi = 0;
        for &endi in &eend {
            let s = pi; let e = endi + 1; let l = e - s;
            let mut i = 0usize;
            while i < l {
                let idx = s + i;
                if !eoc[idx] { i += 1; continue; }
                let p0 = ex[idx];
                if i == 0 {
                    let sp = to_sp(p0);
                    set_cell(&mut cells, &mut ycells, &mut free_cell, &mut cur_cell, cell_null,
                             sp.0 >> PIXEL_BITS, sp.1 >> PIXEL_BITS, band_y, hh, w_i64);
                    rcx = sp.0; rcy = sp.1;
                }
                let ni = if i + 1 < l { i + 1 } else { 0 };
                let nidx = s + ni;
                let p1 = to_sp(ex[nidx]);
                let oc1 = eoc[nidx];

                if oc1 {
                    if let Some(p) = render_line(&mut cells, &mut ycells, &mut free_cell,
                        &mut cur_cell, cell_null, band_y, be, hh, w_i64, rcx, rcy, p1.0, p1.1) {
                        rcx = p.0; rcy = p.1;
                    }
                    i += 1;
                } else {
                    let ei = if i + 2 < l { i + 2 } else { 0 };
                    let eidx = s + ei;
                    let p2 = to_sp(ex[eidx]);
                    render_conic(&mut cells, &mut ycells, &mut free_cell,
                        &mut cur_cell, cell_null, band_y, be, hh, w_i64,
                        &mut rcx, &mut rcy, p1, p2);
                    i += 2;
                }
            }
            pi = e;
        }

        // Sweep
        for y in band_y..be {
            let yi = (y - band_y) as usize;
            if yi >= hh { continue; }
            let mut ci = ycells[yi];
            let mut x = -1i64;
            let mut cover: i64 = 0;

            while ci != NIL {
                let cell = &cells[ci];
                if cover != 0 && cell.x > x {
                    let mut cov = (cover >> 9) as i32;
                    if cov & (i32::MIN) != 0 { cov = !cov; }
                    if cov > 255 { cov = 255; }
                    if cov > 0 {
                        for px in (x+1)..cell.x {
                            if px >= 0 && px < w_i64 {
                                let idx = (y * w_i64 + px) as usize;
                                if idx < pixels.len() {
                                    let p = pixels[idx] as u16 + cov as u16;
                                    pixels[idx] = p.min(255) as u8;
                                }
                            }
                        }
                    }
                }
                cover += cell.cover * (ONE_PIXEL as i64 * 2);
                let area = cover - cell.area;
                if area != 0 {
                    let mut cov = (area >> 9) as i32;
                    if cov & (i32::MIN) != 0 { cov = !cov; }
                    if cov > 255 { cov = 255; }
                    if cov > 0 {
                        let px = cell.x;
                        if px >= 0 && px < w_i64 {
                            let idx = (y * w_i64 + px) as usize;
                            if idx < pixels.len() {
                                let p = pixels[idx] as u16 + cov as u16;
                                pixels[idx] = p.min(255) as u8;
                            }
                        }
                    }
                }
                x = cell.x;
                ci = cell.next;
            }
            if cover != 0 {
                let mut cov = (cover >> 9) as i32;
                if cov & (i32::MIN) != 0 { cov = !cov; }
                if cov > 255 { cov = 255; }
                if cov > 0 {
                    for px in (x+1)..w_i64 {
                        if px >= 0 {
                            let idx = (y * w_i64 + px) as usize;
                            if idx < pixels.len() {
                                let p = pixels[idx] as u16 + cov as u16;
                                pixels[idx] = p.min(255) as u8;
                            }
                        }
                    }
                }
            }
        }
    }
    RasterizedGlyph { width: w, height: h, pixels, xmin: glyph.xmin, ymin: glyph.ymin }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn mg(pts: Vec<(i32,i32)>, oc: Vec<bool>, eps: Vec<u16>) -> ScaledGlyph {
        let nc = eps.len() as u16;
        ScaledGlyph { points: pts, on_curve: oc, end_pts: eps, num_contours: nc,
            lsb: 0, advance_width: 0, xmin: 0, ymin: 0, xmax: 10, ymax: 10 }
    }
    #[test] fn empty_glyph_returns_zero_size() {
        let g = ScaledGlyph { points: vec![], on_curve: vec![], end_pts: vec![], num_contours: 0,
            lsb: 0, advance_width: 0, xmin: 0, ymin: 0, xmax: 0, ymax: 0 };
        assert_eq!(rasterize(&g).width, 0);
    }
    #[test] fn square_renders_nonzero() {
        let r = rasterize(&mg(vec![(0,0),(640,0),(640,640),(0,640)], vec![true,true,true,true], vec![3]));
        assert!(r.pixels.iter().filter(|&&b|b>0).count() > 0);
    }
}
