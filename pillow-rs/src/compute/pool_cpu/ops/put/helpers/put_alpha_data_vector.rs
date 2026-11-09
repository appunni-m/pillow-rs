//! Original serial vector alpha-mask kernels with source and mask layouts fixed before execution.

use super::put_alpha_data::AlphaDataInput;
use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::put::helpers::alpha_mask::{MaskStrategy, normalize_sample};
use crate::compute::pool_cpu::ops::put::helpers::put_alpha_vector::{
    simd_put_alpha_block, write_put_alpha_scalar,
};
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel, Rgb, RgbaImage};
use wide::u8x16;

const RGB_MASK_VECTOR_THRESHOLD: usize = 512 * 512;

pub(crate) struct NativeInput<'a, P: Pixel<Subpixel = u8>> {
    image: &'a ImageBuffer<P, Vec<u8>>,
    mask: MaskStrategy<'a>,
}

impl<'a, P: Pixel<Subpixel = u8>> NativeInput<'a, P> {
    pub(crate) fn new(input: AlphaDataInput<'a, P>) -> Result<Self, PilError> {
        let mask = input.mask.image();
        let dimensions =
            CheckedDims::new(input.image.width(), input.image.height(), P::CHANNEL_COUNT)
                .map_err(|_| unsupported())?;
        if dimensions.total_bytes() != input.image.as_raw().len()
            || input.image.dimensions() != mask.dimensions()
        {
            return Err(unsupported());
        }
        Ok(Self {
            image: input.image,
            mask: input.mask,
        })
    }
}

fn unsupported() -> PilError {
    PilError::NotImplementedError(
        "SIMD PutAlphaData is unsupported for the validated image layout/parameters".into(),
    )
}

pub(super) fn rgb_mask_row<const ONE: bool>(source: &[u8], mask: &[u8], output: &mut [u8]) {
    let width = mask.len();
    let vector_pixels = width / 4 * 4;
    const SHUFFLE: [u8; 16] = [0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15];
    for ((source_block, mask_block), output_block) in source[..vector_pixels * 3]
        .chunks_exact(12)
        .zip(mask[..vector_pixels].chunks_exact(4))
        .zip(output[..vector_pixels * 4].chunks_exact_mut(16))
    {
        let mut padded = [0u8; 16];
        padded[..12].copy_from_slice(source_block);
        if ONE {
            for (alpha, sample) in padded[12..].iter_mut().zip(mask_block) {
                *alpha = normalize_sample::<true>(*sample);
            }
        } else {
            padded[12..].copy_from_slice(mask_block);
        }
        output_block.copy_from_slice(&u8x16::new(padded).shuffle(u8x16::new(SHUFFLE)).to_array());
    }
    for pixel in vector_pixels..width {
        let source_start = pixel * 3;
        let output_start = pixel * 4;
        output[output_start..output_start + 3]
            .copy_from_slice(&source[source_start..source_start + 3]);
        output[output_start + 3] = normalize_sample::<ONE>(mask[pixel]);
    }
}

fn finish<P: Pixel<Subpixel = u8>>(
    width: u32,
    height: u32,
    bytes: Vec<u8>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    let dimensions = CheckedDims::new(width, height, P::CHANNEL_COUNT)?;
    crate::image_utils::typed_bytes_to_buffer(dimensions, bytes)
}

fn preserve_unchanged_lanes<P: Pixel<Subpixel = u8>, O: Pixel<Subpixel = u8>>(
    source: &ImageBuffer<P, Vec<u8>>,
    output: &mut ImageBuffer<O, Vec<u8>>,
) -> Result<(), PilError> {
    if P::CHANNEL_COUNT != O::CHANNEL_COUNT {
        return Ok(());
    }
    let Some(lanes) = source.hidden_pixel_lanes().cloned() else {
        return Ok(());
    };
    if !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(unsupported());
    }
    Ok(())
}

pub(crate) fn rgb_mask(input: NativeInput<'_, Rgb<u8>>) -> Result<RgbaImage, PilError> {
    native_mask::<3, 4, false, _, _>(input)
}

pub(crate) fn native_mask<
    const SOURCE: usize,
    const OUTPUT: usize,
    const CMYK: bool,
    P: Pixel<Subpixel = u8>,
    O: Pixel<Subpixel = u8>,
>(
    input: NativeInput<'_, P>,
) -> Result<ImageBuffer<O, Vec<u8>>, PilError> {
    let NativeInput { image, mask } = input;
    mask.with_samples(
        |samples| native_mask_samples::<SOURCE, OUTPUT, CMYK, true, _, _>(image, samples),
        |samples| native_mask_samples::<SOURCE, OUTPUT, CMYK, false, _, _>(image, samples),
    )
}

fn native_mask_samples<
    const SOURCE: usize,
    const OUTPUT: usize,
    const CMYK: bool,
    const ONE: bool,
    P: Pixel<Subpixel = u8>,
    O: Pixel<Subpixel = u8>,
>(
    source: &ImageBuffer<P, Vec<u8>>,
    mask: &[u8],
) -> Result<ImageBuffer<O, Vec<u8>>, PilError> {
    let width = usize::try_from(source.width())
        .map_err(|_| PilError::ValueError("SIMD PutAlphaData width overflow".into()))?;
    let height = usize::try_from(source.height())
        .map_err(|_| PilError::ValueError("SIMD PutAlphaData height overflow".into()))?;
    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData pixel count overflow".into()))?;
    if SOURCE == 3 && pixels >= RGB_MASK_VECTOR_THRESHOLD {
        let source_stride = width
            .checked_mul(SOURCE)
            .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData row overflow".into()))?;
        let output_stride = width
            .checked_mul(OUTPUT)
            .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData row overflow".into()))?;
        let output_length = pixels
            .checked_mul(OUTPUT)
            .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData output overflow".into()))?;
        let mut output = vec![0u8; output_length];
        for y in 0..height {
            let source_start = y * source_stride;
            let mask_start = y * width;
            let output_start = y * output_stride;
            rgb_mask_row::<ONE>(
                &source.as_raw()[source_start..source_start + source_stride],
                &mask[mask_start..mask_start + width],
                &mut output[output_start..output_start + output_stride],
            );
        }
        crate::compute::record_pipeline_operation_vector_blocks((width / 4 * height) as u64);
        crate::compute::record_pipeline_operation_scalar_tail((width % 4 * height) as u64);
        let mut output = finish(source.width(), source.height(), output)?;
        preserve_unchanged_lanes(source, &mut output)?;
        return Ok(output);
    }
    let (output, vectors, tail) =
        interleave::<SOURCE, OUTPUT, CMYK, ONE>(source.as_raw(), mask, pixels)
            .ok_or_else(unsupported)?;
    if vectors != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vectors);
    }
    if tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(tail);
    }
    let mut output = finish(source.width(), source.height(), output)?;
    preserve_unchanged_lanes(source, &mut output)?;
    Ok(output)
}

fn interleave<const SOURCE: usize, const OUTPUT: usize, const CMYK: bool, const ONE: bool>(
    source: &[u8],
    mask: &[u8],
    pixels: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    let group = match (SOURCE, OUTPUT) {
        (1, 2) | (2, 2) => 8,
        (3, 4) | (4, 4) => 4,
        _ => return None,
    };
    if source.len() != pixels.checked_mul(SOURCE)? || mask.len() != pixels {
        return None;
    }
    let mut output = vec![0u8; pixels.checked_mul(OUTPUT)?];
    let mut pixel = 0usize;
    while pixel + group <= pixels {
        let source_start = pixel * SOURCE;
        let source_bytes = group * SOURCE;
        let output_start = pixel * OUTPUT;
        let output_bytes = group * OUTPUT;
        let mut block = [0u8; 16];
        block[..source_bytes].copy_from_slice(&source[source_start..source_start + source_bytes]);
        let mut alpha = [0u8; 16];
        if ONE {
            for (output, sample) in alpha[..group].iter_mut().zip(&mask[pixel..pixel + group]) {
                *output = normalize_sample::<true>(*sample);
            }
        } else {
            alpha[..group].copy_from_slice(&mask[pixel..pixel + group]);
        }
        let result = simd_put_alpha_block::<SOURCE, OUTPUT, CMYK>(block, alpha)?;
        output[output_start..output_start + output_bytes].copy_from_slice(&result[..output_bytes]);
        pixel += group;
    }
    for index in pixel..pixels {
        write_put_alpha_scalar::<SOURCE, OUTPUT, CMYK>(
            source,
            index * SOURCE,
            &mut output,
            index * OUTPUT,
            normalize_sample::<ONE>(mask[index]),
        )?;
    }
    Some((output, (pixel / group) as u64, (pixels - pixel) as u64))
}
