//! STORAGE / CVT / COMPARISON / CONTROL FLOW opcodes 0x40–0x5F (FreeType dispatch).
//!
//! FreeType ttinterp.c dispatch:
//! 0x40 NPUSHB    — push N bytes
//! 0x41 NPUSHW    — push N words
//! 0x42 WS        — write store
//! 0x43 RS        — read store
//! 0x44 WCVTP     — write CVT (pixel)
//! 0x45 RCVT      — read CVT
//! 0x46 GC        — get coordinate projected (zp2)
//! 0x47 GC        — get coordinate projected (zp0)
//! 0x48 SCFS      — set coordinate from stack
//! 0x49 MD        — measure distance (zp0→zp1)
//! 0x4A MD        — measure distance (zp0→zp0)
//! 0x4B MPPEM     — measure pixels per EM
//! 0x4C MPS       — measure point size
//! 0x4D FLIPON    — enable auto-flip
//! 0x4E FLIPOFF   — disable auto-flip
//! 0x4F DEBUG     — debug call
//! 0x50 LT        — less than
//! 0x51 LTEQ      — less than or equal
//! 0x52 GT        — greater than
//! 0x53 GTEQ      — greater than or equal
//! 0x54 EQ        — equal
//! 0x55 NEQ       — not equal
//! 0x56 ODD       — odd test
//! 0x57 EVEN      — even test
//! 0x58 IF        — if (conditional)
//! 0x59 EIF       — end if
//! 0x5A AND       — logical AND
//! 0x5B OR        — logical OR
//! 0x5C NOT       — logical NOT
//! 0x5D DELTAP1   — delta exception point 1
//! 0x5E SDB       — set delta base
//! 0x5F SDS       — set delta shift

use super::super::exec::ExecContext;
use super::super::graphics::*;
use crate::error::FontError;
use crate::hinting::exec::dot_product;

impl ExecContext {
    pub(crate) fn handle_40_5f(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            // 0x40 NPUSHB (Apple convention)
            0x40 => {
                let n = self.read_bytes(1)[0] as usize;
                let vals = self.read_bytes(n);
                for &v in &vals {
                    self.push(v);
                }
                Ok(1 + n as i32)
            }
            // 0x41 NPUSHW (Apple convention)
            0x41 => {
                let n = self.read_bytes(1)[0] as usize;
                let vals = self.read_words(n);
                for &v in &vals {
                    self.push(v);
                }
                Ok(1 + (n * 2) as i32)
            }
            // 0x42 WS — write store (Apple convention)
            // TrueType pops: location first, then value
            0x42 => {
                let loc = self.pop() as usize;
                let val = self.pop();
                if loc < 4096 { // safety cap
                    if loc >= self.glyf_storage.len() {
                        self.glyf_storage.resize((loc + 64).min(4096), 0);
                    }
                    if loc < self.glyf_storage.len() {
                        self.glyf_storage[loc] = val;
                    }
                }
                Ok(1)
            }
            // 0x43 RS — read store (Apple convention)
            // TrueType pops: location, pushes: value
            0x43 => {
                let loc = self.pop() as usize;
                let val = if loc < self.glyf_storage.len() {
                    self.glyf_storage[loc]
                } else {
                    0
                };
                self.push(val);
                Ok(1)
            }
            // 0x44 WCVTP
            0x44 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc < self.glyf_cvt.len() {
                    self.glyf_cvt[loc] = val;
                }
                Ok(1)
            }
            // 0x45 RCVT
            0x45 => {
                let loc = self.pop() as usize;
                let val = if loc < self.glyf_cvt.len() {
                    self.glyf_cvt[loc]
                } else {
                    0
                };
                self.push(val);
                Ok(1)
            }
            // 0x46 GC (zp2)
            0x46 => {
                let p_idx = self.pop() as usize;
                let val = if p_idx < self.zp2.points.len() {
                    let p = self.zp2.points[p_idx];
                    dot_product(p.x, p.y, &self.gs.proj_vector)
                } else {
                    0
                };
                self.push(val);
                Ok(1)
            }
            // 0x47 GC (zp0)
            0x47 => {
                let p_idx = self.pop() as usize;
                let val = if p_idx < self.zp0.points.len() {
                    let p = self.zp0.points[p_idx];
                    dot_product(p.x, p.y, &self.gs.proj_vector)
                } else {
                    0
                };
                self.push(val);
                Ok(1)
            }
            // 0x48 SCFS
            0x48 => {
                let val = self.pop();
                let p_idx = self.pop() as usize;
                if p_idx < self.zp2.points.len() {
                    let p = self.zp2.points[p_idx];
                    let cur_proj = dot_product(p.x, p.y, &self.gs.proj_vector);
                    let diff = val - cur_proj;
                    let fv = self.gs.free_vector;
                    self.zp2.points[p_idx].x += (fv.x * diff) >> 6;
                    self.zp2.points[p_idx].y += (fv.y * diff) >> 6;
                    if fv.x != 0 {
                        self.zp2.tags[p_idx] |= TOUCH_X;
                    }
                    if fv.y != 0 {
                        self.zp2.tags[p_idx] |= TOUCH_Y;
                    }
                }
                Ok(1)
            }
            // 0x49 MD (zp0→zp1)
            0x49 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let pp1 = if p1 < self.zp1.points.len() {
                    self.zp1.points[p1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let pp2 = if p2 < self.zp2.points.len() {
                    self.zp2.points[p2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let dist = dot_product(pp2.x - pp1.x, pp2.y - pp1.y, &self.gs.proj_vector);
                self.push(dist);
                Ok(1)
            }
            // 0x4A MD (zp0→zp0)
            0x4A => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let pp1 = if p1 < self.zp0.points.len() {
                    self.zp0.points[p1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let pp2 = if p2 < self.zp0.points.len() {
                    self.zp0.points[p2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let dist = dot_product(pp2.x - pp1.x, pp2.y - pp1.y, &self.gs.proj_vector);
                self.push(dist);
                Ok(1)
            }
            // 0x4B MPPEM — returns ppem in F26Dot6 format (ppem * 64)
            0x4B => {
                self.push((self.ppem as i32) << 6);
                Ok(1)
            }
            // 0x4C MPS
            0x4C => {
                self.push(self.point_size);
                Ok(1)
            }
            // 0x4D FLIPON
            0x4D => {
                self.gs.auto_flip = true;
                Ok(1)
            }
            // 0x4E FLIPOFF
            0x4E => {
                self.gs.auto_flip = false;
                Ok(1)
            }
            // 0x4F DEBUG
            0x4F => {
                let _ = self.pop(); // consume flag
                Ok(1)
            }
            // 0x50 LT
            0x50 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b < a { 1 } else { 0 });
                Ok(1)
            }
            // 0x51 LTEQ
            0x51 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b <= a { 1 } else { 0 });
                Ok(1)
            }
            // 0x52 GT
            0x52 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b > a { 1 } else { 0 });
                Ok(1)
            }
            // 0x53 GTEQ
            0x53 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b >= a { 1 } else { 0 });
                Ok(1)
            }
            // 0x54 EQ
            0x54 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b == a { 1 } else { 0 });
                Ok(1)
            }
            // 0x55 NEQ
            0x55 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b != a { 1 } else { 0 });
                Ok(1)
            }
            // 0x56 ODD
            0x56 => {
                let a = self.pop();
                self.push(if (a >> 6) & 1 == 1 { 1 } else { 0 });
                Ok(1)
            }
            // 0x57 EVEN
            0x57 => {
                let a = self.pop();
                self.push(if (a >> 6) & 1 == 0 { 1 } else { 0 });
                Ok(1)
            }
            // 0x58 IF
            0x58 => {
                let cond = self.pop();
                if cond == 0 {
                    self.skip_to_else_or_eif();
                }
                Ok(1)
            }
            // 0x59 EIF
            0x59 => Ok(1),
            // 0x5A AND
            0x5A => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b != 0 && a != 0 { 1 } else { 0 });
                Ok(1)
            }
            // 0x5B OR
            0x5B => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b != 0 || a != 0 { 1 } else { 0 });
                Ok(1)
            }
            // 0x5C NOT
            0x5C => {
                let a = self.pop();
                self.push(if a == 0 { 1 } else { 0 });
                Ok(1)
            }
            // 0x5D DELTAP1 — Delta Exception Point 1 (uses current delta_shift/delta_base)
            0x5D => {
                let n = (self.pop() as usize).min(256);
                for _ in 0..n {
                    let arg = self.pop();
                    let p_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta_code = arg & 0x0F;
                    let shift = (self.gs.delta_shift.min(6)) as i32;
                    let d = if delta_code >= 8 {
                        ((delta_code as i32) - 16) << shift
                    } else {
                        (delta_code as i32) << shift
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
                Ok(1)
            }

            // 0x5E SDB — set delta base
            0x5E => {
                let n = self.pop() as u16;
                self.gs.delta_base = n;
                Ok(1)
            }
            // 0x5F SDS — set delta shift
            0x5F => {
                let n = self.pop() as u16;
                self.gs.delta_shift = n;
                Ok(1)
            }
            _ => {
                log::trace!("[hinting] unimpl 0x{:02X} in range 40-5F", self.opcode);
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
    fn test_ws_rs() {
        let mut ctx = make_ctx();
        ctx.push(0); // location first
        ctx.push(42); // value second (on top)
        ctx.opcode = 0x42;
        ctx.handle_40_5f().unwrap();
        ctx.top = 0;
        ctx.push(0);
        ctx.opcode = 0x43;
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 42);
    }

    #[test]
    fn test_comparison() {
        let mut ctx = make_ctx();
        ctx.push(10);
        ctx.push(20);
        ctx.opcode = 0x50; // LT
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 1); // 20 < 10? false -> 0

        // GT
        ctx.push(10);
        ctx.push(20);
        ctx.opcode = 0x52;
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 0); // 20 > 10? true -> 1

        ctx.push(10);
        ctx.push(20);
        ctx.opcode = 0x54; // EQ
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 0); // equal? no

        ctx.push(20);
        ctx.push(20);
        ctx.opcode = 0x54;
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 1); // equal? yes
    }

    #[test]
    fn test_and_or_not() {
        let mut ctx = make_ctx();
        ctx.push(1);
        ctx.push(0);
        ctx.opcode = 0x5A; // AND
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 0);

        ctx.push(1);
        ctx.push(0);
        ctx.opcode = 0x5B; // OR
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 1);

        ctx.push(0);
        ctx.opcode = 0x5C; // NOT
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 1);
    }

    #[test]
    fn test_odd_even() {
        let mut ctx = make_ctx();
        ctx.push(1 << 6); // 1 pixel (even in 26.6 = 64)
        ctx.opcode = 0x56; // ODD
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.pop(), 1); // 64>>6=1, bit0=1 => ODD=1

        // 64 >> 6 = 1, bit 0 = 1 -> ODD returns 1
    }

    #[test]
    fn test_sdb_sds() {
        let mut ctx = make_ctx();
        ctx.push(10);
        ctx.opcode = 0x5E; // SDB
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.gs.delta_base, 10);

        ctx.push(4);
        ctx.opcode = 0x5F; // SDS
        ctx.handle_40_5f().unwrap();
        assert_eq!(ctx.gs.delta_shift, 4);
    }
}
