//! Pillow-compatible image palette reduction and mapping.

mod fixed_palette;
mod input;
mod maxcoverage;
mod median_cut;
mod octree;
mod operations;
mod web_palette;

pub use input::QuantizePalette;

pub(crate) use fixed_palette::map_pixels_to_fixed_palette;
pub(crate) use maxcoverage::{kmeans_refine, maxcoverage_quantize_rgb};
pub(crate) use median_cut::median_cut_quantize_rgb;
pub(crate) use octree::{quantize_octree_rgb, quantize_octree_rgba};
pub(crate) use web_palette::web_palette_quantize;
