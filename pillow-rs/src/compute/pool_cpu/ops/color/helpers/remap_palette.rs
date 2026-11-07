//! Native one-byte palette-index remapping kernels shared by L and P strategies.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};
use wide::u8x16;

pub(crate) fn scalar(source: &[u8], dest_map: &[u8]) -> Vec<u8> {
    let inverse = inverse_map(dest_map);
    source.iter().map(|&sample| inverse[usize::from(sample)]).collect()
}

pub(crate) fn vector(source: &[u8], dest_map: &[u8]) -> Result<Vec<u8>, PilError> {
    let inverse = inverse_map(dest_map);
    let tables = std::array::from_fn(|table| {
        let start = table * 16;
        u8x16::new(std::array::from_fn(|lane| inverse[start + lane]))
    });
    let mut output = Vec::with_capacity(source.len());
    for chunk in source.chunks(16) {
        let mut padded = [0u8; 16];
        padded[..chunk.len()].copy_from_slice(chunk);
        let mapped = native_lut_chunk(u8x16::new(padded), &tables).to_array();
        output.extend_from_slice(&mapped[..chunk.len()]);
    }
    if output.is_empty() {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks(source.len().div_ceil(16) as u64);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        crate::compute::record_pipeline_operation_path("vector");
    }
    Ok(output)
}

pub(crate) fn palette_image(
    width: u32,
    height: u32,
    bytes: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let image = GrayImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("palette remap output shape mismatch".into()))?;
    Ok(DynamicImage::ImagePalette8(image))
}

fn inverse_map(dest_map: &[u8]) -> [u8; 256] {
    let mut inverse = [0u8; 256];
    for (new_index, &old_index) in dest_map.iter().enumerate() {
        inverse[usize::from(old_index)] = new_index as u8;
    }
    inverse
}

fn native_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let mut output = tables[0].shuffle(low);
    for (index, table) in tables.iter().enumerate().skip(1) {
        let selected = high
            .simd_eq(u8x16::splat(index as u8))
            .select(table.shuffle(low), output);
        output = selected;
    }
    output
}
