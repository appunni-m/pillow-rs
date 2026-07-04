//! Graphics State — the TrueType bytecode interpreter's mutable state.
//!
//! C reference: `TT_GraphicsState` in `ttinterp.h`, initialized by
//! `TT_Load_Context` in `ttobjs.c:891-957`.
//!
//! The graphics state (GS) contains the projection vector, freedom vector,
//! rounding mode, and control parameters that affect how point movements
//! and distance measurements work during bytecode execution.

use crate::fixed::ft_mul_fix;

/// Default CVT cut-in: 17/16 of a pixel in 26.6 format.
/// C: `ttobjs.c` initializes `cvt_cut_in` to 68 (17/16 * 64).
const DEFAULT_CVT_CUT_IN: i32 = 68;

/// Default single-width value: 0 (disabled).
const DEFAULT_SINGLE_WIDTH: i32 = 0;

/// Default single-width cut-in: 0.
const DEFAULT_SINGLE_WIDTH_CUTIN: i32 = 0;

/// Default minimum distance: 1 pixel in 26.6.
const DEFAULT_MINIMUM_DISTANCE: i32 = 64;

/// Default control value cut-in: 17/16 pixel.
const DEFAULT_CONTROL_VALUE_CUTIN: i32 = 68;

/// Rounding mode constants (matching `ttinterp.h`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RoundMode {
    HalfGrid = 0,
    Grid = 1,
    DoubleGrid = 2,
    DownToGrid = 3,
    UpToGrid = 4,
    Off = 5,
    Super = 6,
    Super45 = 7,
}

impl RoundMode {
    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => RoundMode::HalfGrid,
            1 => RoundMode::Grid,
            2 => RoundMode::DoubleGrid,
            3 => RoundMode::DownToGrid,
            4 => RoundMode::UpToGrid,
            5 => RoundMode::Off,
            6 => RoundMode::Super,
            7 => RoundMode::Super45,
            _ => RoundMode::Grid, // safe default
        }
    }
}

/// The interpreter's graphics state.
///
/// Initialized by `TT_Load_Context` (ttobjs.c:891-957) with defaults,
/// then modified by bytecode instructions (SVTCA, SPVTCA, SFVTCA, SROUND,
/// SMD, SCVTCI, SSW, etc.).
#[derive(Debug, Clone)]
pub struct GraphicsState {
    /// Projection vector (2.14 fixed-point). Default: (0x4000, 0) = (1.0, 0).
    /// Used to project distances along a specific axis.
    pub proj_vector: (i32, i32),

    /// Dual projection vector (2.14 fixed-point). Default: (0, 0x4000) = (0, 1.0).
    pub dual_proj_vector: (i32, i32),

    /// Freedom vector (2.14 fixed-point). Default: (0x4000, 0) = (1.0, 0).
    /// Used to determine the direction of point movement.
    pub freedom_vector: (i32, i32),

    /// Rounding mode. Default: Grid (1).
    pub round_state: RoundMode,

    /// Auto-flip flag. When set, MIRP automatically flips the sign of
    /// the CVT distance if the original contour direction opposes the
    /// freedom vector direction.
    pub auto_flip: bool,

    /// CVT cut-in: maximum distance (in 26.6) from a CVT value for
    /// MIRP to consider it a match. Default: 68 (17/16 px).
    pub cvt_cut_in: i32,

    /// Minimum distance (in 26.6) that MDRP/MIRP will produce.
    /// Default: 64 (1 px).
    pub minimum_distance: i32,

    /// Single-width value (in 26.6). When non-zero, MDRP/MIRP use this
    /// as the stem width for all stems. Default: 0 (disabled).
    pub single_width_value: i32,

    /// Single-width cut-in. Default: 0.
    pub single_width_cutin: i32,

    /// Control value cut-in. Default: 68 (17/16 px).
    pub control_value_cutin: i32,

    /// Delta base (for DELTAP opcodes). Default: 9.
    pub delta_base: u32,

    /// Delta shift (for DELTAP opcodes). Default: 3.
    pub delta_shift: u32,

    /// Loop counter (set by LOOPCALL opcode). Default: 1.
    pub loop_counter: i32,

    /// Zone pointers: which zone to use for point references.
    /// 0 = twilight zone, 1 = glyph zone.
    pub zp0: u8,
    pub zp1: u8,
    pub zp2: u8,

    /// Reference points: cached point indices for relative moves.
    /// rp0 is set by SRP0, rp1 by SRP1, rp2 by SRP2.
    /// Also auto-updated by MDRP/MIRP.
    pub rp0: u32,
    pub rp1: u32,
    pub rp2: u32,

    /// Scan control / dropout mode. Default: false.
    pub scan_control: bool,

    /// Scan type (bits 5-7 of glyph tags). Default: 0.
    pub scan_type: u8,

    /// Instruct control (flags from GETINFO). Default: 0.
    pub instruct_control: u8,
}

impl Default for GraphicsState {
    fn default() -> Self {
        GraphicsState {
            // Default: projection along X axis (1.0, 0)
            proj_vector: (0x4000, 0),
            // Default: dual projection along Y axis (0, 1.0)
            dual_proj_vector: (0, 0x4000),
            // Default: freedom along X axis (1.0, 0)
            freedom_vector: (0x4000, 0),
            round_state: RoundMode::Grid,
            auto_flip: true,
            cvt_cut_in: DEFAULT_CVT_CUT_IN,
            minimum_distance: DEFAULT_MINIMUM_DISTANCE,
            single_width_value: DEFAULT_SINGLE_WIDTH,
            single_width_cutin: DEFAULT_SINGLE_WIDTH_CUTIN,
            control_value_cutin: DEFAULT_CONTROL_VALUE_CUTIN,
            delta_base: 9,
            delta_shift: 3,
            loop_counter: 1,
            zp0: 1,
            zp1: 1,
            zp2: 1,
            rp0: 0,
            rp1: 0,
            rp2: 0,
            scan_control: false,
            scan_type: 0,
            instruct_control: 0,
        }
    }
}

impl GraphicsState {
    /// Set projection and freedom vectors to the Y axis.
    /// C: `SVTCA[0]` opcode handler.
    pub fn set_vectors_to_y(&mut self) {
        self.proj_vector = (0, 0x4000);
        self.dual_proj_vector = (0, 0x4000);
        self.freedom_vector = (0, 0x4000);
    }

    /// Set projection and freedom vectors to the X axis.
    /// C: `SVTCA[1]` opcode handler.
    pub fn set_vectors_to_x(&mut self) {
        self.proj_vector = (0x4000, 0);
        self.dual_proj_vector = (0x4000, 0);
        self.freedom_vector = (0x4000, 0);
    }

    /// Set projection vector (only) to Y axis. C: `SPVTCA[0]`.
    pub fn set_proj_to_y(&mut self) {
        self.proj_vector = (0, 0x4000);
    }

    /// Set projection vector (only) to X axis. C: `SPVTCA[1]`.
    pub fn set_proj_to_x(&mut self) {
        self.proj_vector = (0x4000, 0);
    }

    /// Set freedom vector (only) to Y axis. C: `SFVTCA[0]`.
    pub fn set_free_to_y(&mut self) {
        self.freedom_vector = (0, 0x4000);
    }

    /// Set freedom vector (only) to X axis. C: `SFVTCA[1]`.
    pub fn set_free_to_x(&mut self) {
        self.freedom_vector = (0x4000, 0);
    }

    /// Project a 2D vector onto the current projection vector.
    ///
    /// Returns the signed scalar projection in 26.6 format.
    /// C: `TT_Project` / `FT_Project` in ttinterp.c.
    pub fn project(&self, vx: i32, vy: i32) -> i32 {
        // proj_vector is in 2.14 format (0x4000 = 1.0)
        // Result: (vx * proj_x + vy * proj_y) >> 14 → 26.6
        let px = ft_mul_fix(vx, self.proj_vector.0 << 2);
        let py = ft_mul_fix(vy, self.proj_vector.1 << 2);
        px + py
    }

    /// Project a 2D vector onto the dual projection vector.
    pub fn dual_project(&self, vx: i32, vy: i32) -> i32 {
        let px = ft_mul_fix(vx, self.dual_proj_vector.0 << 2);
        let py = ft_mul_fix(vy, self.dual_proj_vector.1 << 2);
        px + py
    }

    /// Move a point along the freedom vector by a given distance.
    ///
    /// Returns (dx, dy) to add to the point's current position.
    /// The distance is in 26.6, and the freedom vector is in 2.14.
    pub fn move_along_free(&self, distance: i32) -> (i32, i32) {
        // Normalize the freedom vector? C does this via FT_MulDiv with
        // the vector length. For axis-aligned vectors this is trivial.
        let dx = ft_mul_fix(distance, self.freedom_vector.0 << 2);
        let dy = ft_mul_fix(distance, self.freedom_vector.1 << 2);
        (dx, dy)
    }

    /// Round a 26.6 value using the current rounding mode.
    /// C: `TT_RoundFunc` dispatch.
    pub fn round(&self, distance: i32) -> i32 {
        fn round_grid(v: i32) -> i32 {
            if v >= 0 {
                (v + 32) & !63
            } else {
                -(((-v) + 32) & !63)
            }
        }
        fn floor_grid(v: i32) -> i32 {
            v & !63
        }
        fn ceil_grid(v: i32) -> i32 {
            if v <= 0 {
                -((-v) & !63)
            } else {
                (v + 63) & !63
            }
        }
        match self.round_state {
            RoundMode::HalfGrid => {
                let base = floor_grid(distance);
                base + 32
            }
            RoundMode::Grid => round_grid(distance),
            RoundMode::DoubleGrid => {
                if distance >= 0 {
                    (distance + 16) & !31
                } else {
                    -(((-distance) + 16) & !31)
                }
            }
            RoundMode::DownToGrid => floor_grid(distance),
            RoundMode::UpToGrid => ceil_grid(distance),
            RoundMode::Off => distance,
            RoundMode::Super | RoundMode::Super45 => {
                // SROUND/S45ROUND need period/phase/threshold from exec context
                // For now, fall through to Grid rounding
                round_grid(distance)
            }
        }
    }
}
