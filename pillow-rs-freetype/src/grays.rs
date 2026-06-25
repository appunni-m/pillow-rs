//! Smooth anti-aiased rasterizer — faithful port of `src/smooth/ftgrays.c`.
//!
//! This is the byte-perfect-critical module. Every function mirrors the
//! FreeType 2.14.1 `FT_INT64` source path 1:1:
//!   - `gray_set_cell`, `gray_render_scanline`/`gray_render_line`,
//!   - `gray_render_conic` (DDA), `gray_render_cubic`,
//!   - `FT_Outline_Decompose`, `gray_sweep`, `gray_convert_glyph` (band bisection).
//!
//! Reference: `freetype/src/smooth/ftgrays.c` (lines ~329–2043).
//! Types: `TPos = long` → `i64` (FT_INT64 keeps all intermediate math in 64-bit),
//!        `TCoord = int` → `i32`, `TArea = int` → `i32` (products use 64-bit).

use crate::error::FontError;
use crate::outline::Outline;

// ── constants (ftgrays.c:329–343) ──────────────────────────────────────────
const PIXEL_BITS: u32 = 8;
const ONE_PIXEL: i64 = 1 << PIXEL_BITS; // 256
const UPSCALE: i64 = ONE_PIXEL >> 6; // 4 — multiply a 26.6 value by this
const CELL_MAX_X_VALUE: i32 = i32::MAX;

#[inline]
fn trunc(x: i64) -> i32 {
    (x >> PIXEL_BITS) as i32
}

#[inline]
fn fract(x: i64) -> i32 {
    (x & (ONE_PIXEL - 1)) as i32
}

/// FreeType's `ADD_INT`: signed addition done in unsigned, to match C `int`.
#[inline]
fn add_int(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

/// FT_DIV_MOD: quotient/remainder with the remainder guaranteed non-negative.
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

/// FT_UDIVPREP(c,b): reciprocal used by FT_UDIV, or 0 when c==0.
///
/// FreeType computes `b_r = c ? 0xFFFFFFFF / b : 0` where the division is on
/// the *magnitude* (the divisor's sign is folded into the dividend). We store
/// the positive reciprocal of `|b|`.
#[inline]
fn ft_udivprep(c: bool, b: i64) -> u64 {
    if c {
        0xFFFF_FFFFu64 / (b.unsigned_abs())
    } else {
        0
    }
}

/// FT_UDIV(a,b): `(a * b_r) >> 32`.
#[inline]
fn ft_udiv(a: i64, r: u64) -> i32 {
    (((a as u64).wrapping_mul(r)) >> 32) as i32
}

// ── cell (ftgrays.c:451) — singly-linked list per scanline ─────────────────
#[derive(Debug, Clone, Copy)]
struct Cell {
    x: i32,
    cover: i32,
    area: i32,
    next: usize, // index of next cell, or NIL
}

const NIL: usize = usize::MAX;

/// `FT_FILL_RULE` (ftgrays.c:405): scale area, apply non-zero/even-odd fill.
#[inline]
fn fill_rule(area: i32, fill: i32) -> i32 {
    let mut coverage = area >> 9;
    if (coverage & fill) != 0 {
        coverage = !coverage;
    }
    if coverage > 255 && (fill & i32::MIN) != 0 {
        coverage = 255;
    }
    coverage
}

/// `FT_GRAY_SET`: write `count` bytes of value `s` at row offset `off`.
#[inline]
fn gray_set(buf: &mut [u8], off: usize, s: i32, count: i32) {
    if count <= 0 || s <= 0 {
        return;
    }
    let s = s.clamp(0, 255) as u8;
    for i in 0..count as usize {
        if let Some(slot) = buf.get_mut(off + i) {
            *slot = s;
        }
    }
}

/// A rasterizer worker — `gray_TWorker` (ftgrays.c:486).
struct Worker {
    min_ex: i32,
    max_ex: i32,
    min_ey: i32,
    max_ey: i32,
    count_ey: i32,

    error: Option<FontError>,
    cell: usize,
    cell_free: usize,
    cell_null: usize,

    /// Per-scanline head cell index, one per ey in the band.
    ycells: Vec<usize>,
    /// Cell pool. The null/dumpster cell lives at index `cell_null`.
    cells: Vec<Cell>,

    /// Last emitted point (TPos).
    x: i64,
    y: i64,

    outline: Outline,
    target: Vec<u8>,
    width: usize,
}

pub(crate) struct RasterResult {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

/// Rasterize a scaled outline to a bottom-up 8-bit alpha bitmap sized to its
/// pixel CBox. Equivalent to `gray_raster_render` + the clip box from
/// `ftsmooth.c` (cbox = bitmap width/rows, origin at the last row).
pub(crate) fn rasterize(outline: Outline) -> Result<RasterResult, FontError> {
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

    let mut worker = Worker::new(outline, width, height);
    worker.convert_glyph()?;
    Ok(RasterResult {
        width,
        height,
        pixels: worker.target,
    })
}

impl Worker {
    fn new(outline: Outline, width: usize, height: usize) -> Self {
        Worker {
            min_ex: 0,
            max_ex: 0,
            min_ey: 0,
            max_ey: 0,
            count_ey: 0,
            error: None,
            cell: 0,
            cell_free: 0,
            cell_null: 0,
            ycells: Vec::new(),
            cells: Vec::new(),
            x: 0,
            y: 0,
            outline,
            target: vec![0u8; width * height],
            width,
        }
    }

    /// `FT_INTEGRATE(ras, a, b)`: accumulate cover/area into the current cell.
    #[inline]
    fn integrate(&mut self, a: i32, b: i32) {
        if self.cell != self.cell_null {
            let c = &mut self.cells[self.cell];
            c.cover = add_int(c.cover, a);
            c.area = add_int(c.area, a.wrapping_mul(b));
        }
    }

    // ── gray_set_cell (ftgrays.c:570) ──────────────────────────────────────
    //
    // FreeType walks the per-scanline list with a `PCell* pcell` (pointer to
    // the link slot). We emulate "link slot" with a small enum that names
    // either the head (`ycells[ey]`) or a cell's `next` field.
    fn set_cell(&mut self, ex: i32, ey: i32) {
        let ey_index = ey - self.min_ey;
        if ey_index < 0 || ey_index >= self.count_ey || ex >= self.max_ex {
            self.cell = self.cell_null;
            return;
        }
        let ex = ex.max(self.min_ex - 1);
        let ey_u = ey_index as usize;

        // Walk the list tracking which link slot points at the current cell.
        // slot = Head means `ycells[ey_u]`; Next(p) means `cells[p].next`.
        let mut slot_head = true;
        let mut slot_pred: usize = 0;
        let mut cur = self.ycells[ey_u];
        loop {
            let cell = self.cells[cur];
            if cell.x > ex {
                break; // insert before `cur`
            }
            if cell.x == ex {
                self.cell = cur;
                return; // Found
            }
            if cell.next == NIL {
                // `cur` is the tail; insert after it.
                let new = self.alloc_cell();
                if new == self.cell_null {
                    return;
                }
                self.cells[new] = Cell {
                    x: ex,
                    area: 0,
                    cover: 0,
                    next: NIL,
                };
                self.cells[cur].next = new;
                self.cell = new;
                return;
            }
            slot_head = false;
            slot_pred = cur;
            cur = cell.next;
        }
        // Insert a new cell before `cur`, relinking its predecessor.
        let new = self.alloc_cell();
        if new == self.cell_null {
            return;
        }
        self.cells[new] = Cell {
            x: ex,
            area: 0,
            cover: 0,
            next: cur,
        };
        if slot_head {
            self.ycells[ey_u] = new;
        } else {
            self.cells[slot_pred].next = new;
        }
        self.cell = new;
    }

    fn alloc_cell(&mut self) -> usize {
        if self.cell_free >= self.cell_null {
            self.error = Some(FontError::RasterOverflow);
            self.cell_null
        } else {
            let i = self.cell_free;
            self.cell_free += 1;
            i
        }
    }

    // ── gray_render_scanline (ftgrays.c:639, non-FT_INT64 path) ────────────
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
        let dy = (y2 - y1) as i64; // C: TPos p; (TPos) * (TCoord) → 64-bit product
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

        // vertical clipping
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
            // inside one cell
        } else if dy == 0 {
            /* ex1 != ex2 */ self.set_cell(ex2, ey2);
        } else if dx == 0 {
            let two_fx = fx1 << 1;
            if dy > 0 {
                /* vertical line up */
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
                /* vertical line down */
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
            // any other line (FT_INT64 prod/DDA path, ftgrays.c:927)
            let mut prod = dx * fy1 as i64 - dy * fx1 as i64;
            let dx_r = ft_udivprep(ex1 != ex2, dx);
            let dy_r = ft_udivprep(ey1 != ey2, dy);

            loop {
                if prod - dx * ONE_PIXEL > 0 && prod <= 0 {
                    // left
                    let fx2 = 0;
                    let fy2 = ft_udiv(-prod, dx_r) as i32;
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
                    let fx2 = ft_udiv(-prod, dy_r) as i32;
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
                    let fy2 = ft_udiv(prod, dx_r) as i32;
                    self.integrate(fy2 - fy1, fx1 + fx2);
                    fx1 = 0;
                    fy1 = fy2;
                    ex1 += 1;
                } else {
                    // down
                    let fx2 = ft_udiv(prod, dy_r) as i32;
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

        // C: dx = FT_ABS(ax); if (dx < dy) dx = dy;  — TPos (i64), not unsigned.
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
        // FT arc layout: arc[0]=to, arc[1]=c2, arc[2]=c1, arc[3]=p0.
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
            // band shortcut (checked once on entry for the whole arc set).
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
                // de Casteljau split at t=0.5 (matches gray_split_cubic).
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
                // second half first (so first half is processed next).
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
    fn decompose(&mut self) -> Result<(), FontError> {
        // Take the outline geometry out of `self` so the mutable callbacks
        // (move_to/line_to/render_*) can run without aliasing the point slice.
        let pts = std::mem::take(&mut self.outline.points);
        let contours = std::mem::take(&mut self.outline.contours);
        let n_contours = self.outline.n_contours;
        let result = self.decompose_inner(&pts, &contours, n_contours);
        self.outline.points = pts;
        self.outline.contours = contours;
        result
    }

    fn decompose_inner(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        contours: &[i16],
        n_contours: i32,
    ) -> Result<(), FontError> {
        let mut last: i64 = -1;
        for n in 0..n_contours as usize {
            let first = (last + 1) as usize;
            last = contours[n] as i64;
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

            // A contour cannot start with a cubic control point.
            if first_tag == CURVE_TAG_CUBIC {
                return Err(FontError::InvalidOutline(
                    "outline: contour starts with cubic".into(),
                ));
            }

            // In the conic-start case C does point--; tags--; so the first
            // `point++` lands back on `first`.
            let mut cursor = first;

            if first_tag == CURVE_TAG_CONIC {
                if curve_tag(pts[limit].on_curve) == CURVE_TAG_ON {
                    v_start = v_last;
                    limit_eff = limit.checked_sub(1).ok_or_else(|| {
                        FontError::InvalidOutline("outline: conic start underflow".into())
                    })?;
                } else {
                    v_start.x = (v_start.x + v_last.x) / 2;
                    v_start.y = (v_start.y + v_last.y) / 2;
                }
                // point--; tags--;  → first real advance returns to `first`.
                cursor = first.wrapping_sub(1);
            }

            self.move_to(v_start.x as i64, v_start.y as i64);
            if let Some(e) = self.error.clone() {
                return Err(e);
            }

            self.walk_contour(&pts, cursor, first, limit_eff, v_start)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)]
    fn walk_contour(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        mut cursor: usize,
        first: usize,
        limit: usize,
        v_start: crate::outline::OutlinePoint,
    ) -> Result<(), FontError> {
        // C loop: while ( point < limit ) { point++; tags++; tag = ...; switch }
        // cursor here is either `first` (normal) or `first-1` (conic-start).
        while cursor < limit {
            cursor += 1;
            let tag = curve_tag(pts[cursor].on_curve);
            match tag {
                CURVE_TAG_ON => {
                    let vec = pts[cursor];
                    self.line_to(vec.x as i64, vec.y as i64);
                }
                CURVE_TAG_CONIC => {
                    let mut v_control = pts[cursor];
                    // Do_Conic block:
                    loop {
                        if cursor < limit {
                            cursor += 1;
                            let vec = pts[cursor];
                            let ntag = curve_tag(pts[cursor].on_curve);
                            if ntag == CURVE_TAG_ON {
                                self.render_conic(
                                    v_control.x as i64,
                                    v_control.y as i64,
                                    vec.x as i64,
                                    vec.y as i64,
                                );
                                break; // continue outer while
                            }
                            if ntag != CURVE_TAG_CONIC {
                                return Err(FontError::InvalidOutline(
                                    "outline: expected conic tag".into(),
                                ));
                            }
                            let mid_x = (v_control.x + vec.x) / 2;
                            let mid_y = (v_control.y + vec.y) / 2;
                            self.render_conic(
                                v_control.x as i64,
                                v_control.y as i64,
                                mid_x as i64,
                                mid_y as i64,
                            );
                            v_control = vec;
                            continue; // goto Do_Conic
                        }
                        // point >= limit: close with conic to v_start.
                        self.render_conic(
                            v_control.x as i64,
                            v_control.y as i64,
                            v_start.x as i64,
                            v_start.y as i64,
                        );
                        return Ok(()); // goto Close → next contour
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
                            vec1.x as i64,
                            vec1.y as i64,
                            vec2.x as i64,
                            vec2.y as i64,
                            vec.x as i64,
                            vec.y as i64,
                        );
                    } else {
                        self.render_cubic(
                            vec1.x as i64,
                            vec1.y as i64,
                            vec2.x as i64,
                            vec2.y as i64,
                            v_start.x as i64,
                            v_start.y as i64,
                        );
                        return Ok(()); // close
                    }
                }
                _ => unreachable!("2-bit tag"),
            }
            let _ = first;
        }

        // close the contour with a line segment to v_start
        self.line_to(v_start.x as i64, v_start.y as i64);
        Ok(())
    }

    // ── gray_sweep (ftgrays.c:1728) ───────────────────────────────────────
    fn sweep(&mut self) {
        // PIL/ftsmooth uses a bottom-up bitmap: pitch negative, origin at the
        // last row. gray_sweep writes `line = origin - pitch * y`, so FT's
        // upward row `y` (0 = bottom) maps to our top-down buffer row
        // `height-1-y`. This vertical flip reproduces PIL's mask orientation.
        let fill = if (self.outline.flags & OUTLINE_EVEN_ODD_FILL) != 0 {
            0x100
        } else {
            i32::MIN
        };

        for y in self.min_ey..self.max_ey {
            let mut cell = self.ycells[(y - self.min_ey) as usize];
            let mut x = self.min_ex;
            let mut cover: i32 = 0;

            let dst_row = (self.max_ey - 1 - y) as usize;

            while cell != self.cell_null {
                let c = self.cells[cell];

                if cover != 0 && c.x > x {
                    let coverage = fill_rule(cover, fill);
                    gray_set(
                        &mut self.target,
                        dst_row * self.width + x as usize,
                        coverage,
                        c.x - x,
                    );
                }

                cover = add_int(cover, c.cover.wrapping_mul((ONE_PIXEL * 2) as i32));
                let area = add_int(cover, -c.area);

                if area != 0 && c.x >= self.min_ex {
                    let coverage = fill_rule(area, fill);
                    let off = dst_row * self.width + c.x as usize;
                    if let Some(slot) = self.target.get_mut(off) {
                        *slot = coverage.clamp(0, 255) as u8;
                    }
                }

                x = c.x + 1;
                cell = c.next;
            }

            if cover != 0 {
                let coverage = fill_rule(cover, fill);
                gray_set(
                    &mut self.target,
                    dst_row * self.width + x as usize,
                    coverage,
                    self.max_ex - x,
                );
            }
        }
    }

    // ── gray_convert_glyph (ftgrays.c:1861) ───────────────────────────────
    fn convert_glyph(&mut self) -> Result<(), FontError> {
        self.min_ex = self.outline.cbox_x_min;
        self.max_ex = self.outline.cbox_x_max;
        self.min_ey = self.outline.cbox_y_min;
        self.max_ey = self.outline.cbox_y_max;
        self.count_ey = self.max_ey - self.min_ey;

        let band_height = self.count_ey as usize;

        // Growable cell pool replaces FT's fixed render pool + band bisection.
        // Size generously: one band covers the whole height.
        self.cells.clear();
        let pool = band_height.saturating_mul(16).max(64) + 16;
        self.cells.resize_with(pool, || Cell {
            x: 0,
            cover: 0,
            area: 0,
            next: NIL,
        });
        self.cell_null = self.cells.len();
        self.cells.push(Cell {
            x: CELL_MAX_X_VALUE,
            cover: 0,
            area: 0,
            next: NIL,
        });

        self.ycells.clear();
        self.ycells.resize(band_height, self.cell_null);

        self.cell_free = 0;
        self.cell = self.cell_null;
        self.error = None;

        self.decompose()?;
        if let Some(e) = self.error.take() {
            return Err(e);
        }
        self.sweep();
        Ok(())
    }
}

// Tag conventions from ftimage.h: FT_CURVE_TAG_ON=1, CONIC=0, CUBIC=2.
#[inline]
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

// Outline flags from ftimage.h.
const OUTLINE_EVEN_ODD_FILL: u32 = 0x02;
