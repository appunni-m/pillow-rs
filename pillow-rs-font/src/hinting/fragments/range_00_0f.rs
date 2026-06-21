//! VECTOR opcodes 0x00–0x0F (FreeType dispatch).
//!
//! FreeType encodes the axis (x/y, parallel/perpendicular) in bit 0 of the
//! opcode — no stack pop.  The reference is FreeType's `ttinterp.c` dispatch.
//!
//! 0x00 SVTCA[y]   — set proj+free+dual to y-axis  (no pop)
//! 0x01 SVTCA[x]   — set proj+free+dual to x-axis  (no pop)
//! 0x02 SPVTCA[y]  — set projection to y-axis      (no pop)
//! 0x03 SPVTCA[x]  — set projection to x-axis      (no pop)
//! 0x04 SFVTCA[y]  — set freedom    to y-axis      (no pop)
//! 0x05 SFVTCA[x]  — set freedom    to x-axis      (no pop)
//! 0x06 SPVTL[//]  — set proj  vector to line p1→p2 (parallel)
//! 0x07 SPVTL[+]   — set proj  vector ⊥ line p1→p2 (perpendicular)
//! 0x08 SFVTL[//]  — set free  vector to line p1→p2 (parallel)
//! 0x09 SFVTL[+]   — set free  vector ⊥ line p1→p2 (perpendicular)
//! 0x0A SPVFS      — set proj  vector from (x,y) on stack
//! 0x0B SFVFS      — set free  vector from (x,y) on stack
//! 0x0C GPV        — get proj  vector → stack
//! 0x0D GFV        — get free  vector → stack
//! 0x0E SFVTPV     — set free  vector = proj vector
//! 0x0F ISECT      — intersection of two line segments

use super::super::exec::ExecContext;
use super::super::graphics::F26Dot6Vector;
use crate::error::FontError;

impl ExecContext {
    pub(crate) fn handle_00_0f(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            // 0x00 SVTCA[y] — axis encoded in bit 0 of opcode, NOT on stack
            0x00 => {
                // Set proj_vector, free_vector, dual_vector to y-axis (0, 1<<6)
                self.gs.proj_vector = F26Dot6Vector::new(0, 1 << 6);
                self.gs.free_vector = self.gs.proj_vector;
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x01 SVTCA[x]
            0x01 => {
                self.gs.proj_vector = F26Dot6Vector::new(1 << 6, 0);
                self.gs.free_vector = self.gs.proj_vector;
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x02 SPVTCA[y] — projection to y-axis
            0x02 => {
                self.gs.proj_vector = F26Dot6Vector::new(0, 1 << 6);
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x03 SPVTCA[x] — projection to x-axis
            0x03 => {
                self.gs.proj_vector = F26Dot6Vector::new(1 << 6, 0);
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x04 SFVTCA[y] — freedom to y-axis
            0x04 => {
                self.gs.free_vector = F26Dot6Vector::new(0, 1 << 6);
                Ok(1)
            }
            // 0x05 SFVTCA[x] — freedom to x-axis
            0x05 => {
                self.gs.free_vector = F26Dot6Vector::new(1 << 6, 0);
                Ok(1)
            }
            // 0x06 SPVTL[//] — set projection vector to line
            0x06 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                // Read from zp0
                let point1 = if p1 < self.zp0.points.len() {
                    self.zp0.points[p1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let point2 = if p2 < self.zp0.points.len() {
                    self.zp0.points[p2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let dx = point2.x - point1.x;
                let dy = point2.y - point1.y;
                let len = ((dx as i64).abs().max((dy as i64).abs())) as i32;
                if len > 0 {
                    self.gs.proj_vector = F26Dot6Vector::new(
                        (dx as i64 * (1 << 6) as i64 / len as i64) as i32,
                        (dy as i64 * (1 << 6) as i64 / len as i64) as i32,
                    );
                }
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x07 SPVTL[+] — set projection vector perpendicular to line
            0x07 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let point1 = if p1 < self.zp0.points.len() {
                    self.zp0.points[p1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let point2 = if p2 < self.zp0.points.len() {
                    self.zp0.points[p2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let dx = point2.x - point1.x;
                let dy = point2.y - point1.y;
                let len = ((dx as i64).abs().max((dy as i64).abs())) as i32;
                if len > 0 {
                    // Perpendicular: swap dx/dy and negate one
                    self.gs.proj_vector = F26Dot6Vector::new(
                        (-dy as i64 * (1 << 6) as i64 / len as i64) as i32,
                        (dx as i64 * (1 << 6) as i64 / len as i64) as i32,
                    );
                }
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x08 SFVTL[//] — set freedom vector to line
            0x08 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let point1 = if p1 < self.zp0.points.len() {
                    self.zp0.points[p1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let point2 = if p2 < self.zp0.points.len() {
                    self.zp0.points[p2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let dx = point2.x - point1.x;
                let dy = point2.y - point1.y;
                let len = ((dx as i64).abs().max((dy as i64).abs())) as i32;
                if len > 0 {
                    self.gs.free_vector = F26Dot6Vector::new(
                        (dx as i64 * (1 << 6) as i64 / len as i64) as i32,
                        (dy as i64 * (1 << 6) as i64 / len as i64) as i32,
                    );
                }
                Ok(1)
            }
            // 0x09 SFVTL[+] — set freedom vector perpendicular
            0x09 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let point1 = if p1 < self.zp0.points.len() {
                    self.zp0.points[p1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let point2 = if p2 < self.zp0.points.len() {
                    self.zp0.points[p2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let dx = point2.x - point1.x;
                let dy = point2.y - point1.y;
                let len = ((dx as i64).abs().max((dy as i64).abs())) as i32;
                if len > 0 {
                    self.gs.free_vector = F26Dot6Vector::new(
                        (-dy as i64 * (1 << 6) as i64 / len as i64) as i32,
                        (dx as i64 * (1 << 6) as i64 / len as i64) as i32,
                    );
                }
                Ok(1)
            }
            // 0x0A SPVFS — set projection vector from stack
            0x0A => {
                let y = self.pop();
                let x = self.pop();
                self.gs.proj_vector = F26Dot6Vector::new(x, y);
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x0B SFVFS — set freedom vector from stack
            0x0B => {
                let y = self.pop();
                let x = self.pop();
                self.gs.free_vector = F26Dot6Vector::new(x, y);
                Ok(1)
            }
            // 0x0C GPV — get projection vector
            0x0C => {
                self.push(self.gs.proj_vector.x);
                self.push(self.gs.proj_vector.y);
                Ok(1)
            }
            // 0x0D GFV — get freedom vector
            0x0D => {
                self.push(self.gs.free_vector.x);
                self.push(self.gs.free_vector.y);
                Ok(1)
            }
            // 0x0E SFVTPV — set freedom vector = projection vector
            0x0E => {
                self.gs.free_vector = self.gs.proj_vector;
                Ok(1)
            }
            // 0x0F ISECT — intersection
            0x0F => {
                let a2 = self.pop() as usize;
                let a1 = self.pop() as usize;
                let b2 = self.pop() as usize;
                let b1 = self.pop() as usize;
                let p_idx = self.pop() as usize;
                let p_a1 = if a1 < self.zp0.points.len() {
                    self.zp0.points[a1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let p_a2 = if a2 < self.zp0.points.len() {
                    self.zp0.points[a2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let p_b1 = if b1 < self.zp1.points.len() {
                    self.zp1.points[b1]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                let p_b2 = if b2 < self.zp1.points.len() {
                    self.zp1.points[b2]
                } else {
                    F26Dot6Vector::new(0, 0)
                };
                if p_idx < self.zp2.points.len() {
                    let a_dx = p_a2.x - p_a1.x;
                    let a_dy = p_a2.y - p_a1.y;
                    let b_dx = p_b2.x - p_b1.x;
                    let b_dy = p_b2.y - p_b1.y;
                    let denom = a_dx as i64 * b_dy as i64 - a_dy as i64 * b_dx as i64;
                    if denom != 0 {
                        let t_num = (p_b1.x - p_a1.x) as i64 * b_dy as i64
                            - (p_b1.y - p_a1.y) as i64 * b_dx as i64;
                        let ix = p_a1.x as i64 + (a_dx as i64 * t_num) / denom;
                        let iy = p_a1.y as i64 + (a_dy as i64 * t_num) / denom;
                        self.zp2.points[p_idx] = F26Dot6Vector::new(ix as i32, iy as i32);
                    }
                }
                Ok(1)
            }
            _ => {
                log::trace!("[hinting] unimpl 0x{:02X} in range 00-0F", self.opcode);
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
    fn test_svtca_y() {
        let mut ctx = make_ctx();
        ctx.opcode = 0x00;
        ctx.handle_00_0f().unwrap();
        assert_eq!(ctx.gs.proj_vector.x, 0);
        assert_eq!(ctx.gs.proj_vector.y, 64);
        assert_eq!(ctx.gs.free_vector.x, 0);
        assert_eq!(ctx.gs.free_vector.y, 64);
    }

    #[test]
    fn test_svtca_x() {
        let mut ctx = make_ctx();
        ctx.opcode = 0x01;
        ctx.handle_00_0f().unwrap();
        assert_eq!(ctx.gs.proj_vector.x, 64);
        assert_eq!(ctx.gs.proj_vector.y, 0);
        assert_eq!(ctx.gs.free_vector.x, 64);
        assert_eq!(ctx.gs.free_vector.y, 0);
    }

    #[test]
    fn test_spvfs() {
        let mut ctx = make_ctx();
        ctx.push(3 << 6); // x = 3px
        ctx.push(4 << 6); // y = 4px
        ctx.opcode = 0x0A;
        ctx.handle_00_0f().unwrap();
        assert_eq!(ctx.gs.proj_vector.x, 3 << 6);
        assert_eq!(ctx.gs.proj_vector.y, 4 << 6);
    }

    #[test]
    fn test_gpv() {
        let mut ctx = make_ctx();
        ctx.gs.proj_vector = F26Dot6Vector::new(7 << 6, 8 << 6);
        ctx.opcode = 0x0C;
        ctx.handle_00_0f().unwrap();
        assert_eq!(ctx.pop(), 8 << 6); // y first
        assert_eq!(ctx.pop(), 7 << 6); // x second
    }

    #[test]
    fn test_sfvtpv() {
        let mut ctx = make_ctx();
        ctx.gs.proj_vector = F26Dot6Vector::new(5 << 6, 6 << 6);
        ctx.gs.free_vector = F26Dot6Vector::new(0, 0);
        ctx.opcode = 0x0E;
        ctx.handle_00_0f().unwrap();
        assert_eq!(ctx.gs.free_vector.x, 5 << 6);
        assert_eq!(ctx.gs.free_vector.y, 6 << 6);
    }
}
