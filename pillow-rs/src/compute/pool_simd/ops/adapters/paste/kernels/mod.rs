//! Mode-specific masked-copy kernels and their native row dispatcher.

mod apply;
mod blend;
mod luma;
mod luma16;
mod rgb_cmyk;

pub(in crate::compute::pool_simd::ops::adapters) use self::apply::native_paste_apply;
use self::blend::{
    native_paste_blend_vector8, native_paste_blend_vector16, native_paste_mask_block,
    native_paste_mask_block_active, native_paste_preblend_vector8, native_paste_preblend_vector16,
    native_paste_rgb_l_mask_block8, native_paste_rgb_l_mask_block16,
};
pub(in crate::compute::pool_simd::ops::adapters) use self::luma::native_paste_la_masked_row;
use self::luma::{native_paste_l_masked_row, native_paste_rgba_l_masked_row};
pub(super) use self::luma16::native_paste_luma16_apply;
use self::rgb_cmyk::native_paste_pa_masked_row;
pub(in crate::compute::pool_simd::ops::adapters) use self::rgb_cmyk::{
    native_paste_cmyk_l_masked_row, native_paste_rgb_l_masked_row,
};
