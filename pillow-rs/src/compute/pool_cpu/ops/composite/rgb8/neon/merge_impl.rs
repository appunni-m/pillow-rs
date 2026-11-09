//! NEON structure-store interleave for native RGB Merge.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::composite::helpers::{merge, native_bytes};
use crate::compute::pool_cpu::ops::composite::ops::MergeLayout;
use crate::compute::registry::support::{StrategyGuard, StrategySelection};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let (width, height) = (source.width(), source.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, 3)?;
    let pixels = dimensions.total_bytes() / 3;
    if selection.guard != StrategyGuard::MergeRgbInterleave {
        return merge::interleave_wide(
            source,
            remaining,
            Mode::ImageRgb8,
            MergeLayout::Three {
                lab_encoding: false,
            },
        );
    }
    let bands = merge::collect_bands(source, remaining, pixels)?;
    let [red, green, blue] = bands.as_slice() else {
        return Err(PilError::InternalError(
            "NEON RGB Merge requires exactly three bands".into(),
        ));
    };
    let output_len = dimensions.total_bytes();
    let (output, vector_blocks, scalar_tail) =
        interleave_rgb_neon(red, green, blue, pixels, output_len);
    let mut image = native_bytes::construct(width, height, Mode::ImageRgb8, output)?;
    merge::preserve_merged_padding(
        &mut image,
        Mode::ImageRgb8,
        MergeLayout::Three {
            lab_encoding: false,
        },
        pixels,
    )?;
    crate::compute::record_pipeline_allocation(output_len);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("neon-interleave");
    Ok(image)
}

#[allow(unsafe_code)]
fn interleave_rgb_neon(
    red: &[u8],
    green: &[u8],
    blue: &[u8],
    pixels: usize,
    output_len: usize,
) -> (Vec<u8>, u64, u64) {
    use std::arch::aarch64::{uint8x16x3_t, vld1q_u8, vst3q_u8};

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::<u8>::with_capacity(output_len);
    // SAFETY: this module is compiled only for AArch64 builds with NEON, and
    // admission requires three equal one-byte bands with at least sixteen
    // pixels. Loads cover complete 16-pixel blocks below vector_pixels;
    // structure stores and scalar-tail stores stay within output_len. The
    // fresh allocation cannot alias the immutable input bands, and its length
    // remains zero until every output byte has been initialized.
    unsafe {
        let red_ptr = red.as_ptr();
        let green_ptr = green.as_ptr();
        let blue_ptr = blue.as_ptr();
        let output_ptr = output.as_mut_ptr();
        let mut pixel = 0usize;
        while pixel + 64 <= vector_pixels {
            let channels_0 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel)),
                vld1q_u8(green_ptr.add(pixel)),
                vld1q_u8(blue_ptr.add(pixel)),
            );
            let channels_1 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel + 16)),
                vld1q_u8(green_ptr.add(pixel + 16)),
                vld1q_u8(blue_ptr.add(pixel + 16)),
            );
            let channels_2 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel + 32)),
                vld1q_u8(green_ptr.add(pixel + 32)),
                vld1q_u8(blue_ptr.add(pixel + 32)),
            );
            let channels_3 = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel + 48)),
                vld1q_u8(green_ptr.add(pixel + 48)),
                vld1q_u8(blue_ptr.add(pixel + 48)),
            );
            vst3q_u8(output_ptr.add(pixel * 3), channels_0);
            vst3q_u8(output_ptr.add((pixel + 16) * 3), channels_1);
            vst3q_u8(output_ptr.add((pixel + 32) * 3), channels_2);
            vst3q_u8(output_ptr.add((pixel + 48) * 3), channels_3);
            pixel += 64;
        }
        while pixel < vector_pixels {
            let channels = uint8x16x3_t(
                vld1q_u8(red_ptr.add(pixel)),
                vld1q_u8(green_ptr.add(pixel)),
                vld1q_u8(blue_ptr.add(pixel)),
            );
            vst3q_u8(output_ptr.add(pixel * 3), channels);
            pixel += 16;
        }
        for pixel in vector_pixels..pixels {
            let offset = pixel * 3;
            output_ptr.add(offset).write(*red.get_unchecked(pixel));
            output_ptr
                .add(offset + 1)
                .write(*green.get_unchecked(pixel));
            output_ptr.add(offset + 2).write(*blue.get_unchecked(pixel));
        }
        output.set_len(output_len);
    }

    (
        output,
        (vector_pixels / 16) as u64,
        ((pixels - vector_pixels) * 3) as u64,
    )
}
