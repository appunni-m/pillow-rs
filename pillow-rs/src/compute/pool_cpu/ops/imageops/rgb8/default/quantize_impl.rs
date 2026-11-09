//! Native RGB8 quantization strategy for the CPU Default architecture.

use crate::compute::pool_cpu::helpers::quantization::{
    kmeans_refine, map_pixels_to_fixed_palette, maxcoverage_quantize_rgb, median_cut_quantize_rgb,
    quantize_octree_rgb,
};
use crate::error::PilError;
use crate::raster::RgbImage;

/// Quantizes the native RGB buffer without creating an intermediate raster.
pub(crate) fn execute(
    image: &RgbImage,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    execute_rgb_samples(image.as_raw(), width, height, colors, kmeans, method)
}

/// Quantizes RGB samples already produced by the source mode's operation path.
pub(crate) fn execute_rgb_samples(
    pixels: &[u8],
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    match method {
        2 => quantize_octree(pixels, width, height, colors),
        1 => Ok(quantize_maxcoverage(pixels, colors, kmeans)),
        0 => Ok(quantize_median_cut(pixels, colors, kmeans)),
        3 => Err(PilError::ValueError(
            "dependency required by this method was not enabled at compile time".into(),
        )),
        _ => Err(PilError::ValueError("quantization error".into())),
    }
}

/// Maps native RGB samples to a caller-supplied RGB palette.
pub(crate) fn execute_with_palette(
    image: &RgbImage,
    palette: &[[u8; 3]],
    dither: bool,
) -> Result<Vec<u8>, PilError> {
    map_pixels_to_fixed_palette(
        image.as_raw(),
        image.width(),
        image.height(),
        palette,
        dither,
    )
}

fn quantize_octree(
    pixels: &[u8],
    width: u32,
    height: u32,
    colors: usize,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    let (indices, rgba_palette) = quantize_octree_rgb(pixels, width, height, colors)?;
    Ok((indices, rgb_palette(&rgba_palette), None))
}

fn quantize_maxcoverage(
    pixels: &[u8],
    colors: usize,
    kmeans: u32,
) -> (Vec<u8>, Vec<u8>, Option<Vec<u8>>) {
    let (indices, palette) = maxcoverage_quantize_rgb(pixels, colors, kmeans);
    (indices, flatten_palette(&palette), None)
}

fn quantize_median_cut(
    pixels: &[u8],
    colors: usize,
    kmeans: u32,
) -> (Vec<u8>, Vec<u8>, Option<Vec<u8>>) {
    let pixel_list: Vec<[u8; 3]> = if kmeans > 0 {
        pixels
            .chunks_exact(3)
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect()
    } else {
        Vec::new()
    };
    let (indices, mut palette) = median_cut_quantize_rgb(pixels, colors);
    let indices = refine_kmeans(indices, &pixel_list, &mut palette, kmeans);
    (indices, flatten_palette(&palette), None)
}

fn refine_kmeans(
    indices: Vec<u8>,
    pixels: &[[u8; 3]],
    palette: &mut [[u8; 3]],
    kmeans: u32,
) -> Vec<u8> {
    if kmeans == 0 {
        return indices;
    }
    let mut indices: Vec<u32> = indices.into_iter().map(u32::from).collect();
    kmeans_refine(pixels, palette, &mut indices, kmeans - 1);
    indices.into_iter().map(|index| index as u8).collect()
}

fn flatten_palette(palette: &[[u8; 3]]) -> Vec<u8> {
    palette
        .iter()
        .flat_map(|color| [color[0], color[1], color[2]])
        .collect()
}

fn rgb_palette(palette: &[u8]) -> Vec<u8> {
    palette
        .chunks_exact(4)
        .flat_map(|color| [color[0], color[1], color[2]])
        .collect()
}
