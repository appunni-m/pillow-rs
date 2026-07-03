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
use crate::error::FontError;

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
        }
    }

    // ── Stack operations (C: stack manipulation in ttinterp.c) ────────

    /// Push a value onto the data stack.
    pub fn push(&mut self, val: i32) {
        self.stack.push(val);
    }

    /// Pop a value from the data stack. Returns Err if stack is empty.
    pub fn pop(&mut self) -> Result<i32, FontError> {
        self.stack.pop().ok_or(FontError::InvalidOutline(
            "bytecode: stack underflow".into(),
        ))
    }

    /// Peek at the top of the stack without removing it.
    #[allow(dead_code)]
    pub fn top(&self) -> Result<i32, FontError> {
        self.stack.last().copied().ok_or(FontError::InvalidOutline(
            "bytecode: stack empty".into(),
        ))
    }

    /// Read a byte from the current code range at the current IP,
    /// then increment IP.
    pub fn fetch_byte(&mut self) -> Result<u8, FontError> {
        let range = match self.cur_range {
            0 => &self.font_range,
            1 => &self.cvt_range,
            _ => &self.glyph_range,
        };
        // For font/cvt ranges, we use self.font_program
        let data = &self.font_program; // TODO: support separate prep/glyph programs
        if self.ip >= range.size {
            return Err(FontError::InvalidOutline(
                "bytecode: IP out of range".into(),
            ));
        }
        let byte = data[range.base + self.ip];
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
    /// The fpgm bytecode is expected to contain only FDEF/ENDF pairs
    /// and storage area setup.
    pub fn run_fpgm(&mut self) -> Result<(), FontError> {
        if self.font_range.size == 0 {
            return Ok(());
        }

        self.cur_range = 1; // font program range
        self.ip = 0;

        while self.ip < self.font_range.size {
            let opcode = self.fetch_byte()?;

            match opcode {
                0x1C => {
                    // FDEF: Function Definition
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let func_num = self.pop()? as u16;
                    if (func_num as usize) < self.functions.len() {
                        let start = self.ip;
                        // Scan forward to find the matching ENDF
                        let mut depth = 1;
                        while depth > 0 && self.ip < self.font_range.size {
                            let b = self.fetch_byte()?;
                            match b {
                                0x1C => depth += 1, // nested FDEF
                                0x1D => depth -= 1, // ENDF
                                _ => {}
                            }
                        }
                        if depth == 0 {
                            let def = DefRecord {
                                range: self.cur_range,
                                start,
                                end: self.ip,
                                opc: func_num,
                                active: true,
                            };
                            self.functions[func_num as usize] = Some(def);
                        }
                    }
                }
                0x1D => {
                    // ENDF outside FDEF — error in fpgm
                    return Err(FontError::InvalidOutline(
                        "bytecode: stray ENDF in font program".into(),
                    ));
                }
                _ => {
                    // Ignore other opcodes during fpgm execution
                    // (storage setup, etc. that fpgm may contain)
                }
            }
        }

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

    /// Get a CVT value.
    #[allow(dead_code)]
    pub fn get_cvt(&self, idx: usize) -> Result<i32, FontError> {
        self.cvt.get(idx).copied().ok_or(FontError::InvalidOutline(
            "bytecode: CVT index out of range".into(),
        ))
    }

    /// Set a CVT value.
    #[allow(dead_code)]
    pub fn set_cvt(&mut self, idx: usize, val: i32) -> Result<(), FontError> {
        if idx >= self.cvt.len() {
            return Err(FontError::InvalidOutline(
                "bytecode: CVT index out of range".into(),
            ));
        }
        self.cvt[idx] = val;
        Ok(())
    }
}
