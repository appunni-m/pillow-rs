//! TrueType bytecode VM.

#![allow(missing_docs)]

use crate::error::FontError;
use crate::tables::FontData;

use super::graphics::*;
use super::opcodes;
use super::round;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CodeRange {
    None,
    Font,
    Cvt,
    Glyph,
}

#[derive(Clone, Debug, Default)]
pub struct FnDef {
    pub range: i32,
    pub start: i32,
    pub end: i32,
    pub opc: u32,
    pub active: bool,
}

#[derive(Clone, Debug)]
pub struct CallRecord {
    pub caller_range: i32,
    pub caller_ip: i32,
    pub cur_count: i32,
    pub def: FnDef,
}

#[derive(Clone, Debug)]
pub struct ExecContext {
    pub gs: GraphicsState,
    pub zp0: Zone,
    pub zp1: Zone,
    pub zp2: Zone,
    /// Tracks which canonical zone each zp pointer refers to:
    /// 0 = twilight, 1 = glyph (pts), -1 = uninitialized.
    pub zp_zone: [i32; 3],
    pub pts: Zone,
    pub twilight: Zone,
    pub code: Vec<u8>,
    pub ip: i32,
    pub opcode: u8,
    pub cur_range: CodeRange,
    pub stack: Vec<i32>,
    pub top: i32,
    pub cvt: Vec<i32>,
    pub storage: Vec<i32>,
    pub glyf_cvt: Vec<i32>,
    pub glyf_storage: Vec<i32>,
    pub fdefs: Vec<FnDef>,
    pub idefs: Vec<FnDef>,
    pub call_stack: Vec<CallRecord>,
    pub call_depth: usize,
    pub point_size: i32,
    pub ppem: u16,
    pub scale: i32,
    pub period: i32,
    pub phase: i32,
    pub threshold: i32,
    pub round_fn: round::RoundFn,
    pub compensation: i32,
    pub grayscale: bool,
}

impl ExecContext {
    pub(crate) fn new(data: &FontData) -> Self {
        let ppem = data.size_pt.ceil() as u16;
        let point_size = (ppem as i32) << 6;

        ExecContext {
            gs: GraphicsState::default(),
            zp0: Zone::new(),
            zp1: Zone::new(),
            zp2: Zone::new(),
            zp_zone: [-1, -1, -1],
            pts: Zone::new(),
            twilight: Zone::new(),
            code: Vec::new(),
            ip: 0,
            opcode: 0,
            cur_range: CodeRange::None,
            stack: vec![0i32; 512],
            top: 0,
            cvt: data.cvt.clone(),
            storage: vec![0i32; 64],
            glyf_cvt: data.cvt.clone(),
            glyf_storage: vec![0i32; 64],
            fdefs: Vec::new(),
            idefs: Vec::new(),
            call_stack: Vec::new(),
            call_depth: 0,
            point_size,
            ppem,
            scale: (ppem as i32) << 6,
            period: 0,
            phase: 0,
            threshold: 0,
            round_fn: round::round_to_grid,
            compensation: 0,
            grayscale: true,
        }
    }

    /// Create a minimal ExecContext for testing (no real font data).
    #[cfg(test)]
    pub(crate) fn new_test() -> Self {
        ExecContext {
            gs: GraphicsState::default(),
            zp0: Zone::new(),
            zp1: Zone::new(),
            zp2: Zone::new(),
            zp_zone: [-1, -1, -1],
            pts: Zone::new(),
            twilight: Zone::new(),
            code: Vec::new(),
            ip: 0,
            opcode: 0,
            cur_range: CodeRange::None,
            stack: vec![0i32; 512],
            top: 0,
            cvt: Vec::new(),
            storage: vec![0i32; 64],
            glyf_cvt: Vec::new(),
            glyf_storage: vec![0i32; 64],
            fdefs: Vec::new(),
            idefs: Vec::new(),
            call_stack: Vec::new(),
            call_depth: 0,
            point_size: 10 << 6,
            ppem: 10,
            scale: 10 << 6,
            period: 0,
            phase: 0,
            threshold: 0,
            round_fn: round::round_to_grid,
            compensation: 0,
            grayscale: true,
        }
    }

    pub fn run(&mut self) -> Result<(), FontError> {
        self.ip = 0;
        self.top = 0; // Reset stack for new program execution
                      // Safety limit to prevent infinite loops from buggy or malicious bytecode
        let max_ops: i32 = 20000;
        let mut ops = 0i32;
        while self.ip < self.code.len() as i32 && ops < max_ops {
            self.opcode = self.code[self.ip as usize];
            let length = self.execute_opcode()?;
            self.ip += length;
            ops += 1;
        }
        if ops >= max_ops {
            log::warn!("[hinting] execution limit reached ({} ops)", max_ops);
        }
        Ok(())
    }

    #[inline]
    pub fn push(&mut self, val: i32) {
        let pos = self.top as usize;
        if pos < self.stack.len() {
            self.stack[pos] = val;
            self.top += 1;
        }
    }

    #[inline]
    pub fn pop(&mut self) -> i32 {
        if self.top > 0 {
            self.top -= 1;
            self.stack[self.top as usize]
        } else {
            0
        }
    }

    #[inline]
    pub fn peek(&self, depth: usize) -> i32 {
        let pos = self.top as usize - 1 - depth;
        if pos < self.stack.len() {
            self.stack[pos]
        } else {
            0
        }
    }

    pub(crate) fn read_bytes(&self, count: usize) -> Vec<i32> {
        let start = (self.ip + 1) as usize;
        let end = (start + count).min(self.code.len());
        self.code[start..end].iter().map(|&b| b as i32).collect()
    }

    pub(crate) fn read_words(&self, count: usize) -> Vec<i32> {
        let start = (self.ip + 1) as usize;
        let mut vals = Vec::with_capacity(count);
        for i in 0..count {
            let off = start + i * 2;
            if off + 1 < self.code.len() {
                let w = i16::from_be_bytes([self.code[off], self.code[off + 1]]) as i32;
                vals.push(w);
            }
        }
        vals
    }

    fn execute_opcode(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            0x00..=0x0F => self.handle_00_0f(),
            0x10..=0x1F => self.handle_10_1f(),
            0x20..=0x3F => self.handle_20_3f(),
            0x40..=0x5F => self.handle_40_5f(),
            0x60..=0x7F => self.handle_60_7f(),
            // Apple convention: 0x80-0xBF = FLIPPT/ROLL/PUSHB/PUSHW etc.
            0x80..=0xBF => self.handle_80_bf(),
            // MDRP — Move Direct Relative Point (0xC0-0xDF).
            0xC0..=0xDF => {
                let flags = opcodes::decode_mirp_flags(self.opcode);
                self.do_mdrp(flags)
            }
            // MIRP — Move Indirect Relative Point (0xE0-0xFF).
            0xE0..=0xFF => {
                let flags = opcodes::decode_mirp_flags(self.opcode);
                self.do_mirp(flags)
            }
        }
    }

    /// MDRP — Move Direct Relative Point (0xC0-0xDF).
    ///
    /// FreeType-equivalent: Ins_MDRP
    ///
    ///  1. Compute original distance using DUALPROJ on original coordinates
    ///  2. Apply single-width cut-in
    ///  3. Round with compensation (or Round_None)
    ///  4. Apply minimum-distance clamping
    ///  5. Compute current projection distance with PROJECT
    ///  6. Move point by `rounded - cur_dist` along the freedom vector
    ///  7. Update reference points
    fn do_mdrp(&mut self, flags: opcodes::MirpFlags) -> Result<i32, FontError> {
        let p_idx = self.pop() as usize;
        let rp_idx = self.gs.rp0 as usize;

        // Validate bounds (zp1 for point, zp0 for rp0 in FreeType)
        if p_idx >= self.zp2.n_points as usize || rp_idx >= self.zp0.n_points as usize {
            self.gs.rp1 = self.gs.rp0;
            self.gs.rp2 = p_idx as u16;
            if flags.set_rp0 {
                self.gs.rp0 = p_idx as u16;
            }
            return Ok(1);
        }

        // Step 1: Original distance using DUALPROJ on ORIGINAL coordinates
        let dx = self.zp2.org[p_idx].x - self.zp0.org[rp_idx].x;
        let dy = self.zp2.org[p_idx].y - self.zp0.org[rp_idx].y;
        let mut org_dist = dot_product(dx, dy, &self.gs.dual_vector);

        // Step 2: Single-width cut-in test
        // |org_dist - single_width_value| < single_width_cut_in
        let swv = self.gs.single_width_value;
        let swc = self.gs.single_width_cut_in;
        if swc > 0 && org_dist < swv + swc && org_dist > swv - swc {
            if org_dist >= 0 {
                org_dist = swv;
            } else {
                org_dist = -swv;
            }
        }

        // Step 3: Round with compensation
        let comp = self.gs.compensation[flags.compensation];
        let distance = if flags.round {
            self.round_abs_ft(org_dist, comp)
        } else {
            // Round_None: just add compensation
            if org_dist >= 0 {
                let v = org_dist + comp;
                if v < 0 {
                    0
                } else {
                    v
                }
            } else {
                let v = org_dist - comp;
                if v > 0 {
                    0
                } else {
                    v
                }
            }
        };

        // Step 4: Minimum-distance clamping (AFTER rounding — matches FreeType)
        let distance = if flags.minimum_distance {
            let min_dist = self.gs.minimum_distance;
            if org_dist >= 0 {
                if distance < min_dist {
                    min_dist
                } else {
                    distance
                }
            } else {
                if distance > -min_dist {
                    -min_dist
                } else {
                    distance
                }
            }
        } else {
            distance
        };

        // Step 5: Current projection distance (PROJECT on cur coordinates)
        let cur_dx = self.zp2.points[p_idx].x - self.zp0.points[rp_idx].x;
        let cur_dy = self.zp2.points[p_idx].y - self.zp0.points[rp_idx].y;
        let cur_dist = dot_product(cur_dx, cur_dy, &self.gs.proj_vector);

        // Step 6: Move point by (distance - cur_dist) along freedom vector
        let move_delta = distance - cur_dist;
        if move_delta != 0 {
            Self::move_along_free(&mut self.zp2, p_idx, move_delta, self.gs.free_vector);
        }

        // Step 7: Update reference points
        self.gs.rp1 = self.gs.rp0;
        self.gs.rp2 = p_idx as u16;
        if flags.set_rp0 {
            self.gs.rp0 = p_idx as u16;
        }

        Ok(1)
    }

    /// MIRP — Move Indirect Relative Point (0xE0-0xFF).
    ///
    /// TrueType spec: pops point_number, cvt_entry_number.
    /// cvt_entry_number = 0 → value 0, otherwise CVT[cvt_entry_number - 1].
    ///
    /// FreeType-equivalent: Ins_MIRP
    ///
    ///  1. Read CVT value (entry = stack_value + 1)
    ///  2. Single-width cut-in on CVT value
    ///  3. Twilight-zone special case: set org/cur point = rp0 + cvt_dist*freeVector/16384
    ///  4. Compute org_dist from ORIGINAL coordinates via DUALPROJ
    ///  5. Compute cur_dist from CURRENT coordinates via PROJECT
    ///  6. Auto-flip
    ///  7. Control-value cut-in (same-zone check)
    ///  8. Round with compensation
    ///  9. Minimum-distance clamping (against org_dist sign)
    /// 10. Move by (distance - cur_dist) along freedom vector
    /// 11. Update reference points
    fn do_mirp(&mut self, flags: opcodes::MirpFlags) -> Result<i32, FontError> {
        // TrueType stack: push cvt_entry, push point_number
        // MIRP pops: point_number (top), then cvt_entry
        if self.top < 2 { return Ok(1); }
        let p_idx = self.pop() as usize;     // top of stack = point_number
        let cvt_idx = self.pop() as usize;   // second = cvt_entry
        let rp_idx = self.gs.rp0 as usize;

        // Validate bounds — skip if OOB (prevents crash on capacity overflow)
        if p_idx >= self.zp2.n_points as usize
            || p_idx >= self.zp2.points.len()
            || rp_idx >= self.zp0.n_points as usize
            || rp_idx >= self.zp0.points.len()
            || rp_idx >= self.zp0.org.len()
        {
            self.gs.rp1 = self.gs.rp0;
            self.gs.rp2 = p_idx as u16;
            if flags.set_rp0 {
                self.gs.rp0 = p_idx as u16;
            }
            return Ok(1);
        }

        // Step 1: CVT value — FreeType: cvtEntry = arg, CVT[cvtEntry-1], cvtEntry=0 → 0
        let mut cvt_dist = if cvt_idx > 0 && cvt_idx - 1 < self.glyf_cvt.len() {
            self.glyf_cvt[cvt_idx - 1]
        } else {
            0
        };

        // Step 2: Single-width cut-in on CVT value
        let delta = if cvt_dist >= self.gs.single_width_value {
            cvt_dist - self.gs.single_width_value
        } else {
            self.gs.single_width_value - cvt_dist
        };
        if delta < self.gs.single_width_cut_in {
            if cvt_dist >= 0 {
                cvt_dist = self.gs.single_width_value;
            } else {
                cvt_dist = -self.gs.single_width_value;
            }
        }

        // Step 3: Twilight-zone special case
        // When zp1 (gep1) is twilight, set org/cur = rp0.org + (cvt_dist * fv / 16384)
        // Bounds check: ensure zp2 has room for p_idx
        if p_idx >= self.zp2.points.len() || p_idx >= self.zp2.org.len() {
            self.gs.rp1 = self.gs.rp0;
            self.gs.rp2 = p_idx as u16;
            if flags.set_rp0 {
                self.gs.rp0 = p_idx as u16;
            }
            return Ok(1);
        }
        if self.gs.gep1 == 0 {
            let fv = self.gs.free_vector;
            let cx = self.zp0.org[rp_idx].x + ((cvt_dist * fv.x + 8192) >> 14);
            let cy = self.zp0.org[rp_idx].y + ((cvt_dist * fv.y + 8192) >> 14);
            self.zp2.org[p_idx].x = cx;
            self.zp2.org[p_idx].y = cy;
            self.zp2.points[p_idx].x = cx;
            self.zp2.points[p_idx].y = cy;
        }

        // Step 4: Original distance (DUALPROJ on ORIGINAL coords)
        let o_dx = self.zp2.org[p_idx].x - self.zp0.org[rp_idx].x;
        let o_dy = self.zp2.org[p_idx].y - self.zp0.org[rp_idx].y;
        let org_dist = dot_product(o_dx, o_dy, &self.gs.dual_vector);

        // Step 5: Current projection distance (PROJECT on CUR coords)
        let c_dx = self.zp2.points[p_idx].x - self.zp0.points[rp_idx].x;
        let c_dy = self.zp2.points[p_idx].y - self.zp0.points[rp_idx].y;
        let cur_dist = dot_product(c_dx, c_dy, &self.gs.proj_vector);

        // Step 6: Auto-flip test
        let mut distance_target = cvt_dist;
        if self.gs.auto_flip && (org_dist ^ distance_target) < 0 {
            distance_target = -distance_target;
        }

        // Step 7: Control-value cut-in (only when both zones are the same)
        if flags.round && self.gs.gep0 == self.gs.gep1 {
            let cv_delta = if distance_target >= org_dist {
                distance_target - org_dist
            } else {
                org_dist - distance_target
            };
            if cv_delta > self.gs.control_value_cut_in {
                distance_target = org_dist;
            }
        }

        // Step 8: Round with compensation
        let comp = self.gs.compensation[flags.compensation];
        let distance = if flags.round {
            self.round_abs_ft(distance_target, comp)
        } else {
            // Round_None: just add compensation
            if distance_target >= 0 {
                let v = distance_target + comp;
                if v < 0 {
                    0
                } else {
                    v
                }
            } else {
                let v = distance_target - comp;
                if v > 0 {
                    0
                } else {
                    v
                }
            }
        };

        // Step 9: Minimum-distance clamping (against org_dist sign)
        let distance = if flags.minimum_distance {
            let min_dist = self.gs.minimum_distance;
            if org_dist >= 0 {
                if distance < min_dist {
                    min_dist
                } else {
                    distance
                }
            } else {
                if distance > -min_dist {
                    -min_dist
                } else {
                    distance
                }
            }
        } else {
            distance
        };

        // Step 10: Move point by (distance - cur_dist) along freedom vector
        let move_delta = distance - cur_dist;
        if move_delta != 0 {
            Self::move_along_free(&mut self.zp2, p_idx, move_delta, self.gs.free_vector);
        }

        // Step 11: Update reference points
        self.gs.rp1 = self.gs.rp0;
        self.gs.rp2 = p_idx as u16;
        if flags.set_rp0 {
            self.gs.rp0 = p_idx as u16;
        }

        Ok(1)
    }

    /// Dispatch rounding based on round_state (FreeType-compatible).
    /// 0 = none/gray, 1 = grid, 2 = double, 3 = down, 4 = up,
    /// 5 = off, 6 = half grid, 7 = super (SROUND/S45ROUND).
    pub(crate) fn round_distance(&self, distance: i32, compensation: i32) -> i32 {
        match self.gs.round_state {
            0 => round::round_off(distance, compensation),
            1 => round::round_to_grid(distance, compensation),
            2 => round::round_to_double_grid(distance, compensation),
            3 => round::round_down_to_grid(distance, compensation),
            4 => round::round_up_to_grid(distance, compensation),
            5 => round::round_off(distance, compensation),
            6 => round::round_to_half_grid(distance, compensation),
            7 => self.round_super_impl(distance, compensation),
            _ => round::round_to_grid(distance, compensation),
        }
    }

    /// FreeType-style absolute rounding (matches func_round signature).
    /// Returns the absolute rounded distance (not a delta).
    /// Compensation is added inside the rounding function, matching FreeType's
    /// Round_None, Round_To_Grid, etc.
    fn round_abs_ft(&self, distance: i32, compensation: i32) -> i32 {
        match self.gs.round_state {
            0 | 5 => {
                // Round_Off / Round_None: just add compensation
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        0
                    } else {
                        v
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        0
                    } else {
                        v
                    }
                }
            }
            1 => {
                // Round_To_Grid: ((distance + comp) + 32) & ~63
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        0
                    } else {
                        (v + 32) & !63
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        0
                    } else {
                        -(((-v) + 32) & !63)
                    }
                }
            }
            2 => {
                // Round_To_Double_Grid: grid at 32-unit intervals
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        0
                    } else {
                        (v + 16) & !31
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        0
                    } else {
                        -(((-v) + 16) & !31)
                    }
                }
            }
            3 => {
                // Round_Down_To_Grid: (distance + comp) & ~63
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        0
                    } else {
                        v & !63
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        0
                    } else {
                        -((-v) & !63)
                    }
                }
            }
            4 => {
                // Round_Up_To_Grid: ((distance + comp) + 63) & ~63
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        0
                    } else {
                        (v + 63) & !63
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        0
                    } else {
                        -(((-v) + 63) & !63)
                    }
                }
            }
            6 => {
                // Round_To_Half_Grid: ((distance + comp) & ~63) + 32
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        32
                    } else {
                        (v & !63) + 32
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        -32
                    } else {
                        -(((-v) & !63) + 32)
                    }
                }
            }
            7 => {
                // Super rounding (SROUND/S45ROUND) — returns absolute
                self.round_super_abs_ft(distance, compensation)
            }
            _ => {
                if distance >= 0 {
                    let v = distance + compensation;
                    if v < 0 {
                        0
                    } else {
                        (v + 32) & !63
                    }
                } else {
                    let v = distance - compensation;
                    if v > 0 {
                        0
                    } else {
                        -(((-v) + 32) & !63)
                    }
                }
            }
        }
    }

    /// Move a point along the freedom vector by the given distance delta.
    /// Matches FreeType's Direct_Move: adds FT_MulFix(delta, fv.x/y) to the
    /// current coordinates and sets the corresponding touch flags.
    fn move_along_free(zone: &mut Zone, p_idx: usize, delta: i32, fv: F26Dot6Vector) {
        if fv.x != 0 {
            // FT_MulFix: (a * b + 32) >> 6  [for F26Dot6 * F16Dot16 -> F26Dot6]
            // Here both are F26Dot6 so we compute (delta * fv.x + 32) >> 6
            let mx = ((delta as i64 * fv.x as i64 + 32) >> 6) as i32;
            zone.points[p_idx].x = zone.points[p_idx].x + mx;
            zone.tags[p_idx] |= TOUCH_X;
        }
        if fv.y != 0 {
            let my = ((delta as i64 * fv.y as i64 + 32) >> 6) as i32;
            zone.points[p_idx].y = zone.points[p_idx].y + my;
            zone.tags[p_idx] |= TOUCH_Y;
        }
    }

    /// FreeType-style absolute super rounding (for round_state == 7).
    fn round_super_abs_ft(&self, distance: i32, compensation: i32) -> i32 {
        let val = distance;
        if val >= 0 {
            if self.period > 0 {
                let inner = val + self.threshold - self.phase + compensation;
                let result = (inner / self.period) * self.period + self.phase;
                if result < self.phase {
                    self.phase
                } else {
                    result
                }
            } else {
                val
            }
        } else {
            if self.period > 0 {
                let abs_val = -val;
                let inner = abs_val + self.threshold - self.phase + compensation;
                let result = -((inner / self.period) * self.period) - self.phase;
                if result > -self.phase {
                    -self.phase
                } else {
                    result
                }
            } else {
                val
            }
        }
    }

    /// Super rounding (SROUND/S45ROUND) using self.period/self.phase/self.threshold.
    ///
    /// Matches FreeType's Round_Super/Round_Super_45 using integer-division rounding
    /// (works for all period values, not just power-of-2):
    ///
    ///   positive: `((d + threshold - phase + c) / period) * period + phase`
    ///   negative: `-(((|d| + threshold - phase + c) / period) * period) - phase`
    ///
    /// Clamped to `phase` (positive) or `-phase` (negative) to prevent wrap-around.
    fn round_super_impl(&self, distance: i32, compensation: i32) -> i32 {
        let val = distance;
        if val >= 0 {
            if self.period > 0 {
                let inner = val + self.threshold - self.phase + compensation;
                let result = (inner / self.period) * self.period + self.phase;
                if result < self.phase {
                    self.phase
                } else {
                    result
                }
            } else {
                val
            }
        } else {
            if self.period > 0 {
                let abs_val = -val;
                let inner = abs_val + self.threshold - self.phase + compensation;
                let result = -((inner / self.period) * self.period) - self.phase;
                if result > -self.phase {
                    -self.phase
                } else {
                    result
                }
            } else {
                val
            }
        }
    }

    /// Move a point in a zone by `distance` F26Dot6 units along the freedom vector,
    /// marking touched axes. FreeType's `Direct_Move`.
    pub(crate) fn direct_move_with_vec(
        zone: &mut Zone,
        fv: &F26Dot6Vector,
        point: usize,
        distance: i32,
    ) {
        if fv.x != 0 {
            zone.points[point].x += (fv.x * distance) >> 6;
            zone.tags[point] |= TOUCH_X;
        }
        if fv.y != 0 {
            zone.points[point].y += (fv.y * distance) >> 6;
            zone.tags[point] |= TOUCH_Y;
        }
    }

    pub(crate) fn select_zone(&mut self, ptr: usize, zone_id: i32) {
        // FreeType convention: zone_id 0 = twilight, zone_id 1 = glyph (pts)
        // Skip re-cloning if this pointer already points to the requested zone.
        // This preserves modifications made through this pointer.
        if ptr < 3 && self.zp_zone[ptr] == zone_id {
            return;
        }
        let src = if zone_id == 1 {
            &self.pts
        } else if zone_id == 0 {
            &self.twilight
        } else {
            return;
        };
        match ptr {
            0 => self.zp0 = src.clone(),
            1 => self.zp1 = src.clone(),
            2 => self.zp2 = src.clone(),
            _ => {}
        }
        if ptr < 3 {
            self.zp_zone[ptr] = zone_id;
        }
    }

    pub(crate) fn get_zone(&mut self, ptr: usize) -> &mut Zone {
        match ptr {
            0 => &mut self.zp0,
            1 => &mut self.zp1,
            2 => &mut self.zp2,
            _ => &mut self.zp0,
        }
    }

    pub(crate) fn hint_glyph(
        &mut self,
        data: &FontData,
        _glyph_index: u16,
        glyph: &mut crate::scaler::ScaledGlyph,
    ) {
        if glyph.num_contours == 0 {
            return;
        }

        let n = glyph.points.len() as u16;

        self.pts.points = glyph
            .points
            .iter()
            .map(|&(x, y)| F26Dot6Vector::new(x, y))
            .collect();
        self.pts.org = self.pts.points.clone();
        self.pts.tags = glyph
            .on_curve
            .iter()
            .map(|&oc| if oc { ON_CURVE } else { 0 })
            .collect();
        self.pts.contours = glyph.end_pts.clone();
        self.pts.n_points = n;
        self.pts.n_contours = glyph.num_contours as u16;

        let twilight_n = n.max(data.maxp.num_glyphs as u16 * 2).min(256);
        self.twilight.allocate_twilight(twilight_n);

        // FreeType: zp[0] = twilight, zp[1] = glyph, zp[2] = glyph
        // IMPORTANT: allocate twilight BEFORE cloning into zp0 — twilight must
        // have proper capacity so MIRP/MDRP can access remote twlight points.
        self.zp0 = self.twilight.clone();
        self.zp1 = self.pts.clone();
        self.zp2 = self.pts.clone();
        self.zp_zone = [0, 1, 1]; // track which zone each zp points to
        self.gs.gep0 = 0;
        self.gs.gep1 = 1;
        self.gs.gep2 = 1;
        self.gs.rp0 = 0;
        self.gs.rp1 = 0;
        self.gs.rp2 = 0;

        self.glyf_cvt.clone_from(&self.cvt);
        self.glyf_storage.clone_from(&self.storage);

        let ins = self.get_glyph_instructions(data, _glyph_index);
        if ins.is_empty() {
            self.iup(0);
            self.iup(1);
            self.copy_hinted_points_back(glyph);
            return;
        }

        self.code = ins.clone();
        self.cur_range = CodeRange::Glyph;
        self.ip = 0;
        if let Err(e) = self.run() {
            log::warn!("[hinting] glyph {} exec error: {}", _glyph_index, e);
        }

        // Sync ALL zone pointers back to self.pts for IUP and copy-back.
        // Each zp pointer that points to the glyph zone may have accumulated
        // modifications that need to reach self.pts.
        for ptr in 0..3u8 {
            if self.zp_zone[ptr as usize] != 1 {
                continue; // skip twilight zone pointers
            }
            let src = match ptr {
                0 => &self.zp0,
                1 => &self.zp1,
                2 => &self.zp2,
                _ => continue,
            };
            if src.points.len() == self.pts.points.len() {
                self.pts.points.clone_from(&src.points);
                self.pts.tags.clone_from(&src.tags);
            }
        }

        self.iup(0);
        self.iup(1);
        self.copy_hinted_points_back(glyph);
    }

    fn get_glyph_instructions(&self, data: &FontData, glyph_index: u16) -> Vec<u8> {
        let (offset, length) = self.get_glyph_data_offset(data, glyph_index);
        if length < 12 {
            return Vec::new();
        }
        let slice = &data.glyf_data[offset..offset + length];
        let nc = i16::from_be_bytes([slice[0], slice[1]]);
        if nc <= 0 {
            return Vec::new();
        }
        let end_pts_end = 10 + (nc as usize) * 2;
        if slice.len() <= end_pts_end + 2 {
            return Vec::new();
        }
        let inst_len = u16::from_be_bytes([slice[end_pts_end], slice[end_pts_end + 1]]) as usize;
        let start = end_pts_end + 2;
        if start + inst_len > slice.len() {
            return Vec::new();
        }
        slice[start..start + inst_len].to_vec()
    }

    fn copy_hinted_points_back(&self, glyph: &mut crate::scaler::ScaledGlyph) {
        let n = self.pts.n_points.min(glyph.points.len() as u16) as usize;
        glyph.points.truncate(n);
        for i in 0..n {
            glyph.points[i] = (self.pts.points[i].x, self.pts.points[i].y);
        }
    }

    fn get_glyph_data_offset(&self, data: &FontData, glyph_index: u16) -> (usize, usize) {
        let idx = glyph_index as usize;
        if data.loca_format == 0 {
            let off = idx * 2;
            if off + 3 > data.loca_data.len() {
                return (0, 0);
            }
            let this =
                u16::from_be_bytes([data.loca_data[off], data.loca_data[off + 1]]) as usize * 2;
            let next =
                u16::from_be_bytes([data.loca_data[off + 2], data.loca_data[off + 3]]) as usize * 2;
            (this, next - this)
        } else {
            let off = idx * 4;
            if off + 7 > data.loca_data.len() {
                return (0, 0);
            }
            let this = u32::from_be_bytes([
                data.loca_data[off],
                data.loca_data[off + 1],
                data.loca_data[off + 2],
                data.loca_data[off + 3],
            ]) as usize;
            let next = u32::from_be_bytes([
                data.loca_data[off + 4],
                data.loca_data[off + 5],
                data.loca_data[off + 6],
                data.loca_data[off + 7],
            ]) as usize;
            (this, next - this)
        }
    }

    /// Interpolate untouched points after hinting.
    fn iup(&mut self, direction: u8) {
        crate::hinting::iup::iup_zone(&mut self.pts, direction);
    }

    /// Compare two original F26Dot6 vectors for projection distance.
    #[allow(dead_code)]
    fn proj_distance(&self, a: &F26Dot6Vector, b: &F26Dot6Vector) -> i32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dot_product(dx, dy, &self.gs.proj_vector)
    }

    /// Skip to ELSE or EIF when IF condition is false.
    /// FreeType-compatible: IF=0x58, ELSE=0x1B, EIF=0x59
    pub(crate) fn skip_to_else_or_eif(&mut self) {
        let mut depth = 1;
        let mut i = self.ip as usize + 1;
        while i < self.code.len() && depth > 0 {
            match self.code[i] {
                0x58 => depth += 1, // nested IF
                0x1B => {
                    // ELSE (FreeType dispatch)
                    if depth == 1 {
                        break;
                    }
                }
                0x59 => depth -= 1, // EIF (FreeType dispatch)
                _ => {}
            }
            i += 1;
        }
        self.ip = (i - 1) as i32;
    }

    /// Skip to EIF when ELSE branch is done.
    /// FreeType-compatible: EIF=0x59
    pub(crate) fn skip_to_eif(&mut self) {
        let mut depth = 1;
        let mut i = self.ip as usize + 1;
        while i < self.code.len() && depth > 0 {
            match self.code[i] {
                0x58 => depth += 1, // IF
                0x59 => depth -= 1, // EIF (FreeType dispatch)
                _ => {}
            }
            i += 1;
        }
        self.ip = (i - 1) as i32;
    }
}

#[inline]
pub(crate) fn dot_product(dx: i32, dy: i32, vec: &F26Dot6Vector) -> i32 {
    // Vectors are in F26Dot6 but dot product normalizes by 64
    (dx as i64 * vec.x as i64 + dy as i64 * vec.y as i64) as i32 / 64
}

#[cfg(test)]
mod sanity_tests {
    use crate::Font;
    use std::path::Path;

    #[test]
    fn test_font_loading() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        for name in ["DejaVuSans", "LiberationSerif"] {
            let path = manifest_dir
                .join("tests")
                .join("fixtures")
                .join("input")
                .join("fonts")
                .join(format!("{}.ttf", name));
            let data = std::fs::read(&path).unwrap();
            let font = Font::truetype(&data, 10.0).unwrap();
            let mask = font.getmask("a").unwrap();
            eprintln!("{}: 'a' mask {}x{}", name, mask.width, mask.height);
            assert!(mask.width > 0);
            // Test at multiple sizes
            for size in [10.0, 12.0, 14.0, 18.0, 24.0, 36.0, 48.0, 72.0] {
                let f = Font::truetype(&data, size).unwrap();
                let m = f.getmask("a").unwrap();
                eprintln!("  {}px: {}x{}", size as u32, m.width, m.height);
            }
        }
    }
}
