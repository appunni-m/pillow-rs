//! Shared SIMD byte lookup kernels and point-operation adapters.

#[cfg(feature = "parallel")]
use super::simd_rows_mut_serial;
use super::{SimdImageShape, SimdLayout, apply_native_rows, has_valid_byte_data, simd_unsupported};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use wide::u8x16;

/// Return the native byte channel count for Pillow's byte-domain point/eval
/// contract. Indexed and color-space modes are included because their
/// materialized buffers contain the same raw samples that Pillow feeds to the
/// LUT: palette indices stay in `Luma8`, while HSV/YCbCr samples stay in
/// `Rgb8`. CMYK is also included: its four C/M/Y/K samples live in `Rgba8`,
/// but the fourth byte is a color sample rather than alpha.
fn native_point_channels(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "CMYK")) => Some(4),
        _ => None,
    }
}

fn shape_native_point_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match (shape.layout, mode) {
        (SimdLayout::Luma8, None | Some("1" | "L" | "P")) => Some(1),
        (SimdLayout::LumaA8, None | Some("LA")) => Some(2),
        (SimdLayout::Rgb8, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        (SimdLayout::Rgba8, None | Some("RGBA" | "CMYK")) => Some(4),
        _ => None,
    }
}
pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::RemapPalette { dest_map } => native_remap_palette_layout(img, mode)
            .is_some_and(|_| dest_map.len() <= 256 && has_valid_remap_bytes(img)),
        // Eval/PointOp keeps its interleaved native layout. The LUT lookup
        // vectorizes each band; scalar work only deinterleaves because `wide`
        // has no portable byte-gather primitive.
        PipelineOp::Eval { lut } => native_point_channels(img, mode).is_some_and(|channels| {
            lut.len() == 256 * channels && has_valid_byte_data(img, channels)
        }),
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::RemapPalette { dest_map } => {
            dest_map.len() <= 256
                && shape.layout == SimdLayout::Luma8
                && matches!(mode, None | Some("L" | "P"))
        }
        PipelineOp::Eval { lut } => {
            shape_native_point_channels(shape, mode).is_some_and(|channels| {
                lut.len() == 256 * channels
                    && shape
                        .width
                        .checked_mul(shape.height)
                        .and_then(|pixels| pixels.checked_mul(channels as u32))
                        .is_some()
            })
        }
        _ => false,
    }
}

pub(super) fn in_place_supported(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::RemapPalette { dest_map } => native_remap_palette_layout(img, mode)
            .is_some_and(|_| dest_map.len() <= 256 && has_valid_remap_bytes(img)),
        PipelineOp::Eval { lut } => native_point_channels(img, mode)
            .is_some_and(|channels| lut.len() == 256 * channels && img.as_bytes().len() >= 16),
        _ => false,
    }
}

pub(super) fn execute_in_place(
    img: &mut DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::RemapPalette { dest_map } => native_remap_palette_in_place(img, mode, dest_map),
        PipelineOp::Eval { lut } => native_point_lut_in_place(img, mode, lut),
        _ => false,
    }
}

/// Apply a composed byte-domain point lookup without widening the native
/// `L`/`RGB` storage. Other modes are rejected by contextual preflight until
/// their alpha, palette, or typed-sample contracts have native kernels; this
/// helper never enters a packed scalar fallback.
pub(super) fn native_lut_tables(lut: &[u8]) -> Option<[u8x16; 16]> {
    if lut.len() != 256 {
        return None;
    }
    let mut tables = [u8x16::splat(0); 16];
    for (index, table) in tables.iter_mut().enumerate() {
        let start = index * 16;
        *table = u8x16::new(<[u8; 16]>::try_from(&lut[start..start + 16]).ok()?);
    }
    Some(tables)
}

#[inline]
pub(super) fn native_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let mut output = tables[0].swizzle_relaxed(low);
    for (index, table) in tables.iter().enumerate().skip(1) {
        let selected = high
            .simd_eq(u8x16::splat(index as u8))
            .select(table.swizzle_relaxed(low), output);
        output = selected;
    }
    output
}

/// Apply a Point LUT with a balanced selection tree. Each nibble lookup stays
/// independent, then four high-bit decisions select its matching table. This
/// shortens the dependent select chain in the hot Point path without changing
/// the shared LUT lowering used by other operations.
#[inline]
fn native_point_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
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

fn native_remap_palette_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    matches!(img, DynamicImage::ImageLuma8(_))
        .then_some(1)
        .filter(|_| matches!(mode, None | Some("L" | "P")))
}

fn has_valid_remap_bytes(img: &DynamicImage) -> bool {
    let Some(expected) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    img.as_bytes().len() == expected
}

fn native_remap_palette_tables(dest_map: &[u8]) -> Option<[u8x16; 16]> {
    if dest_map.len() > 256 {
        return None;
    }
    let mut inverse = [0u8; 256];
    for (new_index, &old_index) in dest_map.iter().enumerate() {
        inverse[usize::from(old_index)] = u8::try_from(new_index).ok()?;
    }
    native_lut_tables(&inverse)
}

/// Apply Pillow's inverse palette map to a one-byte `L`/`P` index plane.
///
/// Every non-empty chunk, including a final partial chunk, is padded into a
/// `u8x16` lookup. This keeps short public images on a real SIMD data path;
/// the zero padding never reaches the output buffer. Palette colors and
/// alpha metadata remain owned by the `Image` facade, as they are for the
/// CPU implementation.
fn native_remap_palette_bytes(bytes: &mut [u8], tables: &[u8x16; 16]) -> (u64, u64) {
    let mut vector_blocks = 0u64;
    for chunk in bytes.chunks_mut(16) {
        let active = chunk.len();
        let mut padded = [0u8; 16];
        padded[..active].copy_from_slice(chunk);
        let output = native_lut_chunk(u8x16::new(padded), tables).to_array();
        chunk.copy_from_slice(&output[..active]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    (vector_blocks, 0)
}

fn native_remap_palette_in_place(
    img: &mut DynamicImage,
    mode: Option<&str>,
    dest_map: &[u8],
) -> bool {
    if native_remap_palette_layout(img, mode).is_none() || !has_valid_remap_bytes(img) {
        return false;
    }
    let Some(tables) = native_remap_palette_tables(dest_map) else {
        return false;
    };
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let (vector_blocks, scalar_tail) = native_remap_palette_bytes(bytes, &tables);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("vector");
    }
    true
}

pub(super) fn native_lut_tables_for_channels(
    lut: &[u8],
    channels: usize,
) -> Option<[[u8x16; 16]; 4]> {
    if !(1..=4).contains(&channels) || lut.len() != 256 * channels {
        return None;
    }
    let mut tables = [[u8x16::splat(0); 16]; 4];
    for channel in 0..channels {
        let start = channel * 256;
        tables[channel] = native_lut_tables(&lut[start..start + 256])?;
    }
    Some(tables)
}

fn native_rgb_lut_bytes(bytes: &mut [u8], tables: &[[u8x16; 16]; 4]) {
    // A block contains sixteen complete pixels, so all three lookups use all
    // vector lanes. Padding is confined to the final incomplete pixel block.
    for chunk in bytes.chunks_mut(48) {
        let mut input = [[0u8; 16]; 3];
        for (lane, pixel) in chunk.chunks_exact(3).enumerate() {
            input[0][lane] = pixel[0];
            input[1][lane] = pixel[1];
            input[2][lane] = pixel[2];
        }
        let output: [[u8; 16]; 3] = std::array::from_fn(|channel| {
            native_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
        });
        for (lane, pixel) in chunk.chunks_exact_mut(3).enumerate() {
            pixel[0] = output[0][lane];
            pixel[1] = output[1][lane];
            pixel[2] = output[2][lane];
        }
    }
}

/// Apply one LUT per native byte band. The table lookup is vectorized with
/// `u8x16`; only the interleaved-band gather/scatter is scalar because the
/// portable `wide` API has no byte-gather instruction. Independent scanlines
/// are processed through `apply_native_rows` so large LUT passes preserve
/// exclusive row ownership without adding Rayon to the SIMD backend.
pub(super) fn native_lut_apply(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    lut: &[u8],
) -> Option<(u64, u64)> {
    let tables = native_lut_tables_for_channels(lut, channels)?;
    let row_stride = width.checked_mul(channels)?;
    let expected_len = row_stride.checked_mul(height)?;
    if row_stride == 0 || bytes.len() != expected_len {
        return None;
    }
    if channels == 1 || channels == 3 {
        let vector_blocks = width
            .div_ceil(16)
            .saturating_mul(channels)
            .saturating_mul(height) as u64;
        apply_native_rows(bytes, width, height, channels, |row| {
            if channels == 1 {
                native_remap_palette_bytes(row, &tables[0]);
            } else {
                native_rgb_lut_bytes(row, &tables);
            }
        });
        return Some((vector_blocks, 0));
    }
    let vector_blocks = row_stride.div_ceil(16).saturating_mul(height) as u64;
    apply_native_rows(bytes, width, height, channels, |row| {
        for (block, chunk) in row.chunks_mut(16).enumerate() {
            let start = block * 16;
            let active = chunk.len();
            let mut input = [[0u8; 16]; 4];
            let mut lanes = [0usize; 4];
            let mut locations = [(0usize, 0usize); 16];
            for lane in 0..active {
                let channel = (start + lane) % channels;
                let slot = lanes[channel];
                input[channel][slot] = chunk[lane];
                locations[lane] = (channel, slot);
                lanes[channel] += 1;
            }
            let output: [[u8; 16]; 4] = std::array::from_fn(|channel| {
                native_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
            });
            for lane in 0..active {
                let (channel, slot) = locations[lane];
                chunk[lane] = output[channel][slot];
            }
        }
    });
    Some((vector_blocks, 0))
}

/// Map one row into a separate output row so a single Point operation reads
/// each source sample once instead of cloning and then rereading the clone.
fn native_lut_map_row(
    source: &[u8],
    destination: &mut [u8],
    channels: usize,
    tables: &[[u8x16; 16]; 4],
) {
    if channels == 1 {
        for (source_chunk, destination_chunk) in source.chunks(16).zip(destination.chunks_mut(16)) {
            let mut padded = [0u8; 16];
            padded[..source_chunk.len()].copy_from_slice(source_chunk);
            let mapped = native_point_lut_chunk(u8x16::new(padded), &tables[0]).to_array();
            destination_chunk.copy_from_slice(&mapped[..destination_chunk.len()]);
        }
        return;
    }

    for (source_chunk, destination_chunk) in source
        .chunks(channels * 16)
        .zip(destination.chunks_mut(channels * 16))
    {
        let active_pixels = source_chunk.len() / channels;
        let mut input = [[0u8; 16]; 4];
        for pixel in 0..active_pixels {
            for channel in 0..channels {
                input[channel][pixel] = source_chunk[pixel * channels + channel];
            }
        }
        let mut mapped = [[0u8; 16]; 4];
        for channel in 0..channels {
            mapped[channel] =
                native_point_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array();
        }
        for pixel in 0..active_pixels {
            for channel in 0..channels {
                destination_chunk[pixel * channels + channel] = mapped[channel][pixel];
            }
        }
    }
}

/// Apply a channel LUT from immutable input to a single newly allocated
/// output. The output buffer's zero initialization is the only extra pass;
/// unlike clone-then-transform, the source data itself is traversed once.
pub(super) fn native_lut_map_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    lut: &[u8],
    tables: &[[u8x16; 16]; 4],
) -> Option<u64> {
    let row_stride = width.checked_mul(channels)?;
    let expected_len = row_stride.checked_mul(height)?;
    if row_stride == 0
        || source.len() != expected_len
        || destination.len() != expected_len
        || lut.len() != channels.checked_mul(256)?
    {
        return None;
    }
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    if channels == 3 {
        return native_rgb_lut_map_rows_neon(source, destination, width, height, lut);
    }
    #[cfg(feature = "parallel")]
    if source.len() >= 256 * 1024 {
        simd_rows_mut_serial!(
            destination,
            row_stride,
            height,
            |row_start, row_end, _y, row| {
                native_lut_map_row(&source[row_start..row_end], row, channels, tables);
            }
        );
    } else {
        for (source_row, destination_row) in source
            .chunks_exact(row_stride)
            .zip(destination.chunks_exact_mut(row_stride))
            .take(height)
        {
            native_lut_map_row(source_row, destination_row, channels, tables);
        }
    }
    #[cfg(not(feature = "parallel"))]
    {
        for (source_row, destination_row) in source
            .chunks_exact(row_stride)
            .zip(destination.chunks_exact_mut(row_stride))
            .take(height)
        {
            native_lut_map_row(source_row, destination_row, channels, tables);
        }
    }
    let vector_blocks = channels
        .saturating_mul(width.div_ceil(16))
        .saturating_mul(height);
    Some(u64::try_from(vector_blocks).unwrap_or(u64::MAX))
}

/// Apply three independent byte LUTs to native RGB using NEON's four-register
/// table lookup. Splitting each 256-byte LUT into four 64-entry banks reduces
/// the portable kernel's sixteen table lookups and fifteen selects per
/// channel to four `TBL` operations and three selects. `vld3q_u8`/`vst3q_u8`
/// keep RGB interleaved at the memory boundary without scalar lane gathers.
#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
#[allow(unsafe_code)]
fn native_rgb_lut_map_rows_neon(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    lut: &[u8],
) -> Option<u64> {
    use core::arch::aarch64 as neon;

    const CHANNELS: usize = 3;
    const LANES: usize = 16;
    if lut.len() != CHANNELS * 256 {
        return None;
    }
    let row_stride = width.checked_mul(CHANNELS)?;
    let expected_len = row_stride.checked_mul(height)?;
    if row_stride == 0 || source.len() != expected_len || destination.len() != expected_len {
        return None;
    }

    // Each TBL4 table bank is one contiguous 64-byte slice. The validated
    // 768-byte LUT makes every four-vector load in-bounds and initialized.
    let tables: [[neon::uint8x16x4_t; 4]; CHANNELS] = unsafe {
        std::array::from_fn(|channel| {
            std::array::from_fn(|bank| {
                let offset = channel * 256 + bank * 64;
                neon::vld1q_u8_x4(lut.as_ptr().add(offset))
            })
        })
    };

    #[inline(always)]
    fn lookup(
        input: core::arch::aarch64::uint8x16_t,
        tables: &[core::arch::aarch64::uint8x16x4_t; 4],
    ) -> core::arch::aarch64::uint8x16_t {
        use core::arch::aarch64 as neon;

        // SAFETY: this helper and its only caller are compiled only for
        // little-endian AArch64 targets with NEON enabled.
        unsafe {
            // Masked low six bits always index the 64 bytes in one TBL4 bank;
            // the upper two bits choose which bank's value survives.
            let indices = neon::vandq_u8(input, neon::vdupq_n_u8(0x3f));
            let high = neon::vshrq_n_u8(input, 6);
            let low_bank = neon::vceqq_u8(
                neon::vandq_u8(high, neon::vdupq_n_u8(1)),
                neon::vdupq_n_u8(0),
            );
            let high_bank = neon::vceqq_u8(
                neon::vandq_u8(high, neon::vdupq_n_u8(2)),
                neon::vdupq_n_u8(0),
            );
            let bank0 = neon::vqtbl4q_u8(tables[0], indices);
            let bank1 = neon::vqtbl4q_u8(tables[1], indices);
            let bank2 = neon::vqtbl4q_u8(tables[2], indices);
            let bank3 = neon::vqtbl4q_u8(tables[3], indices);
            let low_pair = neon::vbslq_u8(low_bank, bank0, bank1);
            let high_pair = neon::vbslq_u8(low_bank, bank2, bank3);
            neon::vbslq_u8(high_bank, low_pair, high_pair)
        }
    }

    let vector_width = width / LANES * LANES;
    for row in 0..height {
        let row_start = row * row_stride;
        let source_row = &source[row_start..row_start + row_stride];
        let destination_row = &mut destination[row_start..row_start + row_stride];
        for pixel in (0..vector_width).step_by(LANES) {
            let byte_offset = pixel * CHANNELS;
            // SAFETY: each vector block covers exactly sixteen complete RGB
            // pixels (48 bytes); `pixel < vector_width <= width` proves the
            // source load and destination store remain within their row
            // slices. The immutable source and mutable destination are
            // separate safe borrows. NEON is enabled by this function's cfg.
            unsafe {
                let rgb = neon::vld3q_u8(source_row.as_ptr().add(byte_offset));
                let mapped = neon::uint8x16x3_t(
                    lookup(rgb.0, &tables[0]),
                    lookup(rgb.1, &tables[1]),
                    lookup(rgb.2, &tables[2]),
                );
                neon::vst3q_u8(destination_row.as_mut_ptr().add(byte_offset), mapped);
            }
        }
        for pixel in vector_width..width {
            let byte_offset = pixel * CHANNELS;
            for channel in 0..CHANNELS {
                let sample = usize::from(source_row[byte_offset + channel]);
                destination_row[byte_offset + channel] = lut[channel * 256 + sample];
            }
        }
    }
    Some((vector_width / LANES * height) as u64)
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
fn native_rgb_affine_lut_coefficients(lut: &[u8]) -> Option<[(u8, u8); 3]> {
    if lut.len() != 3 * 256 {
        return None;
    }
    Some([
        crate::compute::byte_lut_affine_coefficients(&lut[0..256])?,
        crate::compute::byte_lut_affine_coefficients(&lut[256..512])?,
        crate::compute::byte_lut_affine_coefficients(&lut[512..768])?,
    ])
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
#[allow(unsafe_code)]
fn native_rgb_affine_lut_map_rows_neon(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    coefficients: [(u8, u8); 3],
) -> Option<u64> {
    use core::arch::aarch64 as neon;

    const CHANNELS: usize = 3;
    const LANES: usize = 16;
    let row_stride = width.checked_mul(CHANNELS)?;
    let expected_len = row_stride.checked_mul(height)?;
    if row_stride == 0 || source.len() != expected_len || destination.len() != expected_len {
        return None;
    }

    // Each channel's exact affine LUT becomes one wrapping byte multiply and
    // add. These six vectors are reused for every pixel block.
    // SAFETY: these duplicate intrinsics only construct vector constants and
    // have no pointer or memory preconditions.
    let multipliers = unsafe {
        [
            neon::vdupq_n_u8(coefficients[0].0),
            neon::vdupq_n_u8(coefficients[1].0),
            neon::vdupq_n_u8(coefficients[2].0),
        ]
    };
    // SAFETY: these duplicate intrinsics only construct vector constants and
    // have no pointer or memory preconditions.
    let offsets = unsafe {
        [
            neon::vdupq_n_u8(coefficients[0].1),
            neon::vdupq_n_u8(coefficients[1].1),
            neon::vdupq_n_u8(coefficients[2].1),
        ]
    };
    let vector_width = width / LANES * LANES;
    for row in 0..height {
        let row_start = row * row_stride;
        let source_row = &source[row_start..row_start + row_stride];
        let destination_row = &mut destination[row_start..row_start + row_stride];
        for pixel in (0..vector_width).step_by(LANES) {
            let byte_offset = pixel * CHANNELS;
            // SAFETY: every block is sixteen complete RGB pixels (48 bytes);
            // pixel < vector_width <= width proves both interleaved accesses
            // remain in their row slices. NEON is enabled by this function's
            // compile-time target gate, and the borrows do not overlap.
            unsafe {
                let rgb = neon::vld3q_u8(source_row.as_ptr().add(byte_offset));
                let mapped = neon::uint8x16x3_t(
                    neon::vaddq_u8(neon::vmulq_u8(rgb.0, multipliers[0]), offsets[0]),
                    neon::vaddq_u8(neon::vmulq_u8(rgb.1, multipliers[1]), offsets[1]),
                    neon::vaddq_u8(neon::vmulq_u8(rgb.2, multipliers[2]), offsets[2]),
                );
                neon::vst3q_u8(destination_row.as_mut_ptr().add(byte_offset), mapped);
            }
        }
        for pixel in vector_width..width {
            let byte_offset = pixel * CHANNELS;
            for channel in 0..CHANNELS {
                let (multiplier, offset) = coefficients[channel];
                destination_row[byte_offset + channel] = source_row[byte_offset + channel]
                    .wrapping_mul(multiplier)
                    .wrapping_add(offset);
            }
        }
    }
    Some((vector_width / LANES * height) as u64)
}

#[inline]
pub(super) fn native_lut_is_identity(lut: &[u8], channels: usize) -> bool {
    lut.len() == channels.saturating_mul(256)
        && lut.chunks_exact(256).all(|table| {
            table
                .iter()
                .enumerate()
                .all(|(index, &value)| value == index as u8)
        })
}

fn native_point_lut_in_place(img: &mut DynamicImage, mode: Option<&str>, lut: &[u8]) -> bool {
    let Some(channels) = native_point_channels(img, mode) else {
        return false;
    };
    if lut.len() != 256 * channels || !has_valid_byte_data(img, channels) {
        return false;
    }
    if native_lut_is_identity(lut, channels) {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let Some((vector_blocks, scalar_tail)) = native_lut_apply(bytes, width, height, channels, lut)
    else {
        return false;
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    true
}

pub(crate) fn native_point_lut(
    img: &DynamicImage,
    mode: Option<&str>,
    lut: &[u8],
) -> Option<DynamicImage> {
    let channels = native_point_channels(img, mode)?;
    if lut.len() != 256 * channels || !has_valid_byte_data(img, channels) {
        return None;
    }
    // A composed point chain can cancel exactly (for example, eight
    // ImageOps.invert calls).  Preserve the required copy-on-write boundary,
    // but avoid rebuilding the byte-shuffle tables and traversing every row
    // when the composed map is identity.
    if native_lut_is_identity(lut, channels) {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    let source = img.as_bytes();
    let mut output = vec![0u8; source.len()];
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    if channels == 3 {
        let vector_blocks = if let Some(coefficients) = native_rgb_affine_lut_coefficients(lut) {
            native_rgb_affine_lut_map_rows_neon(
                source,
                &mut output,
                img.width() as usize,
                img.height() as usize,
                coefficients,
            )?
        } else {
            native_rgb_lut_map_rows_neon(
                source,
                &mut output,
                img.width() as usize,
                img.height() as usize,
                lut,
            )?
        };
        let result = crate::image_utils::raw_bytes_to_image_allow_empty(
            img.width(),
            img.height(),
            output,
            3,
        )
        .ok()?;
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_path("vector");
        return Some(result);
    }
    let tables = native_lut_tables_for_channels(lut, channels)?;
    let vector_blocks = native_lut_map_rows(
        source,
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        lut,
        &tables,
    )?;
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        img.width(),
        img.height(),
        output,
        channels,
    )
    .ok()?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    Some(result)
}

pub(crate) fn simd_eval(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Eval { lut } = op else {
        return Err(PilError::ValueError("expected Eval op".into()));
    };
    // Public Image.point is represented as Eval after the binding validates
    // the LUT.  Every admitted layout uses the native interleaved byte
    // kernel; unsupported typed, indexed, or mode-converted layouts must be
    // rejected by contextual preflight and never reach a packed scalar
    // implementation inside this adapter.
    native_point_lut(img, mode, lut).ok_or_else(|| simd_unsupported("Eval"))
}

/// Remap native `L`/`P` samples with Pillow's inverse destination map.
///
/// `dest_map[new_index]` names the old index, so the vector LUT stores
/// `inverse[old_index] = new_index`. The output remains a one-byte index/luma
/// plane; palette colors and alpha metadata are carried by the lazy `Image`
/// wrapper rather than being widened into RGB here.
pub fn simd_remap_palette(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::RemapPalette { dest_map } = op else {
        return Err(PilError::ValueError("expected RemapPalette op".into()));
    };
    if native_remap_palette_layout(img, mode).is_none() || !has_valid_remap_bytes(img) {
        return Err(simd_unsupported("RemapPalette"));
    }
    let Some(tables) = native_remap_palette_tables(dest_map) else {
        return Err(simd_unsupported("RemapPalette"));
    };
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = native_remap_palette_bytes(&mut output, &tables);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("vector");
    }
    crate::image_utils::raw_bytes_to_image_allow_empty(img.width(), img.height(), output, 1)
}
