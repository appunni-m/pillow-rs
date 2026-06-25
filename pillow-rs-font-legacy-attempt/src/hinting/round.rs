//! Rounding functions -- matching FreeType's Round_* family exactly.
//!
//! Each function returns the **rounded distance** (not a delta).
//! The caller computes `move_dist = rounded - original_distance`.
//!
//! FreeType reference: `src/truetype/ttinterp.c` Round_To_Grid, etc.
//! Pixel macros (from `ftobjs.h`):
//!   FT_PIX_FLOOR(x)       = (x) & ~63
//!   FT_PIX_ROUND_LONG(x)  = ((x) + 32) & ~63
//!   FT_PIX_CEIL_LONG(x)   = ((x) + 63) & ~63
//!   FT_PAD_ROUND_LONG(x,n)= ((x) + n/2) & ~(n-1)

#![allow(missing_docs)]

/// FreeType's `Round_To_Grid`: `FT_PIX_ROUND_LONG(distance + compensation)`.
#[inline]
pub fn round_to_grid(distance: i32, compensation: i32) -> i32 {
    if distance >= 0 {
        let val = (distance + compensation + 32) & !63;
        if val < 0 {
            0
        } else {
            val
        }
    } else {
        let val = -(((-distance) + compensation + 32) & !63);
        if val > 0 {
            0
        } else {
            val
        }
    }
}

/// FreeType's `Round_To_Double_Grid`: half-pixel grid (32 F26Dot6 units).
///
/// `FT_PAD_ROUND_LONG(distance + compensation, 32)`
/// = `((distance + compensation + 16) & ~31)`
#[inline]
pub fn round_to_double_grid(distance: i32, compensation: i32) -> i32 {
    if distance >= 0 {
        let val = (distance + compensation + 16) & !31;
        if val < 0 {
            0
        } else {
            val
        }
    } else {
        let val = -(((-distance) + compensation + 16) & !31);
        if val > 0 {
            0
        } else {
            val
        }
    }
}

/// FreeType's `Round_Down_To_Grid`: `FT_PIX_FLOOR(distance + compensation)`.
#[inline]
pub fn round_down_to_grid(distance: i32, compensation: i32) -> i32 {
    if distance >= 0 {
        let val = (distance + compensation) & !63;
        if val < 0 {
            0
        } else {
            val
        }
    } else {
        let val = -(((-distance) + compensation) & !63);
        if val > 0 {
            0
        } else {
            val
        }
    }
}

/// FreeType's `Round_Up_To_Grid`: `FT_PIX_CEIL_LONG(distance + compensation)`.
#[inline]
pub fn round_up_to_grid(distance: i32, compensation: i32) -> i32 {
    if distance >= 0 {
        let val = (distance + compensation + 63) & !63;
        if val < 0 {
            0
        } else {
            val
        }
    } else {
        let val = -(((-distance) + compensation + 63) & !63);
        if val > 0 {
            0
        } else {
            val
        }
    }
}

/// FreeType's `Round_None`: distance +/- compensation (no grid rounding).
#[inline]
pub fn round_off(distance: i32, compensation: i32) -> i32 {
    if distance >= 0 {
        let val = distance + compensation;
        if val < 0 {
            0
        } else {
            val
        }
    } else {
        let val = distance - compensation;
        if val > 0 {
            0
        } else {
            val
        }
    }
}

/// FreeType's `Round_To_Half_Grid`: `FT_PIX_FLOOR(distance + compensation) + 32`.
///
/// Rounds to the nearest half-pixel boundary (odd multiples of 32 in F26Dot6).
#[inline]
pub fn round_to_half_grid(distance: i32, compensation: i32) -> i32 {
    if distance >= 0 {
        let val = ((distance + compensation) & !63) + 32;
        if val < 32 {
            32
        } else {
            val
        }
    } else {
        let val = -(((-distance) + compensation) & !63) - 32;
        if val > -32 {
            -32
        } else {
            val
        }
    }
}

/// Round to odd (FreeType does NOT implement this; provided for completeness).
#[inline]
pub fn round_to_odd(distance: i32, compensation: i32) -> i32 {
    let val = distance;
    let rounded = if val >= 0 {
        (val + compensation + 32) & !63
    } else {
        -(((-val) + compensation + 32) & !63)
    };
    if rounded & 0x3F == 0 {
        if val >= 0 {
            rounded + 64
        } else {
            rounded - 64
        }
    } else {
        rounded
    }
}

/// Super-rounding stub (not used directly; exec.rs uses context-aware impl).
#[inline]
pub fn round_super(distance: i32, compensation: i32) -> i32 {
    round_to_grid(distance, compensation)
}

/// Super-rounding 45 stub (not used directly).
#[inline]
pub fn round_super_45(distance: i32, compensation: i32) -> i32 {
    round_to_grid(distance, compensation)
}

pub type RoundFn = fn(distance: i32, compensation: i32) -> i32;
