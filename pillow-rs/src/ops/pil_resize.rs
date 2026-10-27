//! Pillow-compatible resize semantics and coefficient helpers.
//!
//! The family facade preserves the existing crate paths while keeping the
//! sampling, coefficient, pass, mode, and execution responsibilities in
//! focused child modules.

mod alpha;
mod boxed;
mod coefficients;
mod filters;
mod float;
mod horizontal_passes;
mod integer_modes;
mod luma16;
mod resize;
mod samples;
mod vertical_passes;

pub(crate) use alpha::{premultiply_alpha, unpremultiply_alpha, unpremultiply_channel};
pub(crate) use boxed::{
    pil_resize_boxed, pil_resize_boxed_with_parallel_pixel_threshold,
    pil_resize_boxed_with_sparse_rgb_rows,
};
pub(crate) use coefficients::{
    FilterCoeffs, FilterCoeffsF64, compact_resize_coeffs_to_source_span, precompute_coeffs,
    precompute_coeffs_boxed, precompute_coeffs_boxed_for_filter, precompute_coeffs_f64,
    precompute_coeffs_f64_boxed, round_up,
};
pub(crate) use filters::{filter_from_resample, pillow_sin_f64};
#[cfg(target_arch = "x86_64")]
pub(crate) use float::X86FmaToken;
pub(crate) use float::{
    F64MulAdd, PortableFma, f_resize_coefficients_allow_finite_fma,
    f_resize_samples_allow_finite_fma,
};
#[cfg(not(feature = "parallel"))]
pub(crate) use integer_modes::pil_resize_luma_i32_into_window;
pub(crate) use luma16::{luma16_resample_big_endian, luma16_resample_read, luma16_resample_write};
pub(crate) use resize::{pil_preserve_mode, pil_resize, pil_resize_rgbx_into_window};
pub(crate) use samples::{
    f32_samples_borrowed_from_le_bytes, f32_samples_from_le_bytes, i32_samples_from_le_bytes,
};
