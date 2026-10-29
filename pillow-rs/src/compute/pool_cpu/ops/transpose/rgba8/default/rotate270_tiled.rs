//! Packed-word RGBA Rotate270 path used by the Default strategy on x86_64.
//!
//! This kernel uses ordinary native-endian word loads and stores. It has no
//! AVX/SSE requirement and therefore belongs to the mode's Default owner.

use crate::error::PilError;
use crate::pipeline::TransposeMethod;

const PACKED_RGBA_TILE_SIZE: usize = 32;
const CHANNELS: u8 = 4;

pub(crate) fn try_execute(
    image: &super::super::Input,
    method: &TransposeMethod,
) -> Result<Option<super::super::Input>, PilError> {
    let (width, height) = image.dimensions();
    if !matches!(method, TransposeMethod::Rotate270)
        || !super::super::super::helpers::pixel::should_tile_transpose(width, height)
        || width % 4 != 0
        || height % 4 != 0
    {
        return Ok(None);
    }

    let dimensions = crate::checked_dims::CheckedDims::new(height, width, CHANNELS)?;
    let mut output = dimensions.alloc_buffer();
    rotate270_tiled(image.as_raw(), &mut output, width, height);
    let output = super::super::Input::from_raw(height, width, output)
        .ok_or_else(|| PilError::InternalError("packed RGBA Rotate270 shape mismatch".into()))?;
    Ok(Some(output))
}

#[inline(always)]
fn load_rgba_words(row: &[u8]) -> [u32; 4] {
    [
        u32::from_ne_bytes(row[0..4].try_into().expect("RGBA word")),
        u32::from_ne_bytes(row[4..8].try_into().expect("RGBA word")),
        u32::from_ne_bytes(row[8..12].try_into().expect("RGBA word")),
        u32::from_ne_bytes(row[12..16].try_into().expect("RGBA word")),
    ]
}

#[inline(always)]
fn store_rgba_words(row: &mut [u8], words: [u32; 4]) {
    row[0..4].copy_from_slice(&words[0].to_ne_bytes());
    row[4..8].copy_from_slice(&words[1].to_ne_bytes());
    row[8..12].copy_from_slice(&words[2].to_ne_bytes());
    row[12..16].copy_from_slice(&words[3].to_ne_bytes());
}

/// Transposes 4×4 pixel blocks within 32×32 outer tiles.
///
/// Each source row contributes four contiguous native pixels. Their packed
/// words are written as four contiguous destination rows.
fn rotate270_tiled(source: &[u8], output: &mut [u8], width: u32, height: u32) {
    let width = width as usize;
    let height = height as usize;
    let source_stride = width * usize::from(CHANNELS);
    let output_stride = height * usize::from(CHANNELS);
    let tile_size = PACKED_RGBA_TILE_SIZE;

    debug_assert_eq!(output.len(), source_stride * height);
    debug_assert_eq!(width % 4, 0);
    debug_assert_eq!(height % 4, 0);

    for output_y_tile in (0..width).step_by(tile_size) {
        let output_y_end = (output_y_tile + tile_size).min(width);
        for output_x_tile in (0..height).step_by(tile_size) {
            let output_x_end = (output_x_tile + tile_size).min(height);
            for output_y in (output_y_tile..output_y_end).step_by(4) {
                for output_x in (output_x_tile..output_x_end).step_by(4) {
                    transpose_rgba_block(
                        source,
                        output,
                        width,
                        height,
                        output_x,
                        output_y,
                    );
                }
            }
        }
    }
}

fn transpose_rgba_block(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    output_x: usize,
    output_y: usize,
) {
    let source_stride = width * usize::from(CHANNELS);
    let output_stride = height * usize::from(CHANNELS);
    let source_x_offset = output_y * usize::from(CHANNELS);
    let source_y = [
        height - 1 - output_x,
        height - 2 - output_x,
        height - 3 - output_x,
        height - 4 - output_x,
    ];
    let rows = source_y.map(|y| {
        let start = y * source_stride + source_x_offset;
        load_rgba_words(&source[start..start + 16])
    });
    let destination_start = output_y * output_stride + output_x * usize::from(CHANNELS);
    for channel in 0..4 {
        let words = [
            rows[0][channel],
            rows[1][channel],
            rows[2][channel],
            rows[3][channel],
        ];
        let start = destination_start + channel * output_stride;
        store_rgba_words(&mut output[start..start + 16], words);
    }
}
