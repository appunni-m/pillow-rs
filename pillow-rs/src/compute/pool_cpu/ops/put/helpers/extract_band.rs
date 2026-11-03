//! Original serial band algorithms, shared only by modes with the same contract.

use crate::checked_dims::CheckedDims;
use crate::compute::registry::support::{
    InputContract, ImplementationSupport, ModeSupport, OperationId, OutputModeRule,
    PaletteKnowledge, RequestFacts, StrategyInputs,
};
use crate::error::PilError;
use crate::raster::{
    DynamicImage, GrayImage, ImageBuffer, Luma, Mode, Pixel, Rgb, RgbaImage,
};

pub(crate) struct CheckedBand<'a, P: Pixel> {
    pub(crate) input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) dimensions: CheckedDims,
    pub(crate) index: usize,
}

impl<'a, P: Pixel> CheckedBand<'a, P> {
    pub(crate) fn new(input: &'a ImageBuffer<P, Vec<P::Subpixel>>, index: u8)
        -> Result<Self, PilError> {
        let source_bytes_per_pixel = usize::from(P::CHANNEL_COUNT)
            .checked_mul(core::mem::size_of::<P::Subpixel>())
            .and_then(|bytes| u8::try_from(bytes).ok())
            .ok_or_else(|| {
                PilError::InternalError(
                    "ExtractBand source pixel width does not fit its dimension contract".into(),
                )
            })?;
        let dimensions = CheckedDims::new_allow_empty(
            input.width(),
            input.height(),
            source_bytes_per_pixel,
        )?;
        Ok(Self { input, dimensions, index: usize::from(index) })
    }
}

pub(crate) fn facts<P: Pixel>(input: &ImageBuffer<P, Vec<P::Subpixel>>, index: u8)
    -> StrategyInputs {
    StrategyInputs {
        width: input.width(), height: input.height(),
        byte_len: Some(core::mem::size_of_val(input.as_raw().as_slice())),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::ExtractBand { index },
    }
}

pub(crate) const fn output_mode(mode: Mode) -> Mode {
    match mode {
        Mode::ImageLumaA16 | Mode::ImageRgb16 | Mode::ImageRgba16 => Mode::ImageI16,
        Mode::ImageRgb32F | Mode::ImageRgba32F => Mode::ImageF32,
        _ => Mode::ImageLuma8,
    }
}

pub(crate) const fn output_bytes_per_pixel(mode: Mode) -> u8 {
    match output_mode(mode) {
        Mode::ImageI16 => 2,
        Mode::ImageF32 => 4,
        _ => 1,
    }
}

pub(crate) const fn support(
    mode: Mode,
    inputs: &'static [InputContract],
    architectures: &'static [ImplementationSupport],
) -> ModeSupport {
    let output = OutputModeRule::Fixed(output_mode(mode));
    match mode {
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::ExtractBand,
            mode,
            output,
            inputs,
            architectures,
            "native high-depth ExtractBand extends the Pillow mode contract",
        ),
        _ => ModeSupport::cpu(OperationId::ExtractBand, mode, output, inputs, architectures),
    }
}

fn finish_bytes(width: u32, height: u32, samples: Vec<u8>) -> Result<DynamicImage, PilError> {
    if samples.capacity() != 0 {
        crate::compute::record_pipeline_allocation(samples.capacity());
    }
    crate::image_utils::raw_bytes_to_image_allow_empty(width, height, samples, 1)
}

fn gray(width: u32, height: u32) -> GrayImage {
    let output = GrayImage::new(width, height);
    if output.as_raw().capacity() != 0 {
        crate::compute::record_pipeline_allocation(output.as_raw().capacity());
    }
    output
}

pub(crate) fn luma(input: CheckedBand<'_, Luma<u8>>) -> Result<DynamicImage, PilError> {
    let pixel_count = input.dimensions.total_pixels();
    let samples = if input.index < 3 {
        input.input.as_raw().iter().copied().take(pixel_count).collect()
    } else {
        vec![u8::MAX; pixel_count]
    };
    finish_bytes(input.input.width(), input.input.height(), samples)
}

pub(crate) fn rgb(input: CheckedBand<'_, Rgb<u8>>) -> Result<DynamicImage, PilError> {
    let channel = input.index.min(3);
    let pixel_count = input.dimensions.total_pixels();
    let samples = if channel < 3 {
        input.input.as_raw().chunks_exact(3).take(pixel_count)
            .map(|pixel| pixel[channel]).collect()
    } else {
        vec![u8::MAX; pixel_count]
    };
    finish_bytes(input.input.width(), input.input.height(), samples)
}

/// LA and RGBA retain their original four-pixel unrolling and logical tail.
pub(crate) fn grouped<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    input: CheckedBand<'_, P>,
) -> Result<DynamicImage, PilError> {
    let mut gray = gray(input.input.width(), input.input.height());
    let band = input.index.min(CHANNELS - 1);
    let source = input.input.as_raw();
    let output = gray.as_mut();
    let pixel_count = input.dimensions.total_pixels();
    let grouped_pixels = pixel_count / 4 * 4;
    for (dst, pixels) in output[..grouped_pixels].chunks_exact_mut(4)
        .zip(source.chunks_exact(CHANNELS * 4).take(grouped_pixels / 4)) {
        dst[0] = pixels[band];
        dst[1] = pixels[band + CHANNELS];
        dst[2] = pixels[band + CHANNELS * 2];
        dst[3] = pixels[band + CHANNELS * 3];
    }
    let source_tail = source.get(grouped_pixels * CHANNELS..).unwrap_or_default();
    for (dst, pixel) in output[grouped_pixels..].iter_mut()
        .zip(source_tail.chunks_exact(CHANNELS).take(pixel_count - grouped_pixels)) {
        *dst = pixel[band];
    }
    Ok(DynamicImage::ImageLuma8(gray))
}

/// Writes an already validated byte-band request directly to its final slice.
/// Missing source pixels retain the zero fill of the image-producing default.
pub(crate) fn grouped_into<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    input: CheckedBand<'_, P>,
    output: &mut [u8],
) -> Result<(u64, u64, &'static str), PilError> {
    let pixel_count = input.dimensions.total_pixels();
    let Some(source_len) = pixel_count.checked_mul(CHANNELS) else {
        return Err(PilError::InternalError(
            "ExtractBand source length overflow".into(),
        ));
    };
    if output.len() != pixel_count {
        return Err(PilError::InternalError(
            "SIMD ExtractBand output length mismatch".into(),
        ));
    }
    let source = input.input.as_raw();
    if source.len() < source_len {
        output.fill(0);
    }
    let band = input.index.min(CHANNELS - 1);
    let grouped_pixels = pixel_count / 4 * 4;
    for (dst, pixels) in output[..grouped_pixels]
        .chunks_exact_mut(4)
        .zip(source.chunks_exact(CHANNELS * 4).take(grouped_pixels / 4))
    {
        dst[0] = pixels[band];
        dst[1] = pixels[band + CHANNELS];
        dst[2] = pixels[band + CHANNELS * 2];
        dst[3] = pixels[band + CHANNELS * 3];
    }
    let source_tail = source.get(grouped_pixels * CHANNELS..).unwrap_or_default();
    for (dst, pixel) in output[grouped_pixels..]
        .iter_mut()
        .zip(source_tail.chunks_exact(CHANNELS).take(pixel_count - grouped_pixels))
    {
        *dst = pixel[band];
    }
    Ok((0, 0, "cpu"))
}

/// Extracts one CoreOnly high-depth band as a native unsigned 16-bit image.
/// None supplies the mode's opaque maximum for defensive RGB band requests.
pub(crate) fn native_u16<P: Pixel<Subpixel = u16>>(
    input: CheckedBand<'_, P>,
    selected: Option<usize>,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new_allow_empty(input.input.width(), input.input.height(), 2)?;
    let channels = usize::from(P::CHANNEL_COUNT);
    let pixel_count = dimensions.total_pixels();
    let required_samples = pixel_count.checked_mul(channels).ok_or_else(|| {
        PilError::InternalError("ExtractBand source length overflow".into())
    })?;
    let source = input.input.as_raw();
    if source.len() < required_samples {
        return Err(PilError::InternalError(
            "ExtractBand source buffer shape mismatch".into(),
        ));
    }
    let channel = selected.filter(|&index| index < usize::from(P::CHANNEL_COUNT));
    let mut samples = Vec::with_capacity(pixel_count);
    for pixel in source.chunks_exact(channels).take(pixel_count) {
        samples.push(channel.map_or(u16::MAX, |index| pixel[index]));
    }
    if samples.capacity() != 0 {
        crate::compute::record_pipeline_allocation(samples.capacity() * core::mem::size_of::<u16>());
    }
    let image = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(
        input.input.width(),
        input.input.height(),
        samples,
    )
    .ok_or_else(|| PilError::InternalError("ExtractBand u16 buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageI16(image))
}

/// Extracts one CoreOnly high-depth band as the native F32 scalar carrier.
/// None supplies 1.0 for defensive RGB band requests.
pub(crate) fn native_f32<P: Pixel<Subpixel = f32>>(
    input: CheckedBand<'_, P>,
    selected: Option<usize>,
) -> Result<DynamicImage, PilError> {
    let channels = usize::from(P::CHANNEL_COUNT);
    let pixel_count = input.dimensions.total_pixels();
    let required_samples = pixel_count.checked_mul(channels).ok_or_else(|| {
        PilError::InternalError("ExtractBand source length overflow".into())
    })?;
    let source = input.input.as_raw();
    if source.len() < required_samples {
        return Err(PilError::InternalError(
            "ExtractBand source buffer shape mismatch".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(input.input.width(), input.input.height(), 4)?;
    let mut bytes = dimensions.alloc_buffer();
    let channel = selected.filter(|&index| index < channels);
    for (destination, pixel) in bytes
        .chunks_exact_mut(4)
        .zip(source.chunks_exact(channels).take(pixel_count))
    {
        destination.copy_from_slice(&channel.map_or(1.0f32, |index| pixel[index]).to_le_bytes());
    }
    if bytes.capacity() != 0 {
        crate::compute::record_pipeline_allocation(bytes.capacity());
    }
    let image = RgbaImage::from_raw(input.input.width(), input.input.height(), bytes)
        .ok_or_else(|| PilError::InternalError("ExtractBand f32 buffer shape mismatch".into()))?;
    Ok(DynamicImage::ImageF32(image))
}

pub(crate) fn clipped_luma16(
    input: CheckedBand<'_, Luma<u16>>,
) -> Result<DynamicImage, PilError> {
    let pixel_count = input.dimensions.total_pixels();
    let samples = if input.index < 3 {
        input
            .input
            .as_raw()
            .iter()
            .take(pixel_count)
            .map(|sample| (*sample).min(u16::from(u8::MAX)) as u8)
            .collect()
    } else {
        vec![u8::MAX; pixel_count]
    };
    finish_bytes(input.input.width(), input.input.height(), samples)
}

#[cfg(any(
    target_arch = "x86_64",
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) fn vector_output(
    width: u32, height: u32, output: (Vec<u8>, u64, u64), path: &'static str,
) -> Result<DynamicImage, PilError> {
    let (output, blocks, tail) = output;
    if blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
    }
    if tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(tail);
    }
    crate::compute::record_pipeline_operation_path(if blocks == 0 { "scalar-gather" } else { path });
    GrayImage::from_raw(width, height, output).map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD ExtractBand buffer shape mismatch".into()))
}
