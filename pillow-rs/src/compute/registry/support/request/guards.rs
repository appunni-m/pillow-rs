//! Materialized input guards for declared implementation strategies.

mod core;
mod draw;
mod mutation;
mod resize;

pub(super) use core::{
    brightness_vector_guard,
    checked_native_bytes_guard,
    chops_add_subtract_guard,
    chops_equal_dimensions_guard,
    color_saturation_vector_guard,
    convert_target_guard,
    filter3x3_i32_vectorizable_guard,
    filter3x3_native_bytes_vectorizable_guard,
    filter5x5_native_bytes_vectorizable_guard,
    filter5x5_i32_binomial_fma_guard,
    filter_native_byte_blur_guard,
    filter_luma_rank_window_guard,
    filter_native_byte_extrema_guard,
    filter_native_byte_rank_window_guard,
    filter_rgba_gaussian_blur_radius_seven_guard,
    filter_f32_rank_window_guard,
    filter_unsharp_rgb150_guard,
    native_bytes_guard,
    reduce_f32_vector_guard,
    rotate_vector_bytes_guard,
    transform_byte_kernels_guard,
    sharpness_vector_guard,
    thumbnail_i32_stream_guard,
};
pub(super) use resize::{
    resize_byte_boxed_nearest_vector_guard,
    resize_byte_boxed_vector_guard,
    resize_byte_nearest_vector_guard,
    resize_byte_native_vector_guard,
    resize_filtered_fma_guard,
    resize_i32_neon_guard,
    resize_luma16_vector_guard,
};
pub(super) use draw::draw_native_spans_guard;
pub(super) use mutation::{
    alpha_composite_vector_guard,
    alpha_mask_guard,
    blend_native_vector_guard,
    color3dlut_guard,
    composite_native_byte_vector_guard,
    effect_noise_vector_guard,
    effect_spread_bytes_guard,
    expand_vector_guard,
    exact_native_rows_guard,
    extract_band_bytes_guard,
    merge_native_interleave_guard,
    merge_rgb_interleave_guard,
    native_byte_prefix_guard,
    native_crop_border_bytes_guard,
    native_crop_bytes_guard,
    native_mirror_rows_guard,
    native_pointwise_bytes_guard,
    native_transpose_bytes_guard,
    pixel_span_guard,
    pixel_write_run_guard,
    paste_l_mask_neon_guard,
    put_alpha_bytes_guard,
    put_data_native_guard,
    reduce_native_byte_vector_guard,
    rgba_transpose_avx2_guard,
};
