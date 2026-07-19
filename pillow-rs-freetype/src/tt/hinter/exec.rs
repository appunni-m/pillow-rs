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
use super::gs::RoundMode;
use super::iup;
use super::zone::GlyphZone;
use super::{HintScale, NativeHintMode};
use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_div, ft_mul_fix, ft_normalize_2dot14, ft_vector_length};

/// Maximum stack depth. TrueType spec says max 255, but fonts may request
/// more via maxp->maxStackElements. We use a generous default.
const DEFAULT_MAX_STACK: usize = 512;

/// Maximum call stack depth. C uses 10.
const MAX_CALL_DEPTH: usize = 10;

/// Maximum function definitions.
const MAX_FUNCTIONS: usize = 256;

/// Maximum instruction definitions (IDEF).
const MAX_INSTRUCTION_DEFS: usize = 256;

/// C `TT_RunIns` uses `TT_CONFIG_OPTION_MAX_RUNNABLE_OPCODES` as the final
/// malformed-bytecode guard before returning `FT_Err_Execution_Too_Long`.
const MAX_VM_STEPS: u32 = 1_000_000;

#[inline]
fn delta_step(delta_shift: u32) -> i32 {
    if delta_shift <= 6 {
        1i32 << (6 - delta_shift)
    } else {
        0
    }
}

#[inline]
fn tt_mul_fix14(a: i32, b: i32) -> i32 {
    let c = i64::from(a) * i64::from(b);
    ((c + 0x2000 + (c >> 63)) >> 14) as i32
}

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
    /// Stable slot identity for C's mutable `TT_CallRec.Def` pointer.
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
    /// FreeType `TT_Size_Metrics.scale`, used for CVT font-unit scaling.
    pub tt_scale: i32,

    /// Pixels per em (for MPPEM opcode).
    pub ppem: i32,
    /// FreeType `TT_Size_Metrics` ratios for non-square pixel CVT access.
    pub x_ratio: i32,
    pub y_ratio: i32,
    /// Requested point size in 26.6 units (for MPS opcode).
    pub point_size: i32,

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
    max_function_defs: usize,
    num_function_defs: usize,

    /// Instruction definitions (IDEF/ENDF).
    #[allow(dead_code)]
    pub instruction_defs: Vec<Option<DefRecord>>,
    max_instruction_defs: usize,
    num_instruction_defs: usize,

    // ── Call stack ────────────────────────────────────────────────────
    /// Call stack (max 10 levels deep).
    pub call_stack: Vec<CallRecord>,

    // ── Instruction pointer state ─────────────────────────────────────
    /// Current instruction pointer (byte offset within the active code range).
    pub ip: usize,

    /// Which code range is currently executing (0=cvt, 1=font, 2=glyph).
    pub cur_range: u8,

    /// Code range that initiated the current interpreter run.
    ///
    /// C `TT_Set_CodeRange` initializes both `iniRange` and `curRange`, while
    /// `Ins_Goto_CodeRange` changes only `curRange` for CALL/ENDF transitions.
    pub initial_range: u8,

    // ── Flags ─────────────────────────────────────────────────────────
    /// Whether we're hinting a composite glyph.
    pub is_composite: bool,

    /// Pedantic hinting mode (abort on errors).
    pub pedantic_hinting: bool,

    /// The glyph's instruction stream (set per glyph).
    pub glyph_program: Vec<u8>,

    /// The prep/CVT program bytecode (executed once per size change).
    pub cvt_program: Vec<u8>,

    /// Persistent twilight zone for prep and glyph programs.
    pub twilight: GlyphZone,

    /// FreeType v40 backward-compatibility state: bit 2 enables the mode,
    /// bits 0-1 track whether IUP\[y\]/IUP\[x\] have executed.
    pub backward_compatibility: u8,

    /// FreeType TrueType render mode for bytecode GETINFO.
    pub native_hint_mode: NativeHintMode,
}

impl ExecContext {
    /// Create a new execution context.
    ///
    /// # Arguments
    /// * `cvt` — control value table (in 26.6, already scaled)
    /// * `fpgm` — font program bytecode
    /// * `scale` — size, storage, twilight, and target-mode setup
    pub fn new(cvt: &[i32], fpgm: &[u8], scale: &HintScale) -> Self {
        ExecContext {
            gs: GraphicsState::default(),
            x_scale: scale.x_scale,
            y_scale: scale.y_scale,
            tt_scale: scale.tt_scale,
            ppem: scale.ppem,
            x_ratio: scale.x_ratio,
            y_ratio: scale.y_ratio,
            point_size: scale.point_size,
            font_range: CodeRange {
                base: 0,
                size: fpgm.len(),
            },
            cvt_range: CodeRange::default(),
            glyph_range: CodeRange::default(),
            font_program: fpgm.to_vec(),
            stack: Vec::with_capacity(DEFAULT_MAX_STACK),
            storage: vec![0; scale.storage_size.max(1)],
            cvt: cvt.to_vec(),
            functions: vec![None; MAX_FUNCTIONS],
            max_function_defs: scale.max_function_defs,
            num_function_defs: 0,
            instruction_defs: vec![None; MAX_INSTRUCTION_DEFS],
            max_instruction_defs: scale.max_instruction_defs,
            num_instruction_defs: 0,
            call_stack: Vec::with_capacity(MAX_CALL_DEPTH),
            ip: 0,
            cur_range: 0,
            initial_range: 0,
            is_composite: false,
            pedantic_hinting: scale.pedantic_hinting,
            glyph_program: Vec::new(),
            cvt_program: Vec::new(),
            // C `TT_Load_Context` allocates the twilight zone from
            // maxp.maxTwilightPoints.  Some bytecode programs address points
            // well past 16 while building interpolation control points.
            twilight: Self::new_twilight_zone(scale.twilight_points),
            backward_compatibility: 0,
            native_hint_mode: scale.native_hint_mode,
        }
    }

    fn new_twilight_zone(n_points: usize) -> GlyphZone {
        GlyphZone {
            cur_x: vec![0; n_points],
            cur_y: vec![0; n_points],
            org_x: vec![0; n_points],
            org_y: vec![0; n_points],
            orus_x: vec![0; n_points],
            orus_y: vec![0; n_points],
            tags: vec![0; n_points],
            contours: vec![],
            n_points: n_points as u16,
            n_contours: 0,
            first_point: 0,
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
        self.stack
            .last()
            .copied()
            .ok_or(FontError::InvalidOutline("bytecode: stack empty".into()))
    }

    /// Read one byte from the active code range and advance the instruction pointer.
    pub fn fetch_byte(&mut self) -> Result<u8, FontError> {
        let data = if self.cur_range == 1 {
            &self.font_program
        } else {
            &self.glyph_program
        };
        let byte = data.get(self.ip).copied().ok_or(FontError::InvalidOutline(
            "bytecode: IP out of range".into(),
        ))?;
        self.ip += 1;
        Ok(byte)
    }

    /// Read one signed big-endian word from the active code range.
    pub fn fetch_word(&mut self) -> Result<i16, FontError> {
        let hi = self.fetch_byte()?;
        let lo = self.fetch_byte()?;
        Ok(i16::from_be_bytes([hi, lo]))
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
        // FDEF records are registered while their bodies are skipped; CALL later
        // runs those bodies from the font-program code range.
        self.stack.clear();
        self.ip = 0;
        self.cur_range = 1;
        self.initial_range = 1;

        // Empty zone: fpgm runs without glyph points (C: exec->pts.n_points = 0)
        let mut empty_zone = GlyphZone {
            cur_x: vec![],
            cur_y: vec![],
            org_x: vec![],
            org_y: vec![],
            orus_x: vec![],
            orus_y: vec![],
            tags: vec![],
            contours: vec![],
            n_points: 0,
            n_contours: 0,
            first_point: 0,
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
    pub fn run_prep(&mut self, prep_bytes: &[u8], saved_storage: &[i32]) -> Result<(), FontError> {
        // ── TT_Load_Context equivalent (C: ttobjs.c:891-957) ──────
        // C calls this before EVERY program execution (fpgm, prep, glyph).
        // It resets GS, scales CVT from FU to pixel units, zeroes storage.
        self.gs = GraphicsState::default();
        self.gs.auto_flip = true; // C default

        // Scale CVT: face-level CVT values are stored as FWORD*64.  FreeType
        // intentionally divides by 64 before applying the 16.16 size scale;
        // this rounding-sensitive order produces 26.6 pixel values.
        for i in 0..self.cvt.len() {
            self.cvt[i] = crate::fixed::ft_mul_fix(self.cvt[i] / 64, self.tt_scale);
        }

        // Restore the post-fpgm storage snapshot saved by TT_Save_Context.
        for (idx, slot) in self.storage.iter_mut().enumerate() {
            *slot = saved_storage.get(idx).copied().unwrap_or(0);
        }

        // Zero twilight zone (C: FT_ARRAY_ZERO)
        self.twilight = Self::new_twilight_zone(self.twilight.n_points as usize);

        // Set up prep as the CVT program.  FreeType's INSTCTRL only persists
        // size graphics-state flags from this code range.
        self.stack.clear();
        self.cvt_program = prep_bytes.to_vec();
        self.glyph_program.clear();
        self.ip = 0;
        self.cur_range = 0;
        self.initial_range = 0;

        // C: prep runs with zp0=zp1=zp2=0 (twilight zone)
        self.gs.zp0 = 0;
        self.gs.zp1 = 0;
        self.gs.zp2 = 0;

        // Run prep against twilight zone
        let mut empty_glyph = Self::new_twilight_zone(0);
        self.run_program(&mut empty_glyph)?;

        // C: TT_Save_Context only persists selected prep state fields into
        // size->GS. Projection/freedom vectors, round state, reference points,
        // zone pointers, and loop state return to defaults for glyph programs.
        let prep_gs = self.gs.clone();
        self.gs = GraphicsState::default();
        self.gs.minimum_distance = prep_gs.minimum_distance;
        self.gs.control_value_cutin = prep_gs.control_value_cutin;
        self.gs.single_width_cutin = prep_gs.single_width_cutin;
        self.gs.single_width_value = prep_gs.single_width_value;
        self.gs.delta_base = prep_gs.delta_base;
        self.gs.delta_shift = prep_gs.delta_shift;
        self.gs.auto_flip = prep_gs.auto_flip;
        self.gs.instruct_control = prep_gs.instruct_control;
        self.gs.scan_control = prep_gs.scan_control;
        self.gs.scan_type = prep_gs.scan_type;

        Ok(())
    }

    /// Get a value from the storage area.
    #[allow(dead_code)]
    pub fn get_storage(&self, idx: usize) -> Result<i32, FontError> {
        if let Some(value) = self.storage.get(idx) {
            Ok(*value)
        } else if self.pedantic_hinting {
            // C `Ins_RS` (`ttinterp.c:2739-2755`) reports Invalid_Reference
            // for an out-of-range storage index only in pedantic mode.
            Err(FontError::InvalidReference)
        } else {
            Ok(0)
        }
    }

    /// Set a value in the storage area.
    #[allow(dead_code)]
    pub fn set_storage(&mut self, idx: usize, val: i32) -> Result<(), FontError> {
        if let Some(slot) = self.storage.get_mut(idx) {
            *slot = val;
        } else if self.pedantic_hinting {
            // C `Ins_WS` (`ttinterp.c:2765-2798`) ignores this write in
            // normal mode and reports Invalid_Reference in pedantic mode.
            return Err(FontError::InvalidReference);
        }
        Ok(())
    }

    /// Get a CVT value. Returns 0 if index is out of range.
    #[allow(dead_code)]
    pub fn get_cvt(&self, idx: usize) -> Result<i32, FontError> {
        if let Some(value) = self.cvt.get(idx) {
            // C `Read_CVT_Stretched` applies the projection-dependent ratio.
            Ok(ft_mul_fix(*value, self.current_ratio()))
        } else if self.pedantic_hinting {
            // C `Ins_RCVT` (`ttinterp.c:2859-2874`) has the same
            // normal-mode zero and pedantic Invalid_Reference split.
            Err(FontError::InvalidReference)
        } else {
            Ok(0)
        }
    }

    /// Set a CVT value. No-op if index is out of range.
    #[allow(dead_code)]
    pub fn set_cvt(&mut self, idx: usize, val: i32) -> Result<(), FontError> {
        let ratio = self.current_ratio();
        if let Some(slot) = self.cvt.get_mut(idx) {
            // C `Write_CVT_Stretched` stores pixel writes divided by the
            // current projection ratio; reads apply the ratio again.
            *slot = ft_div_fix(val, ratio);
        } else if self.pedantic_hinting {
            // C `Ins_WCVTP` (`ttinterp.c:2809-2824`) ignores this write in
            // normal mode and reports Invalid_Reference in pedantic mode.
            return Err(FontError::InvalidReference);
        }
        Ok(())
    }

    fn set_cvt_raw(&mut self, idx: usize, val: i32) -> Result<(), FontError> {
        if let Some(slot) = self.cvt.get_mut(idx) {
            *slot = val;
        } else if self.pedantic_hinting {
            return Err(FontError::InvalidReference);
        }
        Ok(())
    }

    fn move_cvt(&mut self, idx: usize, delta: i32) -> Result<(), FontError> {
        let ratio = self.current_ratio();
        if let Some(slot) = self.cvt.get_mut(idx) {
            // C `Move_CVT_Stretched` inverse-projects only the delta.
            *slot = slot.wrapping_add(ft_div_fix(delta, ratio));
        } else if self.pedantic_hinting {
            return Err(FontError::InvalidReference);
        }
        Ok(())
    }

    fn current_ratio(&self) -> i32 {
        // Pinned FreeType 2.14.3 `ttinterp.c:Current_Ratio` selects the
        // TT_Size_Metrics ratio from the current projection vector.
        if self.x_ratio == 0x1_0000 && self.y_ratio == 0x1_0000 {
            return 0x1_0000;
        }
        let (proj_x, proj_y) = self.gs.proj_vector;
        if proj_y == 0 {
            self.x_ratio
        } else if proj_x == 0 {
            self.y_ratio
        } else {
            let x = tt_mul_fix14(self.x_ratio, proj_x);
            let y = tt_mul_fix14(self.y_ratio, proj_y);
            ft_vector_length(x, y)
        }
    }

    fn current_ppem(&self) -> i32 {
        // Pinned FreeType 2.14.3 `ttinterp.c:Current_Ppem_Stretched` applies
        // the projection-dependent ratio to the max-axis ppem.
        ft_mul_fix(self.ppem, self.current_ratio())
    }

    fn touch_point(&self, zone: &mut GlyphZone, p: usize) {
        let mut tag = 0u8;
        if self.gs.freedom_vector.0 != 0 {
            tag |= 0x01;
        }
        if self.gs.freedom_vector.1 != 0 {
            tag |= 0x02;
        }
        zone.set_tag(p, tag);
    }

    fn cur_in(&self, glyph: &GlyphZone, zp: u8, p: usize) -> (i32, i32) {
        if zp == 0 {
            self.twilight.cur(p)
        } else {
            glyph.cur(p)
        }
    }

    fn org_in(&self, glyph: &GlyphZone, zp: u8, p: usize) -> (i32, i32) {
        if zp == 0 {
            self.twilight.org(p)
        } else {
            glyph.org(p)
        }
    }

    fn orus_in(&self, glyph: &GlyphZone, zp: u8, p: usize) -> (i32, i32) {
        if zp == 0 {
            self.twilight.orus(p)
        } else {
            glyph.orus(p)
        }
    }

    fn set_cur_in(&mut self, glyph: &mut GlyphZone, zp: u8, p: usize, x: i32, y: i32) {
        if zp == 0 {
            self.twilight.set_cur(p, x, y);
        } else {
            self.set_glyph_cur(glyph, p, x, y);
        }
    }

    fn set_glyph_cur(&self, glyph: &mut GlyphZone, p: usize, x: i32, y: i32) {
        if p >= glyph.cur_x.len() {
            return;
        }

        let mut new_x = x;
        let mut new_y = y;
        if self.backward_compatibility != 0 {
            if self.gs.move_vector.0 != 0 {
                new_x = glyph.cur_x[p];
            }
            if self.gs.move_vector.1 != 0 && self.backward_compatibility == 0x7 {
                new_y = glyph.cur_y[p];
            }
        }

        glyph.set_cur(p, new_x, new_y);
    }

    fn set_org_in(&mut self, glyph: &mut GlyphZone, zp: u8, p: usize, x: i32, y: i32) {
        let zone = if zp == 0 { &mut self.twilight } else { glyph };
        if p < zone.org_x.len() {
            zone.org_x[p] = x;
            zone.org_y[p] = y;
        }
    }

    fn minimum_distance(&self) -> i32 {
        self.gs.minimum_distance
    }

    fn touch_in(&mut self, glyph: &mut GlyphZone, zp: u8, p: usize) {
        let mut tag = 0u8;
        if self.gs.freedom_vector.0 != 0 {
            tag |= 0x01;
        }
        if self.gs.freedom_vector.1 != 0 {
            tag |= 0x02;
        }
        if zp == 0 {
            self.twilight.set_tag(p, tag);
        } else {
            glyph.set_tag(p, tag);
        }
    }

    fn tag_in(&self, glyph: &GlyphZone, zp: u8, p: usize) -> Option<u8> {
        if zp == 0 {
            self.twilight.tags.get(p).copied()
        } else {
            glyph.tags.get(p).copied()
        }
    }

    fn clear_touch_in(&mut self, glyph: &mut GlyphZone, zp: u8, p: usize) {
        let mut mask = 0u8;
        if self.gs.freedom_vector.0 != 0 {
            mask |= 0x01;
        }
        if self.gs.freedom_vector.1 != 0 {
            mask |= 0x02;
        }
        if zp == 0 {
            self.twilight.clear_tag(p, mask);
        } else {
            glyph.clear_tag(p, mask);
        }
    }

    fn active_program_len(&self) -> usize {
        match self.cur_range {
            0 => self.cvt_program.len(),
            1 => self.font_program.len(),
            _ => self.glyph_program.len(),
        }
    }

    fn skip_instruction_operands(
        program: &[u8],
        ip: &mut usize,
        opcode: u8,
    ) -> Result<(), FontError> {
        let operand_bytes = match opcode {
            0xB0..=0xB7 => (opcode - 0xB0 + 1) as usize,
            0xB8..=0xBF => (opcode - 0xB8 + 1) as usize * 2,
            0x40 | 0x41 => {
                let count = *program.get(*ip).ok_or(FontError::CodeOverflow)? as usize;
                *ip += 1;
                if opcode == 0x40 { count } else { count * 2 }
            }
            _ => return Ok(()),
        };
        let remaining = program.len().saturating_sub(*ip);
        if operand_bytes > remaining {
            return Err(FontError::CodeOverflow);
        }
        *ip += operand_bytes;
        Ok(())
    }

    fn define_function(&mut self) -> Result<(), FontError> {
        let func_num = self.pop()? as u16;
        let range = self.cur_range;
        // FreeType `ttinterp.c:3274-3335` allows FDEF in `fpgm` and `prep`,
        // but rejects a definition initiated by glyph execution even if CALL
        // has temporarily switched `curRange` to the font program.
        if self.initial_range == 2 {
            return Err(FontError::InvalidOutline(
                "bytecode: FDEF in glyph program".into(),
            ));
        }
        let func_index = func_num as usize;
        if func_index >= self.functions.len() {
            return Err(FontError::InvalidOutline(
                "bytecode: too many function definitions".into(),
            ));
        }
        let redefining = self.functions[func_index].is_some();
        if !redefining {
            if self.num_function_defs >= self.max_function_defs {
                return Err(FontError::InvalidOutline(
                    "bytecode: too many function definitions".into(),
                ));
            }
            self.num_function_defs += 1;
        }

        let program = if range == 0 {
            &self.cvt_program
        } else {
            &self.font_program
        };
        let start = self.ip;
        let mut scan_ip = self.ip;

        while scan_ip < program.len() {
            let op_ip = scan_ip;
            let op = program[scan_ip];
            scan_ip += 1;
            match op {
                0x2C | 0x89 => {
                    return Err(FontError::InvalidOutline(
                        "bytecode: nested FDEF/IDEF".into(),
                    ));
                }
                0x2D => {
                    self.functions[func_index] = Some(DefRecord {
                        range,
                        start,
                        end: op_ip,
                        opc: func_num,
                        active: true,
                    });
                    self.ip = scan_ip;
                    return Ok(());
                }
                _ => Self::skip_instruction_operands(program, &mut scan_ip, op)?,
            }
        }

        Err(FontError::InvalidOutline(
            "bytecode: unterminated FDEF".into(),
        ))
    }

    fn define_instruction(&mut self) -> Result<(), FontError> {
        let opcode = self.pop()?;
        let range = self.cur_range;
        // FreeType `ttinterp.c:3563-3619` has the same initiating-range and
        // nested-definition rules for IDEF as for FDEF.
        if self.initial_range == 2 {
            return Err(FontError::InvalidOutline(
                "bytecode: IDEF in glyph program".into(),
            ));
        }

        if !(0..=0xFF).contains(&opcode) {
            return Err(FontError::InvalidOutline(
                "bytecode: IDEF opcode out of range".into(),
            ));
        }
        let opcode_index = opcode as usize;
        let redefining = self.instruction_defs[opcode_index].is_some();
        if !redefining {
            if self.num_instruction_defs >= self.max_instruction_defs {
                return Err(FontError::InvalidOutline(
                    "bytecode: too many instruction definitions".into(),
                ));
            }
            self.num_instruction_defs += 1;
        }

        let program = if range == 0 {
            &self.cvt_program
        } else {
            &self.font_program
        };
        let start = self.ip;
        let mut scan_ip = self.ip;

        while scan_ip < program.len() {
            let op_ip = scan_ip;
            let op = program[scan_ip];
            scan_ip += 1;
            match op {
                0x2C | 0x89 => {
                    return Err(FontError::InvalidOutline(
                        "bytecode: nested FDEF/IDEF".into(),
                    ));
                }
                0x2D => {
                    self.instruction_defs[opcode_index] = Some(DefRecord {
                        range,
                        start,
                        end: op_ip,
                        opc: opcode_index as u16,
                        active: true,
                    });
                    self.ip = scan_ip;
                    return Ok(());
                }
                _ => Self::skip_instruction_operands(program, &mut scan_ip, op)?,
            }
        }

        Err(FontError::InvalidOutline(
            "bytecode: unterminated IDEF".into(),
        ))
    }

    fn call_instruction_def(&mut self, opcode: u8) -> bool {
        let Some(def) = self.instruction_defs[opcode as usize].as_ref() else {
            return false;
        };
        if !def.active || self.call_stack.len() >= MAX_CALL_DEPTH {
            return false;
        }

        self.call_stack.push(CallRecord {
            caller_range: self.cur_range,
            caller_ip: self.ip,
            cur_count: 1,
            def_index: opcode as usize,
        });
        self.ip = def.start;
        self.cur_range = def.range;
        true
    }

    fn active_function_def(&self, func_num: i32) -> Result<(usize, usize, u8), FontError> {
        let func_index = usize::try_from(func_num).map_err(|_| {
            FontError::InvalidOutline("bytecode: invalid function reference".into())
        })?;
        let Some(def) = self.functions.get(func_index).and_then(Option::as_ref) else {
            return Err(FontError::InvalidOutline(
                "bytecode: invalid function reference".into(),
            ));
        };
        if !def.active {
            return Err(FontError::InvalidOutline(
                "bytecode: invalid function reference".into(),
            ));
        }
        Ok((func_index, def.start, def.range))
    }

    fn enter_function_call(&mut self, func_num: i32, count: i32) -> Result<(), FontError> {
        // C: ttinterp.c:3395-3549 `Ins_CALL` and `Ins_LOOPCALL` validate the
        // FDEF reference and call-stack room before deciding whether LOOPCALL's
        // repeat count is positive.  Invalid/inactive definitions are errors,
        // not silent no-ops.
        let (def_index, def_start, def_range) = self.active_function_def(func_num)?;
        if self.call_stack.len() >= MAX_CALL_DEPTH {
            return Err(FontError::InvalidOutline(
                "bytecode: call stack overflow".into(),
            ));
        }
        if count <= 0 {
            return Ok(());
        }
        self.call_stack.push(CallRecord {
            caller_range: self.cur_range,
            caller_ip: self.ip,
            cur_count: count,
            def_index,
        });
        self.ip = def_start;
        self.cur_range = def_range;
        Ok(())
    }

    fn point_displacement(
        &self,
        opcode: u8,
        zone: &GlyphZone,
    ) -> Result<Option<(i32, i32, u8, usize)>, FontError> {
        let (ref_zone, ref_pt) = if opcode & 1 != 0 {
            (self.gs.zp0, self.gs.rp1 as usize)
        } else {
            (self.gs.zp1, self.gs.rp2 as usize)
        };
        let point_limit = if ref_zone == 0 {
            self.twilight.n_points as usize
        } else {
            zone.n_points as usize
        };
        if ref_pt >= point_limit {
            // FreeType `Compute_Point_Displacement` validates the selected
            // reference point before SHP/SHC/SHZ movement
            // (`ttinterp.c:4923-4962`).  Non-pedantic execution ignores the
            // instruction; FT_LOAD_PEDANTIC reports Invalid_Reference.
            return if self.pedantic_hinting {
                Err(FontError::InvalidReference)
            } else {
                Ok(None)
            };
        }
        let (cx, cy) = self.cur_in(zone, ref_zone, ref_pt);
        let (ox, oy) = self.org_in(zone, ref_zone, ref_pt);
        let dist = self.gs.project(cx - ox, cy - oy);
        let (dx, dy) = self.gs.move_along_free(dist);
        Ok(Some((dx, dy, ref_zone, ref_pt)))
    }

    fn skip_to_else_or_eif(&mut self) -> Result<(), FontError> {
        let mut depth = 1u32;
        while self.ip < self.active_program_len() {
            let op = self.fetch_byte_glyph()?;
            match op {
                0x58 => depth += 1,
                0x1B if depth == 1 => return Ok(()),
                0x59 => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {
                    let program = match self.cur_range {
                        0 => &self.cvt_program,
                        1 => &self.font_program,
                        _ => &self.glyph_program,
                    };
                    Self::skip_instruction_operands(program, &mut self.ip, op)?;
                }
            }
        }
        Err(FontError::CodeOverflow)
    }

    fn skip_to_eif(&mut self) -> Result<(), FontError> {
        let mut depth = 1u32;
        while self.ip < self.active_program_len() {
            let op = self.fetch_byte_glyph()?;
            match op {
                0x58 => depth += 1,
                0x59 => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                _ => {
                    let program = match self.cur_range {
                        0 => &self.cvt_program,
                        1 => &self.font_program,
                        _ => &self.glyph_program,
                    };
                    Self::skip_instruction_operands(program, &mut self.ip, op)?;
                }
            }
        }
        Err(FontError::CodeOverflow)
    }

    fn line_vector(dx: i32, dy: i32, perpendicular: bool) -> (i32, i32) {
        let (vx, vy) = if perpendicular {
            (dy.wrapping_neg(), dx)
        } else {
            (dx, dy)
        };
        if vx == 0 && vy == 0 {
            return (0x4000, 0);
        }

        // C: `Ins_SxVTL`/`Ins_SDPVTL` call `Normalize`, which uses
        // FreeType's fixed `FT_Vector_NormLen`; host `sqrt` truncation shifts
        // diagonal vectors by one 2.14 unit and changes hinted outlines.
        ft_normalize_2dot14(vx, vy).unwrap_or((0x4000, 0))
    }

    fn get_info(&self, selector: i32) -> i32 {
        let mut result = 0;
        if selector & 1 != 0 {
            result = 40;
        }
        // FreeType v40 sets `exec->grayscale = FALSE` in
        // `tt_loader_init`, so selector bit 5 does not return legacy
        // grayscale.  The subpixel/smoothing bits below are gated by
        // `exec->mode`, and mono deliberately clears them.
        if self.native_hint_mode != NativeHintMode::Mono {
            if selector & 64 != 0 {
                result |= 1 << 13;
            }
            if selector & 256 != 0 && self.native_hint_mode == NativeHintMode::LcdV {
                result |= 1 << 15;
            }
            if selector & 1024 != 0 {
                result |= 1 << 17;
            }
            if selector & 2048 != 0 {
                result |= 1 << 18;
            }
            if selector & 4096 != 0
                && self.native_hint_mode != NativeHintMode::Lcd
                && self.native_hint_mode != NativeHintMode::LcdV
            {
                result |= 1 << 19;
            }
        }
        result
    }

    // ── Glyph program execution ────────────────────────────────────

    /// Set the glyph instruction stream for execution.
    pub fn set_glyph_program(&mut self, ins: &[u8]) {
        // C: TT_Run_Context in ttinterp.c clears exec->top and exec->callTop
        // before each glyph execution.  Some fonts leave stack values behind.
        self.stack.clear();
        self.call_stack.clear();
        self.glyph_program = ins.to_vec();
        self.glyph_range = CodeRange {
            base: 0,
            size: ins.len(),
        };
        self.ip = 0;
        self.cur_range = 2; // glyph
        self.initial_range = 2;
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
            // Pinned FreeType `SkipCode` and `TT_RunIns` report
            // `FT_Err_Code_Overflow` when any opcode operand crosses codeSize.
            return Err(FontError::CodeOverflow);
        }
        let b = program[self.ip];
        self.ip += 1;
        Ok(b)
    }

    /// Main opcode dispatch loop for the glyph program.
    pub fn run_program(&mut self, zone: &mut GlyphZone) -> Result<(), FontError> {
        let mut step_count = 0u32;
        while self.ip < self.active_program_len() {
            if step_count > MAX_VM_STEPS {
                return Err(FontError::ExecutionTooLong);
            }
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
                        let hi = self.fetch_byte_glyph()? as u16;
                        let lo = self.fetch_byte_glyph()? as u16;
                        self.push(i16::from_be_bytes([hi as u8, lo as u8]) as i32);
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
                        let hi = self.fetch_byte_glyph()?;
                        let lo = self.fetch_byte_glyph()?;
                        self.push(i16::from_be_bytes([hi, lo]) as i32);
                    }
                }

                // ── Stack operations ─────────────────────────────
                0x20 => {
                    let v = self.top()?;
                    self.push(v);
                } // DUP
                0x21 => {
                    let _ = self.pop()?;
                } // POP
                0x22 => self.stack.clear(), // CLEAR
                0x23 => {
                    // SWAP
                    let a = self.pop()?;
                    let b = self.pop()?;
                    self.push(a);
                    self.push(b);
                }

                // ── Math ─────────────────────────────────────────
                0x60 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.wrapping_add(b));
                } // ADD
                0x61 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(a.wrapping_sub(b));
                } // SUB
                0x62 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    if b == 0 {
                        return Err(FontError::InvalidOutline(
                            "bytecode: division by zero".into(),
                        ));
                    }
                    // C: ttinterp.c Ins_DIV uses FT_MulDiv_No_Round.
                    // Noto bytecode function 56 depends on truncation here
                    // before FLOOR/SHPIX computes stem-dependent deltas.
                    self.push(crate::fixed::ft_mul_div_no_round(a, 64, b));
                } // DIV
                0x63 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    // C: Ins_MUL in ttinterp.c uses F26.6 stack arithmetic:
                    // FT_MulDiv(a, b, 64), not the 16.16 FT_MulFix helper.
                    self.push(crate::fixed::ft_mul_div(a, b, 64));
                } // MUL
                0x64 => {
                    let a = self.pop()?;
                    self.push(a.wrapping_abs());
                } // ABS
                0x65 => {
                    let a = self.pop()?;
                    self.push(a.wrapping_neg());
                } // NEG
                0x66 => {
                    let a = self.pop()?;
                    // C: Ins_FLOOR/Ins_CEILING in ttinterp.c use pixel-grid
                    // F26.6 rounding macros, not generic fixed floor/ceil.
                    self.push(crate::scaler::ft_pix_floor(a));
                } // FLOOR
                0x67 => {
                    let a = self.pop()?;
                    self.push(crate::scaler::ft_pix_ceil(a));
                } // CEILING

                // ── Storage ──────────────────────────────────────
                0x42 => {
                    // WS: pops index (deeper) then value (top)
                    let val = self.pop()?; // top = value
                    let idx = self.pop()? as usize; // deeper = index
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
                    let val = self.pop()?; // top = value
                    let idx = self.pop()? as usize; // deeper = index
                    self.set_cvt(idx, val)?;
                }
                0x45 => {
                    // RCVT: Read CVT — pops index, pushes value
                    let idx = self.pop()? as usize;
                    let val = self.get_cvt(idx)?;
                    self.push(val);
                }

                // ── Graphics state — vectors ─────────────────────
                0x00 => {
                    self.gs.set_vectors_to_y();
                } // SVTCA[y]
                0x01 => {
                    self.gs.set_vectors_to_x();
                } // SVTCA[x]
                0x02 => {
                    self.gs.set_proj_to_y();
                } // SPVTCA[y]
                0x03 => {
                    self.gs.set_proj_to_x();
                } // SPVTCA[x]
                0x04 => {
                    self.gs.set_free_to_y();
                } // SFVTCA[y]
                0x05 => {
                    self.gs.set_free_to_x();
                } // SFVTCA[x]

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
                0x4B => {
                    self.push(self.current_ppem());
                }
                0x4C => {
                    // C: `Ins_MPS` returns `exec->pointSize` for interpreter
                    // v40 subpixel-minimal mode; `MPPEM` is the ppem value.
                    self.push(self.point_size);
                }

                // ── MD — Measure Distance ────────────────────────
                // MD[a] measures zp0(args[0]) - zp1(args[1]).
                // C: Ins_MD in ttinterp.c:4400-4458.  The original-outline
                // case uses unscaled ORUS coordinates plus size scaling, not
                // the already-scaled org array.
                0x49 | 0x4A => {
                    let k = self.pop()? as usize;
                    let l = self.pop()? as usize;
                    let distance = if opcode & 1 != 0 {
                        let (x1, y1) = self.cur_in(zone, self.gs.zp0, l);
                        let (x2, y2) = self.cur_in(zone, self.gs.zp1, k);
                        self.gs.project(x1 - x2, y1 - y2)
                    } else if self.gs.zp0 == 0 || self.gs.zp1 == 0 {
                        let (x1, y1) = self.org_in(zone, self.gs.zp0, l);
                        let (x2, y2) = self.org_in(zone, self.gs.zp1, k);
                        self.gs.dual_project(x1 - x2, y1 - y2)
                    } else if self.x_scale == self.y_scale {
                        let (x1, y1) = self.orus_in(zone, self.gs.zp0, l);
                        let (x2, y2) = self.orus_in(zone, self.gs.zp1, k);
                        ft_mul_fix(self.gs.dual_project(x1 - x2, y1 - y2), self.x_scale)
                    } else {
                        let (x1, y1) = self.orus_in(zone, self.gs.zp0, l);
                        let (x2, y2) = self.orus_in(zone, self.gs.zp1, k);
                        self.gs.dual_project(
                            ft_mul_fix(x1 - x2, self.x_scale),
                            ft_mul_fix(y1 - y2, self.y_scale),
                        )
                    };
                    self.push(distance);
                }

                // ── GC — Get Coordinate ──────────────────────────
                0x46 => {
                    // GC[0] = get current coordinate of point in zp2
                    // Uses zp2 zone pointer to select zone, then
                    // projects the point's cur position onto proj vector
                    let p = self.pop()? as usize;
                    let (px, py) = self.cur_in(zone, self.gs.zp2, p);
                    let proj = self.gs.project(px, py);
                    self.push(proj);
                }
                0x47 => {
                    // GC[1] = get original coordinate
                    let p = self.pop()? as usize;
                    let (px, py) = self.org_in(zone, self.gs.zp2, p);
                    let proj = self.gs.dual_project(px, py);
                    self.push(proj);
                }

                // ── SCFS — Set Coordinate From Stack ─────────────
                0x48 => {
                    // C `Ins_SCFS` in ttinterp.c:4352-4379 receives
                    // args[0] as the point index and args[1] as the target
                    // coordinate.  The target is on top of the VM stack.
                    let val = self.pop()?;
                    let p = self.pop()? as usize;
                    let point_count = if self.gs.zp2 == 0 {
                        self.twilight.n_points as usize
                    } else {
                        zone.n_points as usize
                    };
                    if p >= point_count {
                        // C `Ins_SCFS` ignores invalid points normally and
                        // reports Invalid_Reference only for pedantic loads.
                        if self.pedantic_hinting {
                            return Err(FontError::InvalidReference);
                        }
                        continue;
                    }
                    let (cx, cy) = self.cur_in(zone, self.gs.zp2, p);
                    let old_proj = self.gs.project(cx, cy);
                    let dist = val - old_proj;
                    let (dx, dy) = self.gs.move_along_free(dist);
                    let new_x = cx + dx;
                    let new_y = cy + dy;
                    self.set_cur_in(zone, self.gs.zp2, p, new_x, new_y);
                    if self.gs.zp2 == 0 {
                        self.set_org_in(zone, self.gs.zp2, p, new_x, new_y);
                    }
                    self.touch_in(zone, self.gs.zp2, p);
                }

                // ── MDAP — Move Direct Absolute Point ────────────
                0x2E | 0x2F => {
                    // MDAP[0]/MDAP[1]: move projected coordinate, optionally rounded.
                    let p = self.pop()? as usize;
                    let (cx, cy) = self.cur_in(zone, self.gs.zp0, p);
                    let proj = self.gs.project(cx, cy);
                    let target = if opcode == 0x2F {
                        self.gs.round(proj)
                    } else {
                        proj
                    };
                    let (dx, dy) = self.gs.move_along_free(target - proj);
                    self.set_cur_in(zone, self.gs.zp0, p, cx + dx, cy + dy);
                    self.touch_in(zone, self.gs.zp0, p);
                    self.gs.rp0 = p as u32;
                    self.gs.rp1 = p as u32;
                }

                // ── MIAP — Move Indirect Absolute Point ──────────
                0x3E | 0x3F => {
                    let cvt_idx = self.pop()? as usize;
                    let p = self.pop()? as usize;
                    let cvt_val = self.get_cvt(cvt_idx)?;
                    if self.gs.zp0 == 0 {
                        let (ox, oy) = self.gs.move_along_raw_free(cvt_val);
                        self.set_org_in(zone, self.gs.zp0, p, ox, oy);
                        self.set_cur_in(zone, self.gs.zp0, p, ox, oy);
                    }
                    let (cx, cy) = self.cur_in(zone, self.gs.zp0, p);
                    let mut distance = cvt_val;
                    let org_dist = self.gs.project(cx, cy);
                    if opcode == 0x3F {
                        let delta = (distance - org_dist).abs();
                        if delta > self.gs.control_value_cutin {
                            distance = org_dist;
                        }
                        distance = self.gs.round(distance);
                    }
                    let delta = distance - org_dist;
                    let (dx, dy) = self.gs.move_along_free(delta);
                    self.set_cur_in(zone, self.gs.zp0, p, cx + dx, cy + dy);
                    self.touch_in(zone, self.gs.zp0, p);
                    self.gs.rp0 = p as u32;
                    self.gs.rp1 = p as u32;
                }

                // ── MDRP — Move Direct Relative Point ────────────
                // C: Ins_MDRP at ttinterp.c:5399-5519
                // Flag bits: round=bit2(0x04), min_dist=bit3(0x08), set_rp0=bit4(0x10)
                0xC0..=0xDF => {
                    let p = self.pop()? as usize;
                    let rp = self.gs.rp0 as usize;

                    let is_twilight = self.gs.zp0 == 0 || self.gs.zp1 == 0;
                    let mut org_dist = if is_twilight {
                        let (rorg_x, rorg_y) = self.org_in(zone, self.gs.zp0, rp);
                        let (oorg_x, oorg_y) = self.org_in(zone, self.gs.zp1, p);
                        self.gs.dual_project(oorg_x - rorg_x, oorg_y - rorg_y)
                    } else {
                        let (rorus_x, rorus_y) = self.orus_in(zone, self.gs.zp0, rp);
                        let (oorus_x, oorus_y) = self.orus_in(zone, self.gs.zp1, p);
                        if self.x_scale == self.y_scale {
                            let dist = self.gs.dual_project(oorus_x - rorus_x, oorus_y - rorus_y);
                            crate::fixed::ft_mul_fix(dist, self.x_scale)
                        } else {
                            self.gs.dual_project(
                                crate::fixed::ft_mul_fix(oorus_x - rorus_x, self.x_scale),
                                crate::fixed::ft_mul_fix(oorus_y - rorus_y, self.y_scale),
                            )
                        }
                    };

                    if self.gs.single_width_cutin > 0
                        && org_dist < self.gs.single_width_value + self.gs.single_width_cutin
                        && org_dist > self.gs.single_width_value - self.gs.single_width_cutin
                    {
                        org_dist = if org_dist >= 0 {
                            self.gs.single_width_value
                        } else {
                            -self.gs.single_width_value
                        };
                    }

                    let mut distance = if (opcode & 0x04) != 0 {
                        self.gs.round(org_dist)
                    } else {
                        org_dist
                    };

                    if (opcode & 0x08) != 0 {
                        let minimum_distance = self.minimum_distance();
                        // FreeType's Ins_MDRP treats a zero original distance
                        // as non-negative for the minimum-distance branch.
                        if org_dist >= 0 {
                            if distance < minimum_distance {
                                distance = minimum_distance;
                            }
                        } else if distance > -minimum_distance {
                            distance = -minimum_distance;
                        }
                    }

                    let (rcx, rcy) = self.cur_in(zone, self.gs.zp0, rp);
                    let (pcx, pcy) = self.cur_in(zone, self.gs.zp1, p);
                    let cur_dist = self.gs.project(pcx - rcx, pcy - rcy);
                    let (dx, dy) = self.gs.move_along_free(distance - cur_dist);
                    self.set_cur_in(zone, self.gs.zp1, p, pcx + dx, pcy + dy);
                    self.touch_in(zone, self.gs.zp1, p);
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
                    let cvt_idx = self.pop()?;
                    let p = self.pop()? as usize;
                    let mut cvt_dist = if cvt_idx < 0 {
                        0
                    } else {
                        self.get_cvt(cvt_idx as usize)?
                    };

                    let rp = self.gs.rp0 as usize;

                    let delta = (cvt_dist - self.gs.single_width_value).abs();
                    if delta < self.gs.single_width_cutin {
                        cvt_dist = if cvt_dist >= 0 {
                            self.gs.single_width_value
                        } else {
                            -self.gs.single_width_value
                        };
                    }

                    if self.gs.zp1 == 0 {
                        let (dx, dy) = self.gs.move_along_free(cvt_dist);
                        let (rox, roy) = self.org_in(zone, self.gs.zp0, rp);
                        self.set_org_in(zone, self.gs.zp1, p, rox + dx, roy + dy);
                        self.set_cur_in(zone, self.gs.zp1, p, rox + dx, roy + dy);
                    }
                    let (rorg_x, rorg_y) = self.org_in(zone, self.gs.zp0, rp);
                    let (oorg_x, oorg_y) = self.org_in(zone, self.gs.zp1, p);
                    let org_dist = self.gs.dual_project(oorg_x - rorg_x, oorg_y - rorg_y);
                    let (rcx, rcy) = self.cur_in(zone, self.gs.zp0, rp);
                    let (pcx, pcy) = self.cur_in(zone, self.gs.zp1, p);
                    let cur_dist = self.gs.project(pcx - rcx, pcy - rcy);

                    if self.gs.auto_flip && (org_dist ^ cvt_dist) < 0 {
                        cvt_dist = -cvt_dist;
                    }

                    let mut distance = if (opcode & 0x04) != 0 {
                        if self.gs.zp0 == self.gs.zp1
                            && (cvt_dist - org_dist).abs() > self.gs.control_value_cutin
                        {
                            cvt_dist = org_dist;
                        }
                        self.gs.round(cvt_dist)
                    } else {
                        cvt_dist
                    };

                    if (opcode & 0x08) != 0 {
                        let minimum_distance = self.minimum_distance();
                        // FreeType's Ins_MIRP treats a zero original distance
                        // as non-negative for the minimum-distance branch.
                        if org_dist >= 0 {
                            if distance < minimum_distance {
                                distance = minimum_distance;
                            }
                        } else if distance > -minimum_distance {
                            distance = -minimum_distance;
                        }
                    }

                    let (dx, dy) = self.gs.move_along_free(distance - cur_dist);
                    self.set_cur_in(zone, self.gs.zp1, p, pcx + dx, pcy + dy);
                    self.touch_in(zone, self.gs.zp1, p);

                    // C: rp1 = rp0, rp2 = point
                    self.gs.rp1 = rp as u32;
                    self.gs.rp2 = p as u32;
                    if (opcode & 0x10) != 0 {
                        self.gs.rp0 = p as u32;
                    }
                }

                // ── ALIGNRP (0x3C) — Align Relative Point ──
                // C: Ins_ALIGNRP (ttinterp.c:5673-5720).
                // Pops GS.loop counter points. For each, snaps position
                // to rp0: distance = PROJECT(cur[p], cur[rp0]), move by -distance
                0x3C => {
                    let loop_count = self.gs.loop_counter as usize;
                    let rp = self.gs.rp0 as usize;
                    let (rcx, rcy) = self.cur_in(zone, self.gs.zp0, rp);
                    for _ in 0..loop_count {
                        let p = self.pop()? as usize;
                        let (pcx, pcy) = self.cur_in(zone, self.gs.zp1, p);
                        let dist = self.gs.project(pcx - rcx, pcy - rcy);
                        let (dx, dy) = self.gs.move_along_free(-dist);
                        self.set_cur_in(zone, self.gs.zp1, p, pcx + dx, pcy + dy);
                        self.touch_in(zone, self.gs.zp1, p);
                    }
                    self.gs.loop_counter = 1; // C: GS.loop = 1
                }

                // ── SHP — Shift points by reference-point displacement ──
                0x32 | 0x33 => {
                    let loop_count = self.gs.loop_counter as usize;
                    let Some((dx, dy, _, _)) = self.point_displacement(opcode, zone)? else {
                        continue;
                    };
                    let point_limit = if self.gs.zp2 == 0 {
                        self.twilight.n_points as usize
                    } else {
                        zone.n_points as usize
                    };
                    for _ in 0..loop_count {
                        let p = self.pop()? as usize;
                        // C `Ins_SHP` ignores out-of-zone points normally but
                        // reports Invalid_Reference for FT_LOAD_PEDANTIC.
                        if p >= point_limit && self.pedantic_hinting {
                            return Err(FontError::InvalidReference);
                        }
                        let (cx, cy) = self.cur_in(zone, self.gs.zp2, p);
                        self.set_cur_in(zone, self.gs.zp2, p, cx + dx, cy + dy);
                        self.touch_in(zone, self.gs.zp2, p);
                    }
                    self.gs.loop_counter = 1;
                }

                // ── SHC — Shift contour by reference-point displacement ──
                0x34 | 0x35 => {
                    let contour = self.pop()? as usize;
                    if contour < zone.contours.len() {
                        let Some((dx, dy, ref_zone, ref_pt)) =
                            self.point_displacement(opcode, zone)?
                        else {
                            continue;
                        };
                        let start = if contour == 0 {
                            0
                        } else {
                            zone.contours[contour - 1] as usize + 1
                        };
                        let limit = zone.contours[contour] as usize + 1;
                        for p in start..limit {
                            if self.gs.zp2 != ref_zone || p != ref_pt {
                                let (cx, cy) = self.cur_in(zone, self.gs.zp2, p);
                                self.set_cur_in(zone, self.gs.zp2, p, cx + dx, cy + dy);
                                self.touch_in(zone, self.gs.zp2, p);
                            }
                        }
                    }
                }

                // ── SHZ — Shift zone by reference-point displacement ──
                0x36 | 0x37 => {
                    let _zone_selector = self.pop()?;
                    let Some((dx, dy, ref_zone, ref_pt)) = self.point_displacement(opcode, zone)?
                    else {
                        continue;
                    };
                    let target_zp = self.gs.zp2;
                    let limit = if target_zp == 0 {
                        self.twilight.n_points as usize
                    } else if zone.n_contours > 0 {
                        zone.n_real_points()
                    } else {
                        0
                    };
                    for p in 0..limit {
                        if target_zp != ref_zone || p != ref_pt {
                            let (cx, cy) = self.cur_in(zone, target_zp, p);
                            self.set_cur_in(zone, target_zp, p, cx + dx, cy + dy);
                        }
                    }
                }

                // ── IUP — Interpolate Untouched Points ────────────
                // C: Ins_IUP (ttinterp.c:6189+); implementation lives in
                // hinter/iup.rs because it operates on whole contours.
                0x30 => {
                    if self.backward_compatibility != 0 {
                        if self.backward_compatibility == 0x7 {
                            continue;
                        }
                        self.backward_compatibility |= 1;
                    }
                    iup::iup_y(zone);
                }
                0x31 => {
                    if self.backward_compatibility != 0 {
                        if self.backward_compatibility == 0x7 {
                            continue;
                        }
                        self.backward_compatibility |= 2;
                    }
                    iup::iup_x(zone);
                }

                // ── Control flow ──────────────────────────────────
                // SLOOP (0x17): set loop counter
                0x17 => {
                    let v = self.pop()?;
                    self.gs.loop_counter = v;
                }
                // LOOPCALL (0x2A): top is function number, deeper is count.
                // C: Ins_LOOPCALL uses args[1] as FDEF id and args[0] as count
                // after the VM has popped two operands into the argument area.
                0x2A => {
                    let func_num = self.pop()?;
                    let count = self.pop()?;
                    self.enter_function_call(func_num, count)?;
                }
                // CALL (0x2B): pop function number from top of stack
                0x2B => {
                    let func_num = self.pop()?;
                    self.enter_function_call(func_num, 1)?;
                }
                0x2D => {
                    // ENDF: return from function
                    if let Some(call) = self.call_stack.pop() {
                        if call.cur_count > 1 {
                            // Pinned FreeType `Ins_ENDF` dereferences the mutable
                            // `TT_CallRec.Def` on every LOOPCALL repeat so an FDEF
                            // redefinition by a broken font changes the next body.
                            let def = self
                                .functions
                                .get(call.def_index)
                                .and_then(Option::as_ref)
                                .ok_or_else(|| {
                                    FontError::InvalidOutline(
                                        "bytecode: missing function definition".into(),
                                    )
                                })?;
                            let def_start = def.start;
                            let def_range = def.range;
                            self.call_stack.push(CallRecord {
                                cur_count: call.cur_count - 1,
                                ..call
                            });
                            self.ip = def_start;
                            self.cur_range = def_range;
                        } else {
                            self.ip = call.caller_ip;
                            self.cur_range = call.caller_range;
                        }
                    }
                }
                0x2C => {
                    self.define_function()?;
                }

                // ── IF (0x58) / ELSE (0x1B) ──────────────────────
                0x58 => {
                    let condition = self.pop()?;
                    if condition == 0 {
                        self.skip_to_else_or_eif()?;
                    }
                }
                0x1B => {
                    self.skip_to_eif()?;
                }

                // ── SHPIX — Shift Pixel ───────────────────────────
                0x38 => {
                    let amount = self.pop()?;
                    let (dx, dy) = self.gs.move_along_raw_free(amount);
                    let in_twilight = self.gs.zp0 == 0 || self.gs.zp1 == 0 || self.gs.zp2 == 0;
                    for _ in 0..self.gs.loop_counter {
                        let p = self.pop()? as usize;
                        let (cx, cy) = self.cur_in(zone, self.gs.zp2, p);
                        if self.backward_compatibility == 0 {
                            self.set_cur_in(zone, self.gs.zp2, p, cx + dx, cy + dy);
                            self.touch_in(zone, self.gs.zp2, p);
                        } else if in_twilight
                            || (self.backward_compatibility != 0x7
                                && ((self.is_composite && self.gs.freedom_vector.1 != 0)
                                    || (self
                                        .tag_in(zone, self.gs.zp2, p)
                                        .is_some_and(|tag| tag & 0x02 != 0))))
                        {
                            // C `Ins_SHPIX` in v40 compatibility mode treats
                            // SHPIX like DELTAP: block until the point is
                            // Y-touched, except twilight and composite-y moves.
                            self.set_cur_in(zone, self.gs.zp2, p, cx, cy + dy);
                            self.touch_in(zone, self.gs.zp2, p);
                        }
                    }
                    self.gs.loop_counter = 1;
                }

                // ── ALIGNPTS (0x27) — Align points ──────────
                0x27 => {
                    let p1 = self.pop()? as usize;
                    let p2 = self.pop()? as usize;
                    let (p1x, p1y) = self.cur_in(zone, self.gs.zp1, p1);
                    let (p2x, p2y) = self.cur_in(zone, self.gs.zp0, p2);
                    let distance = self.gs.project(p1x - p2x, p1y - p2y) / 2;
                    let (dx1, dy1) = self.gs.move_along_free(distance);
                    let (dx2, dy2) = self.gs.move_along_free(-distance);
                    self.set_cur_in(zone, self.gs.zp1, p1, p1x + dx1, p1y + dy1);
                    self.set_cur_in(zone, self.gs.zp0, p2, p2x + dx2, p2y + dy2);
                    self.touch_in(zone, self.gs.zp1, p1);
                    self.touch_in(zone, self.gs.zp0, p2);
                }
                // ── CINDEX (0x25) — Copy indexed element ─────────
                0x25 => {
                    let k = self.pop()? as usize;
                    if k > 0 && k <= self.stack.len() {
                        let v = self.stack[self.stack.len() - k];
                        self.push(v);
                    } else {
                        self.push(0);
                    }
                }
                // ── MINDEX (0x26) — Move indexed element ─────────
                0x26 => {
                    let k = self.pop()? as usize;
                    if k > 0 && k <= self.stack.len() {
                        let v = self.stack.remove(self.stack.len() - k);
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
                    for _ in 0..self.gs.loop_counter {
                        let p = self.pop()? as usize;
                        // Toggle on-curve flag
                        // We don't track this precisely, just mark touched
                        self.touch_point(zone, p);
                    }
                    self.gs.loop_counter = 1;
                }
                // ── ROLL (0x8A) — rotate the top three stack entries ─
                0x8A => {
                    if self.stack.len() >= 3 {
                        let a = self.pop()?;
                        let b = self.pop()?;
                        let c = self.pop()?;
                        self.push(b);
                        self.push(a);
                        self.push(c);
                    }
                }
                // ── JMPR (0x1C) — Jump Relative ──────────────────
                0x1C => {
                    let offset = self.pop()? - 1;
                    self.ip = (self.ip as i32 + offset) as usize;
                }
                // ── JROT (0x78) — pops condition(top), offset(deeper)
                // C: Ins_JROT/Ins_JROF test args[1] and pass args[0] as the
                // relative jump offset.  These are not comparison opcodes.
                0x78 => {
                    let condition = self.pop()?;
                    let offset = self.pop()?;
                    if condition != 0 {
                        self.ip = (self.ip as i32 + offset - 1) as usize;
                    }
                }
                // ── JROF (0x79) — pops condition(top), offset(deeper)
                0x79 => {
                    let condition = self.pop()?;
                    let offset = self.pop()?;
                    if condition == 0 {
                        self.ip = (self.ip as i32 + offset - 1) as usize;
                    }
                }
                // ── SFVTL (0x08-0x09) — Set Freedom Vector To Line ──
                0x08 | 0x09 => {
                    let p_top = self.pop()? as usize;
                    let p_deeper = self.pop()? as usize;
                    let (x1, y1) = self.cur_in(zone, self.gs.zp2, p_deeper);
                    let (x2, y2) = self.cur_in(zone, self.gs.zp1, p_top);
                    self.gs.freedom_vector = Self::line_vector(
                        x2.wrapping_sub(x1),
                        y2.wrapping_sub(y1),
                        opcode & 1 != 0,
                    );
                    self.gs.compute_move_vector();
                }
                // ── SPVFS (0x0A) — Set Proj Vector From Stack ──────
                0x0A => {
                    let y = self.pop()? as i16 as i32;
                    let x = self.pop()? as i16 as i32;
                    if let Some(vector) = ft_normalize_2dot14(x, y) {
                        self.gs.proj_vector = vector;
                    }
                    self.gs.dual_proj_vector = self.gs.proj_vector;
                    self.gs.compute_move_vector();
                }
                // ── SFVFS (0x0B) — Set Freedom Vector From Stack ────
                0x0B => {
                    let y = self.pop()? as i16 as i32;
                    let x = self.pop()? as i16 as i32;
                    if let Some(vector) = ft_normalize_2dot14(x, y) {
                        self.gs.freedom_vector = vector;
                    }
                    self.gs.compute_move_vector();
                }
                // ── SFVTPV (0x0E) — Set Freedom Vector To Proj Vector ─
                0x0E => {
                    self.gs.freedom_vector = self.gs.proj_vector;
                    self.gs.compute_move_vector();
                }
                // ── DEPTH (0x24) — Push stack depth ─────────────────
                0x24 => {
                    self.push(self.stack.len() as i32);
                }
                // ── IP (0x39) — Interpolate Point ───────────────────
                // C: Ins_IP (ttinterp.c:5854-5940).
                // Pops GS.loop points. Interpolates between rp1 and rp2.
                // Uses orus for glyph zone, org for twilight zone.
                // old_range via DUALPROJ, cur_range via PROJECT.
                0x39 => {
                    let loop_count = self.gs.loop_counter;
                    let rp1 = self.gs.rp1 as usize;
                    let rp2 = self.gs.rp2 as usize;
                    let use_twilight_org = self.gs.zp0 == 0 || self.gs.zp1 == 0 || self.gs.zp2 == 0;
                    let (r1_ox, r1_oy) = if use_twilight_org {
                        self.org_in(zone, self.gs.zp0, rp1)
                    } else {
                        self.orus_in(zone, self.gs.zp0, rp1)
                    };
                    let (r2_ox, r2_oy) = if use_twilight_org {
                        self.org_in(zone, self.gs.zp1, rp2)
                    } else {
                        self.orus_in(zone, self.gs.zp1, rp2)
                    };
                    let (r1_cx, r1_cy) = self.cur_in(zone, self.gs.zp0, rp1);
                    let (r2_cx, r2_cy) = self.cur_in(zone, self.gs.zp1, rp2);
                    // C: old_range = DUALPROJ(orgs2 - orgs1), cur_range = PROJECT(curs2 - curs1)
                    let old_range = if use_twilight_org {
                        self.gs.dual_project(r2_ox - r1_ox, r2_oy - r1_oy)
                    } else if self.x_scale == self.y_scale {
                        // C: ttinterp.c Ins_IP projects unscaled ORUS directly
                        // when x/y scales match; scaling here introduces 26.6
                        // rounding drift in interpolated native TT outlines.
                        self.gs.dual_project(r2_ox - r1_ox, r2_oy - r1_oy)
                    } else {
                        self.gs.dual_project(
                            crate::fixed::ft_mul_fix(r2_ox - r1_ox, self.x_scale),
                            crate::fixed::ft_mul_fix(r2_oy - r1_oy, self.y_scale),
                        )
                    };
                    let cur_range = self.gs.project(r2_cx - r1_cx, r2_cy - r1_cy);
                    for _ in 0..loop_count {
                        let p = self.pop()? as usize;
                        let (pox, poy) = if use_twilight_org {
                            self.org_in(zone, self.gs.zp2, p)
                        } else {
                            self.orus_in(zone, self.gs.zp2, p)
                        };
                        let p_old = if use_twilight_org {
                            self.gs.dual_project(pox - r1_ox, poy - r1_oy)
                        } else if self.x_scale == self.y_scale {
                            self.gs.dual_project(pox - r1_ox, poy - r1_oy)
                        } else {
                            self.gs.dual_project(
                                crate::fixed::ft_mul_fix(pox - r1_ox, self.x_scale),
                                crate::fixed::ft_mul_fix(poy - r1_oy, self.y_scale),
                            )
                        };
                        let (pcx, pcy) = self.cur_in(zone, self.gs.zp2, p);
                        let cur_dist = self.gs.project(pcx - r1_cx, pcy - r1_cy);
                        let new_dist = if p_old == 0 {
                            0
                        } else if old_range == 0 {
                            // C Ins_IP uses `org_dist` unchanged when the
                            // reference range collapses.
                            p_old
                        } else {
                            // C: ttinterp.c Ins_IP uses signed FT_MulDiv
                            // directly.  Do not clamp points outside the
                            // reference range; fonts rely on extrapolation.
                            crate::fixed::ft_mul_div(p_old, cur_range, old_range)
                        };
                        let (dx, dy) = self.gs.move_along_free(new_dist - cur_dist);
                        self.set_cur_in(zone, self.gs.zp2, p, pcx + dx, pcy + dy);
                        self.touch_in(zone, self.gs.zp2, p);
                    }
                    self.gs.loop_counter = 1; // C: GS.loop = 1
                }
                // ── LT (0x50) — Less Than ───────────────────────────
                0x50 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a < b { 1 } else { 0 });
                }
                // ── LTEQ (0x51) ─────────────────────────────────────
                0x51 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a <= b { 1 } else { 0 });
                }
                // ── GT (0x52) ───────────────────────────────────────
                0x52 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a > b { 1 } else { 0 });
                }
                // ── GTEQ (0x53) ─────────────────────────────────────
                0x53 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a >= b { 1 } else { 0 });
                }
                // ── EQ (0x54) ───────────────────────────────────────
                0x54 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a == b { 1 } else { 0 });
                }
                // ── NEQ (0x55) ──────────────────────────────────────
                0x55 => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a != b { 1 } else { 0 });
                }
                // ── OR (0x5B) — Logical OR ──────────────────────────
                0x5B => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a != 0 || b != 0 { 1 } else { 0 });
                }
                // ── FLIPRGON (0x81) / FLIPRGOFF (0x82) ─────────────
                0x81 => {
                    let _ = self.pop()?;
                    let _ = self.pop()?;
                } // FLIPRGON
                0x82 => {
                    let _ = self.pop()?;
                    let _ = self.pop()?;
                } // FLIPRGOFF
                // ── SPVTL (0x06-0x07) — Set Projection Vector To Line ──
                // C: Ins_SPVTL (ttinterp.c) — same logic as SFVTL but for proj
                0x06 | 0x07 => {
                    let p_top = self.pop()? as usize;
                    let p_deeper = self.pop()? as usize;
                    let (x1, y1) = self.cur_in(zone, self.gs.zp2, p_deeper);
                    let (x2, y2) = self.cur_in(zone, self.gs.zp1, p_top);
                    let vector = Self::line_vector(
                        x2.wrapping_sub(x1),
                        y2.wrapping_sub(y1),
                        opcode & 1 != 0,
                    );
                    self.gs.proj_vector = vector;
                    self.gs.dual_proj_vector = vector;
                    self.gs.compute_move_vector();
                }
                // ── GPV (0x0C) — Get Projection Vector ─────────────
                0x0C => {
                    // Pushes proj_vector.x and proj_vector.y onto stack
                    self.push(self.gs.proj_vector.0);
                    self.push(self.gs.proj_vector.1);
                }
                // ── GFV (0x0D) — Get Freedom Vector ────────────────
                0x0D => {
                    self.push(self.gs.freedom_vector.0);
                    self.push(self.gs.freedom_vector.1);
                }
                // ── ISECT (0x0F) — Move point to intersection of two lines ──
                // C: Ins_ISECT (ttinterp.c:5725-5811).  The args array is
                // laid out in the original pushed order, so the interpreter
                // pops the five stack entries in reverse.
                0x0F => {
                    let b1 = self.pop()? as usize;
                    let b0 = self.pop()? as usize;
                    let a1 = self.pop()? as usize;
                    let a0 = self.pop()? as usize;
                    let point = self.pop()? as usize;

                    let len0 = if self.gs.zp0 == 0 {
                        self.twilight.cur_x.len()
                    } else {
                        zone.cur_x.len()
                    };
                    let len1 = if self.gs.zp1 == 0 {
                        self.twilight.cur_x.len()
                    } else {
                        zone.cur_x.len()
                    };
                    let len2 = if self.gs.zp2 == 0 {
                        self.twilight.cur_x.len()
                    } else {
                        zone.cur_x.len()
                    };
                    if b0 >= len0 || b1 >= len0 || a0 >= len1 || a1 >= len1 || point >= len2 {
                        continue;
                    }

                    let (b0x, b0y) = self.cur_in(zone, self.gs.zp0, b0);
                    let (b1x, b1y) = self.cur_in(zone, self.gs.zp0, b1);
                    let (a0x, a0y) = self.cur_in(zone, self.gs.zp1, a0);
                    let (a1x, a1y) = self.cur_in(zone, self.gs.zp1, a1);

                    let dbx = b1x.wrapping_sub(b0x);
                    let dby = b1y.wrapping_sub(b0y);
                    let dax = a1x.wrapping_sub(a0x);
                    let day = a1y.wrapping_sub(a0y);
                    let dx = b0x.wrapping_sub(a0x);
                    let dy = b0y.wrapping_sub(a0y);

                    let discriminant = ft_mul_div(dax, dby.wrapping_neg(), 0x40)
                        .wrapping_add(ft_mul_div(day, dbx, 0x40));
                    let dotproduct =
                        ft_mul_div(dax, dbx, 0x40).wrapping_add(ft_mul_div(day, dby, 0x40));

                    let (new_x, new_y) =
                        if 19i64 * i64::from(discriminant).abs() > i64::from(dotproduct).abs() {
                            let val = ft_mul_div(dx, dby.wrapping_neg(), 0x40)
                                .wrapping_add(ft_mul_div(dy, dbx, 0x40));
                            (
                                a0x.wrapping_add(ft_mul_div(val, dax, discriminant)),
                                a0y.wrapping_add(ft_mul_div(val, day, discriminant)),
                            )
                        } else {
                            (
                                a0x.wrapping_add(a1x).wrapping_add(b0x.wrapping_add(b1x)) / 4,
                                a0y.wrapping_add(a1y).wrapping_add(b0y.wrapping_add(b1y)) / 4,
                            )
                        };

                    if self.gs.zp2 == 0 {
                        self.twilight.set_cur(point, new_x, new_y);
                        self.twilight.set_tag(point, 0x03);
                    } else {
                        zone.set_cur(point, new_x, new_y);
                        zone.set_tag(point, 0x03);
                    }
                }
                // ── Rounding modes ────────────────────────────────
                0x18 => {
                    self.gs.round_state = RoundMode::Grid;
                } // RTG
                0x19 => {
                    self.gs.round_state = RoundMode::HalfGrid;
                } // RTHG
                0x3D => {
                    self.gs.round_state = RoundMode::DoubleGrid;
                } // RTDG
                0x7A => {
                    self.gs.round_state = RoundMode::Off;
                } // ROFF
                0x7C => {
                    self.gs.round_state = RoundMode::UpToGrid;
                } // RUTG
                0x7D => {
                    self.gs.round_state = RoundMode::DownToGrid;
                } // RDTG
                0x7E => {
                    let _ = self.pop()?;
                } // SANGW
                0x7F => {
                    let _ = self.pop()?;
                } // AA
                // ── SROUND (0x76) — Super Round ─────────────────────
                0x76 => {
                    let selector = self.pop()?;
                    self.gs.set_super_round(0x4000, selector);
                    self.gs.round_state = RoundMode::Super;
                }
                // ── S45ROUND (0x77) — Super Round 45 ────────────────
                0x77 => {
                    let selector = self.pop()?;
                    self.gs.set_super_round(0x2D41, selector);
                    self.gs.round_state = RoundMode::Super45;
                }
                // ── WCVTF (0x70) — Write CVT in Font Units ──────────
                // C: Ins_WCVTF. Scales value by metrics.scale before writing.
                0x70 => {
                    let val = self.pop()?; // top = value
                    let idx = self.pop()? as usize; // deeper = index
                    // Scale: FT_MulFix(value, scale) then write to CVT
                    let scaled = crate::fixed::ft_mul_fix(val, self.tt_scale);
                    // C `Ins_WCVTF` uses the same normal no-op / pedantic
                    // Invalid_Reference split as WCVTP.
                    // C `Ins_WCVTF` writes raw `FT_MulFix(value, scale)`;
                    // unlike WCVTP it does not use the stretched callback.
                    self.set_cvt_raw(idx, scaled)?;
                }
                // ── GetINFO (0x88) — Get Info ───────────────────────
                0x88 => {
                    let selector = self.pop()?;
                    self.push(self.get_info(selector));
                }

                // ── UTP (0x29) — UnTouch Point ───────────────────
                // C: Ins_UTP. Pops point in zp0 and clears touch bits selected
                // by the current freedom vector.
                0x29 => {
                    let p = self.pop()? as usize;
                    let point_limit = if self.gs.zp0 == 0 {
                        self.twilight.n_points as usize
                    } else {
                        zone.n_points as usize
                    };
                    // C `Ins_UTP` uses the same non-pedantic ignore and
                    // pedantic Invalid_Reference split as SHP.
                    if p >= point_limit && self.pedantic_hinting {
                        return Err(FontError::InvalidReference);
                    }
                    self.clear_touch_in(zone, self.gs.zp0, p);
                }
                // ── MSIRP (0x3A-0x3B) — Move Stack Indirect Relative Point ──
                // Like MIRP, but uses a stack-provided distance and does not
                // apply rounding or control-value cut-in.
                0x3A | 0x3B => {
                    // C: `Ins_MSIRP` in `ttinterp.c` receives args[0] as the
                    // point and args[1] as the distance. With this VM's pop
                    // model, the stack top is the distance produced by the
                    // preceding instruction, followed by the point.
                    let distance = self.pop()?;
                    let p = self.pop()? as usize;
                    let rp = self.gs.rp0 as usize;

                    if self.gs.zp1 == 0 {
                        let (rox, roy) = self.org_in(zone, self.gs.zp0, rp);
                        let (dx, dy) = self.gs.move_along_raw_free(distance);
                        self.set_org_in(zone, self.gs.zp1, p, rox + dx, roy + dy);
                        self.set_cur_in(zone, self.gs.zp1, p, rox + dx, roy + dy);
                    }

                    let (rcx, rcy) = self.cur_in(zone, self.gs.zp0, rp);
                    let (pcx, pcy) = self.cur_in(zone, self.gs.zp1, p);
                    let cur_dist = self.gs.project(pcx - rcx, pcy - rcy);
                    let (dx, dy) = self.gs.move_along_free(distance - cur_dist);
                    self.set_cur_in(zone, self.gs.zp1, p, pcx + dx, pcy + dy);
                    self.touch_in(zone, self.gs.zp1, p);

                    self.gs.rp1 = self.gs.rp0;
                    self.gs.rp2 = p as u32;
                    if opcode & 1 != 0 {
                        self.gs.rp0 = p as u32;
                    }
                }
                // ── AND (0x5A) — Logical AND ───────────────────────
                // C: Ins_AND (ttinterp.c:2588-2601)
                0x5A => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a != 0 && b != 0 { 1 } else { 0 });
                }
                // ── SDB (0x5E) — Set Delta Base ────────────────────
                0x5E => {
                    let v = self.pop()?;
                    self.gs.delta_base = v as u32;
                }
                // ── SDS (0x5F) — Set Delta Shift ───────────────────
                0x5F => {
                    let v = self.pop()?;
                    self.gs.delta_shift = v as u32;
                }
                // ── SDPVTL (0x86-0x87) — Set Dual Projection Vector To Line ──
                0x86 | 0x87 => {
                    let p_top = self.pop()? as usize;
                    let p_deeper = self.pop()? as usize;
                    let (x1, y1) = zone.org(p_deeper);
                    let (x2, y2) = zone.org(p_top);
                    self.gs.dual_proj_vector = Self::line_vector(
                        x2.wrapping_sub(x1),
                        y2.wrapping_sub(y1),
                        opcode & 1 != 0,
                    );
                    self.gs.compute_move_vector();
                }

                // ── DELTAP/DELTAC — Delta exceptions ───────────────
                // C: Ins_DELTAP (ttinterp.c:6300-6395) and
                // Ins_DELTAC (ttinterp.c:6396-6475).
                // Per-ppem point/CVT adjustments. Pops count then
                // (point_index, delta) pairs. Applies delta * F to
                // points (DELTAP) or CVT entries (DELTAC) when the
                // ppem range matches.
                0x5D | 0x71 | 0x72 => {
                    // DELTAP: Move points by delta
                    let count = self.pop()?;
                    let nump = if count < 0 || count > self.stack.len() as i32 / 2 {
                        self.stack.len() as i32 / 2
                    } else {
                        count
                    };
                    // C: P = ppem - delta_base, range offset by opcode.
                    let base_ppem = self.current_ppem();
                    let p = base_ppem
                        - self.gs.delta_base as i32
                        - match opcode {
                            0x71 => 16,
                            0x72 => 32,
                            _ => 0,
                        };
                    if (p & !0xF) != 0 {
                        // P < 0 || P > 15 → skip
                        // Consume the args without processing
                        for _ in 0..nump {
                            let _ = self.pop()?;
                            let _ = self.pop()?;
                        }
                    } else {
                        let ppem_bits = p << 4; // P << 4 for matching
                        let f = delta_step(self.gs.delta_shift);
                        for _ in 0..nump {
                            let a = self.pop()? as usize; // point index
                            let b = self.pop()?; // delta + ppem bits
                            if a < zone.n_points as usize && (b & 0xF0) == ppem_bits {
                                let mut d = (b & 0x0F) - 8;
                                if d >= 0 {
                                    d += 1;
                                }
                                d *= f;
                                let (dx, dy) = self.gs.move_along_free(d);
                                let (cx, cy) = zone.cur(a);
                                if self.backward_compatibility == 0
                                    || (self.backward_compatibility != 0x7
                                        && ((self.is_composite && self.gs.freedom_vector.1 != 0)
                                            || (zone.tag(a) & 0x02) != 0))
                                {
                                    self.set_glyph_cur(zone, a, cx + dx, cy + dy);
                                    self.touch_point(zone, a);
                                }
                            }
                        }
                    }
                }
                0x73..=0x75 => {
                    // DELTAC: Adjust CVT values by delta
                    let count = self.pop()?;
                    let nump = if count < 0 || count > self.stack.len() as i32 / 2 {
                        self.stack.len() as i32 / 2
                    } else {
                        count
                    };
                    let base_ppem = self.current_ppem();
                    let p = base_ppem
                        - self.gs.delta_base as i32
                        - match opcode {
                            0x74 => 16,
                            0x75 => 32,
                            _ => 0,
                        };
                    if (p & !0xF) != 0 {
                        for _ in 0..nump {
                            let _ = self.pop()?;
                            let _ = self.pop()?;
                        }
                    } else {
                        let ppem_bits = p << 4;
                        let f = delta_step(self.gs.delta_shift);
                        for _ in 0..nump {
                            let a = self.pop()? as usize;
                            let b = self.pop()?;
                            if a >= self.cvt.len() {
                                // C `Ins_DELTAC` validates the CVT reference
                                // before testing the encoded ppem.
                                if self.pedantic_hinting {
                                    return Err(FontError::InvalidReference);
                                }
                                continue;
                            }
                            if (b & 0xF0) == ppem_bits {
                                let mut d = (b & 0x0F) - 8;
                                if d >= 0 {
                                    d += 1;
                                }
                                d *= f;
                                // The explicit C-equivalent bounds check above
                                // proves that `move_cvt` cannot fail here.
                                self.move_cvt(a, d)?;
                            }
                        }
                    }
                }
                // ── SZP0 (0x13) — Set Zone Pointer 0 ─────────────
                // C: Ins_SZP0 (ttinterp.h). Pops zp selector, sets GS.zp0.
                0x13 => {
                    self.gs.zp0 = (self.pop()? & 1) as u8;
                }
                // ── SZP1 (0x14) — Set Zone Pointer 1 ─────────────
                0x14 => {
                    self.gs.zp1 = (self.pop()? & 1) as u8;
                }
                // ── SZP2 (0x15) — Set Zone Pointer 2 ─────────────
                0x15 => {
                    self.gs.zp2 = (self.pop()? & 1) as u8;
                }
                // ── SZPS (0x16) — Set Zone Pointers ───────────────
                // C: Sets all three zone pointers from single stack value.
                0x16 => {
                    let v = (self.pop()? & 1) as u8;
                    self.gs.zp0 = v;
                    self.gs.zp1 = v;
                    self.gs.zp2 = v;
                }
                // ── SCVTCI (0x1D) — Set Control Value Table Cut-In ─
                // C: Ins_SCVTCI (ttinterp.c:4087). Pops value → GS.control_value_cutin.
                0x1D => {
                    self.gs.control_value_cutin = self.pop()?;
                }
                // ── SSWCI (0x1E) — Set Single Width Cut-In ────────
                // C: Ins_SSWCI (ttinterp.c:4101). Pops value → GS.single_width_cutin.
                0x1E => {
                    self.gs.single_width_cutin = self.pop()?;
                }
                // ── SSW (0x1F) — Set Single Width ──────────────────
                // C: Ins_SSW (ttinterp.c:4115). Pops value → GS.single_width_value.
                0x1F => {
                    let v = self.pop()?;
                    self.gs.single_width_value = crate::fixed::ft_mul_fix(v, self.tt_scale);
                }
                // ── RAW (0x28) — ??? ───────────────────────────────
                // C: Undocumented. Skip.
                0x28 => {}
                // ── FLIPON (0x4D) ──────────────────────────────────
                // C: Ins_FLIPON (ttinterp.c:4130). Sets auto_flip=true.
                0x4D => {
                    self.gs.auto_flip = true;
                }
                // ── FLIPOFF (0x4E) ─────────────────────────────────
                // C: Ins_FLIPOFF (ttinterp.c:4143). Sets auto_flip=false.
                0x4E => {
                    self.gs.auto_flip = false;
                }
                // ── DEBUG (0x4F) ───────────────────────────────────
                // C: Ins_DEBUG. No-op in release mode.
                0x4F => {}
                // ── ODD (0x56) — Is Odd ────────────────────────────
                0x56 => {
                    let a = self.pop()?;
                    self.push(if (self.gs.round(a) & 64) == 64 { 1 } else { 0 });
                }
                // ── EVEN (0x57) — Is Even ──────────────────────────
                0x57 => {
                    let a = self.pop()?;
                    self.push(if (self.gs.round(a) & 64) == 0 { 1 } else { 0 });
                }
                // ── EIF (0x59) — End If ──────────────────────────
                0x59 => {}
                // ── NOT (0x5C) — Logical NOT ────────────────────────
                0x5C => {
                    let a = self.pop()?;
                    self.push(if a == 0 { 1 } else { 0 });
                }
                // ── DELTAP1 (0x5D) — already handled above ─────────
                // ── ROUND (0x68-0x6B) — Round variants ────────────
                0x68..=0x6B => {
                    let v = self.pop()?;
                    self.push(self.gs.round(v));
                }
                // ── NROUND (0x6C-0x6F) — No Round ─────────────────
                0x6C..=0x6F => {
                    let v = self.pop()?;
                    self.push(v);
                }
                // ── DELTAC1 (0x73) — already handled above ─────────
                // ── Unknown opcodes (FreeType routes these through IDEF) ──
                0x7B | 0x83 | 0x84 => {
                    let _ = self.call_instruction_def(opcode);
                }
                // ── SCANCTRL (0x85) ──────────────────────────────
                // C: Ins_SCANCTRL (ttinterp.c:4739-4781). Rotated and
                // stretched flags are false for the currently supported
                // untransformed size requests, so only ppem predicates apply.
                0x85 => {
                    let v = self.pop()?;
                    let threshold = v & 0xFF;
                    if threshold == 0xFF {
                        self.gs.scan_control = true;
                        continue;
                    }
                    if threshold == 0 {
                        self.gs.scan_control = false;
                        continue;
                    }
                    if v & 0x100 != 0 && self.ppem <= threshold {
                        self.gs.scan_control = true;
                    }
                    if v & 0x800 != 0 && self.ppem > threshold {
                        self.gs.scan_control = false;
                    }
                }
                // ── SDPVTL (0x86) — already covered by 0x87 above ──
                // ── IDEF (0x89) — Instruction Definition ────────────
                0x89 => {
                    self.define_instruction()?;
                }

                // ── MAX (0x8B) — Maximum ────────────────────────
                // C: Ins_MAX. Pops a, b, pushes max(a,b).
                0x8B => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a > b { a } else { b });
                }
                // ── MIN (0x8C) — Minimum ────────────────────────
                0x8C => {
                    let b = self.pop()?;
                    let a = self.pop()?;
                    self.push(if a < b { a } else { b });
                }
                // ── SCANTYPE (0x8D) — Set Scan Type ──────────────
                // C: Ins_SCANTYPE. Pops value, sets GS.scan_type.
                0x8D => {
                    let v = self.pop()?;
                    if v >= 0 {
                        self.gs.scan_type = v as u8;
                    }
                }
                // ── INSTCTRL (0x8E) — Set Instruction Control ────
                // C `Ins_INSTCTRL` reads the selector from `args[1]` and the
                // value from `args[0]` (`ttinterp.c:4678-4734`).
                0x8E => {
                    let selector = self.pop()?;
                    let value = self.pop()?;
                    if !(1..=3).contains(&selector) {
                        // C rejects an invalid selector only under pedantic
                        // hinting; normal execution ignores the instruction.
                        if self.pedantic_hinting {
                            return Err(FontError::InvalidReference);
                        }
                        continue;
                    }

                    let flag = 1u8 << (selector - 1);
                    if value != 0 && value != flag as i32 {
                        // Nonzero values must equal the selector's flag value.
                        if self.pedantic_hinting {
                            return Err(FontError::InvalidReference);
                        }
                        continue;
                    }

                    if self.initial_range == 0 {
                        self.gs.instruct_control &= !flag;
                        self.gs.instruct_control |= value as u8;
                    } else if self.initial_range == 2 && selector == 3 {
                        self.backward_compatibility = ((value as u8) & 4) ^ 4;
                    } else if self.pedantic_hinting {
                        // C `Ins_INSTCTRL` checks `iniRange`, so selectors 1/2
                        // are valid for prep-initiated calls and selector 3 is
                        // valid for glyph-initiated calls into an fpgm FDEF.
                        return Err(FontError::InvalidReference);
                    }
                }
                // ── ADJUST/GX opcodes (FreeType routes these through IDEF
                // when GX variation handlers are inactive).
                0x8F..=0x92 => {
                    let _ = self.call_instruction_def(opcode);
                }

                // ── Unknown opcode ────────────────────────────
                _ => {
                    let _ = self.call_instruction_def(opcode);
                }
            }
        }

        Ok(())
    }
}
