//! Native four-channel CMYK-to-RGB arithmetic shared by architecture routes.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::helpers::channel::{
    gather::gather_interleaved_channel, wide::simd_pack_u16x16,
};
use crate::error::PilError;
use crate::raster::{RgbImage, RgbaImage};
use wide::{u8x16, u16x16};

pub(crate) fn execute_wide(source: &RgbaImage) -> Result<RgbImage, PilError> {
    let mut output = prepare_output(source)?;
    let mut vector_blocks = 0u64;
    for block in source.as_raw().chunks(64) {
        append_wide_block(block, block.len() / 4, &mut output);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish_output(source, output, vector_blocks, 0)
}

#[cfg(target_arch = "x86_64")]
pub(crate) fn execute_ssse3(source: &RgbaImage) -> Result<RgbImage, PilError> {
    let mut output = prepare_output(source)?;
    let mut vector_blocks = 0u64;
    for block in source.as_raw().chunks(64) {
        if block.len() == 64 {
            append_ssse3_block(block, &mut output);
        } else {
            append_wide_block(block, block.len() / 4, &mut output);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    finish_output(source, output, vector_blocks, 0)
}

fn prepare_output(source: &RgbaImage) -> Result<Vec<u8>, PilError> {
    let input = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let output = CheckedDims::new_allow_empty(source.width(), source.height(), 3)?;
    if source.as_raw().len() != input.total_bytes() {
        return Err(PilError::InternalError(
            "CMYK Convert source storage did not match its dimensions".into(),
        ));
    }
    Ok(Vec::with_capacity(output.total_bytes()))
}

fn append_wide_block(source: &[u8], active_pixels: usize, output: &mut Vec<u8>) {
    let mut padded = [0u8; 64];
    padded[..source.len()].copy_from_slice(source);
    let blocks = [0, 16, 32, 48].map(|offset| {
        u8x16::new(
            padded[offset..offset + 16]
                .try_into()
                .expect("CMYK vector block"),
        )
    });
    append_channels(
        gather_interleaved_channel::<4, 0>(&blocks).to_array(),
        gather_interleaved_channel::<4, 1>(&blocks).to_array(),
        gather_interleaved_channel::<4, 2>(&blocks).to_array(),
        gather_interleaved_channel::<4, 3>(&blocks).to_array(),
        active_pixels,
        output,
    );
}

#[cfg(target_arch = "x86_64")]
#[allow(
    unsafe_code,
    reason = "the SSSE3 mode strategy validates the host feature before this kernel is selected"
)]
fn append_ssse3_block(source: &[u8], output: &mut Vec<u8>) {
    let mut channels = [[0u8; 16]; 4];
    for (channel, samples) in channels.iter_mut().enumerate() {
        // SAFETY: source contains sixteen complete CMYK pixels, output has a
        // writable sixteen-byte stack buffer, and this route is selected only
        // when the context confirms the SSSE3 requirement.
        unsafe {
            crate::compute::pool_cpu::helpers::channel::ssse3::gather_interleaved_channel_ssse3_into_raw(
                source,
                samples.as_mut_ptr(),
                4,
                channel,
            );
        }
    }
    append_channels(
        channels[0],
        channels[1],
        channels[2],
        channels[3],
        16,
        output,
    );
}

fn append_channels(
    cyan: [u8; 16],
    magenta: [u8; 16],
    yellow: [u8; 16],
    black: [u8; 16],
    active_pixels: usize,
    output: &mut Vec<u8>,
) {
    let cyan = u16x16::from(u8x16::new(cyan));
    let magenta = u16x16::from(u8x16::new(magenta));
    let yellow = u16x16::from(u8x16::new(yellow));
    let black = u16x16::from(u8x16::new(black));
    let ink = u16x16::splat(u8::MAX.into()) - black;
    let muldiv255 = |value: u16x16| {
        let value = value + u16x16::splat(128);
        ((value >> 8u32) + value) >> 8u32
    };
    let red = simd_pack_u16x16(ink - muldiv255(cyan * ink)).to_array();
    let green = simd_pack_u16x16(ink - muldiv255(magenta * ink)).to_array();
    let blue = simd_pack_u16x16(ink - muldiv255(yellow * ink)).to_array();
    for pixel in 0..active_pixels {
        output.extend_from_slice(&[red[pixel], green[pixel], blue[pixel]]);
    }
}

pub(crate) fn finish_output(
    source: &RgbaImage,
    output: Vec<u8>,
    vector_blocks: u64,
    scalar_tail: u64,
) -> Result<RgbImage, PilError> {
    if vector_blocks == 0 && scalar_tail == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        if vector_blocks != 0 {
            crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        }
        if scalar_tail != 0 {
            crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        }
        crate::compute::record_pipeline_operation_path("vector");
    }
    if output.capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.capacity());
    }
    RgbImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("CMYK Convert RGB output shape mismatch".into()))
}
