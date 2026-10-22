//! Pillow-compatible distinct-color counting algorithms for images.

mod pillow_table;
mod specialized;

#[cfg(test)]
mod tests;

pub(super) use pillow_table::{
    PillowColorCounts, count_pillow_color_scalar_samples, pillow_color_components,
    pillow_color_direct_table_slots, pillow_color_pixel, pillow_color_table_parameters,
    pillow_image_pixel_count,
};
pub(super) use specialized::{
    RgbGrayscaleColorCounts, SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS, ScalarColorDomainCounts,
    getcolors_rgb8_grayscale, getcolors_scalar_byte_domain, getcolors_scalar_i_repeated_runs,
    scalar_byte_domain_is_plausible, scalar_i_prefix_has_long_run,
    scalar_prefix_has_repeated_samples,
};

#[cfg(test)]
pub(super) use pillow_table::PILLOW_COLOR_DIRECT_TABLE_MAX_BYTES;
#[cfg(test)]
pub(super) use specialized::{
    SCALAR_I_REPEATED_RUN_MIN_PREFIX_SAMPLES, SCALAR_I_SPARSE_RUN_COLOR_LIMIT,
};
