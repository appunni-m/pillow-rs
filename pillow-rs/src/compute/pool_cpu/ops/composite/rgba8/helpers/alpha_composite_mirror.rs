//! Wide RGBA AlphaComposite+Mirror kernel and its tail contract.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::compute::pool_cpu::ops::composite::helpers::alpha_composite_vector;

/// Vector-composites and mirrors full-frame RGBA inputs without an intermediate image.
pub(crate) fn execute(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
) -> Result<(Vec<u8>, u64, u64, u64, u64), PilError> {
    if width < 8 || height == 0 {
        return Err(PilError::InternalError(
            "RGBA AlphaComposite mirror vector strategy received unsupported dimensions".into(),
        ));
    }
    let dimensions = CheckedDims::new(width, height, 4)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "SIMD RGBA alpha composite mirror storage does not match its dimensions".into(),
        ));
    }

    let width = width as usize;
    let height = height as usize;
    let row_stride = dimensions.row_stride();
    let vector_pixels = width / 8 * 8;
    let vector_groups_per_row = vector_pixels / 8;
    let mut output = dimensions.alloc_buffer();
    let mut mirror_vector_blocks = 0u64;
    for row in 0..height {
        let row_start = row * row_stride;
        mirror_vector_blocks = process_vector_groups(
            source,
            destination,
            &mut output,
            row_start,
            width,
            vector_pixels,
            mirror_vector_blocks,
        )?;
        process_scalar_tail(source, destination, &mut output, row_start, width, vector_pixels);
    }

    let vector_blocks = (vector_groups_per_row * height) as u64;
    let scalar_tail = ((width - vector_pixels) * height) as u64;
    Ok((output, vector_blocks, scalar_tail, mirror_vector_blocks, scalar_tail))
}

fn process_vector_groups(
    source: &[u8],
    destination: &[u8],
    output: &mut [u8],
    row_start: usize,
    width: usize,
    vector_pixels: usize,
    mut mirror_vector_blocks: u64,
) -> Result<u64, PilError> {
    for output_x in (0..vector_pixels).step_by(8) {
        let source_start = row_start + (width - output_x - 8) * 4;
        let source_block = &source[source_start..source_start + 32];
        let destination_block = &destination[source_start..source_start + 32];
        let output_start = row_start + output_x * 4;
        if composite_repeated_rgba_pair(source_block, destination_block, output, output_start) {
            continue;
        }

        let mut composite = [0u8; 32];
        composite.copy_from_slice(destination_block);
        alpha_composite_vector::alpha_composite_chunk::<4>(source_block, &mut composite);
        write_reversed_vector_group(&composite, output, output_start)?;
        mirror_vector_blocks = mirror_vector_blocks.saturating_add(2);
    }
    Ok(mirror_vector_blocks)
}

fn composite_repeated_rgba_pair(
    source: &[u8],
    destination: &[u8],
    output: &mut [u8],
    output_start: usize,
) -> bool {
    if !alpha_composite_vector::alpha_composite_repeated_pixel_pair::<4>(source, destination) {
        return false;
    }
    let mut result_pixel = [0u8; 4];
    result_pixel.copy_from_slice(&destination[..4]);
    alpha_composite_vector::alpha_composite_scalar_pixel(source, &mut result_pixel, 4);
    alpha_composite_vector::alpha_composite_broadcast_pixel::<4>(
        &result_pixel,
        &mut output[output_start..output_start + 32],
    );
    true
}

fn write_reversed_vector_group(
    composite: &[u8; 32],
    output: &mut [u8],
    output_start: usize,
) -> Result<(), PilError> {
    let reverse = |block: &[u8]| {
        crate::compute::pool_cpu::ops::transpose::helpers::native_mirror::reverse_pixel_block(
            block, 4,
        )
        .ok_or_else(|| PilError::InternalError("SIMD RGBA mirror vector block is incomplete".into()))
    };
    let reversed_right = reverse(&composite[16..])?;
    let reversed_left = reverse(&composite[..16])?;
    output[output_start..output_start + 16].copy_from_slice(&reversed_right);
    output[output_start + 16..output_start + 32].copy_from_slice(&reversed_left);
    Ok(())
}

fn process_scalar_tail(
    source: &[u8],
    destination: &[u8],
    output: &mut [u8],
    row_start: usize,
    width: usize,
    vector_pixels: usize,
) {
    for output_x in vector_pixels..width {
        let source_offset = row_start + (width - 1 - output_x) * 4;
        let output_offset = row_start + output_x * 4;
        output[output_offset..output_offset + 4]
            .copy_from_slice(&destination[source_offset..source_offset + 4]);
        alpha_composite_vector::alpha_composite_scalar_pixel(
            &source[source_offset..source_offset + 4],
            &mut output[output_offset..output_offset + 4],
            4,
        );
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn vector_kernel_matches_scalar_for_rows_and_tails() {
        for width in [8u32, 9, 15, 16, 17, 23] {
            for height in [1u32, 3] {
                for uniform in [false, true] {
                    let length = width as usize * height as usize * 4;
                    let source = make_rgba_bytes(length, uniform, 37);
                    let destination = make_rgba_bytes(length, uniform, 149);
                    let (actual, vector_blocks, scalar_tail, mirror_blocks, mirror_tail) =
                        super::execute(&source, &destination, width, height)
                            .expect("eligible RGBA dimensions have valid byte storage");
                    let expected =
                        crate::compute::pool_cpu::ops::composite::rgba8::default::alpha_composite_mirror_impl::execute(
                            &source,
                            &destination,
                            width,
                            height,
                        )
                        .expect("same-sized RGBA inputs have valid byte storage");
                    let expected_vector_blocks = u64::from(width / 8) * u64::from(height);
                    let expected_scalar_tail = u64::from(width % 8) * u64::from(height);
                    let expected_mirror_blocks = if uniform {
                        0
                    } else {
                        expected_vector_blocks * 2
                    };

                    assert_eq!(actual, expected, "{width}x{height}, uniform={uniform}");
                    assert_eq!(vector_blocks, expected_vector_blocks);
                    assert_eq!(scalar_tail, expected_scalar_tail);
                    assert_eq!(mirror_blocks, expected_mirror_blocks);
                    assert_eq!(mirror_tail, expected_scalar_tail);
                }
            }
        }
    }

    fn make_rgba_bytes(length: usize, uniform: bool, seed: u8) -> Vec<u8> {
        if uniform {
            return [seed, seed.wrapping_add(31), seed.wrapping_add(73), 127]
                .repeat(length / 4);
        }
        (0..length)
            .map(|index| (index as u8).wrapping_mul(seed).wrapping_add(19))
            .collect()
    }
}
