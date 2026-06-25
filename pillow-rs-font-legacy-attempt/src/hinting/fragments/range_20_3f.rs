//! STACK / POINT opcodes 0x20–0x3F (FreeType dispatch).
//!
//! FreeType ttinterp.c dispatch:
//! 0x20 DUP        — duplicate top of stack
//! 0x21 POP        — pop top of stack
//! 0x22 CLEAR      — clear the stack
//! 0x23 SWAP       — swap top two stack elements
//! 0x24 DEPTH      — push stack depth
//! 0x25 CINDEX     — copy indexed element to top
//! 0x26 MINDEX     — move indexed element to top
//! 0x27 ALIGNPTS   — align two points
//! 0x28 RAW        — ??? (undocumented in some specs)
//! 0x29 UTP        — undefine touch point
//! 0x2A LOOPCALL   — loop and call function
//! 0x2B CALL       — call function
//! 0x2C FDEF       — function definition
//! 0x2D ENDF       — end function
//! 0x2E MDAP       — move direct absolute point (no rounding)
//! 0x2F MDAP       — move direct absolute point (with rounding)
//! 0x30 IUP[y]     — interpolate untouched points (y)
//! 0x31 IUP[x]     — interpolate untouched points (x)
//! 0x32 SHP[rp1]   — shift point by rp1
//! 0x33 SHP[rp2]   — shift point by rp2
//! 0x34 SHC[rp1]   — shift contour by rp1
//! 0x35 SHC[rp2]   — shift contour by rp2
//! 0x36 SHZ[rp1]   — shift twilight/zone by rp1
//! 0x37 SHZ[rp2]   — shift twilight/zone by rp2
//! 0x38 SHPIX      — shift point by pixel amount
//! 0x39 IP         — interpolate point
//! 0x3A MSIRP      — move stack indirect relative to point
//! 0x3B MSIRP[1]   — move stack indirect (alt flags?)
//! 0x3C ALIGNRP    — align to rp0
//! 0x3D RTDG       — round to double grid
//! 0x3E MIAP       — move indirect absolute point (rounded)
//! 0x3F MIAP       — move indirect absolute point (no rounding)

use super::super::exec::ExecContext;
use super::super::graphics::*;
use super::super::round;
use crate::error::FontError;
use crate::hinting::exec::dot_product;

// ---------------------------------------------------------------------------
// FreeType-compatible arithmetic helpers
// ---------------------------------------------------------------------------

/// FT_MulFix: multiply F26Dot6 by F2Dot14 with rounding, return F26Dot6.
#[inline]
fn mul_fix(a: i32, b: i32) -> i32 {
    let ab = (a as i64) * (b as i64);
    // Round half away from zero: add 0x2000 + (sign bit).
    let rounding = 0x2000i64.wrapping_add(ab >> 63);
    ((ab.wrapping_add(rounding)) >> 14) as i32
}

/// TT_MulFix14: same arithmetic as mul_fix (F26Dot6 * F2Dot14 with rounding).
/// FreeType uses this specifically for SHPIX.
#[inline]
fn mul_fix14(a: i32, b: i32) -> i32 {
    let ab = (a as i64) * (b as i64);
    let rounding = 0x2000i64.wrapping_add(ab >> 63);
    ((ab.wrapping_add(rounding)) >> 14) as i32
}

// ---------------------------------------------------------------------------
// Move_Zp2_Point — FreeType ttinterp.c line ~4974
// ---------------------------------------------------------------------------

/// Move a point in zp2 (FreeType's Move_Zp2_Point).
///
/// Only the components whose freedom-vector axis is non-zero are updated.
/// Touch flags are set per-axis.
fn move_zp2_point(ctx: &mut ExecContext, point: usize, dx: i32, dy: i32) {
    if ctx.gs.free_vector.x != 0 {
        ctx.zp2.points[point].x = ctx.zp2.points[point].x.wrapping_add(dx);
        ctx.zp2.tags[point] |= TOUCH_X;
    }
    if ctx.gs.free_vector.y != 0 {
        ctx.zp2.points[point].y = ctx.zp2.points[point].y.wrapping_add(dy);
        ctx.zp2.tags[point] |= TOUCH_Y;
    }
}

// ---------------------------------------------------------------------------
// Compute_Point_Displacement — FreeType ttinterp.c line ~4929
// ---------------------------------------------------------------------------

/// Compute the displacement (dx, dy, refp) for SHP / SHC / SHZ.
///
/// `use_rp1`:
/// - `true`  for opcodes with bit 0 = 1 (0x33, 0x35, 0x37) → zp0.rp1
/// - `false` for opcodes with bit 0 = 0 (0x32, 0x34, 0x36) → zp1.rp2
///
/// `target_cur` — the `.cur` slice of the zone being modified (only used to
/// determine whether the reference point lives in that zone, which tells the
/// caller to skip it). Pass `None` if the caller doesn't need to skip.
///
/// Returns `None` when the reference point is out of bounds (FreeType FAILURE).
fn compute_point_displacement(ctx: &ExecContext, use_rp1: bool) -> Option<(i32, i32, usize)> {
    let (zone, rp) = if use_rp1 {
        (&ctx.zp0, ctx.gs.rp1 as usize)
    } else {
        (&ctx.zp1, ctx.gs.rp2 as usize)
    };
    if rp >= zone.points.len() {
        return None;
    }

    // d = PROJECT( zone->cur[p], zone->org[p] )
    let d = dot_product(
        zone.points[rp].x - zone.org[rp].x,
        zone.points[rp].y - zone.org[rp].y,
        &ctx.gs.proj_vector,
    );

    // dx = MulFix(d, moveVector.x), dy = MulFix(d, moveVector.y)
    // Our free_vector is F26Dot6; dividing by 64 gives the F2Dot14 form.
    let mx = mul_fix(d, ctx.gs.free_vector.x >> 6);
    let my = mul_fix(d, ctx.gs.free_vector.y >> 6);

    Some((mx, my, rp))
}

impl ExecContext {
    pub(crate) fn handle_20_3f(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            // 0x20 DUP
            0x20 => {
                let v = self.pop();
                self.push(v);
                self.push(v);
                Ok(1)
            }
            // 0x21 POP
            0x21 => {
                self.pop();
                Ok(1)
            }
            // 0x22 CLEAR
            0x22 => {
                self.top = 0;
                Ok(1)
            }
            // 0x23 SWAP
            0x23 => {
                let a = self.pop();
                let b = self.pop();
                self.push(a);
                self.push(b);
                Ok(1)
            }
            // 0x24 DEPTH
            0x24 => {
                self.push(self.top);
                Ok(1)
            }
            // 0x25 CINDEX
            0x25 => {
                let idx = self.pop() as usize;
                if idx > 0 && idx <= self.top as usize {
                    let val = self.peek(idx - 1);
                    self.push(val);
                } else {
                    self.push(0);
                }
                Ok(1)
            }
            // 0x26 MINDEX
            0x26 => {
                let idx = self.pop() as usize;
                if idx > 0 && idx <= self.top as usize {
                    let pos = (self.top as usize - 1) - (idx - 1);
                    let val = self.stack[pos];
                    for j in pos..(self.top as usize - 1) {
                        self.stack[j] = self.stack[j + 1];
                    }
                    self.stack[self.top as usize - 1] = val;
                }
                Ok(1)
            }
            // 0x27 ALIGNPTS
            0x27 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                if p1 < self.zp1.points.len() && p2 < self.zp2.points.len() {
                    let dist = dot_product(
                        self.zp2.points[p2].x - self.zp1.points[p1].x,
                        self.zp2.points[p2].y - self.zp1.points[p1].y,
                        &self.gs.proj_vector,
                    );
                    let half = dist / 2;
                    let fv = self.gs.free_vector;
                    self.zp1.points[p1].x += (fv.x * half) >> 6;
                    self.zp1.points[p1].y += (fv.y * half) >> 6;
                    self.zp2.points[p2].x -= (fv.x * half) >> 6;
                    self.zp2.points[p2].y -= (fv.y * half) >> 6;
                }
                Ok(1)
            }
            // 0x28 RAW — retained for compatibility
            0x28 => {
                // RAW is deprecated/vendor-specific in most TrueType docs
                log::trace!("[hinting] RAW opcode ignored");
                Ok(1)
            }
            // 0x29 UTP — undefine touch point
            0x29 => {
                let p = self.pop() as usize;
                let zone = self.get_zone(0);
                if p < zone.tags.len() {
                    zone.tags[p] &= !(TOUCH_X | TOUCH_Y);
                }
                Ok(1)
            }
            // 0x2A LOOPCALL
            0x2A => {
                let count = self.pop();
                let fn_idx = self.pop() as usize;
                if fn_idx < self.fdefs.len() && self.fdefs[fn_idx].active && count > 0 {
                    if self.call_depth >= 32 {
                        log::warn!("[hinting] call depth limit exceeded");
                        Ok(1)
                    } else {
                        let def = self.fdefs[fn_idx].clone();
                        let start = def.start;
                        self.call_stack.push(super::super::exec::CallRecord {
                            caller_range: self.cur_range as i32,
                            caller_ip: self.ip + 1,
                            cur_count: count - 1,
                            def: super::super::exec::FnDef {
                                range: def.range,
                                start,
                                end: def.end,
                                opc: def.opc,
                                active: true,
                            },
                        });
                        self.call_depth += 1;
                        self.ip = start;
                        Ok(0)
                    }
                } else {
                    Ok(1)
                }
            }
            // 0x2B CALL
            0x2B => {
                let fn_idx = self.pop() as usize;
                if fn_idx < self.fdefs.len() && self.fdefs[fn_idx].active {
                    if self.call_depth >= 32 {
                        log::warn!("[hinting] call depth limit exceeded");
                        Ok(1)
                    } else {
                        let def = self.fdefs[fn_idx].clone();
                        let start = def.start;
                        self.call_stack.push(super::super::exec::CallRecord {
                            caller_range: self.cur_range as i32,
                            caller_ip: self.ip + 1,
                            cur_count: 0,
                            def: super::super::exec::FnDef {
                                range: def.range,
                                start,
                                end: def.end,
                                opc: def.opc,
                                active: true,
                            },
                        });
                        self.call_depth += 1;
                        self.ip = start;
                        Ok(0)
                    }
                } else {
                    Ok(1)
                }
            }
            // 0x2C FDEF
            0x2C => {
                let fn_idx = self.pop() as usize;
                let start = self.ip + 1;
                if fn_idx >= self.fdefs.len() {
                    self.fdefs
                        .resize(fn_idx + 16, super::super::exec::FnDef::default());
                }
                self.fdefs[fn_idx] = super::super::exec::FnDef {
                    range: self.cur_range as i32,
                    start,
                    end: 0,
                    opc: fn_idx as u32,
                    active: true,
                };
                // Skip to ENDF
                let mut depth = 1;
                let mut i = self.ip as usize + 1;
                while i < self.code.len() && depth > 0 {
                    if self.code[i] == 0x2C {
                        depth += 1;
                    } else if self.code[i] == 0x2D {
                        depth -= 1;
                    }
                    i += 1;
                }
                self.fdefs[fn_idx].end = (i - 1) as i32;
                self.ip = (i - 1) as i32;
                Ok(1)
            }
            // 0x2D ENDF
            0x2D => {
                if self.call_stack.is_empty() {
                    return Ok(1);
                }
                let do_loop = self.call_stack.last().map_or(false, |r| r.cur_count > 0);
                if do_loop {
                    let record = self.call_stack.last_mut().unwrap();
                    record.cur_count -= 1;
                    self.ip = record.def.start;
                } else {
                    if let Some(record) = self.call_stack.last() {
                        self.cur_range = match record.caller_range {
                            1 => super::super::exec::CodeRange::Font,
                            2 => super::super::exec::CodeRange::Cvt,
                            3 => super::super::exec::CodeRange::Glyph,
                            _ => super::super::exec::CodeRange::None,
                        };
                        self.ip = record.caller_ip;
                    }
                    self.call_stack.pop();
                    self.call_depth -= 1;
                }
                Ok(0)
            }
            // 0x2E MDAP
            0x2E => {
                let p = self.pop() as u16;
                self.gs.rp0 = p;
                self.gs.rp1 = p;
                let touch_x = self.gs.proj_vector.x != 0;
                let touch_y = self.gs.proj_vector.y != 0;
                let zone = self.get_zone(0);
                if (p as usize) < zone.points.len() {
                    if touch_x {
                        zone.tags[p as usize] |= TOUCH_X;
                    }
                    if touch_y {
                        zone.tags[p as usize] |= TOUCH_Y;
                    }
                }
                Ok(1)
            }
            // 0x2F MDAP[1] — Move Direct Absolute Point (with rounding)
            // Computes the point's projection from rp0[0], rounds it, then
            // moves the point to the rounded position along the freedom vector.
            // FreeType-equivalent: Ins_MDAP with (opcode & 1) branch.
            0x2F => {
                let p = self.pop() as usize;
                let rp_idx = self.gs.rp0 as usize;
                let (old_dist, needs_move) = {
                    let zone = self.get_zone(0);
                    if p < zone.points.len() && rp_idx < zone.points.len() {
                        let dx = zone.points[p].x - zone.points[rp_idx].x;
                        let dy = zone.points[p].y - zone.points[rp_idx].y;
                        (dot_product(dx, dy, &self.gs.proj_vector), true)
                    } else {
                        (0, false)
                    }
                };
                if needs_move {
                    let new_dist = self.round_distance(old_dist, (self.opcode & 3) as i32);
                    let delta = new_dist - old_dist;
                    let fv = self.gs.free_vector;
                    let zone = self.get_zone(0);
                    if delta != 0 {
                        Self::direct_move_with_vec(zone, &fv, p, delta);
                    } else {
                        if fv.x != 0 {
                            zone.tags[p] |= TOUCH_X;
                        }
                        if fv.y != 0 {
                            zone.tags[p] |= TOUCH_Y;
                        }
                    }
                }
                self.gs.rp0 = p as u16;
                self.gs.rp1 = p as u16;
                Ok(1)
            }
            // 0x30 IUP[y] — FreeType: direction 0 = Y
            0x30 => {
                crate::hinting::iup::iup_zone(&mut self.pts, 1);
                Ok(1)
            }
            // 0x31 IUP[x] — FreeType: direction 1 = X
            0x31 => {
                crate::hinting::iup::iup_zone(&mut self.pts, 0);
                Ok(1)
            }
            // ------------------------------------------------------------------
            // 0x32 SHP[rp2]  —  FreeType Ins_SHP, opcode bit 0 = 0 → zp1.rp2
            // 0x33 SHP[rp1]  —  FreeType Ins_SHP, opcode bit 0 = 1 → zp0.rp1
            // ------------------------------------------------------------------
            // FreeType Compute_Point_Displacement:
            //   opcode & 1  → zp = zp0, p = rp1
            //   !(opcode & 1) → zp = zp1, p = rp2
            //   d = PROJECT(zp->cur[p], zp->org[p])
            //   *x = MulFix(d, moveVector.x)
            //   *y = MulFix(d, moveVector.y)
            // Then for each point in the LOOP: Move_Zp2_Point(exc, point, dx, dy).
            0x32 | 0x33 => {
                let use_rp1 = (self.opcode & 1) != 0; // 0x33 → rp1, 0x32 → rp2
                let loop_count = self.gs.loop_count.max(1);

                // Check stack depth
                if self.top < loop_count {
                    if self.gs.instruct_control & 0x04 != 0 {
                        log::warn!("[hinting] SHP: too few arguments");
                    }
                    self.gs.loop_count = 1;
                    return Ok(1);
                }

                self.top -= loop_count;

                let Some((dx, dy, _refp)) = compute_point_displacement(self, use_rp1) else {
                    self.gs.loop_count = 1;
                    return Ok(1);
                };

                // Pop points in reverse (FreeType: --args, then *(args - 1))
                for i in 0..loop_count {
                    let point = self.stack[(self.top + i) as usize] as usize;
                    if point < self.zp2.points.len() {
                        move_zp2_point(self, point, dx, dy);
                    } else if self.gs.instruct_control & 0x04 != 0 {
                        log::warn!("[hinting] SHP: point {} out of bounds", point);
                    }
                }
                self.gs.loop_count = 1;
                Ok(1)
            }
            // ------------------------------------------------------------------
            // 0x34 SHC[rp2]  — FreeType Ins_SHC, bit 0 = 0 → zp1.rp2
            // 0x35 SHC[rp1]  — FreeType Ins_SHC, bit 0 = 1 → zp0.rp1
            // ------------------------------------------------------------------
            // Pops a contour index from the stack, shifts all points in that
            // contour (using zp2.contours for bounds). Skips the reference point
            // if it falls inside the contour. The contour bounds are determined
            // from zp2 (not pts).
            0x34 | 0x35 => {
                let use_rp1 = (self.opcode & 1) != 0;
                let contour = self.pop() as usize;

                // Bounds check on contour.
                // FreeType: contour_limit = (gep2 == 0) ? 1 : zp2.n_contours
                // (zone 0 = twilight, zone 1+ = glyph with contours)
                let contour_limit = if self.gs.gep2 == 0 {
                    1
                } else {
                    self.zp2.n_contours as usize
                };
                if contour >= contour_limit {
                    if self.gs.instruct_control & 0x04 != 0 {
                        log::warn!("[hinting] SHC: contour {} out of bounds", contour);
                    }
                    return Ok(1);
                }

                let Some((dx, dy, refp)) = compute_point_displacement(self, use_rp1) else {
                    return Ok(1);
                };

                // Determine contour range from zp2.contours (FreeType uses
                // exc->zp2.contours, adjusting for first_point).
                if self.zp2.contours.is_empty() {
                    return Ok(1);
                }
                let start: usize = if contour == 0 {
                    0
                } else {
                    self.zp2.contours[contour - 1] as usize + 1
                };
                let end: usize = if self.gs.gep2 == 0 {
                    // Twilight zone: no real contours — use n_points as limit
                    self.zp2.n_points as usize
                } else {
                    self.zp2.contours[contour] as usize + 1
                };

                for p in start..end {
                    if p < self.zp2.points.len() && p != refp {
                        move_zp2_point(self, p, dx, dy);
                    }
                }
                Ok(1)
            }
            // ------------------------------------------------------------------
            // 0x36 SHZ[rp2] — FreeType Ins_SHZ, bit 0 = 0 → zp1.rp2
            // 0x37 SHZ[rp1] — FreeType Ins_SHZ, bit 0 = 1 → zp0.rp1
            // ------------------------------------------------------------------
            // Pops a zone selector (0=twilight, 1=pts sans phantom points).
            // Skips the reference point if it falls within the zone.
            // NOTE: FreeType's SHZ does NOT move phantom points (last 4 of pts).
            0x36 | 0x37 => {
                let use_rp1 = (self.opcode & 1) != 0;
                let z = self.pop() as usize;

                let Some((dx, dy, refp)) = compute_point_displacement(self, use_rp1) else {
                    return Ok(1);
                };

                // Select the target zone: 0=twilight, 1=pts (sans phantom pts)
                let (limit, points, _org) = match z {
                    0 => {
                        // Twilight zone
                        let n = self.twilight.n_points as usize;
                        (n, &mut self.twilight.points, &mut self.twilight.org)
                    }
                    1 => {
                        // Pts zone: skip last 4 phantom points
                        let n = self.pts.n_points.max(4) as usize - 4;
                        (n, &mut self.pts.points, &mut self.pts.org)
                    }
                    _ => {
                        if self.gs.instruct_control & 0x04 != 0 {
                            log::warn!("[hinting] SHZ: invalid zone {}", z);
                        }
                        return Ok(1);
                    }
                };

                if dx != 0 {
                    for i in 0..limit {
                        if i != refp {
                            points[i].x = points[i].x.wrapping_add(dx);
                        }
                    }
                }
                if dy != 0 {
                    for i in 0..limit {
                        if i != refp {
                            points[i].y = points[i].y.wrapping_add(dy);
                        }
                    }
                }
                Ok(1)
            }
            // ------------------------------------------------------------------
            // 0x38 SHPIX — FreeType Ins_SHPIX
            // ------------------------------------------------------------------
            // Pops a pixel amount (F26Dot6) then LOOP points from the stack.
            // dx = MulFix14(amount, freeVector.x)
            // dy = MulFix14(amount, freeVector.y)
            // For each point: Move_Zp2_Point(exc, point, dx, dy)
            0x38 => {
                let amount = self.pop(); // F26Dot6 pixel amount
                let loop_count = self.gs.loop_count.max(1);

                if self.top < loop_count {
                    if self.gs.instruct_control & 0x04 != 0 {
                        log::warn!("[hinting] SHPIX: too few arguments");
                    }
                    self.gs.loop_count = 1;
                    return Ok(1);
                }

                self.top -= loop_count;

                // FreeType: dx = TT_MulFix14(args[0], GS.freeVector.x)
                let dx = mul_fix14(amount, self.gs.free_vector.x >> 6);
                let dy = mul_fix14(amount, self.gs.free_vector.y >> 6);

                for i in 0..loop_count {
                    let point = self.stack[(self.top + i) as usize] as usize;
                    if point < self.zp2.points.len() {
                        move_zp2_point(self, point, dx, dy);
                    } else if self.gs.instruct_control & 0x04 != 0 {
                        log::warn!("[hinting] SHPIX: point {} out of bounds", point);
                    }
                }
                self.gs.loop_count = 1;
                Ok(1)
            }
            // ------------------------------------------------------------------
            // 0x39 IP — FreeType Ins_IP
            // ------------------------------------------------------------------
            // Interpolation between rp1 and rp2. Each point is moved so its
            // relative position between rp1 and rp2 is preserved.
            //
            // Key FreeType semantics:
            //   - Original distances use DUALPROJ (dual_vector) on the original
            //     positions (org for twilight, points/org for the projected distance)
            //   - Current distances use PROJECT (proj_vector)
            //   - Handles the twilight zone specially (use org, not orus)
            //   - Supports LOOP variable
            0x39 => {
                let loop_count = self.gs.loop_count.max(1);

                if self.top < loop_count {
                    if self.gs.instruct_control & 0x04 != 0 {
                        log::warn!("[hinting] IP: too few arguments");
                    }
                    self.gs.loop_count = 1;
                    return Ok(1);
                }

                self.top -= loop_count;

                // Bounds check rp1 (in zp0)
                let rp1 = self.gs.rp1 as usize;
                let rp2 = self.gs.rp2 as usize;
                if rp1 >= self.zp0.points.len() {
                    self.gs.loop_count = 1;
                    return Ok(1);
                }

                // Determine if we're in the twilight zone (any zone pointer at zone 0).
                // FreeType: zone 0 = twilight, zone 1+ = glyph.
                let twilight = self.gs.gep0 == 0 || self.gs.gep1 == 0 || self.gs.gep2 == 0;

                // old_range  = DUALPROJ( rp2_orig, rp1_orig )  — original distance
                // cur_range  = PROJECT(  rp2_cur,  rp1_cur  )  — current distance
                let old_range = if twilight || rp2 >= self.zp1.points.len() {
                    0i64
                } else {
                    dot_product(
                        self.zp1.org[rp2].x - self.zp0.org[rp1].x,
                        self.zp1.org[rp2].y - self.zp0.org[rp1].y,
                        &self.gs.dual_vector,
                    ) as i64
                };

                let cur_range = if rp2 >= self.zp1.points.len() {
                    0i64
                } else {
                    dot_product(
                        self.zp1.points[rp2].x - self.zp0.points[rp1].x,
                        self.zp1.points[rp2].y - self.zp0.points[rp1].y,
                        &self.gs.proj_vector,
                    ) as i64
                };

                for i in 0..loop_count {
                    let point = self.stack[(self.top + i) as usize] as usize;
                    if point >= self.zp2.points.len() {
                        if self.gs.instruct_control & 0x04 != 0 {
                            log::warn!("[hinting] IP: point {} out of bounds", point);
                        }
                        continue;
                    }

                    // org_dist = DUALPROJ( point_org, rp1_org )
                    let org_dist = dot_product(
                        self.zp2.org[point].x - self.zp0.org[rp1].x,
                        self.zp2.org[point].y - self.zp0.org[rp1].y,
                        &self.gs.dual_vector,
                    ) as i64;

                    // cur_dist = PROJECT( point_cur, rp1_cur )
                    let cur_dist = dot_product(
                        self.zp2.points[point].x - self.zp0.points[rp1].x,
                        self.zp2.points[point].y - self.zp0.points[rp1].y,
                        &self.gs.proj_vector,
                    ) as i64;

                    // Compute new_dist (FreeType FT_MulDiv with rounding)
                    let new_dist = if org_dist != 0 {
                        if old_range != 0 {
                            // new_dist = (org_dist * cur_range) / old_range  (with rounding)
                            let numer = org_dist * cur_range;
                            let (q, r) = (numer / old_range, numer % old_range);
                            if r.abs() * 2 >= old_range.abs() {
                                q + if numer.signum() == old_range.signum() {
                                    1
                                } else {
                                    -1
                                }
                            } else {
                                q
                            }
                        } else {
                            // When old_range == 0: new_dist = org_dist (MS behavior)
                            org_dist
                        }
                    } else {
                        0i64
                    };

                    // delta = new_dist - cur_dist  (the amount to move along the freedom vector)
                    let delta = (new_dist - cur_dist) as i32;
                    if delta != 0 {
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * delta) >> 6;
                        let fy = (fv.y * delta) >> 6;
                        self.zp2.points[point].x = self.zp2.points[point].x.wrapping_add(fx);
                        self.zp2.points[point].y = self.zp2.points[point].y.wrapping_add(fy);
                        if fv.x != 0 {
                            self.zp2.tags[point] |= TOUCH_X;
                        }
                        if fv.y != 0 {
                            self.zp2.tags[point] |= TOUCH_Y;
                        }
                    }
                }
                self.gs.loop_count = 1;
                Ok(1)
            }
            // 0x3A MSIRP
            0x3A => {
                let dist = self.pop();
                let p_idx = self.pop() as usize;
                let rp_idx = self.gs.rp0 as usize;
                if p_idx < self.zp2.points.len() && rp_idx < self.zp0.points.len() {
                    let rp = self.zp0.points[rp_idx];
                    let p = &mut self.zp2.points[p_idx];
                    let cur_dist = dot_product(p.x - rp.x, p.y - rp.y, &self.gs.proj_vector);
                    let diff = dist - cur_dist;
                    let fv = self.gs.free_vector;
                    p.x += (fv.x * diff) >> 6;
                    p.y += (fv.y * diff) >> 6;
                }
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            // 0x3B MSIRP[1] — same as MSIRP but without rp1 update?
            0x3B => {
                let dist = self.pop();
                let p_idx = self.pop() as usize;
                let rp_idx = self.gs.rp0 as usize;
                if p_idx < self.zp2.points.len() && rp_idx < self.zp0.points.len() {
                    let rp = self.zp0.points[rp_idx];
                    let p = &mut self.zp2.points[p_idx];
                    let cur_dist = dot_product(p.x - rp.x, p.y - rp.y, &self.gs.proj_vector);
                    let diff = dist - cur_dist;
                    let fv = self.gs.free_vector;
                    p.x += (fv.x * diff) >> 6;
                    p.y += (fv.y * diff) >> 6;
                }
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            // 0x3C ALIGNRP
            0x3C => {
                let p_idx = self.pop() as usize;
                let rp_idx = self.gs.rp0 as usize;
                if p_idx < self.zp2.points.len() && rp_idx < self.zp0.points.len() {
                    let rp = self.zp0.points[rp_idx];
                    let dx = rp.x - self.zp2.points[p_idx].x;
                    let dy = rp.y - self.zp2.points[p_idx].y;
                    let dist = dot_product(dx, dy, &self.gs.proj_vector);
                    let fv = self.gs.free_vector;
                    self.zp2.points[p_idx].x += (fv.x * dist) >> 6;
                    self.zp2.points[p_idx].y += (fv.y * dist) >> 6;
                    self.zp2.tags[p_idx] |= TOUCH_X | TOUCH_Y;
                }
                Ok(1)
            }
            // 0x3D RTDG
            0x3D => {
                self.gs.round_state = 2;
                self.round_fn = round::round_to_double_grid;
                Ok(1)
            }
            // 0x3E MIAP — move indirect absolute point (rounded)
            // TrueType stack: push cvt_entry, push point_number
            // MIAP pops: point_number (top), then cvt_entry
            0x3E => {
                if self.top < 2 { return Ok(1); }
                let p_idx = self.pop() as usize;       // top = point_number
                let cvt_idx = self.pop() as usize;     // next = cvt_entry
                let cvt_val = if cvt_idx < self.glyf_cvt.len() {
                    self.glyf_cvt[cvt_idx]
                } else {
                    0
                };
                if p_idx < self.zp0.points.len() {
                    let p = self.zp0.points[p_idx];
                    let cur_proj = dot_product(p.x, p.y, &self.gs.proj_vector);
                    let diff = cvt_val - cur_proj;
                    let fv = self.gs.free_vector;
                    self.zp0.points[p_idx].x += (fv.x * diff) >> 6;
                    self.zp0.points[p_idx].y += (fv.y * diff) >> 6;
                    if fv.x != 0 {
                        self.zp0.tags[p_idx] |= TOUCH_X;
                    }
                    if fv.y != 0 {
                        self.zp0.tags[p_idx] |= TOUCH_Y;
                    }
                }
                self.gs.rp2 = self.gs.rp1;
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            // 0x3F MIAP (no rounding)
            0x3F => {
                let cvt_idx = self.pop() as usize;
                let p_idx = self.pop() as usize;
                let cvt_val = if cvt_idx < self.glyf_cvt.len() {
                    self.glyf_cvt[cvt_idx]
                } else {
                    0
                };
                if p_idx < self.zp0.points.len() {
                    let p = self.zp0.points[p_idx];
                    let cur_proj = dot_product(p.x, p.y, &self.gs.proj_vector);
                    let diff = cvt_val - cur_proj;
                    let fv = self.gs.free_vector;
                    self.zp0.points[p_idx].x += (fv.x * diff) >> 6;
                    self.zp0.points[p_idx].y += (fv.y * diff) >> 6;
                    if fv.x != 0 {
                        self.zp0.tags[p_idx] |= TOUCH_X;
                    }
                    if fv.y != 0 {
                        self.zp0.tags[p_idx] |= TOUCH_Y;
                    }
                }
                self.gs.rp2 = self.gs.rp1;
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            _ => {
                log::trace!("[hinting] unimpl 0x{:02X} in range 20-3F", self.opcode);
                Ok(1)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ctx() -> ExecContext {
        ExecContext::new_test()
    }

    #[test]
    fn test_dup() {
        let mut ctx = make_ctx();
        ctx.push(42);
        ctx.opcode = 0x20;
        ctx.handle_20_3f().unwrap();
        assert_eq!(ctx.pop(), 42);
        assert_eq!(ctx.pop(), 42);
    }

    #[test]
    fn test_clear() {
        let mut ctx = make_ctx();
        ctx.push(1);
        ctx.push(2);
        ctx.push(3);
        ctx.opcode = 0x22;
        ctx.handle_20_3f().unwrap();
        assert_eq!(ctx.top, 0);
    }

    #[test]
    fn test_depth() {
        let mut ctx = make_ctx();
        ctx.push(10);
        ctx.push(20);
        ctx.opcode = 0x24;
        ctx.handle_20_3f().unwrap();
        assert_eq!(ctx.pop(), 2); // depth (on top after DEPTH)
        assert_eq!(ctx.pop(), 20);
        assert_eq!(ctx.pop(), 10);
    }
}
