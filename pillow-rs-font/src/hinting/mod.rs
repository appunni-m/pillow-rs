//! TrueType bytecode interpreter (hinting engine).
//!
//! Manages FPGM/PREP execution and per-glyph hinting via a stack-based
//! bytecode VM that matches FreeType 2.6.x output.

#![allow(missing_docs)]

pub mod graphics;
pub mod exec;
pub mod round;
pub mod opcodes;
pub mod iup;

use crate::tables::FontData;
use exec::ExecContext;

/// The hinting engine: manages FPGM/PREP execution and per-glyph hinting.
pub struct HintingEngine {
    pub exec: ExecContext,
    pub fpgm_ready: bool,
    pub cvt_ready: bool,
    pub last_ppem: u16,
}

impl HintingEngine {
    pub fn new(data: &FontData) -> Self {
        let exec = ExecContext::new(data);
        let mut engine = HintingEngine {
            exec,
            fpgm_ready: false,
            cvt_ready: false,
            last_ppem: 0,
        };
        if !data.fpgm.is_empty() {
            engine.exec.code = data.fpgm.clone();
            engine.exec.cur_range = exec::CodeRange::Font;
            if let Err(e) = engine.exec.run() {
                log::warn!("[hinting] FPGM execution failed: {}", e);
            }
            engine.fpgm_ready = true;
        }
        engine
    }

    pub fn ensure_prep(&mut self, data: &FontData, ppem: u16) {
        if ppem == self.last_ppem && self.cvt_ready {
            return;
        }
        self.reset_for_size(data, ppem);
    }

    fn reset_for_size(&mut self, data: &FontData, ppem: u16) {
        self.exec.cvt = data.cvt.clone();
        self.exec.glyf_cvt = data.cvt.clone();
        self.exec.glyf_storage = vec![0i32; self.exec.storage.len().max(32)];

        if !data.prep.is_empty() {
            self.exec.code = data.prep.clone();
            self.exec.cur_range = exec::CodeRange::Cvt;
            if let Err(e) = self.exec.run() {
                log::warn!("[hinting] PREP execution failed: {}", e);
            }
        }
        self.cvt_ready = true;
        self.last_ppem = ppem;
    }

    pub fn hint_glyph(
        &mut self,
        data: &FontData,
        glyph_index: u16,
        glyph: &mut crate::scaler::ScaledGlyph,
    ) {
        self.exec.hint_glyph(data, glyph_index, glyph);
    }
}
