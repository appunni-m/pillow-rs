//! Numeric rounding shared by CPU and GPU resampling.

/// Pillow's `ROUND_UP`: `(int)((value) >= 0 ? value + 0.5 : value - 0.5)`.
pub(crate) fn round_up(value: f64) -> f64 {
    if value >= 0.0 {
        (value + 0.5).trunc()
    } else {
        (value - 0.5).trunc()
    }
}
