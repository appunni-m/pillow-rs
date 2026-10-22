//! Shared imports for registry initialization modules.

pub(super) use super::Registry;
pub(super) use crate::compute::pool_cpu::ops::chops::{
    op_chops_add, op_chops_add_modulo, op_chops_constant, op_chops_darker, op_chops_difference,
    op_chops_duplicate, op_chops_hard_light, op_chops_invert, op_chops_lighter,
    op_chops_logical_and, op_chops_logical_or, op_chops_logical_xor, op_chops_multiply,
    op_chops_offset, op_chops_overlay, op_chops_screen, op_chops_soft_light, op_chops_subtract,
    op_chops_subtract_modulo,
};
pub(super) use crate::compute::pool_cpu::ops::color::{
    op_convert, op_convert_lab, op_extract_band, op_remap_palette,
};
pub(super) use crate::compute::pool_cpu::ops::draw::{
    op_draw_arc, op_draw_chord, op_draw_circle, op_draw_ellipse, op_draw_line, op_draw_pieslice,
    op_draw_point, op_draw_polygon, op_draw_rectangle, op_draw_rounded_rect,
};
pub(super) use crate::compute::pool_cpu::ops::effects::{
    op_alpha_composite, op_blend_module, op_color3dlut, op_composite_module, op_effect_noise,
    op_effect_spread, op_eval, op_merge, op_paste, op_put_alpha, op_put_alpha_data, op_put_data,
    op_put_pixel, op_transform,
};
pub(super) use crate::compute::pool_cpu::ops::enhance::{
    op_enhance_brightness, op_enhance_color_saturation, op_enhance_contrast, op_enhance_sharpness,
};
pub(super) use crate::compute::pool_cpu::ops::filter::{
    execute_box_blur, execute_box_blur_xy_with_passes, execute_filter3x3, execute_filter5x5,
    execute_gaussian_blur, execute_max_filter_with_mode, execute_median_filter_with_mode,
    execute_min_filter_with_mode, execute_rank_filter_with_mode,
};
pub(super) use crate::compute::pool_cpu::ops::geometry::{
    execute_crop, execute_reduce, execute_resize, execute_resize_boxed, execute_rotate,
    execute_thumbnail, execute_transpose,
};
pub(super) use crate::compute::pool_cpu::ops::imageops::{
    op_autocontrast, op_colorize, op_contain, op_cover, op_crop_border, op_equalize,
    op_equalize_with_mask, op_expand, op_fit, op_flip, op_invert, op_mirror, op_pad, op_posterize,
    op_scale, op_solarize,
};
pub(super) use crate::compute::pool_simd::ops::adapters;
pub(super) use crate::error::PilError;
pub(super) use crate::pipeline::PipelineOp;
pub(super) use crate::raster::DynamicImage;
