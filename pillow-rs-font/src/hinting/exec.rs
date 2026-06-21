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
        // Safety limit to prevent infinite loops from buggy or malicious bytecode
        let max_ops: i32 = 65536;
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
            // RS — Read Store (use glyph-local copy)
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
            // WS — Write Store (use glyph-local copy)
            0x42 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc >= self.glyf_storage.len() {
                    self.glyf_storage.resize(loc + 64, 0);
                }
                self.glyf_storage[loc] = val;
                Ok(1)
            }
            // RCVT — Read CVT (use glyph-local copy)
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
            // GC — Get Coordinate Projected
            0x46 => {
                let p_idx = self.pop() as usize;
                let val = if p_idx < self.zp2.points.len() {
                    let p = self.zp2.points[p_idx];
                    dot_product(p.x, p.y, &self.gs.proj_vector)
                } else { 0 };
                self.push(val);
                Ok(1)
            }
            // WCVTP — Write CVT (pixel coords, use glyph-local copy)
            0x44 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc >= self.glyf_cvt.len() {
                    // CVT entries are fixed-size; silently ignore out-of-bounds writes
                } else {
                    self.glyf_cvt[loc] = val;
                }
                Ok(1)
            }
            // WCVTF — Write CVT (font units -> pixels, use glyph-local copy)
            0x70 => {
                let val = self.pop();
                let loc = self.pop() as usize;
                if loc < self.glyf_cvt.len() {
                    // WCVTF stores the value directly (no scaling needed, it's already
                    // in F26Dot6 -- the value was pushed as a pixel measurement)
                    self.glyf_cvt[loc] = val;
                }
                Ok(1)
            }
            // SCFS — Set Coordinate From Stack using freedom vector
            0x48 => {
                let val = self.pop();
                let p_idx = self.pop() as usize;
                if p_idx < self.zp2.points.len() {
                    let p = self.zp2.points[p_idx];
                    let cur_proj = dot_product(p.x, p.y, &self.gs.proj_vector);
                    let diff = val - cur_proj;
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * diff) >> 6;
                    let fy = (fv.y * diff) >> 6;
                    self.zp2.points[p_idx].x += fx;
                    self.zp2.points[p_idx].y += fy;
                    if fv.x != 0 { self.zp2.tags[p_idx] |= TOUCH_X; }
                    if fv.y != 0 { self.zp2.tags[p_idx] |= TOUCH_Y; }
                }
                Ok(1)
            }
            // MD — Measure Distance between two points
            0x49 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                let pp1 = if p1 < self.zp1.points.len() { self.zp1.points[p1] } else { F26Dot6Vector::new(0, 0) };
                let pp2 = if p2 < self.zp2.points.len() { self.zp2.points[p2] } else { F26Dot6Vector::new(0, 0) };
                let dist = dot_product(pp2.x - pp1.x, pp2.y - pp1.y, &self.gs.proj_vector);
                self.push(dist);
                Ok(1)
            }
            // MPPEM — Measure Pixels Per EM
            0x4B => {
                self.push(self.ppem as i32);
                Ok(1)
            }
            // IP — Interpolate Point between rp1 and rp2
            0x39 => {
                let p_idx = self.pop() as usize;
                let rp1_idx = self.gs.rp1 as usize;
                let rp2_idx = self.gs.rp2 as usize;

                if p_idx < self.zp2.points.len() && rp1_idx < self.zp0.points.len() && rp2_idx < self.zp0.points.len() {
                    let o1 = self.zp0.org[rp1_idx];
                    let o2 = self.zp0.org[rp2_idx];
                    let p1 = self.zp0.points[rp1_idx];
                    let p2 = self.zp0.points[rp2_idx];
                    let pp = self.zp2.points[p_idx];
                    let pp_org = self.zp2.org[p_idx];

                    let org_dist = dot_product(o2.x - o1.x, o2.y - o1.y, &self.gs.proj_vector);
                    let cur_dist = dot_product(p2.x - p1.x, p2.y - p1.y, &self.gs.proj_vector);

                    if org_dist != 0 {
                        let po = dot_product(pp_org.x - o1.x, pp_org.y - o1.y, &self.gs.proj_vector);
                        let p1_proj = dot_product(p1.x, p1.y, &self.gs.proj_vector);
                        let scaled = ((po as i64) * (cur_dist as i64)) / (org_dist as i64);
                        let new_pos = p1_proj + scaled as i32;

                        let cur_proj = dot_product(pp.x, pp.y, &self.gs.proj_vector);
                        let diff = new_pos - cur_proj;
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * diff) >> 6;
                        let fy = (fv.y * diff) >> 6;
                        self.zp2.points[p_idx].x += fx;
                        self.zp2.points[p_idx].y += fy;
                    }
                }
                Ok(1)
            }
            // MSIRP — Move Stack Indirect Relative to Point
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
                    let fx = (fv.x * diff) >> 6;
                    let fy = (fv.y * diff) >> 6;
                    p.x += fx;
                    p.y += fy;
                }
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            // ALIGNRP — Align Point to RP0
            0x3C => {
                let p_idx = self.pop() as usize;
                let rp_idx = self.gs.rp0 as usize;
                if p_idx < self.zp2.points.len() && rp_idx < self.zp0.points.len() {
                    let rp = self.zp0.points[rp_idx];
                    let p = &mut self.zp2.points[p_idx];
                    let dx = rp.x - p.x;
                    let dy = rp.y - p.y;
                    let dist = dot_product(dx, dy, &self.gs.proj_vector);
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * dist) >> 6;
                    let fy = (fv.y * dist) >> 6;
                    p.x += fx;
                    p.y += fy;
                    self.zp2.tags[p_idx] |= TOUCH_X | TOUCH_Y;
                }
                Ok(1)
            }
            // RTDG — Round To Double Grid
            0x3D => {
                self.gs.round_state = 2;
                self.round_fn = round::round_to_double_grid;
                Ok(1)
            }
            // MIAP — Move Indirect Absolute Point
            0x3E => {
                let cvt_idx = self.pop() as usize;
                let p_idx = self.pop() as usize;
                let cvt_val = if cvt_idx < self.glyf_cvt.len() { self.glyf_cvt[cvt_idx] } else { 0 };
                if p_idx < self.zp0.points.len() {
                    let p = self.zp0.points[p_idx];
                    let cur_proj = dot_product(p.x, p.y, &self.gs.proj_vector);
                    let diff = cvt_val - cur_proj;
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * diff) >> 6;
                    let fy = (fv.y * diff) >> 6;
                    self.zp0.points[p_idx].x += fx;
                    self.zp0.points[p_idx].y += fy;
                    if fv.x != 0 { self.zp0.tags[p_idx] |= TOUCH_X; }
                    if fv.y != 0 { self.zp0.tags[p_idx] |= TOUCH_Y; }
                }
                self.gs.rp2 = self.gs.rp1;
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            // MIAP2 — Move Indirect Absolute Point (no rounding, same as MIAP for now)
            0x3F => {
                let cvt_idx = self.pop() as usize;
                let p_idx = self.pop() as usize;
                let cvt_val = if cvt_idx < self.glyf_cvt.len() { self.glyf_cvt[cvt_idx] } else { 0 };
                if p_idx < self.zp0.points.len() {
                    let p = self.zp0.points[p_idx];
                    let cur_proj = dot_product(p.x, p.y, &self.gs.proj_vector);
                    let diff = cvt_val - cur_proj;
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * diff) >> 6;
                    let fy = (fv.y * diff) >> 6;
                    self.zp0.points[p_idx].x += fx;
                    self.zp0.points[p_idx].y += fy;
                    if fv.x != 0 { self.zp0.tags[p_idx] |= TOUCH_X; }
                    if fv.y != 0 { self.zp0.tags[p_idx] |= TOUCH_Y; }
                }
                self.gs.rp2 = self.gs.rp1;
                self.gs.rp1 = self.gs.rp0;
                self.gs.rp0 = p_idx as u16;
                Ok(1)
            }
            // DELTAC1 — Delta Exception CVT 1
            0x71 => {
                let n = self.pop() as usize;
                for _ in 0..n {
                    let arg = self.pop();
                    let c_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta = arg & 0x0F;
                    let d = if delta >= 8 { ((delta as i32) - 16) << (self.gs.delta_shift as i32) }
                            else { (delta as i32) << (self.gs.delta_shift as i32) };
                    if c_idx < self.glyf_cvt.len() { self.glyf_cvt[c_idx] += d; }
                }
                Ok(1)
            }
            // DELTAC2 — Delta Exception CVT 2
            0x72 => {
                let n = self.pop() as usize;
                for _ in 0..n {
                    let arg = self.pop();
                    let c_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta = arg & 0x0F;
                    let d = if delta >= 8 { ((delta as i32) - 16) << 4 }
                            else { (delta as i32) << 4 };
                    if c_idx < self.glyf_cvt.len() { self.glyf_cvt[c_idx] += d; }
                }
                Ok(1)
            }
            // DELTAC3 — Delta Exception CVT 3
            0x73 => {
                let n = self.pop() as usize;
                for _ in 0..n {
                    let arg = self.pop();
                    let c_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta = arg & 0x0F;
                    let d = if delta >= 8 { ((delta as i32) - 16) << 5 }
                            else { (delta as i32) << 5 };
                    if c_idx < self.glyf_cvt.len() { self.glyf_cvt[c_idx] += d; }
                }
                Ok(1)
            }
            // SROUND — Set Rounding State
            // period raw bits: 0→0.5px(32), 1→1px(64), 2→2px(128), 3→4px(256)
            0x76 => {
                self.gs.round_state = 7;
                let n = self.pop();
                let raw_period = ((n >> 6) & 3) as i32;
                self.period = 32 << raw_period;
                self.phase = (n >> 4) & 3;
                self.threshold = (n >> 2) & 3;
                Ok(1)
            }
            // S45ROUND — Set Rounding State (45-degree)
            0x77 => {
                self.gs.round_state = 7;
                let n = self.pop();
                let raw_period = ((n >> 6) & 3) as i32;
                self.period = 32 << raw_period;
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
            // IF
            0x58 => {
                let cond = self.pop();
                if cond == 0 {
                    self.skip_to_else_or_eif();
                    // ip now points at ELSE or EIF, run loop will advance past it
                }
                Ok(1)
            }
            // ELSE
            0x59 => {
                self.skip_to_eif();
                Ok(1)
            }
            // EIF
            0x5A => {
                Ok(1)
            }
            // DELTAP1 — Delta Exception Point 1
            0x5D => {
                let n = self.pop() as usize;
                for _ in 0..n {
                    let arg = self.pop();
                    let p_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta = arg & 0x0F;
                    let d = if delta >= 8 { ((delta as i32) - 16) << (self.gs.delta_shift as i32) }
                            else { (delta as i32) << (self.gs.delta_shift as i32) };
                    if p_idx < self.zp0.points.len() {
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * d) >> 6;
                        let fy = (fv.y * d) >> 6;
                        self.zp0.points[p_idx].x += fx;
                        self.zp0.points[p_idx].y += fy;
                        if fv.x != 0 { self.zp0.tags[p_idx] |= TOUCH_X; }
                        if fv.y != 0 { self.zp0.tags[p_idx] |= TOUCH_Y; }
                    }
                }
                Ok(1)
            }
            // DELTAP2 — Delta Exception Point 2
            0x5E => {
                let saved_shift = self.gs.delta_shift;
                let saved_base = self.gs.delta_base;
                self.gs.delta_shift = 4;
                self.gs.delta_base = 16;
                let n = self.pop() as usize;
                for _ in 0..n {
                    let arg = self.pop();
                    let p_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta = arg & 0x0F;
                    let d = if delta >= 8 { ((delta as i32) - 16) << (self.gs.delta_shift as i32) }
                            else { (delta as i32) << (self.gs.delta_shift as i32) };
                    if p_idx < self.zp0.points.len() {
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * d) >> 6;
                        let fy = (fv.y * d) >> 6;
                        self.zp0.points[p_idx].x += fx;
                        self.zp0.points[p_idx].y += fy;
                        if fv.x != 0 { self.zp0.tags[p_idx] |= TOUCH_X; }
                        if fv.y != 0 { self.zp0.tags[p_idx] |= TOUCH_Y; }
                    }
                }
                self.gs.delta_shift = saved_shift;
                self.gs.delta_base = saved_base;
                Ok(1)
            }
            // DELTAP3 — Delta Exception Point 3
            0x5F => {
                let saved_shift = self.gs.delta_shift;
                let saved_base = self.gs.delta_base;
                self.gs.delta_shift = 5;
                self.gs.delta_base = 17;
                let n = self.pop() as usize;
                for _ in 0..n {
                    let arg = self.pop();
                    let p_idx = ((arg >> 4) & 0xFF) as usize;
                    let delta = arg & 0x0F;
                    let d = if delta >= 8 { ((delta as i32) - 16) << (self.gs.delta_shift as i32) }
                            else { (delta as i32) << (self.gs.delta_shift as i32) };
                    if p_idx < self.zp0.points.len() {
                        let fv = self.gs.free_vector;
                        let fx = (fv.x * d) >> 6;
                        let fy = (fv.y * d) >> 6;
                        self.zp0.points[p_idx].x += fx;
                        self.zp0.points[p_idx].y += fy;
                        if fv.x != 0 { self.zp0.tags[p_idx] |= TOUCH_X; }
                        if fv.y != 0 { self.zp0.tags[p_idx] |= TOUCH_Y; }
                    }
                }
                self.gs.delta_shift = saved_shift;
                self.gs.delta_base = saved_base;
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

            // SVTCA — set vectors to coordinate axis
            0x00 => {
                let axis = self.pop();
                self.gs.proj_vector = if axis != 0 {
                    F26Dot6Vector::new(1 << 6, 0)
                } else {
                    F26Dot6Vector::new(0, 1 << 6)
                };
                self.gs.free_vector = self.gs.proj_vector;
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // SPVTCA — set projection vector to coordinate axis
            0x02 => {
                let axis = self.pop();
                self.gs.proj_vector = if axis != 0 {
                    F26Dot6Vector::new(1 << 6, 0)
                } else {
                    F26Dot6Vector::new(0, 1 << 6)
                };
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // SFVTCA — set freedom vector to coordinate axis
            0x04 => {
                let axis = self.pop();
                self.gs.free_vector = if axis != 0 {
                    F26Dot6Vector::new(1 << 6, 0)
                } else {
                    F26Dot6Vector::new(0, 1 << 6)
                };
                Ok(1)
            }
            // SPVFS — set projection vector from stack
            0x08 => {
                let y = self.pop();
                let x = self.pop();
                self.gs.proj_vector = F26Dot6Vector::new(x, y);
                self.gs.dual_vector = self.gs.proj_vector;
                Ok(1)
            }
            // ISECT — Intersection of lines
            0x0F => {
                let a2 = self.pop() as usize;
                let a1 = self.pop() as usize;
                let b2 = self.pop() as usize;
                let b1 = self.pop() as usize;
                let p_idx = self.pop() as usize;
                let p_a1 = if a1 < self.zp0.points.len() { self.zp0.points[a1] } else { F26Dot6Vector::new(0, 0) };
                let p_a2 = if a2 < self.zp0.points.len() { self.zp0.points[a2] } else { F26Dot6Vector::new(0, 0) };
                let p_b1 = if b1 < self.zp1.points.len() { self.zp1.points[b1] } else { F26Dot6Vector::new(0, 0) };
                let p_b2 = if b2 < self.zp1.points.len() { self.zp1.points[b2] } else { F26Dot6Vector::new(0, 0) };
                if p_idx < self.zp2.points.len() {
                    let a_dx = p_a2.x - p_a1.x;
                    let a_dy = p_a2.y - p_a1.y;
                    let b_dx = p_b2.x - p_b1.x;
                    let b_dy = p_b2.y - p_b1.y;
                    let denom = a_dx * b_dy - a_dy * b_dx;
                    if denom != 0 {
                        let t = (p_b1.x - p_a1.x) * b_dy - (p_b1.y - p_a1.y) * b_dx;
                        let intersection_x = p_a1.x + (a_dx * t) / denom;
                        let intersection_y = p_a1.y + (a_dy * t) / denom;
                        self.zp2.points[p_idx] = F26Dot6Vector::new(intersection_x, intersection_y);
                    }
                }
                Ok(1)
            }
            // RTG — Round To Grid
            0x1C => {
                self.gs.round_state = 1;
                self.round_fn = round::round_to_grid;
                Ok(1)
            }
            // RDTG — Round Down To Grid
            0x1D => {
                self.gs.round_state = 3;
                self.round_fn = round::round_down_to_grid;
                Ok(1)
            }
            // RUTG — Round Up To Grid
            0x1E => {
                self.gs.round_state = 4;
                self.round_fn = round::round_up_to_grid;
                Ok(1)
            }
            // ROFF — Round Off
            0x1F => {
                self.gs.round_state = 5;
                self.round_fn = round::round_off;
                Ok(1)
            }
            // ALIGNPTS — align two points
            0x27 => {
                let p2 = self.pop() as usize;
                let p1 = self.pop() as usize;
                if p1 < self.zp1.points.len() && p2 < self.zp2.points.len() {
                    let dist = (self.gs.proj_vector.x
                        * (self.zp2.points[p2].x - self.zp1.points[p1].x)
                        + self.gs.proj_vector.y
                            * (self.zp2.points[p2].y - self.zp1.points[p1].y))
                        >> 6;
                    let half = dist / 2;
                    let fv = self.gs.free_vector;
                    let dx = (fv.x * half) >> 6;
                    let dy = (fv.y * half) >> 6;
                    self.zp1.points[p1].x += dx;
                    self.zp1.points[p1].y += dy;
                    self.zp2.points[p2].x -= dx;
                    self.zp2.points[p2].y -= dy;
                }
                Ok(1)
            }
            // SHP — shift point by last point
            0x32 => {
                let p_idx = self.pop() as usize;
                let last_rp = self.gs.rp1 as usize;
                if p_idx < self.zp2.points.len() && last_rp < self.zp0.points.len() {
                    let delta_x = self.zp0.points[last_rp].x - self.zp0.org[last_rp].x;
                    let delta_y = self.zp0.points[last_rp].y - self.zp0.org[last_rp].y;
                    let proj_delta = dot_product(delta_x, delta_y, &self.gs.proj_vector);
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * proj_delta) >> 6;
                    let fy = (fv.y * proj_delta) >> 6;
                    self.zp2.points[p_idx].x += fx;
                    self.zp2.points[p_idx].y += fy;
                    if fv.x != 0 { self.zp2.tags[p_idx] |= TOUCH_X; }
                    if fv.y != 0 { self.zp2.tags[p_idx] |= TOUCH_Y; }
                }
                Ok(1)
            }
            // SHC — Shift Contour
            0x34 => {
                let c_idx = self.pop() as usize;
                let last_rp = self.gs.rp1 as usize;
                if last_rp < self.zp0.points.len() {
                    let delta_x = self.zp0.points[last_rp].x - self.zp0.org[last_rp].x;
                    let delta_y = self.zp0.points[last_rp].y - self.zp0.org[last_rp].y;
                    let proj_delta = dot_product(delta_x, delta_y, &self.gs.proj_vector);
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * proj_delta) >> 6;
                    let fy = (fv.y * proj_delta) >> 6;

                    let mut start = 0usize;
                    for (ci, &end) in self.pts.contours.iter().enumerate() {
                        if ci == c_idx {
                            for p in start..=end as usize {
                                if p < self.zp2.points.len() {
                                    self.zp2.points[p].x += fx;
                                    self.zp2.points[p].y += fy;
                                    if fv.x != 0 { self.zp2.tags[p] |= TOUCH_X; }
                                    if fv.y != 0 { self.zp2.tags[p] |= TOUCH_Y; }
                                }
                            }
                            break;
                        }
                        start = end as usize + 1;
                    }
                }
                Ok(1)
            }
            // SHZ — Shift Zone
            0x36 => {
                let z = self.pop() as usize;
                let last_rp = self.gs.rp1 as usize;
                if last_rp < self.zp0.points.len() {
                    let delta_x = self.zp0.points[last_rp].x - self.zp0.org[last_rp].x;
                    let delta_y = self.zp0.points[last_rp].y - self.zp0.org[last_rp].y;
                    let proj_delta = dot_product(delta_x, delta_y, &self.gs.proj_vector);
                    let fv = self.gs.free_vector;
                    let fx = (fv.x * proj_delta) >> 6;
                    let fy = (fv.y * proj_delta) >> 6;

                    let zone = match z {
                        0 => &mut self.zp0,
                        1 => &mut self.zp1,
                        _ => &mut self.zp2,
                    };
                    for p in 0..zone.n_points as usize {
                        if p < zone.points.len() {
                            zone.points[p].x += fx;
                            zone.points[p].y += fy;
                        }
                    }
                }
                Ok(1)
            }
            // LOOPCALL
            0x2A => {
                let count = self.pop();
                let fn_idx = self.pop() as usize;
                if fn_idx < self.fdefs.len() && self.fdefs[fn_idx].active && count > 0 {
                    let def = self.fdefs[fn_idx].clone();
                    let start = def.start;
                    self.call_stack.push(CallRecord {
                        caller_range: self.cur_range as i32,
                        caller_ip: self.ip + 1,
                        cur_count: 1,
                        def,
                    });
                    self.call_depth += 1;
                    self.ip = start - 1;
                }
                Ok(0)
            }
            // CALL
            0x2B => {
                let fn_idx = self.pop() as usize;
                if fn_idx < self.fdefs.len() && self.fdefs[fn_idx].active {
                    let def = self.fdefs[fn_idx].clone();
                    let start = def.start;
                    self.call_stack.push(CallRecord {
                        caller_range: self.cur_range as i32,
                        caller_ip: self.ip + 1,
                        cur_count: 0,
                        def,
                    });
                    self.call_depth += 1;
                    self.ip = start - 1;
                }
                Ok(0)
            }
            // FDEF
            0x2C => {
                let fn_idx = self.pop() as usize;
                let start = self.ip + 1;
                if fn_idx >= self.fdefs.len() {
                    self.fdefs.resize(fn_idx + 16, FnDef::default());
                }
                self.fdefs[fn_idx] = FnDef {
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
            // ENDF
            0x2D => {
                if let Some(record) = self.call_stack.last() {
                    self.cur_range = match record.caller_range {
                        1 => CodeRange::Font,
                        2 => CodeRange::Cvt,
                        3 => CodeRange::Glyph,
                        _ => CodeRange::None,
                    };
                    self.ip = record.caller_ip;
                    self.call_stack.pop();
                    self.call_depth -= 1;
                }
                Ok(0)
            }

            // MDRP — Move Direct Relative Point (32 variants)
            0xC0..=0xDF => {
                let flags = opcodes::decode_mirp_flags(self.opcode);
                self.do_mdrp(flags)
            }
            // MIRP — Move Indirect Relative Point (32 variants)
            0xE0..=0xFF => {
                let flags = opcodes::decode_mirp_flags(self.opcode);
                self.do_mirp(flags)
            }

            _ => {
                // NOOP for unimplemented opcodes
                log::trace!("[hinting] unimplemented opcode 0x{:02X}", self.opcode);
                Ok(1)
            }
        }
    }

    fn do_mdrp(&mut self, flags: opcodes::MirpFlags) -> Result<i32, FontError> {
        let p_idx = self.pop() as usize;
        let rp_idx = self.gs.rp0 as usize;

        // Get points from zp2 and zp0
        let (p, rp) = if p_idx < self.zp2.points.len() && rp_idx < self.zp0.points.len() {
            (self.zp2.points[p_idx], self.zp0.points[rp_idx])
        } else {
            self.gs.rp2 = self.gs.rp1;
            self.gs.rp1 = self.gs.rp0;
            self.gs.rp0 = p_idx as u16;
            return Ok(1);
        };

        let dx = p.x - rp.x;
        let dy = p.y - rp.y;
        let original_distance = dot_product(dx, dy, &self.gs.proj_vector);

        // Apply minimum distance
        let distance = if original_distance.abs() < self.gs.minimum_distance {
            if original_distance >= 0 { self.gs.minimum_distance } else { -self.gs.minimum_distance }
        } else {
            original_distance
        };

        // Round
        let rounded = if flags.round {
            self.round_distance(distance, self.gs.compensation[0])
        } else {
            distance
        };

        // Update reference points
        self.gs.rp2 = self.gs.rp1;
        self.gs.rp1 = self.gs.rp0;
        self.gs.rp0 = p_idx as u16;

        // Move point along freedom vector
        let move_dist = rounded - original_distance;
        if move_dist != 0 {
            let fv = self.gs.free_vector;
            let fx = (fv.x * move_dist) >> 6;
            let fy = (fv.y * move_dist) >> 6;
            self.zp2.points[p_idx].x += fx;
            self.zp2.points[p_idx].y += fy;
            if fv.x != 0 { self.zp2.tags[p_idx] |= TOUCH_X; }
            if fv.y != 0 { self.zp2.tags[p_idx] |= TOUCH_Y; }
        }

        Ok(1)
    }

    fn do_mirp(&mut self, flags: opcodes::MirpFlags) -> Result<i32, FontError> {
        let cvt_idx = self.pop() as usize;
        let p_idx = self.pop() as usize;
        let rp_idx = self.gs.rp0 as usize;

        let (p, rp) = if p_idx < self.zp2.points.len() && rp_idx < self.zp0.points.len() {
            (self.zp2.points[p_idx], self.zp0.points[rp_idx])
        } else {
            self.gs.rp2 = self.gs.rp1;
            self.gs.rp1 = self.gs.rp0;
            self.gs.rp0 = p_idx as u16;
            return Ok(1);
        };

        let dx = p.x - rp.x;
        let dy = p.y - rp.y;
        let original_distance = dot_product(dx, dy, &self.gs.proj_vector);

        // CVT distance
        let cvt_val = if cvt_idx < self.glyf_cvt.len() { self.glyf_cvt[cvt_idx] } else { 0 };

        // Apply cut-in logic
        let distance = self.apply_cut_in(original_distance, cvt_val);

        // Apply minimum distance
        let clamped = if distance.abs() < self.gs.minimum_distance {
            if distance >= 0 { self.gs.minimum_distance } else { -self.gs.minimum_distance }
        } else {
            distance
        };

        // Round
        let rounded = if flags.round {
            self.round_distance(clamped, self.gs.compensation[0])
        } else {
            clamped
        };

        // Update reference points
        self.gs.rp2 = self.gs.rp1;
        self.gs.rp1 = self.gs.rp0;
        self.gs.rp0 = p_idx as u16;

        // Move point along freedom vector
        let move_dist = rounded - original_distance;
        if move_dist != 0 {
            let fv = self.gs.free_vector;
            let fx = (fv.x * move_dist) >> 6;
            let fy = (fv.y * move_dist) >> 6;
            self.zp2.points[p_idx].x += fx;
            self.zp2.points[p_idx].y += fy;
            if fv.x != 0 { self.zp2.tags[p_idx] |= TOUCH_X; }
            if fv.y != 0 { self.zp2.tags[p_idx] |= TOUCH_Y; }
        }

        Ok(1)
    }

    fn apply_cut_in(&self, original: i32, cvt_val: i32) -> i32 {
        let diff = (original - cvt_val).abs();
        if diff > self.gs.single_width_cut_in {
            if original.abs() < self.gs.single_width_value.abs() {
                return original;
            }
            if diff > self.gs.control_value_cut_in {
                return original;
            }
            cvt_val
        } else {
            original
        }
    }

    /// Dispatch rounding based on round_state.
    fn round_distance(&self, distance: i32, compensation: i32) -> i32 {
        match self.gs.round_state {
            1 => round::round_to_grid(distance, compensation),
            2 => round::round_to_double_grid(distance, compensation),
            3 => round::round_down_to_grid(distance, compensation),
            4 => round::round_up_to_grid(distance, compensation),
            5 => round::round_off(distance, compensation),
            7 => self.round_super_impl(distance, compensation),
            _ => round::round_to_grid(distance, compensation),
        }
    }

    /// Super rounding (SROUND/S45ROUND) using self.period/self.phase/self.threshold.
    fn round_super_impl(&self, distance: i32, _compensation: i32) -> i32 {
        let val = distance;
        if val >= 0 {
            let r = if self.threshold > 0 {
                if val % self.period < self.threshold {
                    (val / self.period) * self.period + self.phase
                } else {
                    ((val + self.period - 1) / self.period) * self.period + self.phase
                }
            } else {
                ((val + self.period / 2) / self.period) * self.period + self.phase
            };
            r - val
        } else {
            let abs_val = -val;
            let r = if self.threshold > 0 {
                if abs_val % self.period < self.threshold {
                    (abs_val / self.period) * self.period + self.phase
                } else {
                    ((abs_val + self.period - 1) / self.period) * self.period + self.phase
                }
            } else {
                ((abs_val + self.period / 2) / self.period) * self.period + self.phase
            };
            -(r - abs_val)
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
    /// Handles nested IF/EIF blocks.
    fn skip_to_else_or_eif(&mut self) {
        let mut depth = 1;
        let mut i = self.ip as usize + 1;
        while i < self.code.len() && depth > 0 {
            match self.code[i] {
                0x58 => depth += 1, // nested IF
                0x59 => {
                    if depth == 1 {
                        break;
                    }
                }
                0x5A => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        // Position ip at the ELSE or EIF (the run loop will add 1)
        self.ip = (i - 1) as i32;
    }

    /// Skip to EIF when ELSE branch is done.
    fn skip_to_eif(&mut self) {
        let mut depth = 1;
        let mut i = self.ip as usize + 1;
        while i < self.code.len() && depth > 0 {
            match self.code[i] {
                0x58 => depth += 1,
                0x5A => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        // Position ip at EIF (the run loop will add 1)
        self.ip = (i - 1) as i32;
    }
}

#[inline]
fn dot_product(dx: i32, dy: i32, vec: &F26Dot6Vector) -> i32 {
    // Vectors are in F26Dot6 but dot product normalizes by 64
    (dx as i64 * vec.x as i64 + dy as i64 * vec.y as i64) as i32 / 64
}
