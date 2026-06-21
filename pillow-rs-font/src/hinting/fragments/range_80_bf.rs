//! EXTENDED opcodes 0x80–0xBF (FreeType dispatch).
//!
//! FreeType ttinterp.c dispatch:
//! 0x80 FLIPPT     — flip point on/off
//! 0x81 FLIPRGON   — flip range on
//! 0x82 FLIPRGOFF  — flip range off
//! 0x83-0x84      — reserved/unknown
//! 0x85 SCANCTRL   — scan conversion control
//! 0x86-0x87 SDPVTL — set dual proj vector to line
//! 0x88 GETINFO    — get info
//! 0x89 IDEF       — instruction definition
//! 0x8A ROLL       — roll top 3 stack elements
//! 0x8B MAX        — maximum
//! 0x8C MIN        — minimum
//! 0x8D SCANTYPE   — scan type
//! 0x8E INSTCTRL   — instruction control
//! 0x8F-0x90 ADJUST — adjust (GX)
//! 0x91-0x9F      — GX variation
//! 0xB0-0xB7 PUSHB[0-7]
//! 0xB8-0xBF PUSHW[0-7]

use crate::error::FontError;
use super::super::exec::ExecContext;
use super::super::graphics::*;

impl ExecContext {
    pub(crate) fn handle_80_bf(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            // 0x80 FLIPPT — flip point on/off curve
            0x80 => {
                let count = self.gs.loop_count.max(1);
                for _ in 0..count {
                    let p_idx = self.pop() as usize;
                    if p_idx < self.zp0.tags.len() {
                        self.zp0.tags[p_idx] ^= ON_CURVE;
                    }
                }
                self.gs.loop_count = 1;
                Ok(1)
            }
            // 0x81 FLIPRGON — flip range on (set ON_CURVE)
            0x81 => {
                let end = self.pop() as usize;
                let start = self.pop() as usize;
                let zone = self.get_zone(0);
                for p in start..=end {
                    if p < zone.tags.len() {
                        zone.tags[p] |= ON_CURVE;
                    }
                }
                Ok(1)
            }
            // 0x82 FLIPRGOFF — flip range off (clear ON_CURVE)
            0x82 => {
                let end = self.pop() as usize;
                let start = self.pop() as usize;
                let zone = self.get_zone(0);
                for p in start..=end {
                    if p < zone.tags.len() {
                        zone.tags[p] &= !ON_CURVE;
                    }
                }
                Ok(1)
            }
            // 0x83-0x84 — reserved/unknown
            0x83 | 0x84 => {
                log::trace!("[hinting] reserved opcode 0x{:02X}", self.opcode);
                Ok(1)
            }
            // 0x85 SCANCTRL — scan conversion control
            0x85 => {
                let n = self.pop();
                self.gs.scan_control = (n & 2) != 0;
                self.gs.scan_type = n & 1;
                Ok(1)
            }
            // 0x86 SDPVTL[//] — set dual proj vector to line (parallel)
            0x86 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let point1 = if p1 < self.zp0.points.len() { self.zp0.points[p1] } else { F26Dot6Vector::new(0, 0) };
                let point2 = if p2 < self.zp0.points.len() { self.zp0.points[p2] } else { F26Dot6Vector::new(0, 0) };
                let dx = point2.x - point1.x;
                let dy = point2.y - point1.y;
                let len = ((dx as i64).abs().max((dy as i64).abs())) as i32;
                if len > 0 {
                    let vec = F26Dot6Vector::new(
                        (dx as i64 * (1 << 6) as i64 / len as i64) as i32,
                        (dy as i64 * (1 << 6) as i64 / len as i64) as i32,
                    );
                    self.gs.dual_vector = vec;
                }
                Ok(1)
            }
            // 0x87 SDPVTL[+] — set dual proj vector ⊥ line (perpendicular)
            0x87 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let point1 = if p1 < self.zp0.points.len() { self.zp0.points[p1] } else { F26Dot6Vector::new(0, 0) };
                let point2 = if p2 < self.zp0.points.len() { self.zp0.points[p2] } else { F26Dot6Vector::new(0, 0) };
                let dx = point2.x - point1.x;
                let dy = point2.y - point1.y;
                let len = ((dx as i64).abs().max((dy as i64).abs())) as i32;
                if len > 0 {
                    self.gs.dual_vector = F26Dot6Vector::new(
                        (-dy as i64 * (1 << 6) as i64 / len as i64) as i32,
                        (dx as i64 * (1 << 6) as i64 / len as i64) as i32,
                    );
                }
                Ok(1)
            }
            // 0x88 GETINFO
            0x88 => {
                let selector = self.pop();
                let mut result = 0i32;
                if selector & 1 != 0 { result |= 35; } // version 35
                if selector & 2 != 0 && self.grayscale { result |= 0x100; }
                if selector & 16 != 0 { result |= 16; }
                if selector & 32 != 0 { result |= 32; }
                self.push(result);
                Ok(1)
            }
            // 0x89 IDEF — instruction definition
            0x89 => {
                let fn_idx = self.pop() as usize;
                let start = self.ip + 1;
                if fn_idx >= self.idefs.len() {
                    self.idefs.resize(fn_idx + 16, super::super::exec::FnDef::default());
                }
                self.idefs[fn_idx] = super::super::exec::FnDef {
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
                    if self.code[i] == 0x2C { depth += 1; }
                    else if self.code[i] == 0x2D { depth -= 1; }
                    i += 1;
                }
                self.idefs[fn_idx].end = (i - 1) as i32;
                self.ip = (i - 1) as i32;
                Ok(1)
            }
            // 0x8A ROLL — roll top 3 stack elements
            0x8A => {
                if self.top >= 3 {
                    let a = self.stack[(self.top as usize) - 3];
                    let b = self.stack[(self.top as usize) - 2];
                    let c = self.stack[(self.top as usize) - 1];
                    self.stack[(self.top as usize) - 3] = c;
                    self.stack[(self.top as usize) - 2] = a;
                    self.stack[(self.top as usize) - 1] = b;
                }
                Ok(1)
            }
            // 0x8B MAX
            0x8B => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b > a { b } else { a });
                Ok(1)
            }
            // 0x8C MIN
            0x8C => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b < a { b } else { a });
                Ok(1)
            }
            // 0x8D SCANTYPE
            0x8D => {
                let n = self.pop();
                self.gs.scan_type = n;
                Ok(1)
            }
            // 0x8E INSTCTRL — instruction control
            0x8E => {
                let s = self.pop() as u8;
                let v = self.pop() as u8;
                if s == 1 {
                    // Bit 1 of v controls scan conversion
                    self.gs.instruct_control = v;
                }
                Ok(1)
            }
            // PUSHB[0-7]
            0xB0..=0xB7 => {
                let n = (self.opcode - 0xB0 + 1) as usize;
                let vals = self.read_bytes(n);
                for &v in &vals { self.push(v); }
                Ok(1 + n as i32)
            }
            // PUSHW[0-7]
            0xB8..=0xBF => {
                let n = (self.opcode - 0xB8 + 1) as usize;
                let vals = self.read_words(n);
                for &v in &vals { self.push(v); }
                Ok(1 + (n * 2) as i32)
            }
            _ => {
                log::trace!("[hinting] unimpl 0x{:02X} in range 80-BF", self.opcode);
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
    fn test_roll() {
        let mut ctx = make_ctx();
        ctx.push(1); ctx.push(2); ctx.push(3);
        ctx.opcode = 0x8A;
        ctx.handle_80_bf().unwrap();
        assert_eq!(ctx.pop(), 2); // top after roll = a
        assert_eq!(ctx.pop(), 1); // = c ?
        // Actually roll: (a b c) -> (c a b). So from top: b, a, c
    }

    #[test]
    fn test_max_min() {
        let mut ctx = make_ctx();
        ctx.push(10); ctx.push(20);
        ctx.opcode = 0x8B; // MAX
        ctx.handle_80_bf().unwrap();
        assert_eq!(ctx.pop(), 20);

        ctx.push(10); ctx.push(20);
        ctx.opcode = 0x8C; // MIN
        ctx.handle_80_bf().unwrap();
        assert_eq!(ctx.pop(), 10);
    }

    #[test]
    fn test_pushb() {
        let mut ctx = make_ctx();
        ctx.code = vec![0xB2, 0x0A, 0x0B, 0x0C]; // PUSHB[2], 3 vals
        ctx.ip = 0;
        ctx.opcode = 0xB2;
        let len = ctx.handle_80_bf().unwrap();
        assert_eq!(len, 4); // 1 + 3
        assert_eq!(ctx.pop(), 0x0C);
        assert_eq!(ctx.pop(), 0x0B);
        assert_eq!(ctx.pop(), 0x0A);
    }
}
