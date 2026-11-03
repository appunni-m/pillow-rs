//! Pillow-compatible resize semantics and coefficient helpers.
//!
//! The family facade preserves the existing crate paths while keeping the
//! sampling, coefficient, pass, mode, and execution responsibilities in
//! focused child modules.
// TODO(pillow-rs#3ptb): restore parallel Resize scheduling in the dedicated
// `pool_parallel` owner; this shared compatibility path stays serial.

mod alpha;
#[cfg(test)]
mod boxed;
mod coefficients;
mod filters;
mod float;
mod horizontal_passes;
mod integer_modes;
mod luma16;
#[cfg(test)]
mod resize;
mod samples;
mod vertical_passes;

pub(crate) use alpha::{premultiply_alpha, unpremultiply_alpha, unpremultiply_channel, unpremultiply_rgba};
#[cfg(test)]
pub(crate) use boxed::pil_resize_boxed;
pub(crate) use coefficients::{
    FilterCoeffs, FilterCoeffsF64, compact_resize_coeffs_to_source_span, precompute_coeffs,
    precompute_coeffs_boxed, precompute_coeffs_boxed_for_filter, precompute_coeffs_f64,
    precompute_coeffs_f64_boxed,
};
pub(crate) use filters::{filter_from_resample, pillow_sin_f64};
pub(crate) use horizontal_passes::horizontal_pass_rgb_row;
#[cfg(target_arch = "x86_64")]
pub(crate) use float::X86FmaToken;
pub(crate) use float::{
    F64MulAdd, PortableFma, f_resize_coefficients_allow_finite_fma,
    f_resize_samples_allow_finite_fma,
};
pub(crate) use luma16::luma16_resample_big_endian;
#[cfg(test)]
pub(crate) use resize::{pil_preserve_mode, pil_resize};
pub(crate) use vertical_passes::vertical_pass_rgb_row;
pub(crate) use samples::{
    f32_samples_borrowed_from_le_bytes, f32_samples_from_le_bytes, i32_samples_from_le_bytes,
};
