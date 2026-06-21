//! REFERENCE POINT / ZONE / CONTROL opcodes 0x10–0x1F (FreeType dispatch).
//!
//! FreeType ttinterp.c dispatch:
//! 0x10 SRP0      — set reference point 0
//! 0x11 SRP1      — set reference point 1
//! 0x12 SRP2      — set reference point 2
//! 0x13 SZP0      — set zone pointer 0
//! 0x14 SZP1      — set zone pointer 1
//! 0x15 SZP2      — set zone pointer 2
//! 0x16 SZPS      — set zone pointers (all three)
//! 0x17 SLOOP     — set LOOP variable
//! 0x18 RTG       — round to grid
//! 0x19 RTHG      — round to half grid
//! 0x1A SMD       — set minimum distance
//! 0x1B ELSE      — ELSE (control flow)
//! 0x1C JMPR      — jump relative
//! 0x1D SCVTCI    — set control value table cut-in
//! 0x1E SSWCI     — set single width cut-in
//! 0x1F SSW       — set single width value

use crate::error::FontError;
use super::super::exec::ExecContext;
use super::super::round;

impl ExecContext {
    pub(crate) fn handle_10_1f(&mut self) -> Result<i32, FontError> {
        match self.opcode {
            // 0x10 SRP0
            0x10 => {
                let n = self.pop() as u16;
                self.gs.rp0 = n;
                Ok(1)
            }
            // 0x11 SRP1
            0x11 => {
                let n = self.pop() as u16;
                self.gs.rp1 = n;
                Ok(1)
            }
            // 0x12 SRP2
            0x12 => {
                let n = self.pop() as u16;
                self.gs.rp2 = n;
                Ok(1)
            }
            // 0x13 SZP0
            0x13 => {
                let n = self.pop();
                self.gs.gep0 = n as u16;
                self.select_zone(0, n);
                Ok(1)
            }
            // 0x14 SZP1
            0x14 => {
                let n = self.pop();
                self.gs.gep1 = n as u16;
                self.select_zone(1, n);
                Ok(1)
            }
            // 0x15 SZP2
            0x15 => {
                let n = self.pop();
                self.gs.gep2 = n as u16;
                self.select_zone(2, n);
                Ok(1)
            }
            // 0x16 SZPS
            0x16 => {
                let n = self.pop();
                self.gs.gep0 = n as u16;
                self.gs.gep1 = n as u16;
                self.gs.gep2 = n as u16;
                self.select_zone(0, n);
                self.select_zone(1, n);
                self.select_zone(2, n);
                Ok(1)
            }
            // 0x17 SLOOP
            0x17 => {
                let n = self.pop();
                self.gs.loop_count = n;
                Ok(1)
            }
            // 0x18 RTG — round to grid
            0x18 => {
                self.gs.round_state = 1;
                self.round_fn = round::round_to_grid;
                Ok(1)
            }
            // 0x19 RTHG — round to half grid
            0x19 => {
                // Round to half grid: rounds +-0.5 pixels
                self.gs.round_state = 6;
                self.round_fn = round::round_to_half_grid;
                Ok(1)
            }
            // 0x1A SMD — set minimum distance
            0x1A => {
                let n = self.pop();
                self.gs.minimum_distance = n;
                Ok(1)
            }
            // 0x1B ELSE — ELSE (end of IF-true branch)
            0x1B => {
                self.skip_to_eif();
                Ok(1)
            }
            // 0x1C JMPR — jump relative
            0x1C => {
                let offset = self.pop();
                if offset != 0 {
                    self.ip += offset;
                    Ok(0)
                } else {
                    Ok(1)
                }
            }
            // 0x1D SCVTCI — set control value table cut-in
            0x1D => {
                let n = self.pop();
                self.gs.control_value_cut_in = n;
                Ok(1)
            }
            // 0x1E SSWCI — set single width cut-in
            0x1E => {
                let n = self.pop();
                self.gs.single_width_cut_in = n;
                Ok(1)
            }
            // 0x1F SSW — set single width value
            0x1F => {
                let n = self.pop();
                // n is in F26Dot6, stored as single_width_value
                self.gs.single_width_value = n;
                Ok(1)
            }
            _ => {
                log::trace!("[hinting] unimpl 0x{:02X} in range 10-1F", self.opcode);
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
    fn test_srp0() {
        let mut ctx = make_ctx();
        ctx.push(42);
        ctx.opcode = 0x10;
        ctx.handle_10_1f().unwrap();
        assert_eq!(ctx.gs.rp0, 42);
    }

    #[test]
    fn test_sloop() {
        let mut ctx = make_ctx();
        ctx.push(5);
        ctx.opcode = 0x17;
        ctx.handle_10_1f().unwrap();
        assert_eq!(ctx.gs.loop_count, 5);
    }

    #[test]
    fn test_smd() {
        let mut ctx = make_ctx();
        ctx.push(64);
        ctx.opcode = 0x1A;
        ctx.handle_10_1f().unwrap();
        assert_eq!(ctx.gs.minimum_distance, 64);
    }

    #[test]
    fn test_single_width() {
        let mut ctx = make_ctx();
        ctx.push(128);
        ctx.opcode = 0x1F;
        ctx.handle_10_1f().unwrap();
        assert_eq!(ctx.gs.single_width_value, 128);
    }

    #[test]
    fn test_jmpr() {
        let mut ctx = make_ctx();
        ctx.push(5);
        ctx.top = 0; // ip advance
        ctx.opcode = 0x1C;
        ctx.handle_10_1f().unwrap();
    }
}
