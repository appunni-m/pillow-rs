//! Shared 128-bit byte-LUT kernels for prepared Eval architecture strategies.

use crate::error::PilError;
use wide::u8x16;

type LutTables = [[u8x16; 16]; 4];

pub(crate) struct Kernel<'a, const CHANNELS: usize> {
    source: &'a [u8],
    width: u32,
    height: u32,
    lut: &'a [u8],
    identity: bool,
}

impl<'a, const CHANNELS: usize> Kernel<'a, CHANNELS> {
    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    pub(crate) fn source(&self) -> &[u8] {
        self.source
    }

    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    pub(crate) const fn width(&self) -> u32 {
        self.width
    }

    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    pub(crate) const fn height(&self) -> u32 {
        self.height
    }

    #[cfg(all(
        target_arch = "aarch64",
        target_feature = "neon",
        target_endian = "little"
    ))]
    pub(crate) fn lut(&self) -> &[u8] {
        self.lut
    }

    pub(crate) const fn is_identity(&self) -> bool {
        self.identity
    }
}

pub(crate) struct Output {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) bytes: Vec<u8>,
}

pub(crate) struct Mapped {
    pub(crate) output: Output,
    pub(crate) vector_blocks: u64,
    pub(crate) scalar_tail: Option<u64>,
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
    Ok(Kernel {
        source,
        width,
        height,
        lut,
        identity,
    })
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
    finish(map_wide(kernel)?)
}

fn copy_identity<const CHANNELS: usize>(kernel: Kernel<'_, CHANNELS>) -> Result<Output, PilError> {
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

pub(crate) fn finish(mapped: Mapped) -> Result<Output, PilError> {
    crate::compute::record_pipeline_operation_vector_blocks(mapped.vector_blocks);
    if let Some(scalar_tail) = mapped.scalar_tail {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path("vector");
    Ok(mapped.output)
}

fn map_wide<const CHANNELS: usize>(kernel: Kernel<'_, CHANNELS>) -> Result<Mapped, PilError> {
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
    let tables = native_lut_tables::<CHANNELS>(kernel.lut)?;
    let mut bytes = vec![0u8; expected_len];
    for y in 0..height {
        let start = y * row_stride;
        map_native_lut_row::<CHANNELS>(
            &kernel.source[start..start + row_stride],
            &mut bytes[start..start + row_stride],
            &tables,
        );
    }
    let blocks = CHANNELS
        .saturating_mul(width.div_ceil(16))
        .saturating_mul(height);
    Ok(Mapped {
        output: Output {
            width: kernel.width,
            height: kernel.height,
            bytes,
        },
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
    let lookups: [u8x16; 16] = std::array::from_fn(|index| tables[index].shuffle(low));
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
        for (source_chunk, destination_chunk) in source.chunks(16).zip(destination.chunks_mut(16)) {
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
