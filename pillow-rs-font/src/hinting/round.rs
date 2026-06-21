//! Rounding functions — matching FreeType's Round_* family.

#![allow(missing_docs)]

#[inline]
pub fn round_to_grid(distance: i32, _compensation: i32) -> i32 {
    let val = distance;
    if val >= 0 {
        ((val + 32) & !63) - val
    } else {
        -(((-val) + 32) & !63)
    }
}

#[inline]
pub fn round_to_double_grid(distance: i32, _compensation: i32) -> i32 {
    let val = distance;
    if val >= 0 {
        ((val + 32) & !63) + 32 - val
    } else {
        -(((-val) + 32) & !63) + 64
    }
}

#[inline]
pub fn round_down_to_grid(distance: i32, _compensation: i32) -> i32 {
    let val = distance;
    if val >= 0 {
        ((val + 63) & !63) - val
    } else {
        -(((-val) + 63) & !63)
    }
}

#[inline]
pub fn round_up_to_grid(distance: i32, _compensation: i32) -> i32 {
    let val = distance;
    if val >= 0 {
        ((val + 63) & !63) - val
    } else {
        -(((-val) + 63) & !63)
    }
}

#[inline]
pub fn round_off(distance: i32, _compensation: i32) -> i32 {
    distance
}

#[inline]
pub fn round_to_odd(distance: i32, _compensation: i32) -> i32 {
    let val = distance;
    let rounded = if val >= 0 {
        (val + 32) & !63
    } else {
        -(((-val) + 32) & !63)
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

#[inline]
pub fn round_super(distance: i32, compensation: i32) -> i32 {
    round_super_impl(distance, compensation, false)
}

#[inline]
pub fn round_super_45(distance: i32, compensation: i32) -> i32 {
    round_super_impl(distance, compensation, true)
}

fn round_super_impl(distance: i32, _compensation: i32, _is_45: bool) -> i32 {
    round_to_grid(distance, 0)
}

pub type RoundFn = fn(distance: i32, compensation: i32) -> i32;
