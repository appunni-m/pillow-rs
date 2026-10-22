//! SIMD adapter for native-L ImageOps.colorize.

use super::{SimdImageShape, SimdLayout, native_lut_tables, simd_unsupported};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, RgbImage};
use wide::u8x16;

pub(super) fn supports_for_image(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma8(_))
        && matches!(mode, None | Some("L"))
        && img.width() != 0
        && img.height() != 0
        && (img.width() as usize)
            .checked_mul(img.height() as usize)
            .is_some_and(|pixels| img.as_bytes().len() == pixels)
}

pub(super) fn supports_for_shape(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape.layout == SimdLayout::Luma8
        && matches!(mode, None | Some("L"))
        && shape.width != 0
        && shape.height != 0
}

/// Apply a 256-entry colorize LUT using a balanced selection tree. Keep the
/// general LUT helper's lowering unchanged for its other users.
#[inline]
fn colorize_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let lookups: [u8x16; 16] = std::array::from_fn(|index| tables[index].swizzle_relaxed(low));
    let bit0 = (high & u8x16::splat(1)).simd_eq(u8x16::splat(0));
    let bit1 = (high & u8x16::splat(2)).simd_eq(u8x16::splat(0));
    let bit2 = (high & u8x16::splat(4)).simd_eq(u8x16::splat(0));
    let bit3 = (high & u8x16::splat(8)).simd_eq(u8x16::splat(0));
    let level1: [u8x16; 8] =
        std::array::from_fn(|index| bit0.select(lookups[index * 2], lookups[index * 2 + 1]));
    let level2: [u8x16; 4] =
        std::array::from_fn(|index| bit1.select(level1[index * 2], level1[index * 2 + 1]));
    let level3: [u8x16; 2] =
        std::array::from_fn(|index| bit2.select(level2[index * 2], level2[index * 2 + 1]));
    bit3.select(level3[0], level3[1])
}

/// Apply ImageOps.colorize's three per-value LUTs to native `L` samples.
///
/// Each sixteen-pixel group performs three vectorized LUT gathers and four
/// vectorized four-pixel RGB interleaves. A partial group is padded and stores
/// only its valid output bytes.
#[inline]
fn colorize_bytes(img: &DynamicImage, lut: &[[u8; 256]; 3]) -> Option<(Vec<u8>, u64, u64)> {
    if !matches!(img, DynamicImage::ImageLuma8(_)) {
        return None;
    }
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source = img.as_bytes();
    let output_bytes = pixels.checked_mul(3)?;
    if pixels == 0 || source.len() != pixels {
        return None;
    }
    let tables = [
        native_lut_tables(&lut[0])?,
        native_lut_tables(&lut[1])?,
        native_lut_tables(&lut[2])?,
    ];
    // Store complete RGB pixels so output construction does not zero-fill a
    // second full frame before overwriting every output byte.
    let mut output = Vec::<[u8; 3]>::with_capacity(output_bytes / 3);
    let mut vector_blocks = 0u64;
    if pixels >= 65_536 {
        for start in (0..pixels).step_by(16) {
            let active = (pixels - start).min(16);
            let mut padded = [0u8; 16];
            padded[..active].copy_from_slice(&source[start..start + active]);
            let input = u8x16::new(padded);
            let red = colorize_lut_chunk(input, &tables[0]).to_array();
            let green = colorize_lut_chunk(input, &tables[1]).to_array();
            let blue = colorize_lut_chunk(input, &tables[2]).to_array();

            // Interleave into exact RGB pixels without zero-filling a second
            // full-frame byte vector.
            let mut packed = [[0u8; 3]; 16];
            for lane in 0..active {
                packed[lane] = [red[lane], green[lane], blue[lane]];
            }
            output.extend_from_slice(&packed[..active]);
            vector_blocks = vector_blocks.saturating_add(4);
        }
    } else {
        let interleave = u8x16::new([0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11, 12, 13, 14, 15]);
        for start in (0..pixels).step_by(16) {
            let active = (pixels - start).min(16);
            let mut padded = [0u8; 16];
            padded[..active].copy_from_slice(&source[start..start + active]);
            let input = u8x16::new(padded);
            let red = colorize_lut_chunk(input, &tables[0]).to_array();
            let green = colorize_lut_chunk(input, &tables[1]).to_array();
            let blue = colorize_lut_chunk(input, &tables[2]).to_array();

            for group in 0..4 {
                let lane = group * 4;
                let packed = u8x16::new([
                    red[lane],
                    red[lane + 1],
                    red[lane + 2],
                    red[lane + 3],
                    green[lane],
                    green[lane + 1],
                    green[lane + 2],
                    green[lane + 3],
                    blue[lane],
                    blue[lane + 1],
                    blue[lane + 2],
                    blue[lane + 3],
                    0,
                    0,
                    0,
                    0,
                ])
                .swizzle_relaxed(interleave)
                .to_array();
                let group_pixels = active.saturating_sub(lane).min(4);
                if group_pixels != 0 {
                    let pixels = [
                        [packed[0], packed[1], packed[2]],
                        [packed[3], packed[4], packed[5]],
                        [packed[6], packed[7], packed[8]],
                        [packed[9], packed[10], packed[11]],
                    ];
                    output.extend_from_slice(&pixels[..group_pixels]);
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    }
    Some((output.into_flattened(), vector_blocks, 0))
}

pub(crate) fn simd_colorize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Colorize {
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    } = op
    else {
        return Err(PilError::ValueError("expected Colorize op".into()));
    };
    if !matches!(mode, None | Some("L")) {
        return Err(simd_unsupported("Colorize"));
    }
    let lut = crate::compute::pool_cpu::ops::imageops::colorize_lut(
        black,
        white,
        *mid,
        *blackpoint,
        *midpoint,
        *whitepoint,
    );
    let Some((output, vector_blocks, scalar_tail)) = colorize_bytes(img, &lut) else {
        return Err(simd_unsupported("Colorize"));
    };
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    RgbImage::from_raw(img.width(), img.height(), output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("SIMD colorize buffer mismatch".into()))
}
