//! Pillow-compatible distinct-color counting algorithms for images.

mod pillow_table;
mod specialized;

pub(crate) use pillow_table::{
    PillowColorCounts, count_pillow_color_scalar_samples, pillow_color_components,
    pillow_color_direct_table_slots, pillow_color_pixel, pillow_color_table_parameters,
};
pub(crate) use specialized::{
    RgbGrayscaleColorCounts, SCALAR_I_REPEATED_RUN_MIN_DIRECT_SLOTS, ScalarColorDomainCounts,
    getcolors_rgb8_grayscale, getcolors_scalar_byte_domain, getcolors_scalar_i_repeated_runs,
    scalar_byte_domain_is_plausible, scalar_i_prefix_has_long_run,
    scalar_prefix_has_repeated_samples,
};
