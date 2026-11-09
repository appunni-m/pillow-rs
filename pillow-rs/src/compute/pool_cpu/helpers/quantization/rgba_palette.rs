//! Pillow palette-plane extraction from the native RGBA octree result.

use super::RgbaPixelSource;
use super::octree::{quantize_octree_rgba, quantize_octree_rgba_source};
use crate::error::PilError;

/// Applies the RGBA FASTOCTREE policy and separates Pillow's palette channels.
pub(crate) fn quantize_rgba_samples(
    samples: &[u8],
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), PilError> {
    let (indices, palette) = quantize_octree_rgba(samples, width, height, colors, true)?;
    Ok(split_rgba_palette(indices, palette))
}

/// Applies FASTOCTREE to a mode-defined RGBA source without an RGBA image.
pub(crate) fn quantize_rgba_source<S>(
    source: &S,
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>), PilError>
where
    S: RgbaPixelSource + ?Sized,
{
    let (indices, palette) = quantize_octree_rgba_source(source, width, height, colors)?;
    Ok(split_rgba_palette(indices, palette))
}

fn split_rgba_palette(indices: Vec<u8>, palette: Vec<u8>) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    (
        indices,
        palette
            .chunks_exact(4)
            .flat_map(|color| [color[0], color[1], color[2]])
            .collect(),
        palette.chunks_exact(4).map(|color| color[3]).collect(),
    )
}
