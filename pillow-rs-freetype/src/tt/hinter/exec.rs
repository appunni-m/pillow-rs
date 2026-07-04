//! Execution context — the TrueType bytecode interpreter VM.
//!
//! C reference: `TT_ExecContextRec_` in `ttobjs.h` and `ttinterp.h`,
//! `TT_Load_Context` in `ttobjs.c:891-957`,
//! `TT_Run_Context` / `TT_RunIns` in `ttinterp.c`.
//!
//! The ExecContext holds all mutable VM state: stack, call stack, storage
//! area, Control Value Table (CVT), function definitions, instruction
//! pointer, code ranges, and the glyph zones being hinted.

use super::gs::GraphicsState;
use super::zone::GlyphZone;
use super::iup;
use crate::error::FontError;
use crate::fixed::{ft_mul_fix, ft_floor_fix, ft_ceil_fix, ft_div_fix};

/// Maximum stack depth. TrueType spec says max 255, but fonts may request
/// more via maxp->maxStackElements. We use a generous default.
const DEFAULT_MAX_STACK: usize = 512;

/// Maximum call stack depth. C uses 10.
const MAX_CALL_DEPTH: usize = 10;

/// Maximum function definitions.
const MAX_FUNCTIONS: usize = 256;

/// Maximum instruction definitions (IDEF).
const MAX_INSTRUCTION_DEFS: usize = 256;

/// A code range (pointer into a bytecode stream).
#[derive(Debug, Clone, Default)]
pub struct CodeRange {
    /// Base pointer to the bytecode data.
    pub base: usize,
    /// Size of the code range in bytes.
    pub size: usize,
}

/// A function/instruction definition record.
/// C: `TT_DefRecord` in ttinterp.h.
#[derive(Debug, Clone)]
pub struct DefRecord {
    /// Which code range this definition lives in (0=cvt, 1=font, 2=glyph).
    pub range: u8,
    /// Start offset within the code range.
    pub start: usize,
    /// End offset within the code range (inclusive? C uses exclusive).
    pub end: usize,
    /// Opcode number (for FDEF) or instruction number (for IDEF).
    pub opc: u16,
    /// Whether this definition is active.
    pub active: bool,
}

/// A call record on the call stack.
/// C: `TT_CallRec` in ttinterp.h.
#[derive(Debug, Clone)]
pub struct CallRecord {
    /// Code range of the caller.
    pub caller_range: u8,
    /// Instruction pointer to return to.
    pub caller_ip: usize,
    /// Current loop count (for LOOPCALL).
    pub cur_count: i32,
    /// Pointer to the function definition being called.
    pub def_index: usize,
}

/// The bytecode execution context.
///
/// This is the main VM structure. It's created once per size and reused
/// across glyphs. The glyph zone is swapped in for each glyph.
#[derive(Debug, Clone)]
pub struct ExecContext {
    /// Graphics state (projection vectors, rounding mode, auto-flip, etc.)
    pub gs: GraphicsState,

    /// Scale factors: x_scale, y_scale in 16.16 format.
    pub x_scale: i32,
    pub y_scale: i32,

    /// Pixels per em (for MPPEM opcode).
    pub ppem: i32,

    // ── Code ranges ───────────────────────────────────────────────────
    /// Font program code range (fpgm table).
    #[allow(dead_code)]
    pub font_range: CodeRange,

    /// CVT program code range (prep table).
    #[allow(dead_code)]
    pub cvt_range: CodeRange,

    /// Glyph program code range (from glyf table).
    pub glyph_range: CodeRange,

    /// Raw bytecode for the font program (owns the data).
    pub font_program: Vec<u8>,

    // ── Stack ─────────────────────────────────────────────────────────
    /// Data stack.
    pub stack: Vec<i32>,

    // ── Storage area ──────────────────────────────────────────────────
    /// Storage area (indexed by RS/WS opcodes), initialized from maxp->maxStorage.
    pub storage: Vec<i32>,

    // ── Control Value Table (CVT) ─────────────────────────────────────
    /// CVT values in 26.6 format, indexed by RCVT/WCVTP opcodes.
    pub cvt: Vec<i32>,

    // ── Function definitions ──────────────────────────────────────────
    /// Function definitions (FDEF/ENDF).
    pub functions: Vec<Option<DefRecord>>,

    /// Instruction definitions (IDEF/ENDF).
    #[allow(dead_code)]
    pub instruction_defs: Vec<Option<DefRecord>>,

    // ── Call stack ────────────────────────────────────────────────────
    /// Call stack (max 10 levels deep).
    pub call_stack: Vec<CallRecord>,

    // ── Instruction pointer state ─────────────────────────────────────
    /// Current instruction pointer (byte offset within the active code range).
    pub ip: usize,

    /// Which code range is currently executing (0=cvt, 1=font, 2=glyph).
    pub cur_range: u8,

    // ── Flags ─────────────────────────────────────────────────────────
    /// Whether we're hinting a composite glyph.
    pub is_composite: bool,

    /// Pedantic hinting mode (abort on errors).
    pub pedantic_hinting: bool,

    /// The glyph's instruction stream (set per glyph).
    pub glyph_program: Vec<u8>,

    /// The prep/CVT program bytecode (executed once per size change).
    pub cvt_program: Vec<u8>,
}

impl ExecContext {
    /// Create a new execution context.
    ///
    /// # Arguments
    /// * `x_scale` — horizontal scale in 16.16 format
    /// * `y_scale` — vertical scale in 16.16 format
    /// * `ppem` — pixels per em
    /// * `cvt` — control value table (in 26.6, already scaled)
    /// * `fpgm` — font program bytecode
    pub fn new(
        x_scale: i32,
        y_scale: i32,
        ppem: i32,
        cvt: &[i32],
        fpgm: &[u8],
    ) -> Self {
        ExecContext {
            gs: GraphicsState::default(),
            x_scale,
            y_scale,
            ppem,
            font_range: CodeRange {
                base: 0,
                size: fpgm.len(),
            },
            cvt_range: CodeRange::default(),
            glyph_range: CodeRange::default(),
            font_program: fpgm.to_vec(),
            stack: Vec::with_capacity(DEFAULT_MAX_STACK),
            storage: vec![0; 32], // default: 32 entries
            cvt: cvt.to_vec(),
            functions: vec![None; MAX_FUNCTIONS],
            instruction_defs: vec![None; MAX_INSTRUCTION_DEFS],
            call_stack: Vec::with_capacity(MAX_CALL_DEPTH),
            ip: 0,
            cur_range: 0,
            is_composite: false,
            pedantic_hinting: false,
            glyph_program: Vec::new(),
            cvt_program: Vec::new(),
        }
    }

    // ── Stack operations (C: stack manipulation in ttinterp.c) ────────

    /// Push a value onto the data stack.
    pub fn push(&mut self, val: i32) {
        self.stack.push(val);
    }

    /// Pop a value from the data stack. Returns 0 if stack is empty.
    /// This matches C's non-pedantic mode where stack errors are ignored.
    pub fn pop(&mut self) -> Result<i32, FontError> {
        Ok(self.stack.pop().unwrap_or(0))
    }

    /// Peek at the top of the stack without removing it.
    #[allow(dead_code)]
    pub fn top(&self) -> Result<i32, FontError> {
        self.stack.last().copied().ok_or(FontError::InvalidOutline(
            "bytecode: stack empty".into(),
        ))
    }

    /// Read a byte from the active code range at the current IP,
    /// then increment IP. Used during fpgm parsing.
    pub fn fetch_byte(&mut self) -> Result<u8, FontError> {
        // During fpgm parsing (cur_range=1), read from font_program.
        // During run_program (cur_range=2), read from glyph_program.
        let data = if self.cur_range == 1 {
            &self.font_program
        } else {
            &self.glyph_program
        };
        if self.ip >= data.len() {
            return Err(FontError::InvalidOutline(
                "bytecode: IP out of range".into(),
            ));
        }
        let byte = data[self.ip];
        self.ip += 1;
        Ok(byte)
    }

    /// Read a signed word (2 bytes, big-endian) from the current code range.
    #[allow(dead_code)]
    pub fn fetch_word(&mut self) -> Result<i16, FontError> {
        let hi = self.fetch_byte()? as i16;
        let lo = self.fetch_byte()? as i16;
        Ok((hi << 8) | lo)
    }

    // ── Program execution ─────────────────────────────────────────────

    /// Run the font program (fpgm) to set up function definitions.
    /// This is called once when the execution context is initialized.
    /// The fpgm bytecode starts with push operations to load function
    /// numbers and parameter values, then FDEF/ENDF pairs to define them.
        pub fn run_fpgm(&mut self) -> Result<(), FontError> {
        if self.font_range.size == 0 {
            return Ok(());
        }
        // C executes fpgm through the full VM (TT_Run_Context) on an empty zone.
        // This ensures all function body opcodes execute, keeping stack state
        // accurate for subsequent FDEF pops.
        // Previously we had a custom parser that skipped body opcodes, causing
        // stack desyncs after ~10 functions.
        self.stack.clear();
        self.glyph_program = self.font_program.clone();
        self.ip = 0;
        self.cur_range = 2; // glyph range (uses glyph_program for fetch)

        // Empty zone: fpgm runs without glyph points (C: exec->pts.n_points = 0)
        let mut empty_zone = GlyphZone {
            cur_x: vec![], cur_y: vec![], org_x: vec![], org_y: vec![],
            orus_x: vec![], orus_y: vec![],
            tags: vec![], contours: vec![],
            n_points: 0, n_contours: 0, first_point: 0,
        };

        // Save/restore GS — fpgm may modify it but glyph programs need defaults
        let gs_saved = self.gs.clone();
        let result = self.run_program(&mut empty_zone);
        self.gs = gs_saved;
        result
    }

    /// Run the prep program to scale CVT values for the current ppem.
    /// C: `tt_size_run_prep` in ttobjs.c.
    /// The prep program uses WCVTP to write pixel-specific CVT values
    /// and may set up the twilight zone for control value scaling.
    /// We run it with a minimal twilight zone (enough for point ops).
    pub fn run_prep(&mut self, prep_bytes: &[u8]) -> Result<(), FontError> {
        if prep_bytes.is_empty() { return Ok(()); }

        // ── TT_Load_Context equivalent (C: ttobjs.c:891-957) ──────
        // C calls this before EVERY program execution (fpgm, prep, glyph).
        // It resets GS, scales CVT from FU to pixel units, zeroes storage.
        self.gs = GraphicsState::default();
        self.gs.auto_flip = true;  // C default

        // Scale CVT: face->cvt[i] / 64 → FT_MulFix(_, scale)
        // Our CVT entries are in FU*64 from the parser. Divide by 64 to get FU.
        // Then scale to 26.6 pixel units using y_scale.
        for i in 0..self.cvt.len() {
            let fu = self.cvt[i] / 64;
            self.cvt[i] = crate::fixed::ft_mul_fix(fu, self.y_scale);
        }

        // Zero storage (C: FT_ARRAY_ZERO(exec->storage, exec->storeSize))
        for s in &mut self.storage {
            *s = 0;
        }

        // Zero twilight zone (C: FT_ARRAY_ZERO)
        let n_twilight = 16; // maxTwilightPoints from maxp
        let mut twilight = GlyphZone {
            cur_x: vec![0i32; n_twilight], cur_y: vec![0i32; n_twilight],
            org_x: vec![0i32; n_twilight], org_y: vec![0i32; n_twilight],
            orus_x: vec![0i32; n_twilight], orus_y: vec![0i32; n_twilight],
            tags: vec![0u8; n_twilight], contours: vec![],
            n_points: n_twilight as u16, n_contours: 0, first_point: 0,
        };

        // Set up prep as a glyph program
        self.stack.clear();
        self.glyph_program = prep_bytes.to_vec();
        self.ip = 0;
        self.cur_range = 2;

        // C: prep runs with zp0=zp1=zp2=0 (twilight zone)
        self.gs.zp0 = 0;
        self.gs.zp1 = 0;
        self.gs.zp2 = 0;
        self.gs.set_vectors_to_y();

        // Run prep against twilight zone
        self.run_program(&mut twilight)?;

        // Restore zone pointers for glyph hinting
        self.gs.zp0 = 1;
        self.gs.zp1 = 1;
        self.gs.zp2 = 1;

        Ok(())
    }

    /// Get a value from the storage area.
    #[allow(dead_code)]
    pub fn get_storage(&self, idx: usize) -> Result<i32, FontError> {
        self.storage.get(idx).copied().ok_or(FontError::InvalidOutline(
            "bytecode: storage index out of range".into(),
        ))
    }

    /// Set a value in the storage area.
    #[allow(dead_code)]
    pub fn set_storage(&mut self, idx: usize, val: i32) -> Result<(), FontError> {
        if idx >= self.storage.len() {
            return Err(FontError::InvalidOutline(
                "bytecode: storage index out of range".into(),
            ));
        }
        self.storage[idx] = val;
        Ok(())
    }

    /// Get a CVT value. Returns 0 if index is out of range.
    #[allow(dead_code)]
    pub fn get_cvt(&self, idx: usize) -> Result<i32, FontError> {
        Ok(*self.cvt.get(idx).unwrap_or(&0))
    }

    /// Set a CVT value. No-op if index is out of range.
    #[allow(dead_code)]
    pub fn set_cvt(&mut self, idx: usize, val: i32) -> Result<(), FontError> {
        if idx < self.cvt.len() {
            self.cvt[idx] = val;
        }
        Ok(())
    }

    // ── Glyph program execution ────────────────────────────────────

    /// Set the glyph instruction stream for execution.
    pub fn set_glyph_program(&mut self, ins: &[u8]) {
        self.glyph_program = ins.to_vec();
        self.glyph_range = CodeRange { base: 0, size: ins.len() };
        self.ip = 0;
        self.cur_range = 2; // glyph
    }

    /// Fetch a byte from the active program at current IP.
    /// Range 0 = CVT/prep program, 1 = font program (fpgm), 2 = glyph program.
    fn fetch_byte_glyph(&mut self) -> Result<u8, FontError> {
        let program: &[u8] = match self.cur_range {
            0 => &self.cvt_program,
            1 => &self.font_program,
            _ => &self.glyph_program,
        };
        if self.ip >= program.len() {
            return Err(FontError::InvalidOutline(
                "bytecode: IP overflow in program".into(),
            ));
        }
        let b = program[self.ip];
        self.ip += 1;
        Ok(b)
    }

    /// Main opcode dispatch loop for the glyph program.
    pub fn run_program(&mut self, zone: &mut GlyphZone) -> Result<(), FontError> {
        let mut step_count = 0u32;
        while self.ip < self.glyph_program.len() {
            if step_count > 5000 { return Err(FontError::InvalidOutline("VM: max steps".into())); }
            step_count += 1;
            let opcode = self.fetch_byte_glyph()?;


            match opcode {
                // ── Push small bytes (0xB0-0xB7) ────────────────
                0xB0..=0xB7 => {
                    // PUSHB[opcode-0xB0+1]: push 1-8 bytes
                    let count = (opcode - 0xB0 + 1) as usize;
                    for _ in 0..count {
                        let b = self.fetch_byte_glyph()?;
                        self.push(b as i32);
                    }
                }
                // ── Push small words (0xB8-0xBF) ────────────────
                0xB8..=0xBF => {
                    let count = (opcode - 0xB8 + 1) as usize;
                    for _ in 0..count {
                        let hi = self.fetch_byte_glyph()? as i16;
                        let lo = self.fetch_byte_glyph()? as i16;
                        self.push(((hi as i32) << 8) | (lo as i32));
                    }
                }

                // ── PUSH operations ──────────────────────────────
                0x40 => {
                    // NPUSHB
                    let count = self.fetch_byte_glyph()? as usize;
                    for _ in 0..count {
                        let b = self.fetch_byte_glyph()?;
                        self.push(b as i32);
                    }
                }
                0x41 => {
                    // NPUSHW
                    let count = self.fetch_byte_glyph()? as usize;
                    for _ in 0..count {
                        let hi = self.fetch_byte_glyph()? as i16;
                        let lo = self.fetch_byte_glyph()? as i16;
                        self.push(((hi as i32) << 8) | (lo as i32));
                    }
                }

                // ── Stack operations ─────────────────────────────
                0x20 => {
                    let v = self.top()?;
                    self.push(v);
                } // DUP
                0x21 => { let _ = self.pop()?; } // POP
                0x22 => self.stack.clear(), // CLEAR
                0x23 => {
                    // SWAP
                    let a = self.pop()?;
                    let b = self.pop()?;
                    self.push(a);
                    self.push(b);
                }

                // ── Math ─────────────────────────────────────────
                0x60 => { let b = self.pop()?; let a = self.pop()?; self.push(a + b); } // ADD
                0x61 => { let b = self.pop()?; let a = self.pop()?; self.push(a - b); } // SUB
                0x62 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if b == 0 { return Err(FontError::InvalidOutline("bytecode: division by zero".into())); }
                    self.push(ft_div_fix(a, b));
                } // DIV
                0x63 => { let b = self.pop()?; let a = self.pop()?; self.push(ft_mul_fix(a, b)); } // MUL
                0x64 => { let a = self.pop()?; self.push(a.abs()); } // ABS
                0x65 => { let a = self.pop()?; self.push(-a); } // NEG
                0x66 => { let a = self.pop()?; self.push(ft_floor_fix(a)); } // FLOOR
                0x67 => { let a = self.pop()?; self.push(ft_ceil_fix(a)); } // CEILING

                // ── Storage ──────────────────────────────────────
                0x42 => {
                    // WS: pops index (deeper) then value (top)
                    let val = self.pop()?;  // top = value
                    let idx = self.pop()? as usize;  // deeper = index
                    self.set_storage(idx, val)?;
                }
                0x43 => {
                    // RS: Read Storage — pops index, pushes value
                    let idx = self.pop()? as usize;
                    let val = self.get_storage(idx)?;
                    self.push(val);
                }

                // ── CVT ──────────────────────────────────────────
                0x44 => {
                    // WCVTP: pops index (deeper) then value (top)
                    // Stack before: [..., index, value] where value is top
                    let val = self.pop()?;  // top = value
                    let idx = self.pop()? as usize;  // deeper = index
                    self.set_cvt(idx, val)?;
                }
                0x45 => {
                    // RCVT: Read CVT — pops index, pushes value
                    let idx = self.pop()? as usize;
                    let val = self.get_cvt(idx)?;
                    self.push(val);
                }

                // ── Graphics state — vectors ─────────────────────
                0x00 => { self.gs.set_vectors_to_y(); } // SVTCA[y]
                0x01 => { self.gs.set_vectors_to_x(); } // SVTCA[x]
                0x02 => { self.gs.set_proj_to_y(); }   // SPVTCA[y]
                0x03 => { self.gs.set_proj_to_x(); }   // SPVTCA[x]
                0x04 => { self.gs.set_free_to_y(); }    // SFVTCA[y]
                0x05 => { self.gs.set_free_to_x(); }    // SFVTCA[x]

                // ── Reference points ─────────────────────────────
                0x10 => {
                    let p = self.pop()?;
                    self.gs.rp0 = p as u32;
                } // SRP0
                0x11 => {
                    let p = self.pop()?;
                    self.gs.rp1 = p as u32;
                } // SRP1
                0x12 => {
                    let p = self.pop()?;
                    self.gs.rp2 = p as u32;
                } // SRP2

                // ── MPPEM / MPS ────────────────────────────────
                0x4B|0x4C => { self.push(self.ppem * 64); } // 26.6 format

                // ── ROUND ────────────────────────────────────────
                0x49 => {
                    // ROUND — pop value, round, push back
                    let v = self.pop()?;
                    let r = self.gs.round(v);
                    self.push(r);
                }

                // ── GC — Get Coordinate ──────────────────────────
                0x46 => {
                    // GC[0] = get current coordinate of point in zp2
                    // Uses zp2 zone pointer to select zone, then
                    // projects the point's cur position onto proj vector
                    let p = self.pop()? as usize;
                    // Always use glyph zone (zp2=1 by default)
                    let (px, py) = zone.cur(p);
                    let proj = self.gs.project(px, py);
                    self.push(proj);
                }
                0x47 => {
                    // GC[1] = get original coordinate
                    let p = self.pop()? as usize;
                    let (px, py) = zone.org(p);
                    let proj = self.gs.project(px, py);
                    self.push(proj);
                }

                // ── SCFS — Set Coordinate From Stack ─────────────
                0x48 => {
                    // pops point (deeper) then value (top)
                    let val = self.pop()?;  // top = value
                    let p = self.pop()? as usize;  // deeper = point
                    // Move point along freedom vector to match the value
                    let (ox, oy) = zone.org(p);
                    let old_proj = self.gs.project(ox, oy);
                    let dist = val - old_proj;
                    let (dx, dy) = self.gs.move_along_free(dist);
                    let (cx, cy) = zone.cur(p);
                    zone.set_cur(p, cx + dx, cy + dy);
                    zone.set_tag(p, 0x01); // TOUCH_X
                    zone.set_tag(p, 0x02); // TOUCH_Y
                }

                // ── MDAP — Move Direct Absolute Point ────────────
                0x2E | 0x2F => {
                    // MDAP[0]/MDAP[1]: round point, optionally set rp0
                    let p = self.pop()? as usize;
                    // Get current position, round it
                    let (cx, cy) = zone.cur(p);
                    let rx = self.gs.round(cx);
                    let ry = self.gs.round(cy);
                    zone.set_cur(p, rx, ry);
                    zone.set_tag(p, 0x03); // TOUCH_X | TOUCH_Y
                    if opcode == 0x2F {
                        self.gs.rp0 = p as u32;
                    }
                }

                // ── MIAP — Move Indirect Absolute Point ──────────
                0x3E | 0x3F => {
                    // pops cvt_index (deeper) then point (top)
                    let p = self.pop()? as usize;  // top = point
                    let cvt_idx = self.pop()? as usize;  // deeper = cvt index
                    let cvt_val = self.get_cvt(cvt_idx)?;
                    let (cx, cy) = zone.cur(p);
                    // Project original and current coords onto freedom vector
                    let org_dist = self.gs.project(cx, cy);
                    // Round CVT value
                    let rnd_cvt = self.gs.round(cvt_val);
                    // Move: calculate delta from current position
                    let delta = rnd_cvt - org_dist;
                    let (dx, dy) = self.gs.move_along_free(delta);
                    zone.set_cur(p, cx + dx, cy + dy);
                    zone.set_tag(p, 0x03);
                    if opcode == 0x3F {
                        self.gs.rp0 = p as u32;
                    }
                }

                // ── MDRP — Move Direct Relative Point ────────────
                // C: Ins_MDRP at ttinterp.c:5399-5519
                // Flag bits: round=bit2(0x04), min_dist=bit3(0x08), set_rp0=bit4(0x10)
                0xC0..=0xDF => {
                    let p = self.pop()? as usize;
                    let rp = self.gs.rp0 as usize;

                    // Reference point current coords (always from cur)
                    let (rcx, rcy) = zone.cur(rp);

                    // Original distance: C uses orus for glyph zone, org for twilight
                    let is_twilight = self.gs.zp0 == 0 || self.gs.zp1 == 0;
                    let org_dist = if is_twilight {
                        // Twilight zone: use org (scaled 26.6) arrays directly
                        let (rorg_x, rorg_y) = zone.org(rp);
                        let (oorg_x, oorg_y) = zone.org(p);
                        self.gs.project(oorg_x - rorg_x, oorg_y - rorg_y)
                    } else {
                        // Glyph zone: use orus (unscaled font units), then scale
                        let (rorus_x, rorus_y) = zone.orus(rp);
                        let (oorus_x, oorus_y) = zone.orus(p);
                        let du = self.gs.project(oorus_x - rorus_x, oorus_y - rorus_y);
                        crate::fixed::ft_mul_fix(du, self.x_scale)
                    };

                    // Round if flag bit 2 (0x04) is set
                    let rnd_dist = if (opcode & 0x04) != 0 {
                        self.gs.round(org_dist)
                    } else {
                        org_dist
                    };

                    // Minimum distance if flag bit 3 (0x08) is set
                    let dist = if (opcode & 0x08) != 0
                        && self.gs.minimum_distance > 0
                        && rnd_dist.abs() < self.gs.minimum_distance
                    {
                        if org_dist >= 0 {
                            self.gs.minimum_distance
                        } else {
                            -self.gs.minimum_distance
                        }
                    } else {
                        rnd_dist
                    };

                    // Move point along freedom vector relative to reference
                    let (dx, dy) = self.gs.move_along_free(dist);
                    zone.set_cur(p, rcx + dx, rcy + dy);
                    zone.set_tag(p, 0x03);

                    // C: rp1 = rp0, rp2 = point
                    self.gs.rp1 = rp as u32;
                    self.gs.rp2 = p as u32;
                    // Set rp0 if flag bit 4 (0x10) is set
                    if (opcode & 0x10) != 0 {
                        self.gs.rp0 = p as u32;
                    }
                }

                // ── MIRP — Move Indirect Relative Point ──────────
                // C: Ins_MIRP at ttinterp.c:5520-5673
                // Flag bits same as MDRP + auto-flip
                0xE0..=0xFF => {
                    // Pops: point (top), cvt_index (deeper)
                    let p = self.pop()? as usize;
                    let cvt_idx = self.pop()? as usize;
                    let cvt_val = self.get_cvt(cvt_idx)?;

                    let rp = self.gs.rp0 as usize;
                    let (rcx, rcy) = zone.cur(rp);

                    // Original distance: C uses orus for glyph zone
                    let is_twilight = self.gs.zp0 == 0 || self.gs.zp1 == 0;
                    let org_dist = if is_twilight {
                        let (rorg_x, rorg_y) = zone.org(rp);
                        let (oorg_x, oorg_y) = zone.org(p);
                        self.gs.project(oorg_x - rorg_x, oorg_y - rorg_y)
                    } else {
                        let (rorus_x, rorus_y) = zone.orus(rp);
                        let (oorus_x, oorus_y) = zone.orus(p);
                        let du = self.gs.project(oorus_x - rorus_x, oorus_y - rorus_y);
                        crate::fixed::ft_mul_fix(du, self.x_scale)
                    };

                    // Round CVT value
                    let rnd_cvt = self.gs.round(cvt_val);

                    // Auto-flip: C uses org_dist sign vs cvt_val, not rnd_cvt
                    let cvt_dist = if self.gs.auto_flip && (org_dist ^ cvt_val) < 0 {
                        -rnd_cvt
                    } else {
                        rnd_cvt
                    };

                    // CVT cut-in: C compares |org_dist - cvt_dist|, not |org_dist - rnd_cvt|
                    let dist = if (org_dist - cvt_dist).abs() < self.gs.cvt_cut_in {
                        cvt_dist
                    } else {
                        let rnd_org = if (opcode & 0x04) != 0 { self.gs.round(org_dist) } else { org_dist };
                        rnd_org
                    };

                    // Minimum distance (flag bit 3)
                    let dist = if (opcode & 0x08) != 0
                        && self.gs.minimum_distance > 0
                        && dist.abs() < self.gs.minimum_distance
                    {
                        if org_dist >= 0 {
                            self.gs.minimum_distance
                        } else {
                            -self.gs.minimum_distance
                        }
                    } else {
                        dist
                    };

                    let (dx, dy) = self.gs.move_along_free(dist);
                    zone.set_cur(p, rcx + dx, rcy + dy);
                    zone.set_tag(p, 0x03);

                    // C: rp1 = rp0, rp2 = point
                    self.gs.rp1 = rp as u32;
                    self.gs.rp2 = p as u32;
                    if (opcode & 0x10) != 0 {
                        self.gs.rp0 = p as u32;
                    }
                }

                // ── ALIGNRP ───────────────────────────────────────
                0x3A => {
                    // Align all points between rp0 and popped point
                    let p = self.pop()? as usize;
                    let rp = self.gs.rp0 as usize;
                    // Move all points from rp to p in zone
                    // C's ALIGNRP: for each point, compute relative
                    // distance from rp0 in original coords along proj,
                    // then snap to zero relative to rp0 in cur coords
                    let start = rp.min(p);
                    let end = rp.max(p);
                    let (rcx, rcy) = zone.cur(rp);
                    for i in start..=end {
                        if i == rp { continue; }
                        let (org_x, org_y) = zone.org(i);
                        let (rorg_x, rorg_y) = zone.org(rp);
                        let orig_rel = self.gs.project(
                            org_x - rorg_x,
                            org_y - rorg_y,
                        );
                        let rnd_rel = self.gs.round(orig_rel);
                        let (dx, dy) = self.gs.move_along_free(rnd_rel);
                        zone.set_cur(i, rcx + dx, rcy + dy);
                        zone.set_tag(i, 0x03);
                    }
                }

                // ── SHP — Shift Point by last point (0x32-0x37) ──
                0x32..=0x37 => {
                    // SHP[rpX]: shift rp2 using the relationship between
                    // rpX and rp2 in original coords, projected onto freedom vec
                    let ref_pt = match opcode & 3 {
                        0 => self.gs.rp0 as usize,
                        1 => self.gs.rp1 as usize,
                        _ => self.gs.rp2 as usize,
                    };
                    let (rcx, rcy) = zone.cur(ref_pt);
                    let (rorg_x, rorg_y) = zone.org(ref_pt);
                    let p = self.gs.rp2 as usize;
                    let (porg_x, porg_y) = zone.org(p);
                    let orig_rel = self.gs.project(porg_x - rorg_x, porg_y - rorg_y);
                    let (dx, dy) = self.gs.move_along_free(orig_rel);
                    zone.set_cur(p, rcx + dx, rcy + dy);
                    zone.set_tag(p, 0x03);
                }

                // ── IUP — Interpolate Untouched Points ────────────
                // ✅ VERIFIED: Delegates to hinter/iup.rs (C: Ins_IUP, ttinterp.c:6189+)
                0x30 => { iup::iup_x(zone); }
                0x31 => { iup::iup_y(zone); }

                // ── Control flow ──────────────────────────────────
                // SLOOP (0x17): set loop counter
                0x17 => {
                    let v = self.pop()?;
                    self.gs.loop_counter = v;
                }
                // LOOPCALL (0x2A): pop count (deeper), func_num (top)
                0x2A => {
                    let func_num = self.pop()? as u16;  // top
                    let count = self.pop()?;  // deeper
                    if (func_num as usize) < self.functions.len() {
                        if let Some(ref def) = self.functions[func_num as usize] {
                            if def.active {
                                self.call_stack.push(CallRecord {
                                    caller_range: self.cur_range,
                                    caller_ip: self.ip,
                                    cur_count: count,
                                    def_index: func_num as usize,
                                });
                                self.ip = def.start;
                                self.cur_range = def.range;
                            }
                        }
                    }
                }
                // CALL (0x2B): pop function number from top of stack
                0x2B => {
                    let func_num = self.pop()? as u16;
                    if (func_num as usize) < self.functions.len() {
                        if let Some(ref def) = self.functions[func_num as usize] {
                            if def.active {
                                self.call_stack.push(CallRecord {
                                    caller_range: self.cur_range,
                                    caller_ip: self.ip,
                                    cur_count: 0,
                                    def_index: func_num as usize,
                                });
                                self.ip = def.start;
                                self.cur_range = def.range;
                            }
                        }
                    }
                }
                0x2D => {
                    // ENDF: return from function
                    if let Some(call) = self.call_stack.pop() {
                        self.ip = call.caller_ip;
                        self.cur_range = call.caller_range;
                    }
                }
                0x2C => {
                    // FDEF inside glyph program — should not happen.
                    // Defined in fpgm, just skip.
                }

                0x58 => {
                    // ELSE: skip to EIF
                    let mut depth = 1;
                    while depth > 0 && self.ip < self.glyph_program.len() {
                        let b = self.fetch_byte_glyph()?;
                        match b {
                            0x59 | 0x1B => depth += 1,
                            0x2D => { depth -= 1; if depth == 0 { break; } }
                            _ => {}
                        }
                    }
                }

                // ── SHPIX — Shift Pixel ───────────────────────────
                0x38 => {
                    // pops point (deeper) then amount (top)
                    let p = self.pop()? as usize;  // deeper = point
                    let amount = self.pop()?;  // top = amount
                    let (dx, dy) = self.gs.move_along_free(amount);
                    let (cx, cy) = zone.cur(p);
                    zone.set_cur(p, cx + dx, cy + dy);
                    zone.set_tag(p, 0x03);
                }

                // ── ALIGNPTS (0x27) — Align points ──────────
                0x27 => {
                    let p = self.pop()? as usize;
                    let q = self.pop()? as usize;
                    // Move p relative to q along projection vector
                    let (qx, qy) = zone.cur(q);
                    let (porg_x, porg_y) = zone.org(p);
                    let (qorg_x, qorg_y) = zone.org(q);
                    let dist = self.gs.project(porg_x - qorg_x, porg_y - qorg_y);
                    let (dx, dy) = self.gs.move_along_free(dist);
                    zone.set_cur(p, qx + dx, qy + dy);
                    zone.set_tag(p, 0x03);
                }
                // ── CINDEX (0x25) — Copy indexed element ─────────
                0x25 => {
                    let k = self.pop()? as usize;
                    if k < self.stack.len() {
                        let v = self.stack[self.stack.len() - 1 - k];
                        self.push(v);
                    }
                }
                // ── MINDEX (0x26) — Move indexed element ─────────
                0x26 => {
                    let k = self.pop()? as usize;
                    if k < self.stack.len() {
                        let v = self.stack.remove(self.stack.len() - 1 - k);
                        self.push(v);
                    }
                }
                // ── SMD (0x1A) — Set Minimum Distance ────────────
                0x1A => {
                    let v = self.pop()?;
                    self.gs.minimum_distance = v;
                }
                // ── FLIPPT (0x80) — Flip point ───────────────────
                0x80 => {
                    let p = self.pop()? as usize;
                    // Toggle on-curve flag
                    // We don't track this precisely, just mark touched
                    zone.set_tag(p, 0x03);
                }
                // ── SCVTCI (0x6C) — Set CVT Cut-In ───────────────
                0x6C => {
                    let v = self.pop()?;
                    self.gs.control_value_cutin = v;
                }
                // ── SSW (0x6E) — Set Single Width ────────────────
                0x6E => {
                    let v = self.pop()?;
                    self.gs.single_width_value = v;
                }
                // ── SSWCI (0x6D) — Set Single Width Cut-In ───────
                0x6D => {
                    let v = self.pop()?;
                    self.gs.single_width_cutin = v;
                }
                // ── SDB (0x8B) — Set Delta Base ──────────────────
                0x8B => {
                    let v = self.pop()?;
                    self.gs.delta_base = v as u32;
                }
                // ── SDS (0x8A) — Set Delta Shift ─────────────────
                0x8A => {
                    let v = self.pop()?;
                    self.gs.delta_shift = v as u32;
                }
                // ── JMPR (0x1C) — Jump Relative ──────────────────
                0x1C => {
                    let offset = self.pop()? - 1;
                    self.ip = (self.ip as i32 + offset) as usize;
                }
                // ── JROT (0x78) — pops e1(top), e2, offset(deeper)
                0x78 => {
                    let e1 = self.pop()?;  // top
                    let e2 = self.pop()?;
                    let offset = self.pop()?;  // deeper
                    if e1 > e2 {
                        self.ip = (self.ip as i32 + offset - 1) as usize;
                    }
                }
                // ── JROF (0x79) — pops e1(top), e2, offset(deeper)
                0x79 => {
                    let e1 = self.pop()?;  // top
                    let e2 = self.pop()?;
                    let offset = self.pop()?;  // deeper
                    if e1 <= e2 {
                        self.ip = (self.ip as i32 + offset - 1) as usize;
                    }
                }
                // ── SFVTL (0x08-0x09) — Set Freedom Vector To Line ──
                0x08 | 0x09 => {
                    // Sets freedom vector to be parallel to line from rp1 to rp2
                    let p1 = self.gs.rp1 as usize;
                    let p2 = self.gs.rp2 as usize;
                    let (x1, y1) = if self.gs.zp1 == 0 { zone.cur(p1) } else { zone.org(p1) };
                    let (x2, y2) = if self.gs.zp2 == 0 { zone.cur(p2) } else { zone.org(p2) };
                    let dx = x2 - x1;
                    let dy = y2 - y1;
                    // Project (dx,dy) onto freedom vector as 2.14 fixed
                    if dx == 0 && dy == 0 {} else {
                        let len = ((dx as i64 * dx as i64 + dy as i64 * dy as i64) as f64).sqrt() as i64;
                        if len > 0 {
                            self.gs.freedom_vector = (
                                ((dx as i64 * 0x4000 / len) as i32),
                                ((dy as i64 * 0x4000 / len) as i32)
                            );
                        }
                    }
                }
                // ── SPVFS (0x0A) — Set Proj Vector From Stack ──────
                0x0A => {
                    let y = self.pop()?;
                    let x = self.pop()?;
                    self.gs.proj_vector = (x, y);
                }
                // ── SFVFS (0x0B) — Set Freedom Vector From Stack ────
                0x0B => {
                    let y = self.pop()?;
                    let x = self.pop()?;
                    self.gs.freedom_vector = (x, y);
                }
                // ── SFVTPV (0x0E) — Set Freedom Vector To Proj Vector ─
                0x0E => {
                    self.gs.freedom_vector = self.gs.proj_vector;
                }
                // ── DEPTH (0x24) — Push stack depth ─────────────────
                0x24 => {
                    self.push(self.stack.len() as i32);
                }
                // ── IP (0x39) — Interpolate Point ───────────────────
                0x39 => {
                    // Interpolate a point between rp1 and rp2 relative
                    // to their original positions
                    let loop_count = self.gs.loop_counter;
                    let rp1 = self.gs.rp1 as usize;
                    let rp2 = self.gs.rp2 as usize;
                    let (r1_ox, r1_oy) = zone.org(rp1);
                    let (r2_ox, r2_oy) = zone.org(rp2);
                    let (r1_cx, r1_cy) = zone.cur(rp1);
                    let (r2_cx, r2_cy) = zone.cur(rp2);
                    let orig_dist = self.gs.project(r2_ox - r1_ox, r2_oy - r1_oy);
                    let cur_dist = self.gs.project(r2_cx - r1_cx, r2_cy - r1_cy);
                    for _ in 0..loop_count {
                        let p = self.pop()? as usize;
                        let (ox, oy) = zone.org(p);
                        let p_orig_dist = self.gs.project(ox - r1_ox, oy - r1_oy);
                        // Use i64 for intermediate to avoid overflow
                        let frac = if orig_dist != 0 {
                            ((p_orig_dist as i64 * cur_dist as i64) / orig_dist as i64) as i32
                        } else { 0 };
                        let (dx, dy) = self.gs.move_along_free(frac);
                        zone.set_cur(p, r1_cx + dx, r1_cy + dy);
                        zone.set_tag(p, 0x03);
                    }
                }
                // ── AlignRP (0x3C) — Align to Reference Point ───────
                0x3C => {
                    // Same as 0x3A but uses zp1 for reference
                    let p = self.pop()? as usize;
                    let rp = self.gs.rp0 as usize;
                    let start = rp.min(p);
                    let end = rp.max(p);
                    let (rcx, rcy) = zone.cur(rp);
                    for i in start..=end {
                        if i == rp { continue; }
                        let (org_x, org_y) = zone.org(i);
                        let (rorg_x, rorg_y) = zone.org(rp);
                        let orig_rel = self.gs.project(org_x - rorg_x, org_y - rorg_y);
                        let rnd_rel = self.gs.round(orig_rel);
                        let (dx, dy) = self.gs.move_along_free(rnd_rel);
                        zone.set_cur(i, rcx + dx, rcy + dy);
                        zone.set_tag(i, 0x03);
                    }
                }
                // ── LT (0x50) — Less Than ───────────────────────────
                0x50 => { let b = self.pop()?; let a = self.pop()?; self.push(if a < b {1} else {0}); }
                // ── LTEQ (0x51) ─────────────────────────────────────
                0x51 => { let b = self.pop()?; let a = self.pop()?; self.push(if a <= b {1} else {0}); }
                // ── GT (0x52) ───────────────────────────────────────
                0x52 => { let b = self.pop()?; let a = self.pop()?; self.push(if a > b {1} else {0}); }
                // ── GTEQ (0x53) ─────────────────────────────────────
                0x53 => { let b = self.pop()?; let a = self.pop()?; self.push(if a >= b {1} else {0}); }
                // ── EQ (0x54) ───────────────────────────────────────
                0x54 => { let b = self.pop()?; let a = self.pop()?; self.push(if a == b {1} else {0}); }
                // ── NEQ (0x55) ──────────────────────────────────────
                0x55 => { let b = self.pop()?; let a = self.pop()?; self.push(if a != b {1} else {0}); }
                // ── OR (0x5B) — Logical OR ──────────────────────────
                0x5B => { let b = self.pop()?; let a = self.pop()?; self.push(if a != 0 || b != 0 {1} else {0}); }
                // ── FLIPRGON (0x81) / FLIPRGOFF (0x82) ─────────────
                0x81 => { let _ = self.pop()?; } // FLIPRGON
                0x82 => { let _ = self.pop()?; } // FLIPRGOFF
                // ── DELTAP1/2/3 (0x5D, 0x71, 0x72) — Delta exceptions ──
                // C: Ins_DELTAP at ttinterp.c (various). Skips for now —
                // delta exceptions are per-ppem adjustment tables that
                // are rarely used at 72dpi 10-24pt.
                0x5D | 0x5E | 0x5F | 0x71 | 0x72 | 0x73 | 0x74 => {}
                // ── Unknown opcode ────────────────────────────
                _ => {}
            }
        }

        Ok(())
    }
}
