//! ARITHMETIC / ROUNDING / DELTA opcodes 0x60–0x7F (FreeType dispatch).
//!
//! FreeType ttinterp.c dispatch:
//! 0x60 ADD       — add
//! 0x61 SUB       — subtract
//! 0x62 DIV       — divide
//! 0x63 MUL       — multiply
//! 0x64 ABS       — absolute value
//! 0x65 NEG       — negate
//! 0x66 FLOOR     — floor
//! 0x67 CEILING   — ceiling
//! 0x68 ROUND[0]  — round (grid)
//! 0x69 ROUND[1]  — round (double grid)
//! 0x6A ROUND[2]  — round (down)
//! 0x6B ROUND[3]  — round (up)
//! 0x6C NROUND[0] — no rounding (grid)
//! 0x6D NROUND[1] — no rounding (double grid)
//! 0x6E NROUND[2] — no rounding (down)
//! 0x6F NROUND[3] — no rounding (up)
//! 0x70 WCVTF     — write CVT (font units)
//! 0x71 DELTAP2   — delta exception point 2
//! 0x72 DELTAP3   — delta exception point 3
//! 0x73 DELTAC1   — delta exception CVT 1
//! 0x74 DELTAC2   — delta exception CVT 2
//! 0x75 DELTAC3   — delta exception CVT 3
//! 0x76 SROUND    — set round state
//! 0x77 S45ROUND  — set 45° round state
//! 0x78 JROT      — jump relative on true
//! 0x79 JROF      — jump relative on false
//! 0x7A ROFF      — round off
//! 0x7B —         — (unused/reserved)
//! 0x7C RUTG      — round up to grid
//! 0x7D RDTG      — round down to grid
//! 0x7E SANGW     — set angle weight
//! 0x7F AA        — adjust angle

use super::super::exec::ExecContext;
#[allow(unused_imports)]
use super::super::graphics::*;
use super::super::round;
use crate::error::FontError;

impl ExecContext {
    pub(crate) fn handle_60_7f(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            // 0x60 ADD
            0x60 => {
                let a = self.pop();
                let b = self.pop();
                self.push(b + a);
                Ok(1)
            }
            // 0x61 SUB
            0x61 => {
                let a = self.pop();
                let b = self.pop();
                self.push(b - a);
                Ok(1)
            }
            // 0x62 DIV
            0x62 => {
                let a = self.pop();
                let b = self.pop();
                if a == 0 {
                    self.push(0);
                } else {
                    self.push(b / a);
                }
                Ok(1)
            }
            // 0x63 MUL
            0x63 => {
                let a = self.pop();
                let b = self.pop();
                self.push(b * a);
                Ok(1)
            }
            // 0x64 ABS
            0x64 => {
                let v = self.pop();
                self.push(v.abs());
                Ok(1)
            }
            // 0x65 NEG
            0x65 => {
                let v = self.pop();
                self.push(-v);
                Ok(1)
            }
            // 0x66 FLOOR
            0x66 => {
                let v = self.pop();
                self.push(v & !63);
                Ok(1)
            }
            // 0x67 CEILING
            0x67 => {
                let v = self.pop();
                self.push((v + 63) & !63);
                Ok(1)
            }
            // 0x68 ROUND[0] — round to grid (standard rounding)
            0x68 => {
                let val = self.pop();
                // ROUND pops grid period from stack when bit 6 is set
                // For now: apply current rounding
                let result = self.round_distance(val, 0);
                self.push(val + result);
                Ok(1)
            }
            // 0x69 ROUND[1]
            0x69 => {
                let val = self.pop();
                let result = self.round_distance(val, 0);
                self.push(val + result);
                Ok(1)
            }
            // 0x6A ROUND[2]
            0x6A => {
                let val = self.pop();
                let result = self.round_distance(val, 0);
                self.push(val + result);
                Ok(1)
            }
            // 0x6B ROUND[3]
            0x6B => {
                let val = self.pop();
                let result = self.round_distance(val, 0);
                self.push(val + result);
                Ok(1)
            }
            // 0x6C NROUND[0] — no rounding
            0x6C => {
                let val = self.pop();
                self.push(val);
                Ok(1)
            }
            // 0x6D NROUND[1]
            0x6D => {
                let val = self.pop();
                self.push(val);
                Ok(1)
            }
            // 0x6E NROUND[2]
            0x6E => {
                let val = self.pop();
                self.push(val);
                Ok(1)
            }
            // 0x6F NROUND[3]
            0x6F => {
                let val = self.pop();
                self.push(val);
                Ok(1)
            }
            // 0x70 WCVTF
            0x70 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc < self.glyf_cvt.len() {
                    self.glyf_cvt[loc] = val;
                }
                Ok(1)
            }
            // 0x71 DELTAP2 — Delta Exception Point 2 (FreeType: uses shift=4, base=16)
            0x71 => {
                let saved_shift = self.gs.delta_shift;
                let saved_base = self.gs.delta_base;
                self.gs.delta_shift = 4;
                self.gs.delta_base = 16;
                let n = (self.pop() as usize).min(256);
                for _ in 0..n {
                    let arg = self.pop();
                    let p_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta_code = arg & 0x0F;
                    let d = if delta_code >= 8 {
                        ((delta_code as i32) - 16) << (self.gs.delta_shift as i32)
                    } else {
                        (delta_code as i32) << (self.gs.delta_shift as i32)
                    };
                    if p_idx < self.zp0.points.len() {
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * d) >> 6;
                        let fy = (fv.y * d) >> 6;
                        self.zp0.points[p_idx].x += fx;
                        self.zp0.points[p_idx].y += fy;
                        if fv.x != 0 {
                            self.zp0.tags[p_idx] |= TOUCH_X;
                        }
                        if fv.y != 0 {
                            self.zp0.tags[p_idx] |= TOUCH_Y;
                        }
                    }
                }
                self.gs.delta_shift = saved_shift;
                self.gs.delta_base = saved_base;
                Ok(1)
            }

            // 0x72 DELTAP3 — Delta Exception Point 3 (FreeType: uses shift=5, base=17)
            0x72 => {
                let saved_shift = self.gs.delta_shift;
                let saved_base = self.gs.delta_base;
                self.gs.delta_shift = 5;
                self.gs.delta_base = 17;
                let n = (self.pop() as usize).min(256);
                for _ in 0..n {
                    let arg = self.pop();
                    let p_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta_code = arg & 0x0F;
                    let d = if delta_code >= 8 {
                        ((delta_code as i32) - 16) << (self.gs.delta_shift as i32)
                    } else {
                        (delta_code as i32) << (self.gs.delta_shift as i32)
                    };
                    if p_idx < self.zp0.points.len() {
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * d) >> 6;
                        let fy = (fv.y * d) >> 6;
                        self.zp0.points[p_idx].x += fx;
                        self.zp0.points[p_idx].y += fy;
                        if fv.x != 0 {
                            self.zp0.tags[p_idx] |= TOUCH_X;
                        }
                        if fv.y != 0 {
                            self.zp0.tags[p_idx] |= TOUCH_Y;
                        }
                    }
                }
                self.gs.delta_shift = saved_shift;
                self.gs.delta_base = saved_base;
                Ok(1)
            }

            // 0x73 DELTAC1 — Delta Exception CVT 1 (same formula as DELTAP1)
            0x73 => {
                let n = (self.pop() as usize).min(256);
                for _ in 0..n {
                    let arg = self.pop();
                    let c_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta_code = arg & 0x0F;
                    let d = if delta_code >= 8 {
                        ((delta_code as i32) - 16) << (self.gs.delta_shift as i32)
                    } else {
                        (delta_code as i32) << (self.gs.delta_shift as i32)
                    };
                    if c_idx < self.glyf_cvt.len() {
                        self.glyf_cvt[c_idx] += d;
                    }
                }
                Ok(1)
            }

            // 0x74 DELTAC2 — Delta Exception CVT 2 (FreeType: shift=4)
            0x74 => {
                let n = (self.pop() as usize).min(256);
                for _ in 0..n {
                    let arg = self.pop();
                    let c_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta_code = arg & 0x0F;
                    let d = if delta_code >= 8 {
                        ((delta_code as i32) - 16) << 4
                    } else {
                        (delta_code as i32) << 4
                    };
                    if c_idx < self.glyf_cvt.len() {
                        self.glyf_cvt[c_idx] += d;
                    }
                }
                Ok(1)
            }

            // 0x75 DELTAC3 — Delta Exception CVT 3 (FreeType: shift=5)
            0x75 => {
                let n = (self.pop() as usize).min(256);
                for _ in 0..n {
                    let arg = self.pop();
                    let c_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta_code = arg & 0x0F;
                    let d = if delta_code >= 8 {
                        ((delta_code as i32) - 16) << 5
                    } else {
                        (delta_code as i32) << 5
                    };
                    if c_idx < self.glyf_cvt.len() {
                        self.glyf_cvt[c_idx] += d;
                    }
                }
                Ok(1)
            }

            // 0x76 SROUND -- FreeType SetSuperRound
            0x76 => {
                self.gs.round_state = 7;
                let n = self.pop();
                // Period/phase/threshold matching FreeType's SetSuperRound.
                // GridPeriod = 0x4000 (1.0 in F2Dot14 = 16384).
                const GRID_PERIOD: i32 = 0x4000;
                let period_f2dot14 = match ((n >> 6) & 3) as i32 {
                    0 => GRID_PERIOD / 2,
                    1 => GRID_PERIOD,
                    2 => GRID_PERIOD * 2,
                    3 => GRID_PERIOD,
                    _ => GRID_PERIOD,
                };
                let phase_f2dot14 = match ((n >> 4) & 3) as i32 {
                    0 => 0,
                    1 => period_f2dot14 / 4,
                    2 => period_f2dot14 / 2,
                    3 => period_f2dot14 * 3 / 4,
                    _ => 0,
                };
                let threshold_f2dot14 = if (n & 0x0F) == 0 {
                    period_f2dot14 - 1
                } else {
                    ((n & 0x0F) - 4) * period_f2dot14 / 8
                };
                self.period = period_f2dot14 >> 8;
                self.phase = phase_f2dot14 >> 8;
                self.threshold = threshold_f2dot14 >> 8;
                Ok(1)
            }
            // 0x77 S45ROUND -- FreeType SetSuperRound (same formula as SROUND)
            0x77 => {
                self.gs.round_state = 7;
                let n = self.pop();
                const GRID_PERIOD: i32 = 0x4000;
                let period_f2dot14 = match ((n >> 6) & 3) as i32 {
                    0 => GRID_PERIOD / 2,
                    1 => GRID_PERIOD,
                    2 => GRID_PERIOD * 2,
                    3 => GRID_PERIOD,
                    _ => GRID_PERIOD,
                };
                let phase_f2dot14 = match ((n >> 4) & 3) as i32 {
                    0 => 0,
                    1 => period_f2dot14 / 4,
                    2 => period_f2dot14 / 2,
                    3 => period_f2dot14 * 3 / 4,
                    _ => 0,
                };
                let threshold_f2dot14 = if (n & 0x0F) == 0 {
                    period_f2dot14 - 1
                } else {
                    ((n & 0x0F) - 4) * period_f2dot14 / 8
                };
                self.period = period_f2dot14 >> 8;
                self.phase = phase_f2dot14 >> 8;
                self.threshold = threshold_f2dot14 >> 8;
                Ok(1)
            }
            // 0x78 JROT
            0x78 => {
                let offset = self.pop();
                let cond = self.pop();
                if cond != 0 && offset != 0 {
                    self.ip += offset;
                    return Ok(0);
                }
                Ok(1)
            }
            // 0x79 JROF
            0x79 => {
                let offset = self.pop();
                let cond = self.pop();
                if cond == 0 && offset != 0 {
                    self.ip += offset;
                    return Ok(0);
                }
                Ok(1)
            }
            // 0x7A ROFF — round off (no rounding)
            0x7A => {
                self.gs.round_state = 5;
                self.round_fn = round::round_off;
                Ok(1)
            }
            // 0x7B — reserved/unused
            0x7B => {
                log::trace!("[hinting] reserved opcode 0x7B");
                Ok(1)
            }
            // 0x7C RUTG — round up to grid
            0x7C => {
                self.gs.round_state = 4;
                self.round_fn = round::round_up_to_grid;
                Ok(1)
            }
            // 0x7D RDTG — round down to grid
            0x7D => {
                self.gs.round_state = 3;
                self.round_fn = round::round_down_to_grid;
                Ok(1)
            }
            // 0x7E SANGW — set angle weight (no-op, deprecated)
            0x7E => {
                let _ = self.pop();
                log::trace!("[hinting] SANGW ignored");
                Ok(1)
            }
            // 0x7F AA — adjust angle (no-op, deprecated)
            0x7F => {
                let _ = self.pop();
                log::trace!("[hinting] AA ignored");
                Ok(1)
            }
            _ => {
                log::trace!("[hinting] unimpl 0x{:02X} in range 60-7F", self.opcode);
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
    fn test_add() {
        let mut ctx = make_ctx();
        ctx.push(10);
        ctx.push(20);
        ctx.opcode = 0x60;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 30);
    }

    #[test]
    fn test_sub() {
        let mut ctx = make_ctx();
        ctx.push(30);
        ctx.push(10);
        ctx.opcode = 0x61;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 20);
    }

    #[test]
    fn test_div() {
        let mut ctx = make_ctx();
        ctx.push(15);
        ctx.push(3);
        ctx.opcode = 0x62;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 5);
    }

    #[test]
    fn test_mul() {
        let mut ctx = make_ctx();
        ctx.push(3);
        ctx.push(5);
        ctx.opcode = 0x63;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 15);
    }

    #[test]
    fn test_abs_neg() {
        let mut ctx = make_ctx();
        ctx.push(-42);
        ctx.opcode = 0x64;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 42);

        ctx.push(42);
        ctx.opcode = 0x65;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), -42);
    }

    #[test]
    fn test_floor_ceil() {
        let mut ctx = make_ctx();
        ctx.push(100);
        ctx.opcode = 0x66; // FLOOR
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 64); // 100 & !63 = 64

        ctx.push(100);
        ctx.opcode = 0x67; // CEILING
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.pop(), 128); // (100+63) & !63 = 128
    }

    #[test]
    fn test_round_off() {
        let mut ctx = make_ctx();
        ctx.opcode = 0x7A;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.gs.round_state, 5);
    }

    #[test]
    fn test_round_modes() {
        let mut ctx = make_ctx();
        ctx.opcode = 0x7C;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.gs.round_state, 4); // RUTG

        ctx.opcode = 0x7D;
        ctx.handle_60_7f().unwrap();
        assert_eq!(ctx.gs.round_state, 3); // RDTG
    }
}
