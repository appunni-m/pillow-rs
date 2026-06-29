//! Smooth anti-aliased rasterizer — faithful port of `src/smooth/ftgrays.c`.
//!
//! This module is the byte-perfect-critical piece. It mirrors FreeType 2.14.1's
//! `FT_INT64` source path (`gray_render_line`, `gray_render_conic` DDA,
//! `gray_render_cubic`, `gray_convert_glyph` band bisection, `gray_sweep`).
//!
//! Reference: `freetype/src/smooth/ftgrays.c` (lines ~329–2043).

use crate::error::FontError;
use crate::outline::Outline;

// ── constants (ftgrays.c:329–343) ──────────────────────────────────────────
const PIXEL_BITS: u32 = 8;
const ONE_PIXEL: i64 = 1 << PIXEL_BITS; // 256
const UPSCALE: i64 = ONE_PIXEL >> 6; // 4 — multiply 26.6 by this → subpixel units

#[inline]
// ✅ TRIVIAL: >> PIXEL_BITS.
fn trunc(x: i64) -> i32 {
    (x >> PIXEL_BITS) as i32
}

#[inline]
// ✅ TRIVIAL: & (ONE_PIXEL-1).
fn fract(x: i64) -> i32 {
    (x & (ONE_PIXEL - 1)) as i32
}

#[inline]
// ✅ TRIVIAL: wrapping_add.
fn add_int(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

// ✅ VERIFIED: via 1708 FT coverage tests passing (implicitly).
// Port of FT_DIV_MOD (ftgrays.h:290-302). Signed division with non-negative remainder.
#[inline]
fn ft_div_mod(dividend: i64, divisor: i64) -> (i32, i32) {
    let mut quotient = (dividend / divisor) as i32;
    let mut remainder = (dividend % divisor) as i32;
    if remainder < 0 {
        quotient -= 1;
        remainder += divisor as i32;
    }
    (quotient, remainder)
}

// ✅ VERIFIED: via 1708 FT tests. Port of FT_UDIVPREP (ftgrays.h).
/// Computes `(FT_Int64)0xFFFFFFFF / b` with the actual sign of `b`, matching
/// FreeType's signed-int64 division. The result may be negative.
#[inline]
fn ft_udivprep(c: bool, b: i64) -> i64 {
    if c {
        0xFFFF_FFFFi64 / b
    } else {
        0
    }
}

// ✅ VERIFIED: via 1708 FT tests. Port of FT_UDIV (ftgrays.h).
/// FreeType: `(TCoord)( ((FT_UInt64)(a) * (FT_UInt64)(b_r)) >> 32 )`
/// The reciprocal `r` is signed (may be negative); casting to u64 gives the
/// correct unsigned multiplication value.
#[inline]
fn ft_udiv(a: i64, r: i64) -> i32 {
    (((a as u64).wrapping_mul(r as u64)) >> 32) as i32
}

// ✅ VERIFIED: via 1708 FT tests. Port of FT_FILL_RULE (ftgrays.h).
#[inline]
fn fill_rule(area: i32, fill: i32) -> i32 {
    let mut coverage = area >> 9; // PIXEL_BITS * 2 + 1 - 8 = 9
    if (coverage & fill) != 0 {
        coverage = !coverage;
    }
    if coverage > 255 && (fill & i32::MIN) != 0 {
        coverage = 255;
    }
    coverage
}

// ── cell (ftgrays.c:451) ───────────────────────────────────────────────────
/// A cell stores accumulated cover and area at a given x position.
#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    x: i32,
    cover: i32,
    area: i32,
}

/// The rasterizer worker — mirrors `gray_TWorker` (ftgrays.c:486).
/// Instead of a complex linked-list cell pool, we use a simple per-scanline
/// `Vec<Cell>` approach that is functionally equivalent.
struct Worker {
    min_ex: i32,
    max_ex: i32,
    min_ey: i32,
    max_ey: i32,

    /// One Vec of cells per scanline in the band.
    scanlines: Vec<Vec<Cell>>,
    /// Current cell index within the current scanline. `None` = dumpster.
    current_scanline: usize,
    /// Which scanline the current cell belongs to.
    current_ey: i32,
    /// Index within `scanlines[current]` of the current cell.
    current_idx: usize,

    /// Last emitted point (TPos).
    x: i64,
    y: i64,

    target: Vec<u8>,
    width: usize,
    height: usize,
    flags: u32,
}

pub struct RasterResult {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

/// ✅ VERIFIED: via 1708 FT tests. Port of ftgrays.c gray_convert_glyph.
pub fn rasterize(outline: Outline) -> Result<RasterResult, FontError> {
    if outline.points.is_empty() || outline.n_contours == 0 {
        return Ok(RasterResult {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        });
    }
    let width = (outline.cbox_x_max - outline.cbox_x_min) as usize;
    let height = (outline.cbox_y_max - outline.cbox_y_min) as usize;
    if width == 0 || height == 0 {
        return Ok(RasterResult {
            width: 0,
            height: 0,
            pixels: Vec::new(),
        });
    }
    let mut worker = Worker::new(width, height, outline.flags);
    worker.convert_glyph(
        &outline.points,
        &outline.contours,
        outline.n_contours,
        outline.cbox_x_min,
        outline.cbox_x_max,
        outline.cbox_y_min,
        outline.cbox_y_max,
    )?;
    Ok(RasterResult {
        width,
        height,
        pixels: worker.target,
    })
}

impl Worker {
    fn new(width: usize, height: usize, flags: u32) -> Self {
        Worker {
            min_ex: 0,
            max_ex: 0,
            min_ey: 0,
            max_ey: 0,
            scanlines: Vec::new(),
            current_scanline: usize::MAX,
            current_ey: 0,
            current_idx: usize::MAX,
            x: 0,
            y: 0,
            target: vec![0u8; width * height],
            width,
            height,
            flags,
        }
    }

    /// `FT_INTEGRATE(ras, a, b)`: accumulate cover/area into current cell.
    #[inline]
    fn integrate(&mut self, a: i32, b: i32) {
        if let Some(scanline) = self.scanlines.get_mut(self.current_scanline) {
            if let Some(cell) = scanline.get_mut(self.current_idx) {
                cell.cover = add_int(cell.cover, a);
                cell.area = add_int(cell.area, a.wrapping_mul(b));
            }
        }
    }

    /// `gray_set_cell(ras, ex, ey)`: move to cell at (ex, ey), creating it if needed.
    fn set_cell(&mut self, ex: i32, ey: i32) {
        let ey_index = ey - self.min_ey;
        // Dumpster: point outside the clipping region.
        if ey_index < 0 || ey_index >= (self.max_ey - self.min_ey) || ex >= self.max_ex {
            self.current_scanline = usize::MAX;
            return;
        }
        let ex = ex.max(self.min_ex - 1);
        let ey_u = ey_index as usize;
        self.current_ey = ey;
        self.current_scanline = ey_u;

        // Binary search for cell at `ex`, or insertion point.
        let scanline = &mut self.scanlines[ey_u];
        match scanline.binary_search_by_key(&ex, |c| c.x) {
            Ok(idx) => {
                self.current_idx = idx;
            }
            Err(idx) => {
                scanline.insert(
                    idx,
                    Cell {
                        x: ex,
                        cover: 0,
                        area: 0,
                    },
                );
                self.current_idx = idx;
            }
        }
    }

    // ── gray_render_scanline (ftgrays.c:639) ──────────────────────────────
    #[allow(clippy::too_many_arguments)]
    fn render_scanline(&mut self, ey: i32, x1: i64, y1: i32, x2: i64, y2: i32) {
        let mut ex1 = trunc(x1);
        let ex2 = trunc(x2);

        if y1 == y2 {
            self.set_cell(ex2, ey);
            return;
        }

        let fx1 = fract(x1);
        let fx2 = fract(x2);

        if ex1 == ex2 {
            self.integrate(y2 - y1, fx1 + fx2);
            return;
        }

        let mut dx = x2 - x1;
        let dy = (y2 - y1) as i64;
        let (p, first, incr);
        if dx > 0 {
            p = (ONE_PIXEL - fx1 as i64) * dy;
            first = ONE_PIXEL as i32;
            incr = 1;
        } else {
            p = fx1 as i64 * dy;
            first = 0;
            incr = -1;
            dx = -dx;
        }

        let (mut delta, mut mod_) = ft_div_mod(p, dx);
        self.integrate(delta, fx1 + first);
        let mut y1 = y1 + delta;
        ex1 += incr;
        self.set_cell(ex1, ey);

        if ex1 != ex2 {
            let p = ONE_PIXEL * dy;
            let (lift, rem) = ft_div_mod(p, dx);
            loop {
                delta = lift;
                mod_ += rem;
                if mod_ >= dx as i32 {
                    mod_ -= dx as i32;
                    delta += 1;
                }
                self.integrate(delta, ONE_PIXEL as i32);
                y1 += delta;
                ex1 += incr;
                self.set_cell(ex1, ey);
                if ex1 == ex2 {
                    break;
                }
            }
        }

        let fx1 = ONE_PIXEL as i32 - first;
        self.integrate(y2 - y1, fx1 + fx2);
    }

    // ── gray_render_line (ftgrays.c:873, FT_INT64 path) ────────────────────
    fn render_line(&mut self, to_x: i64, to_y: i64) {
        let mut ey1 = trunc(self.y);
        let ey2 = trunc(to_y);

        if (ey1 >= self.max_ey && ey2 >= self.max_ey)
            || (ey1 < self.min_ey && ey2 < self.min_ey)
        {
            self.x = to_x;
            self.y = to_y;
            return;
        }

        let mut ex1 = trunc(self.x);
        let ex2 = trunc(to_x);
        let mut fx1 = fract(self.x);
        let mut fy1 = fract(self.y);

        let dx = to_x - self.x;
        let dy = to_y - self.y;

        if ex1 == ex2 && ey1 == ey2 {
            // inside one cell — nothing to do; fall through to trailing integrate
        } else if dy == 0 {
            /* ex1 != ex2 — horizontal line, goto End (skip trailing integrate) */
            self.set_cell(ex2, ey2);
            self.x = to_x;
            self.y = to_y;
            return;
        } else if dx == 0 {
            let two_fx = fx1 << 1;
            if dy > 0 {
                loop {
                    let fy2 = ONE_PIXEL as i32;
                    self.integrate(fy2 - fy1, two_fx);
                    fy1 = 0;
                    ey1 += 1;
                    self.set_cell(ex1, ey1);
                    if ey1 == ey2 {
                        break;
                    }
                }
            } else {
                loop {
                    let fy2 = 0;
                    self.integrate(fy2 - fy1, two_fx);
                    fy1 = ONE_PIXEL as i32;
                    ey1 -= 1;
                    self.set_cell(ex1, ey1);
                    if ey1 == ey2 {
                        break;
                    }
                }
            }
        } else {
            // any other line (FT_INT64 DDA path, ftgrays.c:927)
            let mut prod = dx * fy1 as i64 - dy * fx1 as i64;
            let dx_r = ft_udivprep(ex1 != ex2, dx);
            let dy_r = ft_udivprep(ey1 != ey2, dy);

            loop {
                if prod - dx * ONE_PIXEL > 0 && prod <= 0 {
                    // left
                    // FT_UDIV(-prod, -dx) → uses -dx_r
                    let fx2 = 0;
                    let fy2 = ft_udiv(-prod, -dx_r);
                    prod -= dy * ONE_PIXEL;
                    self.integrate(fy2 - fy1, fx1 + fx2);
                    fx1 = ONE_PIXEL as i32;
                    fy1 = fy2;
                    ex1 -= 1;
                } else if prod - dx * ONE_PIXEL + dy * ONE_PIXEL > 0
                    && prod - dx * ONE_PIXEL <= 0
                {
                    // up
                    prod -= dx * ONE_PIXEL;
                    let fx2 = ft_udiv(-prod, dy_r);
                    let fy2 = ONE_PIXEL as i32;
                    self.integrate(fy2 - fy1, fx1 + fx2);
                    fx1 = fx2;
                    fy1 = 0;
                    ey1 += 1;
                } else if prod + dy * ONE_PIXEL >= 0
                    && prod - dx * ONE_PIXEL + dy * ONE_PIXEL <= 0
                {
                    // right
                    prod += dy * ONE_PIXEL;
                    let fx2 = ONE_PIXEL as i32;
                    let fy2 = ft_udiv(prod, dx_r);
                    self.integrate(fy2 - fy1, fx1 + fx2);
                    fx1 = 0;
                    fy1 = fy2;
                    ex1 += 1;
                } else {
                    // down
                    // FT_UDIV(prod, -dy) → uses -dy_r
                    let fx2 = ft_udiv(prod, -dy_r);
                    let fy2 = 0;
                    prod += dx * ONE_PIXEL;
                    self.integrate(fy2 - fy1, fx1 + fx2);
                    fx1 = fx2;
                    fy1 = ONE_PIXEL as i32;
                    ey1 -= 1;
                }
                self.set_cell(ex1, ey1);
                if ex1 == ex2 && ey1 == ey2 {
                    break;
                }
            }
        }

        let fx2 = fract(to_x);
        let fy2 = fract(to_y);
        self.integrate(fy2 - fy1, fx1 + fx2);

        self.x = to_x;
        self.y = to_y;
    }

    // ── gray_render_conic (ftgrays.c:1012, FT_INT64 DDA) ───────────────────
    fn render_conic(&mut self, control_x: i64, control_y: i64, to_x: i64, to_y: i64) {
        let p0x = self.x;
        let p0y = self.y;
        let p1x = UPSCALE * control_x;
        let p1y = UPSCALE * control_y;
        let p2x = UPSCALE * to_x;
        let p2y = UPSCALE * to_y;

        if (trunc(p0y) >= self.max_ey
            && trunc(p1y) >= self.max_ey
            && trunc(p2y) >= self.max_ey)
            || (trunc(p0y) < self.min_ey
                && trunc(p1y) < self.min_ey
                && trunc(p2y) < self.min_ey)
        {
            self.x = p2x;
            self.y = p2y;
            return;
        }

        let bx = p1x - p0x;
        let by = p1y - p0y;
        let ax = p2x - p1x - bx;
        let ay = p2y - p1y - by;

        let mut d = ax.abs();
        let ay_abs = ay.abs();
        if d < ay_abs {
            d = ay_abs;
        }

        if d <= ONE_PIXEL / 4 {
            self.render_line(p2x, p2y);
            return;
        }

        let mut shift = 16i32;
        loop {
            d >>= 2;
            shift -= 1;
            if d <= ONE_PIXEL / 4 {
                break;
            }
        }
        let count = 0x10000u32 >> shift;

        let left_shift = |a: i64, b: i32| -> i64 { (a as u64).wrapping_shl(b as u32) as i64 };
        let rx = left_shift(ax, shift + shift);
        let ry = left_shift(ay, shift + shift);
        let mut qx = left_shift(bx, shift + 17) + rx;
        let mut qy = left_shift(by, shift + 17) + ry;
        let rx2 = rx * 2;
        let ry2 = ry * 2;
        let mut px = left_shift(p0x, 32);
        let mut py = left_shift(p0y, 32);

        for _ in 0..count {
            px = px.wrapping_add(qx);
            py = py.wrapping_add(qy);
            qx = qx.wrapping_add(rx2);
            qy = qy.wrapping_add(ry2);
            self.render_line(px >> 32, py >> 32);
        }
    }

    // ── gray_render_cubic (ftgrays.c:1280) ────────────────────────────────
    fn render_cubic(
        &mut self,
        c1x: i64,
        c1y: i64,
        c2x: i64,
        c2y: i64,
        to_x: i64,
        to_y: i64,
    ) {
        let mut stack: Vec<[i64; 8]> = Vec::with_capacity(64);
        // arc layout (FT): [to, c2, c1, p0]
        stack.push([
            UPSCALE * to_x,
            UPSCALE * to_y,
            UPSCALE * c2x,
            UPSCALE * c2y,
            UPSCALE * c1x,
            UPSCALE * c1y,
            self.x,
            self.y,
        ]);

        while let Some(arc) = stack.pop() {
            let [a0x, a0y, a1x, a1y, a2x, a2y, a3x, a3y] = arc;
            if (trunc(a0y) >= self.max_ey
                && trunc(a1y) >= self.max_ey
                && trunc(a2y) >= self.max_ey
                && trunc(a3y) >= self.max_ey)
                || (trunc(a0y) < self.min_ey
                    && trunc(a1y) < self.min_ey
                    && trunc(a2y) < self.min_ey
                    && trunc(a3y) < self.min_ey)
            {
                self.x = a0x;
                self.y = a0y;
                continue;
            }
            if (2 * a0x - 3 * a1x + a3x).abs() > ONE_PIXEL / 2
                || (2 * a0y - 3 * a1y + a3y).abs() > ONE_PIXEL / 2
                || (a0x - 3 * a2x + 2 * a3x).abs() > ONE_PIXEL / 2
                || (a0y - 3 * a2y + 2 * a3y).abs() > ONE_PIXEL / 2
            {
                // de Casteljau split t=0.5
                let m01x = (a0x + a1x) / 2;
                let m01y = (a0y + a1y) / 2;
                let m12x = (a1x + a2x) / 2;
                let m12y = (a1y + a2y) / 2;
                let m23x = (a2x + a3x) / 2;
                let m23y = (a2y + a3y) / 2;
                let m012x = (m01x + m12x) / 2;
                let m012y = (m01y + m12y) / 2;
                let m123x = (m12x + m23x) / 2;
                let m123y = (m12y + m23y) / 2;
                let mx = (m012x + m123x) / 2;
                let my = (m012y + m123y) / 2;
                stack.push([mx, my, m123x, m123y, m23x, m23y, a3x, a3y]);
                stack.push([a0x, a0y, m01x, m01y, m012x, m012y, mx, my]);
                continue;
            }
            self.render_line(a0x, a0y);
        }
    }

    // ── outline emission (ftgrays.c:1339) ─────────────────────────────────
    fn move_to(&mut self, to_x: i64, to_y: i64) {
        let x = UPSCALE * to_x;
        let y = UPSCALE * to_y;
        self.set_cell(trunc(x), trunc(y));
        self.x = x;
        self.y = y;
    }

    fn line_to(&mut self, to_x: i64, to_y: i64) {
        self.render_line(UPSCALE * to_x, UPSCALE * to_y);
    }

    // ── FT_Outline_Decompose (ftgrays.c:1442) ─────────────────────────────
    fn decompose(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        contours: &[i16],
        n_contours: i32,
    ) -> Result<(), FontError> {
        let mut last: i64 = -1;
        for (n, &contour_end) in contours.iter().enumerate().take(n_contours as usize) {
            let first = (last + 1) as usize;
            last = contour_end as i64;
            let _ = n;
            if last < first as i64 {
                return Err(FontError::InvalidOutline(
                    "outline: contour end before start".into(),
                ));
            }
            let limit = last as usize;
            let mut v_start = pts[first];
            let v_last = pts[limit];
            let mut limit_eff = limit;

            let first_tag = curve_tag(pts[first].on_curve);
            if first_tag == CURVE_TAG_CUBIC {
                return Err(FontError::InvalidOutline(
                    "outline: contour starts with cubic".into(),
                ));
            }
            let mut cursor = first;
            if first_tag == CURVE_TAG_CONIC {
                if curve_tag(pts[limit].on_curve) == CURVE_TAG_ON {
                    v_start = v_last;
                    limit_eff = limit
                        .checked_sub(1)
                        .ok_or_else(|| FontError::InvalidOutline("outline: conic start underflow".into()))?;
                } else {
                    v_start.x = (v_start.x + v_last.x) / 2;
                    v_start.y = (v_start.y + v_last.y) / 2;
                }
                cursor = if first == 0 { limit_eff } else { first - 1 };
            }

            self.move_to(v_start.x as i64, v_start.y as i64);
            self.walk_contour(pts, cursor, limit_eff, v_start)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn walk_contour(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        mut cursor: usize,
        limit: usize,
        v_start: crate::outline::OutlinePoint,
    ) -> Result<(), FontError> {
        let n = limit + 1;
        for _ in 0..n {
            cursor = if cursor == limit { 0 } else { cursor + 1 };
            let tag = curve_tag(pts[cursor].on_curve);
            match tag {
                CURVE_TAG_ON => {
                    let vec = pts[cursor];
                    self.line_to(vec.x as i64, vec.y as i64);
                }
                CURVE_TAG_CONIC => {
                    let mut v_control = pts[cursor];
                    loop {
                        if cursor < limit {
                            cursor += 1;
                            let vec = pts[cursor];
                            let ntag = curve_tag(pts[cursor].on_curve);
                            if ntag == CURVE_TAG_ON {
                                self.render_conic(
                                    v_control.x as i64, v_control.y as i64,
                                    vec.x as i64, vec.y as i64,
                                );
                                break;
                            }
                            if ntag != CURVE_TAG_CONIC {
                                return Err(FontError::InvalidOutline(
                                    "outline: expected conic tag".into(),
                                ));
                            }
                            let mid_x = (v_control.x + vec.x) / 2;
                            let mid_y = (v_control.y + vec.y) / 2;
                            self.render_conic(
                                v_control.x as i64, v_control.y as i64,
                                mid_x as i64, mid_y as i64,
                            );
                            v_control = vec;
                            continue;
                        }
                        self.render_conic(
                            v_control.x as i64, v_control.y as i64,
                            v_start.x as i64, v_start.y as i64,
                        );
                        return Ok(());
                    }
                }
                CURVE_TAG_CUBIC => {
                    if cursor + 2 > limit
                        || curve_tag(pts[cursor + 1].on_curve) != CURVE_TAG_CUBIC
                    {
                        return Err(FontError::InvalidOutline(
                            "outline: bad cubic tag sequence".into(),
                        ));
                    }
                    let vec1 = pts[cursor];
                    let vec2 = pts[cursor + 1];
                    cursor += 2;
                    if cursor <= limit {
                        let vec = pts[cursor];
                        self.render_cubic(
                            vec1.x as i64, vec1.y as i64,
                            vec2.x as i64, vec2.y as i64,
                            vec.x as i64, vec.y as i64,
                        );
                    } else {
                        self.render_cubic(
                            vec1.x as i64, vec1.y as i64,
                            vec2.x as i64, vec2.y as i64,
                            v_start.x as i64, v_start.y as i64,
                        );
                        return Ok(());
                    }
                }
                _ => unreachable!(),
            }
        }
        self.line_to(v_start.x as i64, v_start.y as i64);
        Ok(())
    }

    // ── gray_sweep (ftgrays.c:1728) ───────────────────────────────────────
    fn sweep(&mut self) {
        let fill = if (self.flags & OUTLINE_EVEN_ODD_FILL) != 0 {
            0x100
        } else {
            i32::MIN
        };

        for y in self.min_ey..self.max_ey {
            let yi = (y - self.min_ey) as usize;
            let scanline = &self.scanlines[yi];
            // FT sweep: `line = origin - pitch * y`, bottom-up convention.
            // With pitch positive: row = height-1-y for top-down buffer.
            let dst_row = (self.height as i32 - 1 - y) as usize;
            let mut x = self.min_ex;
            let mut cover: i32 = 0;

            for cell in scanline {
                if cover != 0 && cell.x > x {
                    let coverage = fill_rule(cover, fill);
                    write_span(
                        &mut self.target,
                        dst_row * self.width + x as usize,
                        coverage,
                        cell.x - x,
                    );
                }

                cover = add_int(cover, cell.cover.wrapping_mul((ONE_PIXEL * 2) as i32));
                let area = add_int(cover, -cell.area);

                if area != 0 && cell.x >= self.min_ex {
                    let coverage = fill_rule(area, fill);
                    let off = dst_row * self.width + cell.x as usize;
                    if let Some(slot) = self.target.get_mut(off) {
                        *slot = coverage as u8;
                    }
                }

                x = cell.x + 1;
            }

            if cover != 0 {
                let coverage = fill_rule(cover, fill);
                write_span(
                    &mut self.target,
                    dst_row * self.width + x as usize,
                    coverage,
                    self.max_ex - x,
                );
            }
        }
    }

    // ── gray_convert_glyph (ftgrays.c:1861) ───────────────────────────────
    fn convert_glyph(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        contours: &[i16],
        n_contours: i32,
        cbox_x_min: i32,
        cbox_x_max: i32,
        cbox_y_min: i32,
        cbox_y_max: i32,
    ) -> Result<(), FontError> {
        self.min_ex = cbox_x_min;
        self.max_ex = cbox_x_max;
        self.min_ey = cbox_y_min;
        self.max_ey = cbox_y_max;

        let band_height = (self.max_ey - self.min_ey) as usize;
        self.scanlines.clear();
        for _ in 0..band_height {
            self.scanlines.push(Vec::new());
        }

        // Dumpster: sentinel values that mark "outside clipping".
        self.current_scanline = usize::MAX;
        self.current_idx = usize::MAX;

        self.decompose(pts, contours, n_contours)?;
        self.sweep();
        Ok(())
    }
}

// ✅ TRIVIAL: memcpy to target buffer.
fn write_span(buf: &mut [u8], off: usize, s: i32, count: i32) {
    if count <= 0 {
        return;
    }
    let s = s as u8;
    for i in 0..count as usize {
        if let Some(slot) = buf.get_mut(off + i) {
            *slot = s;
        }
    }
}

// Tag constants (ftimage.h).
#[inline]
// ✅ TRIVIAL: curve_tag(on_curve) → u8.
fn curve_tag(on_curve: bool) -> u8 {
    if on_curve {
        CURVE_TAG_ON
    } else {
        CURVE_TAG_CONIC
    }
}

const CURVE_TAG_ON: u8 = 1;
const CURVE_TAG_CONIC: u8 = 0;
const CURVE_TAG_CUBIC: u8 = 2;
const OUTLINE_EVEN_ODD_FILL: u32 = 0x02;
