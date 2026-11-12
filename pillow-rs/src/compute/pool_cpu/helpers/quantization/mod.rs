//! Native palette algorithms shared by CPU quantization and color conversion.

mod fixed_palette;
mod maxcoverage;
mod median_cut;
mod octree;
mod rgb_source;
mod rgba_palette;
mod rgba_source;
mod web_palette;

pub(crate) use fixed_palette::{
    map_luma_to_fixed_palette, map_native_to_fixed_palette, map_pixels_to_fixed_palette,
};
pub(crate) use maxcoverage::{
    kmeans_refine, kmeans_refine_source, maxcoverage_quantize_rgb, maxcoverage_quantize_source,
};
pub(crate) use median_cut::{median_cut_quantize_rgb, median_cut_quantize_source};
pub(crate) use octree::{quantize_octree_rgb, quantize_octree_rgb_source};
pub(crate) use rgb_source::RgbPixelSource;
pub(crate) use rgba_palette::{quantize_rgba_samples, quantize_rgba_source};
pub(crate) use rgba_source::RgbaPixelSource;
pub(crate) use web_palette::{
    identity_grayscale_palette, web_palette_colors, web_palette_quantize,
};
