//! Shared 128-bit byte-LUT kernels for prepared Eval architecture strategies.

use crate::error::PilError;
use wide::u8x16;

type LutTables = [[u8x16; 16]; 4];

pub(crate) struct Kernel<'a, const CHANNELS: usize> {
    source: &'a [u8],
    width: u32,
    height: u32,
    lut: &'a [u8],
    tables: Option<LutTables>,
    identity: bool,
}

pub(crate) struct Output {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes: Vec<u8>,
}

struct Mapped {
    output: Output,
    vector_blocks: u64,
    scalar_tail: Option<u64>,
}

pub(crate) fn prepare<'a, const CHANNELS: usize>(
    source: &'a [u8],
    width: u32,
    height: u32,
    lut: &'a [u8],
) -> Result<Kernel<'a, CHANNELS>, PilError> {
    let expected_entries = CHANNELS
        .checked_mul(256)
        .ok_or_else(|| PilError::InternalError("Eval LUT size overflowed".into()))?;
    if !(1..=4).contains(&CHANNELS) || lut.len() != expected_entries {
        return Err(PilError::InternalError(
            "Eval vector LUT did not match its native mode".into(),
        ));
    }
    let identity = native_lut_is_identity::<CHANNELS>(lut);
    let tables = (!identity).then(|| native_lut_tables::<CHANNELS>(lut)).transpose()?;
    Ok(Kernel { source, width, height, lut, tables, identity })
}

pub(crate) fn execute_sse2<const CHANNELS: usize>(
    kernel: Kernel<'_, CHANNELS>,
) -> Result<Output, PilError> {
    if kernel.identity {
        return copy_identity(kernel);
    }
    finish(map_wide(kernel)?)
}

pub(crate) fn execute_wasm_simd128<const CHANNELS: usize>(
    kernel: Kernel<'_, CHANNELS>,
) -> Result<Output, PilError> {
    if kernel.identity {
        return copy_identity(kernel);
    }
    finish(map_wide(kernel)?)
}

pub(crate) fn execute_neon<const CHANNELS: usize>(
    kernel: Kernel<'_, CHANNELS>,
) -> Result<Output, PilError> {
    if kernel.identity {
        return copy_identity(kernel);
    }
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    if CHANNELS == 3 {
        let mapped = if let Some(coefficients) = neon_affine_coefficients(kernel.lut) {
            neon_rgb_affine_map(&kernel, coefficients)
        } else {
            neon_rgb_lut_map(&kernel)
        };
        if let Some(mapped) = mapped {
            return finish(mapped);
        }
    }
    finish(map_wide(kernel)?)
}

fn copy_identity<const CHANNELS: usize>(
    kernel: Kernel<'_, CHANNELS>,
) -> Result<Output, PilError> {
    let row_stride = (kernel.width as usize)
        .checked_mul(CHANNELS)
        .ok_or_else(|| PilError::InternalError("Eval vector row overflowed".into()))?;
    let expected_len = row_stride
        .checked_mul(kernel.height as usize)
        .ok_or_else(|| PilError::InternalError("Eval vector output size overflowed".into()))?;
    if row_stride == 0 || kernel.source.len() != expected_len {
        return Err(PilError::InternalError(
            "Eval vector source did not match its native mode".into(),
        ));
    }
    crate::compute::record_pipeline_operation_path("native-copy");
    Ok(Output {
        width: kernel.width,
        height: kernel.height,
        bytes: kernel.source.to_vec(),
    })
}

fn finish(mapped: Mapped) -> Result<Output, PilError> {
    crate::compute::record_pipeline_operation_vector_blocks(mapped.vector_blocks);
    if let Some(scalar_tail) = mapped.scalar_tail {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path("vector");
    Ok(mapped.output)
}

fn map_wide<const CHANNELS: usize>(
    kernel: Kernel<'_, CHANNELS>,
) -> Result<Mapped, PilError> {
    let width = kernel.width as usize;
    let height = kernel.height as usize;
    let row_stride = width
        .checked_mul(CHANNELS)
        .ok_or_else(|| PilError::InternalError("Eval vector row overflowed".into()))?;
    let expected_len = row_stride
        .checked_mul(height)
        .ok_or_else(|| PilError::InternalError("Eval vector output size overflowed".into()))?;
    if row_stride == 0 || kernel.source.len() != expected_len {
        return Err(PilError::InternalError(
            "Eval vector source did not match its native mode".into(),
        ));
    }
    let mut bytes = vec![0u8; expected_len];
    let tables = kernel.tables.as_ref().ok_or_else(|| {
        PilError::InternalError("Eval vector LUT tables were not prepared".into())
    })?;
    for y in 0..height {
        let start = y * row_stride;
        map_native_lut_row::<CHANNELS>(
            &kernel.source[start..start + row_stride],
            &mut bytes[start..start + row_stride],
            tables,
        );
    }
    let blocks = CHANNELS
        .saturating_mul(width.div_ceil(16))
        .saturating_mul(height);
    Ok(Mapped {
        output: Output { width: kernel.width, height: kernel.height, bytes },
        vector_blocks: u64::try_from(blocks).unwrap_or(u64::MAX),
        scalar_tail: Some(0),
    })
}

fn native_lut_is_identity<const CHANNELS: usize>(lut: &[u8]) -> bool {
    (0..CHANNELS).all(|channel| {
        let start = channel * 256;
        lut[start..start + 256]
            .iter()
            .enumerate()
            .all(|(sample, &mapped)| mapped == sample as u8)
    })
}

fn native_lut_tables<const CHANNELS: usize>(lut: &[u8]) -> Result<LutTables, PilError> {
    let mut tables = [[u8x16::splat(0); 16]; 4];
    for channel in 0..CHANNELS {
        let start = channel * 256;
        for index in 0..16 {
            let offset = start + index * 16;
            let values = <[u8; 16]>::try_from(&lut[offset..offset + 16]).map_err(|_| {
                PilError::InternalError("Eval vector LUT block was incomplete".into())
            })?;
            tables[channel][index] = u8x16::new(values);
        }
    }
    Ok(tables)
}

#[inline]
fn native_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let lookups: [u8x16; 16] =
        std::array::from_fn(|index| tables[index].swizzle_relaxed(low));
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

fn map_native_lut_row<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    tables: &LutTables,
) {
    if CHANNELS == 1 {
        for (source_chunk, destination_chunk) in
            source.chunks(16).zip(destination.chunks_mut(16))
        {
            let mut padded = [0u8; 16];
            padded[..source_chunk.len()].copy_from_slice(source_chunk);
            let mapped = native_lut_chunk(u8x16::new(padded), &tables[0]).to_array();
            destination_chunk.copy_from_slice(&mapped[..destination_chunk.len()]);
        }
        return;
    }
    let pixel_span = CHANNELS * 16;
    for (source_chunk, destination_chunk) in source
        .chunks(pixel_span)
        .zip(destination.chunks_mut(pixel_span))
    {
        let active_pixels = source_chunk.len() / CHANNELS;
        let mut input = [[0u8; 16]; 4];
        for pixel in 0..active_pixels {
            for channel in 0..CHANNELS {
                input[channel][pixel] = source_chunk[pixel * CHANNELS + channel];
            }
        }
        let mapped: [[u8; 16]; 4] = std::array::from_fn(|channel| {
            native_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
        });
        for pixel in 0..active_pixels {
            for channel in 0..CHANNELS {
                destination_chunk[pixel * CHANNELS + channel] = mapped[channel][pixel];
            }
        }
    }
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
fn neon_affine_coefficients(lut: &[u8]) -> Option<[(u8, u8); 3]> {
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
fn neon_rgb_lut_map<const CHANNELS: usize>(
    kernel: &Kernel<'_, CHANNELS>,
) -> Option<Mapped> {
    use core::arch::aarch64 as neon;

    const CHANNELS_RGB: usize = 3;
    const LANES: usize = 16;
    let row_stride = (kernel.width as usize).checked_mul(CHANNELS_RGB)?;
    let expected_len = row_stride.checked_mul(kernel.height as usize)?;
    if row_stride == 0 || kernel.source.len() != expected_len {
        return None;
    }
    // SAFETY: preparation validates three complete 256-entry LUTs. Each
    // offset selects a complete 64-byte bank within the borrowed LUT.
    let tables: [[neon::uint8x16x4_t; 4]; CHANNELS_RGB] = unsafe {
        std::array::from_fn(|channel| {
            std::array::from_fn(|bank| {
                let offset = channel * 256 + bank * 64;
                neon::vld1q_u8_x4(kernel.lut.as_ptr().add(offset))
            })
        })
    };
    #[inline(always)]
    fn lookup(
        input: core::arch::aarch64::uint8x16_t,
        tables: &[core::arch::aarch64::uint8x16x4_t; 4],
    ) -> core::arch::aarch64::uint8x16_t {
        use core::arch::aarch64 as neon;
        // SAFETY: input and lookup tables are valid NEON values, and the
        // masked indices are always in the 0..64 range covered by each bank.
        unsafe {
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
    let width = kernel.width as usize;
    let height = kernel.height as usize;
    let vector_width = width / LANES * LANES;
    let mut bytes = vec![0u8; expected_len];
    for row in 0..height {
        let row_start = row * row_stride;
        let source_row = &kernel.source[row_start..row_start + row_stride];
        let destination_row = &mut bytes[row_start..row_start + row_stride];
        for pixel in (0..vector_width).step_by(LANES) {
            let byte_offset = pixel * CHANNELS_RGB;
            // SAFETY: vector_width is rounded down to complete groups of
            // sixteen RGB pixels, so both 48-byte accesses stay in this row.
            // The source and destination slices are disjoint and NEON is
            // enabled by this function's compile-time target gate.
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
            let byte_offset = pixel * CHANNELS_RGB;
            for channel in 0..CHANNELS_RGB {
                let sample = usize::from(source_row[byte_offset + channel]);
                destination_row[byte_offset + channel] = kernel.lut[channel * 256 + sample];
            }
        }
    }
    Some(Mapped {
        output: Output { width: kernel.width, height: kernel.height, bytes },
        vector_blocks: u64::try_from(vector_width / LANES * height).unwrap_or(u64::MAX),
        scalar_tail: None,
    })
}

#[cfg(all(
    target_arch = "aarch64",
    target_feature = "neon",
    target_endian = "little"
))]
#[allow(unsafe_code)]
fn neon_rgb_affine_map<const CHANNELS: usize>(
    kernel: &Kernel<'_, CHANNELS>,
    coefficients: [(u8, u8); 3],
) -> Option<Mapped> {
    use core::arch::aarch64 as neon;

    const CHANNELS_RGB: usize = 3;
    const LANES: usize = 16;
    let row_stride = (kernel.width as usize).checked_mul(CHANNELS_RGB)?;
    let expected_len = row_stride.checked_mul(kernel.height as usize)?;
    if row_stride == 0 || kernel.source.len() != expected_len {
        return None;
    }
    // SAFETY: duplicate intrinsics only construct vector constants.
    let multipliers = unsafe {
        [
            neon::vdupq_n_u8(coefficients[0].0),
            neon::vdupq_n_u8(coefficients[1].0),
            neon::vdupq_n_u8(coefficients[2].0),
        ]
    };
    // SAFETY: duplicate intrinsics only construct vector constants.
    let offsets = unsafe {
        [
            neon::vdupq_n_u8(coefficients[0].1),
            neon::vdupq_n_u8(coefficients[1].1),
            neon::vdupq_n_u8(coefficients[2].1),
        ]
    };
    let width = kernel.width as usize;
    let height = kernel.height as usize;
    let vector_width = width / LANES * LANES;
    let mut bytes = vec![0u8; expected_len];
    for row in 0..height {
        let row_start = row * row_stride;
        let source_row = &kernel.source[row_start..row_start + row_stride];
        let destination_row = &mut bytes[row_start..row_start + row_stride];
        for pixel in (0..vector_width).step_by(LANES) {
            let byte_offset = pixel * CHANNELS_RGB;
            // SAFETY: vector_width is rounded down to complete groups of
            // sixteen RGB pixels, so both 48-byte accesses stay in this row.
            // The source and destination slices are disjoint and NEON is
            // enabled by this function's compile-time target gate.
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
            let byte_offset = pixel * CHANNELS_RGB;
            for channel in 0..CHANNELS_RGB {
                let (multiplier, offset) = coefficients[channel];
                destination_row[byte_offset + channel] = source_row[byte_offset + channel]
                    .wrapping_mul(multiplier)
                    .wrapping_add(offset);
            }
        }
    }
    Some(Mapped {
        output: Output { width: kernel.width, height: kernel.height, bytes },
        vector_blocks: u64::try_from(vector_width / LANES * height).unwrap_or(u64::MAX),
        scalar_tail: None,
    })
}
