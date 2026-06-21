//! TrueType bytecode VM.

#![allow(missing_docs)]

use crate::error::FontError;
use crate::tables::FontData;

use super::graphics::*;
use super::round;

#[derive(Copy, Clone, PartialEq)]
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

pub struct ExecContext {
    pub gs: GraphicsState,
    pub zp0: Zone,
    pub zp1: Zone,
    pub zp2: Zone,
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
    pub fn new(data: &FontData) -> Self {
        let ppem = data.size_pt.ceil() as u16;
        let point_size = (ppem as i32) << 6;

        ExecContext {
            gs: GraphicsState::default(),
            zp0: Zone::new(),
            zp1: Zone::new(),
            zp2: Zone::new(),
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

    pub fn run(&mut self) -> Result<(), FontError> {
        self.ip = 0;
        while self.ip < self.code.len() as i32 {
            self.opcode = self.code[self.ip as usize];
            let length = self.execute_opcode()?;
            self.ip += length;
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

    fn read_bytes(&self, count: usize) -> Vec<i32> {
        let start = (self.ip + 1) as usize;
        let end = (start + count).min(self.code.len());
        self.code[start..end].iter().map(|&b| b as i32).collect()
    }

    fn read_words(&self, count: usize) -> Vec<i32> {
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
            // NPUSHB
            0x40 => {
                let n = self.read_bytes(1)[0] as usize;
                let vals = self.read_bytes(n);
                for &v in &vals {
                    self.push(v);
                }
                Ok(1 + n as i32)
            }
            // NPUSHW
            0x41 => {
                let n = self.read_bytes(1)[0] as usize;
                let vals = self.read_words(n);
                for &v in &vals {
                    self.push(v);
                }
                Ok(1 + (n * 2) as i32)
            }
            // PUSHB[0-7]
            0xB0..=0xB7 => {
                let n = (self.opcode - 0xB0 + 1) as usize;
                let vals = self.read_bytes(n);
                for &v in &vals {
                    self.push(v);
                }
                Ok(1 + n as i32)
            }
            // PUSHW[0-7]
            0xB8..=0xBF => {
                let n = (self.opcode - 0xB8 + 1) as usize;
                let vals = self.read_words(n);
                for &v in &vals {
                    self.push(v);
                }
                Ok(1 + (n * 2) as i32)
            }
            // DUP
            0x20 => {
                let v = self.pop();
                self.push(v);
                self.push(v);
                Ok(1)
            }
            // POP
            0x21 => {
                self.pop();
                Ok(1)
            }
            // CLEAR
            0x22 => {
                self.top = 0;
                Ok(1)
            }
            // SWAP
            0x23 => {
                let a = self.pop();
                let b = self.pop();
                self.push(a);
                self.push(b);
                Ok(1)
            }
            // DEPTH
            0x24 => {
                self.push(self.top);
                Ok(1)
            }
            // ADD
            0x62 => {
                let a = self.pop();
                let b = self.pop();
                self.push(b + a);
                Ok(1)
            }
            // SUB
            0x63 => {
                let a = self.pop();
                let b = self.pop();
                self.push(b - a);
                Ok(1)
            }
            // DIV
            0x64 => {
                let a = self.pop();
                let b = self.pop();
                if a == 0 {
                    self.push(0);
                } else {
                    self.push(b / a);
                }
                Ok(1)
            }
            // MUL
            0x65 => {
                let a = self.pop();
                let b = self.pop();
                self.push(b * a);
                Ok(1)
            }
            // ABS
            0x66 => {
                let v = self.pop();
                self.push(v.abs());
                Ok(1)
            }
            // NEG
            0x67 => {
                let v = self.pop();
                self.push(-v);
                Ok(1)
            }
            // FLOOR
            0x68 => {
                let v = self.pop();
                self.push(v & !63);
                Ok(1)
            }
            // CEILING
            0x69 => {
                let v = self.pop();
                self.push((v + 63) & !63);
                Ok(1)
            }
            // RS — Read Store
            0x43 => {
                let loc = self.pop() as usize;
                let val = if loc < self.storage.len() {
                    self.storage[loc]
                } else {
                    0
                };
                self.push(val);
                Ok(1)
            }
            // WS — Write Store
            0x42 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc >= self.storage.len() {
                    self.storage.resize(loc + 64, 0);
                }
                self.storage[loc] = val;
                Ok(1)
            }
            // RCVT — Read CVT
            0x45 => {
                let loc = self.pop() as usize;
                let val = if loc < self.cvt.len() {
                    self.cvt[loc]
                } else {
                    0
                };
                self.push(val);
                Ok(1)
            }
            // WCVTP — Write CVT (pixel coords)
            0x44 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc >= self.cvt.len() {
                    // CVT entries are fixed-size; silently ignore out-of-bounds writes
                } else {
                    self.cvt[loc] = val;
                }
                Ok(1)
            }
            // WCVTF — Write CVT (font units -> pixels)
            0x70 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc < self.cvt.len() {
                    // WCVTF stores the value directly (no scaling needed, it's already
                    // in F26Dot6 -- the value was pushed as a pixel measurement)
                    self.cvt[loc] = val;
                }
                Ok(1)
            }
            // MPPEM — Measure Pixels Per EM
            0x4B => {
                self.push(self.ppem as i32);
                Ok(1)
            }
            // RTDG — Round To Double Grid
            0x3D => {
                self.round_fn = round::round_to_double_grid;
                Ok(1)
            }
            // SROUND — Set Rounding State
            0x76 => {
                let n = self.pop();
                self.period = ((n >> 6) & 7) as i32;
                self.phase = (n >> 4) & 3;
                self.threshold = (n >> 2) & 3;
                // Skip if reserved bits make no sense
                Ok(1)
            }
            // S45ROUND — Set Rounding State (45-degree)
            0x77 => {
                let n = self.pop();
                self.period = ((n >> 6) & 7) as i32;
                self.phase = (n >> 4) & 3;
                self.threshold = (n >> 2) & 3;
                Ok(1)
            }
            // SLOOP — Set LOOP variable
            0x17 => {
                let n = self.pop();
                self.gs.loop_count = n;
                Ok(1)
            }
            // SMD — Set Minimum Distance
            0x18 => {
                let n = self.pop();
                self.gs.minimum_distance = n;
                Ok(1)
            }
            // SCVTCI — Set Control Value Table Cut-In
            0x19 => {
                let n = self.pop();
                self.gs.control_value_cut_in = n;
                Ok(1)
            }
            // SSWCI — Set Single Width Cut-In
            0x1A => {
                let n = self.pop();
                self.gs.single_width_cut_in = n;
                Ok(1)
            }
            // SSW — Set Single Width Value
            0x1B => {
                let n = self.pop();
                self.gs.single_width_value = n;
                Ok(1)
            }
            // SRP0 — Set Reference Point 0
            0x10 => {
                let n = self.pop() as u16;
                self.gs.rp0 = n;
                Ok(1)
            }
            // SRP1 — Set Reference Point 1
            0x11 => {
                let n = self.pop() as u16;
                self.gs.rp1 = n;
                Ok(1)
            }
            // SRP2 — Set Reference Point 2
            0x12 => {
                let n = self.pop() as u16;
                self.gs.rp2 = n;
                Ok(1)
            }
            // SZP0 — Set Zone Pointer 0
            0x13 => {
                let n = self.pop();
                self.select_zone(0, n);
                Ok(1)
            }
            // SZP1 — Set Zone Pointer 1
            0x14 => {
                let n = self.pop();
                self.select_zone(1, n);
                Ok(1)
            }
            // SZP2 — Set Zone Pointer 2
            0x15 => {
                let n = self.pop();
                self.select_zone(2, n);
                Ok(1)
            }
            // SZPS — Set Zone Pointers
            0x16 => {
                let n = self.pop();
                self.select_zone(0, n);
                self.select_zone(1, n);
                self.select_zone(2, n);
                Ok(1)
            }
            // MDAP — Move Direct Absolute Point (no rounding)
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
            // MDAP2 — Move Direct Absolute Point (with rounding if P touched)
            0x2F => {
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
            // FLIPON / FLIPOFF
            0x4D => {
                self.gs.auto_flip = true;
                Ok(1)
            }
            0x4E => {
                self.gs.auto_flip = false;
                Ok(1)
            }
            // DEBUG
            0x4F => {
                let _ = self.pop(); // consume flag
                Ok(1)
            }
            // LT
            0x50 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b < a { 1 } else { 0 });
                Ok(1)
            }
            // LTEQ
            0x51 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b <= a { 1 } else { 0 });
                Ok(1)
            }
            // GT
            0x52 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b > a { 1 } else { 0 });
                Ok(1)
            }
            // GTEQ
            0x53 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b >= a { 1 } else { 0 });
                Ok(1)
            }
            // EQ
            0x54 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b == a { 1 } else { 0 });
                Ok(1)
            }
            // NEQ
            0x55 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b != a { 1 } else { 0 });
                Ok(1)
            }
            // AND
            0x56 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b != 0 && a != 0 { 1 } else { 0 });
                Ok(1)
            }
            // OR
            0x57 => {
                let a = self.pop();
                let b = self.pop();
                self.push(if b != 0 || a != 0 { 1 } else { 0 });
                Ok(1)
            }
            // NOT
            0x58 => {
                let a = self.pop();
                self.push(if a == 0 { 1 } else { 0 });
                Ok(1)
            }
            // ODD
            0x7B => {
                let a = self.pop();
                self.push(if (a >> 6) & 1 == 1 { 1 } else { 0 });
                Ok(1)
            }
            // EVEN
            0x7C => {
                let a = self.pop();
                self.push(if (a >> 6) & 1 == 0 { 1 } else { 0 });
                Ok(1)
            }
            // JMPR — Jump relative
            0x7A => {
                let offset = self.pop();
                self.ip += offset;
                Ok(0) // ip adjustment already applied
            }
            // JROT — Jump relative on true
            0x78 => {
                let offset = self.pop();
                let cond = self.pop();
                if cond != 0 {
                    self.ip += offset;
                    return Ok(0);
                }
                Ok(1)
            }
            // JROF — Jump relative on false
            0x79 => {
                let offset = self.pop();
                let cond = self.pop();
                if cond == 0 {
                    self.ip += offset;
                    return Ok(0);
                }
                Ok(1)
            }
            // MPS — Measure Point Size
            0x4C => {
                self.push(self.point_size);
                Ok(1)
            }
            // CINDEX — Copy Indexed value to top of stack
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
            // MINDEX — Move Indexed value to top of stack
            0x26 => {
                let idx = self.pop() as usize;
                if idx > 0 && idx <= self.top as usize {
                    let pos = (self.top as usize - 1) - (idx - 1);
                    let val = self.stack[pos];
                    // Shift elements down
                    for j in pos..(self.top as usize - 1) {
                        self.stack[j] = self.stack[j + 1];
                    }
                    self.stack[self.top as usize - 1] = val;
                }
                Ok(1)
            }
            // GETINFO
            0x88 => {
                let selector = self.pop();
                let mut result = 0i32;
                // Bit 0: set if TrueType rendering engine
                if selector & 1 != 0 {
                    result |= 35; // version 35
                }
                // Bit 1: set if grayscale
                if selector & 2 != 0 && self.grayscale {
                    result |= 2;
                }
                // Bit 2: set if ClearType subpixel
                if selector & 4 != 0 {
                    // No subpixel
                }
                // Bit 3: set if ClearType vertical
                if selector & 8 != 0 {
                    // No ClearType vertical
                }
                // Bit 4: set if GASP compatible
                if selector & 16 != 0 {
                    result |= 16;
                }
                // Bit 5: set if subpixel positioned
                if selector & 32 != 0 {
                    result |= 32;
                }
                // Bit 6: set if ClearType compatible width
                if selector & 64 != 0 {
                    // No ClearType width
                }
                self.push(result);
                Ok(1)
            }

            _ => {
                // NOOP for unimplemented opcodes
                log::trace!("[hinting] unimplemented opcode 0x{:02X}", self.opcode);
                Ok(1)
            }
        }
    }

    fn select_zone(&mut self, ptr: usize, zone_id: i32) {
        let src = if zone_id == 0 {
            &self.pts
        } else if zone_id == 1 {
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
    }

    fn get_zone(&mut self, ptr: usize) -> &mut Zone {
        match ptr {
            0 => &mut self.zp0,
            1 => &mut self.zp1,
            2 => &mut self.zp2,
            _ => &mut self.zp0,
        }
    }

    pub fn hint_glyph(
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

        self.zp0 = self.pts.clone();
        self.zp1 = self.pts.clone();
        self.zp2 = self.pts.clone();
        self.gs.rp0 = 0;
        self.gs.rp1 = 0;
        self.gs.rp2 = 0;

        let twilight_n = n.max(data.maxp.num_glyphs as u16 * 2).min(256);
        self.twilight.allocate_twilight(twilight_n);

        self.glyf_cvt.clone_from(&self.cvt);
        self.glyf_storage.clone_from(&self.storage);

        let ins = self.get_glyph_instructions(data, _glyph_index);
        if ins.is_empty() {
            self.iup(0);
            self.iup(1);
            self.copy_hinted_points_back(glyph);
            return;
        }

        self.code = ins;
        self.cur_range = CodeRange::Glyph;
        self.ip = 0;
        if let Err(e) = self.run() {
            log::warn!("[hinting] glyph {} exec error: {}", _glyph_index, e);
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
        let inst_len =
            u16::from_be_bytes([slice[end_pts_end], slice[end_pts_end + 1]]) as usize;
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
            let this = u16::from_be_bytes([data.loca_data[off], data.loca_data[off + 1]])
                as usize
                * 2;
            let next =
                u16::from_be_bytes([data.loca_data[off + 2], data.loca_data[off + 3]])
                    as usize
                    * 2;
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

    // IUP — placeholder, will be replaced in Task 5 with the full iup module
    fn iup(&mut self, _direction: u8) {}

    /// Compare two original F26Dot6 vectors for projection distance.
    #[allow(dead_code)]
    fn proj_distance(&self, a: &F26Dot6Vector, b: &F26Dot6Vector) -> i32 {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        dot_product(dx, dy, &self.gs.proj_vector)
    }
}

#[inline]
fn dot_product(dx: i32, dy: i32, vec: &F26Dot6Vector) -> i32 {
    // Vectors are in F26Dot6 but dot product normalizes by 64
    (dx as i64 * vec.x as i64 + dy as i64 * vec.y as i64) as i32 / 64
}
