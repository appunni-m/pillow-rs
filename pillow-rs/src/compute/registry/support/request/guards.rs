//! Materialized input guards for declared implementation strategies.

mod composition;
mod core;
mod draw;
mod mutation;
mod resize;
mod rotation;

pub(super) use composition::{
    alpha_composite_avx512f_guard, alpha_composite_vector_guard, blend_native_avx512f_guard,
    blend_native_vector_guard, composite_l_mask_neon_guard, composite_native_byte_vector_guard,
    merge_avx512f_interleave_guard, merge_native_interleave_guard, merge_rgb_interleave_guard,
    paste_l_mask_neon_guard,
};
pub(super) use core::{
    brightness_vector_guard, checked_native_bytes_guard, chops_add_subtract_guard,
    chops_equal_dimensions_guard, chops_offset_native_bytes_guard, chops_offset_native_i16_guard,
    color_saturation_avx512f_guard, color_saturation_vector_guard, contrast_vector_guard,
    convert_target_guard, filter_f32_avx512_rank_window_guard, filter_f32_rank_window_guard,
    filter_i32_avx512_rank_window_guard, filter_luma_rank_window_guard,
    filter_native_byte_blur_guard, filter_native_byte_extrema_guard,
    filter_native_byte_rank_window_guard, filter_rgba_gaussian_blur_radius_seven_avx512f_guard,
    filter_rgba_gaussian_blur_radius_seven_guard, filter_unsharp_rgb150_guard,
    filter3x3_i32_avx512f_guard, filter3x3_i32_vectorizable_guard,
    filter3x3_native_bytes_vectorizable_guard, filter5x5_i32_avx512f_guard,
    filter5x5_i32_binomial_fma_guard, filter5x5_i32_vectorizable_guard,
    filter5x5_native_bytes_vectorizable_guard, get_extrema_i32_avx512f_guard, native_bytes_guard,
    reduce_f32_vector_guard, reduce_i32_vector_guard, sharpness_vector_guard,
    thumbnail_i32_avx512f_guard, thumbnail_i32_stream_guard, transform_byte_kernels_guard,
    transform_i16_affine_guard,
};
pub(super) use draw::draw_native_spans_guard;
pub(super) use mutation::{
    alpha_mask_guard, color3dlut_guard, effect_mandelbrot_vector_guard, effect_noise_vector_guard,
    effect_spread_bytes_guard, exact_native_rows_guard, expand_vector_guard,
    extract_band_bytes_guard, native_byte_avx512_invert_guard, native_byte_avx512_pointwise_guard,
    native_byte_prefix_guard, native_crop_border_bytes_guard, native_crop_bytes_guard,
    native_mirror_rows_guard, native_pointwise_bytes_guard,
    native_pointwise_bytes_including_empty_guard, native_transpose_bytes_guard,
    native_typed_avx512_invert_guard, pixel_span_guard, pixel_write_run_guard,
    put_alpha_bytes_guard, put_data_native_guard, reduce_avx512_byte_2x2_guard,
    reduce_native_byte_vector_guard, rgba_transpose_avx2_guard, rgba_transpose_x86_64_tiled_guard,
    transpose_avx512f_native32_guard,
};
pub(super) use resize::{
    resize_avx512_nearest_guard, resize_avx512f_filtered_guard,
    resize_byte_boxed_nearest_vector_guard, resize_byte_boxed_vector_guard,
    resize_byte_native_vector_guard, resize_byte_nearest_vector_guard, resize_f32_vector_guard,
    resize_filtered_fma_guard, resize_i32_neon_guard, resize_luma16_vector_guard,
};
pub(super) use rotation::rotate_vector_bytes_guard;
