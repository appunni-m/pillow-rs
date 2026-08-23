//! SIMD adapter wrappers — bridge `pool_simd::ops::scalar` functions to the
//! registry's `SimdOpFn` signature.
//!
//! Each adapter:
//! 1. Extracts packed u32 RGBA pixels from `DynamicImage`
//! 2. Calls the scalar SIMD function
//! 3. Reconstructs `DynamicImage` from the result

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::{
    ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod, TransposeMethod,
};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, Luma, RgbImage, RgbaImage,
};
use std::sync::Arc;
use wide::{f32x8, i16x8, u8x16, u16x8, u16x16, u32x8};

// ── Helper: mode string → encoding ─────────────────────────────────────

/// Convert PIL mode string to SIMD mode code.
/// 0=L, 1=LA, 2=RGB, 3=RGBA, 4=CMYK
fn mode_to_u32(img: &DynamicImage, mode: Option<&str>) -> u32 {
    match mode {
        // Most ordinary Pillow modes have no explicit tag. Derive those from
        // the native raster so L/LA/RGB pipelines exercise the matching SIMD
        // lane instead of being treated as RGBA storage.
        None => dynimg_mode(img),
        Some("RGBA") => 3,
        Some("RGB") => 2,
        Some("LA" | "PA") => 1,
        Some("L" | "1" | "P") => 0,
        _ => 3, // default to RGBA
    }
}

/// Convert ColorMode to SIMD mode code.
fn color_mode_to_u32(cm: &ColorMode) -> u32 {
    match cm {
        ColorMode::L | ColorMode::Mode1 => 0,
        ColorMode::LA => 1,
        ColorMode::RGB => 2,
        ColorMode::RGBA => 3,
        ColorMode::CMYK => 4,
        _ => 3, // fallback
    }
}

/// Convert ResampleFilter to SIMD filter code (0=nearest, 1=bilinear).
fn filter_to_u32(f: &ResampleFilter) -> u32 {
    match f {
        ResampleFilter::Nearest | ResampleFilter::Box => 0,
        _ => 1,
    }
}

/// The packed SIMD resize kernel covers nearest-neighbor and bilinear input
/// paths. Direct public RGB resize with bilinear filtering is handled by the
/// shared exact Rust resampler in `simd_resize`; the other ImageOps wrappers
/// retain their established packed bilinear path until its coefficient-table
/// arithmetic is ported exactly.
fn simd_resize_filter_supported(filter: &ResampleFilter) -> bool {
    matches!(filter, ResampleFilter::Nearest | ResampleFilter::Bilinear)
}

/// Pack a `(r,g,b,a)` tuple into a u32 for SIMD functions.
fn pack_rgba(c: (u8, u8, u8, u8)) -> u32 {
    (c.0 as u32) | ((c.1 as u32) << 8) | ((c.2 as u32) << 16) | ((c.3 as u32) << 24)
}

/// Derive SIMD mode code from a DynamicImage's channel count.
/// 0=L (1ch), 1=LA (2ch), 2=RGB (3ch), 3=RGBA (4ch)
fn dynimg_mode(img: &DynamicImage) -> u32 {
    match img {
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageLuma16(_) => 0,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageLumaA16(_) => 1,
        DynamicImage::ImageRgb8(_)
        | DynamicImage::ImageRgb16(_)
        | DynamicImage::ImageRgb32F(_) => 2,
        DynamicImage::ImageRgba8(_)
        | DynamicImage::ImageRgba16(_)
        | DynamicImage::ImageRgba32F(_) => 3,
    }
}

/// Convert transform mode strings to the packed kernel's logical channel
/// layout. Unlike most SIMD operations, affine transforms must distinguish
/// three-byte modes from four-byte non-alpha storage such as CMYK.
fn transform_mode_to_u32(img: &DynamicImage, mode: Option<&str>) -> u32 {
    match mode {
        Some("HSV" | "YCbCr") => 2,
        Some("CMYK") => 4,
        _ => mode_to_u32(img, mode),
    }
}

/// Return whether the image must stay in its native scalar representation.
///
/// The SIMD pixel buffer is deliberately an RGBA8 packing.  That is a valid
/// representation for ordinary byte images, but it is not a valid sample
/// domain for `F`, `I`, or unsigned 16-bit luma images: converting those modes
/// through `to_rgba8()` changes the values before the geometry kernel sees
/// them.  Keep those paths in the shared pure-Rust geometry implementation,
/// which operates on the native representation and is also used by CPU.
fn uses_native_scalar_mode(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(mode, Some("F" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N"))
        || matches!(img, DynamicImage::ImageLuma16(_))
}

fn native_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some(4),
        _ => None,
    }
}

// Native byte transforms always operate on L/LA/RGB/RGBA layouts. Keeping
// their active-channel masks in one table avoids duplicating an input-driven
// branch in every closure monomorphization of `native_byte_transform`.
const NATIVE_BYTE_ACTIVE_MASKS: [[u8; 16]; 5] = [
    [0; 16],
    [u8::MAX; 16],
    [
        u8::MAX,
        0,
        u8::MAX,
        0,
        u8::MAX,
        0,
        u8::MAX,
        0,
        u8::MAX,
        0,
        u8::MAX,
        0,
        u8::MAX,
        0,
        u8::MAX,
        0,
    ],
    [u8::MAX; 16],
    [
        u8::MAX,
        u8::MAX,
        u8::MAX,
        0,
        u8::MAX,
        u8::MAX,
        u8::MAX,
        0,
        u8::MAX,
        u8::MAX,
        u8::MAX,
        0,
        u8::MAX,
        u8::MAX,
        u8::MAX,
        0,
    ],
];

#[inline]
fn native_byte_transform_bytes<F>(bytes: &mut [u8], channels: usize, transform: &F) -> Option<()>
where
    F: Fn(u8x16) -> u8x16,
{
    // Native-byte callers pass only channel counts 1..=4, so this lookup is
    // total for every supported native byte image.
    let active = NATIVE_BYTE_ACTIVE_MASKS[channels];
    let active_vector = u8x16::new(active);
    let inactive = u8x16::splat(u8::MAX) - active_vector;
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = u8x16::new(<[u8; 16]>::try_from(&*chunk).ok()?);
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        chunk.copy_from_slice(&output.to_array());
    }
    let remainder = chunks.into_remainder();
    for (index, value) in remainder.iter_mut().enumerate() {
        let input = u8x16::new([*value; 16]);
        let transformed = transform(input).to_array()[0];
        let mask = active[index % 16];
        *value = (transformed & mask) | (*value & !mask);
    }
    Some(())
}

#[inline]
fn native_byte_transform<F>(
    img: &DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> Option<DynamicImage>
where
    F: Fn(u8x16) -> u8x16,
{
    match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 1, &transform)?;
            Some(DynamicImage::ImageLuma8(result))
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 2, &transform)?;
            Some(DynamicImage::ImageLumaA8(result))
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 3, &transform)?;
            Some(DynamicImage::ImageRgb8(result))
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 4, &transform)?;
            Some(DynamicImage::ImageRgba8(result))
        }
        _ => None,
    }
}

/// Invert selected byte channels with one portable 16-byte vector operation.
///
/// `wide` selects the target's safe SIMD implementation (NEON, SSE2, or its
/// scalar fallback) without introducing unsafe code into this crate. The mask
/// keeps alpha bytes unchanged when Pillow's operation does not invert alpha,
/// while still allowing LA/RGBA to use the same interleaved path.
fn invert_native_bytes(bytes: &mut [u8], channels: usize, invert_alpha: bool) {
    if !(1..=4).contains(&channels) {
        return;
    }

    let active_channels = if !invert_alpha && matches!(channels, 2 | 4) {
        channels - 1
    } else {
        channels
    };
    let mut active = [0u8; 16];
    for (index, slot) in active.iter_mut().enumerate() {
        let channel = index % channels;
        *slot = if channel < active_channels {
            u8::MAX
        } else {
            0
        };
    }
    let active = u8x16::new(active);
    let inactive = u8x16::splat(u8::MAX) - active;
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let Ok(input) = <[u8; 16]>::try_from(&*chunk) else {
            continue;
        };
        let input = u8x16::new(input);
        let inverted = u8x16::splat(u8::MAX) - input;
        let output = (inverted & active) | (input & inactive);
        chunk.copy_from_slice(&output.to_array());
    }
    let remainder = chunks.into_remainder();
    for (index, value) in remainder.iter_mut().enumerate() {
        let channel = index % channels;
        if channel < active_channels {
            *value = u8::MAX - *value;
        }
    }
}

fn apply_native_rows<F>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    transform: F,
) where
    F: Fn(&mut [u8]) + Send + Sync,
{
    #[cfg(feature = "parallel")]
    let row_stride = width.saturating_mul(channels);
    #[cfg(feature = "parallel")]
    if bytes.len() >= 256 * 1024 {
        crate::par_rows_mut!(
            bytes,
            row_stride,
            height,
            |_row_start, _row_end, _y, row| {
                transform(row);
            }
        );
    } else {
        transform(bytes);
    }
    #[cfg(not(feature = "parallel"))]
    let _ = (width, height, channels);
    #[cfg(not(feature = "parallel"))]
    transform(bytes);
}

/// Apply an 8-bit point operation directly to the image's native byte
/// storage. The packed `u32` adapter is still used for modes and operations
/// that need its logical lane representation, but ordinary byte images do not
/// need an RGBA expansion merely to invert their channels.
fn native_invert(
    img: &DynamicImage,
    mode: Option<&str>,
    invert_alpha: bool,
) -> Option<DynamicImage> {
    let mut result = img.clone();
    let (width, height) = result.dimensions();
    match &mut result {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 1, |row| {
                invert_native_bytes(row, 1, false);
            });
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 2, |row| {
                invert_native_bytes(row, 2, invert_alpha);
            });
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 3, |row| {
                invert_native_bytes(row, 3, false);
            });
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 4, |row| {
                invert_native_bytes(row, 4, invert_alpha);
            });
        }
        _ => return None,
    }
    Some(result)
}

/// Apply a composed byte-domain point lookup without widening the native
/// `L`/`RGB` storage. The packed adapter remains the fallback for all other
/// modes, whose alpha, palette, or typed-sample semantics need more context.
fn native_lut_tables(lut: &[u8]) -> Option<[u8x16; 16]> {
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
fn native_lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
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

pub(crate) fn native_point_lut(
    img: &DynamicImage,
    mode: Option<&str>,
    lut: &[u8],
) -> Option<DynamicImage> {
    let mut result = img.clone();
    let (width, height) = result.dimensions();
    match &mut result {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            if let Some(tables) = native_lut_tables(lut) {
                let bytes = image.as_mut();
                apply_native_rows(bytes, width as usize, height as usize, 1, |row| {
                    let mut chunks = row.chunks_exact_mut(16);
                    for chunk in &mut chunks {
                        let Ok(input) = <[u8; 16]>::try_from(&*chunk) else {
                            return;
                        };
                        let input = u8x16::new(input);
                        chunk.copy_from_slice(&native_lut_chunk(input, &tables).to_array());
                    }
                    for value in chunks.into_remainder() {
                        *value = lut[*value as usize];
                    }
                });
            } else {
                return None;
            }
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB")) => {
            if lut.len() != 3 * 256 {
                return None;
            }
            apply_native_rows(image.as_mut(), width as usize, height as usize, 3, |row| {
                for pixel in row.chunks_exact_mut(3) {
                    pixel[0] = lut[pixel[0] as usize];
                    pixel[1] = lut[256 + pixel[1] as usize];
                    pixel[2] = lut[512 + pixel[2] as usize];
                }
            });
        }
        _ => return None,
    }
    Some(result)
}

/// Convert native luma samples to Pillow's four-byte CMYK storage without
/// first allocating an RGB grayscale image. For an `L`/`LA` source Pillow's
/// CMYK branch is exact: C=M=Y=0 and K=255-L; alpha is not a CMYK sample.
/// Four pixels are assembled into one portable 16-byte vector, while the
/// scalar tail handles images whose pixel count is not divisible by four.
fn native_luma_to_cmyk(img: &DynamicImage) -> Option<DynamicImage> {
    let (width, height) = img.dimensions();
    let (bytes, stride) = match img {
        DynamicImage::ImageLuma8(image) => (image.as_raw().as_slice(), 1usize),
        DynamicImage::ImageLumaA8(image) => (image.as_raw().as_slice(), 2usize),
        _ => return None,
    };
    let pixel_count = (width as usize).checked_mul(height as usize)?;
    let output_len = pixel_count.checked_mul(4)?;
    let mut output = vec![0u8; output_len];
    let cmyk_mask = u8x16::new([
        0,
        0,
        0,
        u8::MAX,
        0,
        0,
        0,
        u8::MAX,
        0,
        0,
        0,
        u8::MAX,
        0,
        0,
        0,
        u8::MAX,
    ]);
    let mut processed = 0usize;
    for (destination, source) in output
        .chunks_exact_mut(16)
        .zip(bytes.chunks_exact(stride * 4))
    {
        let mut luma = [0u8; 16];
        for pixel in 0..4 {
            luma[pixel * 4 + 3] = source[pixel * stride];
        }
        let converted = (u8x16::splat(u8::MAX) - u8x16::new(luma)) & cmyk_mask;
        destination.copy_from_slice(&converted.to_array());
        processed += 4;
    }
    for (destination, source) in output[processed * 4..]
        .chunks_exact_mut(4)
        .zip(bytes[processed * stride..].chunks_exact(stride))
    {
        destination[3] = u8::MAX - source[0];
    }
    RgbaImage::from_raw(width, height, output).map(DynamicImage::ImageRgba8)
}

/// Apply a non-indexed `PutPixel` directly to the source's native byte
/// layout. The operation preserves the source mode, so a later mode-changing
/// operation must observe the written luma/alpha/channel samples rather than
/// a packed RGBA approximation. Typed I/F storage and palette-index writes
/// intentionally remain on their established adapters.
fn native_put_pixel(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    let PipelineOp::PutPixel {
        x,
        y,
        color,
        palette_index,
    } = op
    else {
        return None;
    };
    if *palette_index
        || matches!(
            mode,
            Some("I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N" | "P" | "PA")
        )
    {
        return None;
    }
    if *x >= img.width() || *y >= img.height() {
        return None;
    }
    let mut result = img.clone();
    match &mut result {
        DynamicImage::ImageLuma8(image) => image.put_pixel(*x, *y, Luma([color.0])),
        DynamicImage::ImageLumaA8(image) => {
            image.put_pixel(*x, *y, crate::raster::LumaA([color.0, color.3]))
        }
        DynamicImage::ImageRgb8(image) => {
            image.put_pixel(*x, *y, crate::raster::Rgb([color.0, color.1, color.2]))
        }
        DynamicImage::ImageRgba8(image) => image.put_pixel(
            *x,
            *y,
            crate::raster::Rgba([color.0, color.1, color.2, color.3]),
        ),
        _ => return None,
    }
    Some(result)
}

/// Apply one of the exact byte-domain ImageChops blend formulas without
/// widening ordinary images to packed RGBA pixels.
///
/// The SIMD adapter still has packed scalar fallbacks for indexed, typed, and
/// mode-converted images. These four native variants are the common public
/// byte layouts, and preserving them avoids two full-frame conversions for
/// every dual-image operation in a pipeline.
#[inline]
fn native_blend_byte(left: u8, right: u8, screen: bool) -> u8 {
    if screen {
        255 - ((255 - left) as u16 * (255 - right) as u16 / 255) as u8
    } else {
        (left as u16 * right as u16 / 255) as u8
    }
}

fn apply_native_blend_rows(
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    screen: bool,
) -> bool {
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(output.len())
        || left.len() != output.len()
        || right.len() != output.len()
    {
        return false;
    }
    let apply_row = |left_row: &[u8], right_row: &[u8], output_row: &mut [u8]| {
        for ((output, &left), &right) in output_row.iter_mut().zip(left_row).zip(right_row) {
            *output = native_blend_byte(left, right, screen);
        }
    };

    #[cfg(feature = "parallel")]
    if output.len() >= 256 * 1024 {
        crate::par_rows_mut!(output, row_stride, height, |row_start, row_end, _y, row| {
            apply_row(&left[row_start..row_end], &right[row_start..row_end], row);
        });
    } else {
        for row_index in 0..height {
            let row_start = row_index * row_stride;
            apply_row(
                &left[row_start..row_start + row_stride],
                &right[row_start..row_start + row_stride],
                &mut output[row_start..row_start + row_stride],
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for row_index in 0..height {
        let row_start = row_index * row_stride;
        apply_row(
            &left[row_start..row_start + row_stride],
            &right[row_start..row_start + row_stride],
            &mut output[row_start..row_start + row_stride],
        );
    }
    true
}

fn native_chops_blend(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    screen: bool,
) -> Option<DynamicImage> {
    if img.dimensions() != other.dimensions() {
        return None;
    }
    match (img, other) {
        (DynamicImage::ImageLuma8(left), DynamicImage::ImageLuma8(right))
            if matches!(mode, None | Some("1" | "L" | "P")) =>
        {
            let mut result = left.clone();
            if !apply_native_blend_rows(
                left.as_raw(),
                right.as_raw(),
                result.as_mut(),
                img.width() as usize,
                img.height() as usize,
                1,
                screen,
            ) {
                return None;
            }
            Some(DynamicImage::ImageLuma8(result))
        }
        (DynamicImage::ImageLumaA8(left), DynamicImage::ImageLumaA8(right))
            if matches!(mode, None | Some("LA" | "PA")) =>
        {
            let mut result = left.clone();
            if !apply_native_blend_rows(
                left.as_raw(),
                right.as_raw(),
                result.as_mut(),
                img.width() as usize,
                img.height() as usize,
                2,
                screen,
            ) {
                return None;
            }
            Some(DynamicImage::ImageLumaA8(result))
        }
        (DynamicImage::ImageRgb8(left), DynamicImage::ImageRgb8(right))
            if matches!(mode, None | Some("RGB")) =>
        {
            let mut result = left.clone();
            if !apply_native_blend_rows(
                left.as_raw(),
                right.as_raw(),
                result.as_mut(),
                img.width() as usize,
                img.height() as usize,
                3,
                screen,
            ) {
                return None;
            }
            Some(DynamicImage::ImageRgb8(result))
        }
        (DynamicImage::ImageRgba8(left), DynamicImage::ImageRgba8(right))
            if matches!(mode, None | Some("RGBA")) =>
        {
            let mut result = left.clone();
            if !apply_native_blend_rows(
                left.as_raw(),
                right.as_raw(),
                result.as_mut(),
                img.width() as usize,
                img.height() as usize,
                4,
                screen,
            ) {
                return None;
            }
            Some(DynamicImage::ImageRgba8(result))
        }
        _ => None,
    }
}

#[inline(always)]
fn apply_native_alpha_scalar_pixel(
    source_pixel: &[u8],
    destination_pixel: &mut [u8],
    channels: usize,
) {
    let source_alpha = source_pixel[channels - 1] as f64 / 255.0;
    let destination_alpha = destination_pixel[channels - 1] as f64 / 255.0;
    let output_alpha = source_alpha + destination_alpha * (1.0 - source_alpha);
    if output_alpha <= 0.0 {
        return;
    }
    let inverse_source_alpha = 1.0 - source_alpha;
    let color_channels = channels - 1;
    for channel in 0..color_channels {
        destination_pixel[channel] = ((source_pixel[channel] as f64 * source_alpha
            + destination_pixel[channel] as f64 * destination_alpha * inverse_source_alpha)
            / output_alpha)
            .round()
            .clamp(0.0, 255.0) as u8;
    }
    destination_pixel[channels - 1] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn apply_native_alpha_row(source: &[u8], output: &mut [u8], channels: usize) {
    match channels {
        2 => {
            for (source_pixel, destination_pixel) in
                source.chunks_exact(2).zip(output.chunks_exact_mut(2))
            {
                apply_native_alpha_scalar_pixel(source_pixel, destination_pixel, 2);
            }
        }
        4 => {
            for (source_pixel, destination_pixel) in
                source.chunks_exact(4).zip(output.chunks_exact_mut(4))
            {
                apply_native_alpha_scalar_pixel(source_pixel, destination_pixel, 4);
            }
        }
        _ => {
            let pixels = source.len() / channels;
            for pixel in 0..pixels {
                let start = pixel * channels;
                apply_native_alpha_scalar_pixel(
                    &source[start..start + channels],
                    &mut output[start..start + channels],
                    channels,
                );
            }
        }
    }
}

fn apply_native_alpha_rows(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
) -> bool {
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(output.len()) || source.len() != output.len() {
        return false;
    }

    #[cfg(feature = "parallel")]
    if output.len() >= 256 * 1024 {
        crate::par_rows_mut!(output, row_stride, height, |row_start, row_end, _y, row| {
            apply_native_alpha_row(&source[row_start..row_end], row, channels);
        });
    } else {
        for (y, row) in output.chunks_exact_mut(row_stride).take(height).enumerate() {
            let row_start = y * row_stride;
            apply_native_alpha_row(&source[row_start..row_start + row_stride], row, channels);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output.chunks_exact_mut(row_stride).take(height).enumerate() {
        let row_start = y * row_stride;
        apply_native_alpha_row(&source[row_start..row_start + row_stride], row, channels);
    }
    true
}

fn native_alpha_composite(
    destination: &DynamicImage,
    source: &DynamicImage,
    mode: Option<&str>,
    dest: (i32, i32),
    src: (i32, i32),
) -> Option<DynamicImage> {
    if dest != (0, 0) || src != (0, 0) || destination.dimensions() != source.dimensions() {
        return None;
    }
    match (destination, source) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(source))
            if matches!(mode, None | Some("1" | "L" | "P")) =>
        {
            Some(DynamicImage::ImageLuma8(source.clone()))
        }
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(source))
            if matches!(mode, None | Some("RGB")) =>
        {
            Some(DynamicImage::ImageRgb8(source.clone()))
        }
        (DynamicImage::ImageLumaA8(destination), DynamicImage::ImageLumaA8(source))
            if matches!(mode, None | Some("LA")) =>
        {
            let mut output = destination.clone();
            if apply_native_alpha_rows(
                source.as_raw(),
                output.as_mut(),
                destination.width() as usize,
                destination.height() as usize,
                2,
            ) {
                Some(DynamicImage::ImageLumaA8(output))
            } else {
                None
            }
        }
        (DynamicImage::ImageRgba8(destination), DynamicImage::ImageRgba8(source))
            if matches!(mode, None | Some("RGBA")) =>
        {
            let mut output = destination.clone();
            if apply_native_alpha_rows(
                source.as_raw(),
                output.as_mut(),
                destination.width() as usize,
                destination.height() as usize,
                4,
            ) {
                Some(DynamicImage::ImageRgba8(output))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Apply an exact byte-wise Chops operation without widening the native image.
///
/// These formulas are lane-local and preserve Pillow's byte semantics exactly:
/// min/max, absolute difference, modulo add/subtract, and logical operations
/// do not need the widening and rounding used by multiply/screen. The helper
/// keeps the scalar tail outside the vector loop and returns `None` for typed
/// or mode-converted layouts that cannot safely stay in their native bytes.
fn native_chops_bytewise<F, G>(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    vector_op: F,
    scalar_op: G,
) -> Option<DynamicImage>
where
    F: Fn(u8x16, u8x16) -> u8x16 + Send + Sync,
    G: Fn(u8, u8) -> u8 + Send + Sync,
{
    let channels = match (img, other) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_))
            if matches!(mode, None | Some("1" | "L" | "P")) =>
        {
            1
        }
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_))
            if matches!(mode, None | Some("LA" | "PA")) =>
        {
            2
        }
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_))
            if matches!(mode, None | Some("RGB")) =>
        {
            3
        }
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(_))
            if matches!(mode, None | Some("RGBA")) =>
        {
            4
        }
        _ => return None,
    };
    if img.dimensions() != other.dimensions() {
        return None;
    }
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || left.len() % channels != 0 {
        return None;
    }

    let mut result = img.clone();
    let output = match &mut result {
        DynamicImage::ImageLuma8(image) => image.as_mut(),
        DynamicImage::ImageLumaA8(image) => image.as_mut(),
        DynamicImage::ImageRgb8(image) => image.as_mut(),
        DynamicImage::ImageRgba8(image) => image.as_mut(),
        _ => return None,
    };
    let Some(row_stride) = img.width().checked_mul(channels as u32) else {
        return None;
    };
    let row_stride = row_stride as usize;
    let height = img.height() as usize;
    if row_stride.checked_mul(height) != Some(output.len()) {
        return None;
    }
    let apply_row = |left_row: &[u8], right_row: &[u8], output_row: &mut [u8]| {
        let vector_len = output_row.len() / 16 * 16;
        for start in (0..vector_len).step_by(16) {
            let Ok(left_chunk) = <[u8; 16]>::try_from(&left_row[start..start + 16]) else {
                return;
            };
            let Ok(right_chunk) = <[u8; 16]>::try_from(&right_row[start..start + 16]) else {
                return;
            };
            let left_chunk = u8x16::new(left_chunk);
            let right_chunk = u8x16::new(right_chunk);
            output_row[start..start + 16]
                .copy_from_slice(&vector_op(left_chunk, right_chunk).to_array());
        }
        for index in vector_len..output_row.len() {
            output_row[index] = scalar_op(left_row[index], right_row[index]);
        }
    };
    #[cfg(feature = "parallel")]
    if output.len() >= 256 * 1024 {
        crate::par_rows_mut!(output, row_stride, height, |row_start, row_end, _y, row| {
            apply_row(&left[row_start..row_end], &right[row_start..row_end], row);
        });
    } else {
        for row_index in 0..height {
            let row_start = row_index * row_stride;
            apply_row(
                &left[row_start..row_start + row_stride],
                &right[row_start..row_start + row_stride],
                &mut output[row_start..row_start + row_stride],
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for row_index in 0..height {
        let row_start = row_index * row_stride;
        apply_row(
            &left[row_start..row_start + row_stride],
            &right[row_start..row_start + row_stride],
            &mut output[row_start..row_start + row_stride],
        );
    }
    Some(preserve_mode(img, result))
}

macro_rules! native_bytewise_chops {
    ($name:ident, $vector_op:expr, $scalar_op:expr) => {
        fn $name(
            img: &DynamicImage,
            other: &DynamicImage,
            mode: Option<&str>,
        ) -> Option<DynamicImage> {
            native_chops_bytewise(img, other, mode, $vector_op, $scalar_op)
        }
    };
}

native_bytewise_chops!(
    native_chops_darker,
    |left: u8x16, right: u8x16| left.min(right),
    |left: u8, right: u8| left.min(right)
);
native_bytewise_chops!(
    native_chops_lighter,
    |left: u8x16, right: u8x16| left.max(right),
    |left: u8, right: u8| left.max(right)
);
native_bytewise_chops!(
    native_chops_difference,
    |left: u8x16, right: u8x16| left.max(right) - left.min(right),
    |left: u8, right: u8| left.abs_diff(right)
);
native_bytewise_chops!(
    native_chops_add_modulo,
    |left: u8x16, right: u8x16| left + right,
    |left: u8, right: u8| left.wrapping_add(right)
);
native_bytewise_chops!(
    native_chops_subtract_modulo,
    |left: u8x16, right: u8x16| left - right,
    |left: u8, right: u8| left.wrapping_sub(right)
);
native_bytewise_chops!(
    native_chops_add_clamped,
    |left: u8x16, right: u8x16| left.saturating_add(right),
    |left: u8, right: u8| left.saturating_add(right)
);
native_bytewise_chops!(
    native_chops_subtract_clamped,
    |left: u8x16, right: u8x16| left.saturating_sub(right),
    |left: u8, right: u8| left.saturating_sub(right)
);
native_bytewise_chops!(
    native_chops_logical_and,
    |left: u8x16, right: u8x16| left & right,
    |left: u8, right: u8| left & right
);
native_bytewise_chops!(
    native_chops_logical_or,
    |left: u8x16, right: u8x16| left | right,
    |left: u8, right: u8| left | right
);
native_bytewise_chops!(
    native_chops_logical_xor,
    |left: u8x16, right: u8x16| left ^ right,
    |left: u8, right: u8| left ^ right
);

/// Fuse `multiply(other) → screen(other)` for matching native 8-bit layouts.
///
/// The multiply truncation is intentionally kept between the two formulas;
/// this is algebraically one traversal but not a reordered approximation of
/// Pillow's two public operations.
#[inline]
fn simd_div255(value: u16x16) -> u16x16 {
    // For 0 <= value <= 65025, this is exactly floor(value / 255), including
    // both endpoints.  It replaces an integer divide without changing the
    // intermediate truncation required by ImageChops.multiply/screen.
    let incremented = value + u16x16::splat(1);
    (incremented + (incremented >> 8u32)) >> 8u32
}

#[inline]
fn simd_pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}

#[inline]
fn simd_fused_multiply_screen_row(
    left_bytes: &[u8],
    right_bytes: &[u8],
    output: &mut [u8],
) -> bool {
    if left_bytes.len() != right_bytes.len() || left_bytes.len() != output.len() {
        return false;
    }

    let mut left_chunks = left_bytes.chunks_exact(16);
    let mut right_chunks = right_bytes.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for ((left_chunk, right_chunk), output_chunk) in left_chunks
        .by_ref()
        .zip(right_chunks.by_ref())
        .zip(output_chunks.by_ref())
    {
        let Ok(left) = <[u8; 16]>::try_from(left_chunk) else {
            return false;
        };
        let Ok(right) = <[u8; 16]>::try_from(right_chunk) else {
            return false;
        };
        let left = u16x16::from(u8x16::new(left));
        let right = u16x16::from(u8x16::new(right));
        let multiplied = simd_div255(left * right);
        let result = multiplied + right - simd_div255(multiplied * right);
        output_chunk.copy_from_slice(&simd_pack_u16x16(result).to_array());
    }
    for ((&left, &right), output) in left_chunks
        .remainder()
        .iter()
        .zip(right_chunks.remainder())
        .zip(output_chunks.into_remainder())
    {
        let multiplied = (left as u32 * right as u32 / 255) as u8;
        *output = (255u32 - ((255 - multiplied as u32) * (255 - right as u32) / 255)) as u8;
    }
    true
}

pub(crate) fn simd_fused_multiply_screen(
    img: &DynamicImage,
    first_other: &Arc<Image>,
    second_other: &Arc<Image>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if mode.is_some() || !first_other.shares_execution_source(second_other) {
        return Ok(None);
    }
    let other = materialize_chops_operand(first_other, mode)?;
    let channels = match (img, &other) {
        (DynamicImage::ImageLuma8(_), DynamicImage::ImageLuma8(_)) => 1usize,
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_)) => 2,
        (DynamicImage::ImageRgb8(_), DynamicImage::ImageRgb8(_)) => 3,
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(_)) => 4,
        _ => return Ok(None),
    };
    if img.dimensions() != other.dimensions() {
        return Ok(None);
    }

    let left_bytes = img.as_bytes();
    let right_bytes = other.as_bytes();
    let mut output = vec![0u8; left_bytes.len()];
    let (width, height) = img.dimensions();
    let row_stride = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD fused Chops row stride overflow".into()))?;
    if row_stride == 0 || row_stride.checked_mul(height as usize) != Some(output.len()) {
        return Ok(None);
    }

    #[cfg(feature = "parallel")]
    if output.len() >= 256 * 1024 {
        crate::par_rows_mut!(
            &mut output,
            row_stride,
            height as usize,
            |row_start, row_end, _y, row| {
                let _ = simd_fused_multiply_screen_row(
                    &left_bytes[row_start..row_end],
                    &right_bytes[row_start..row_end],
                    row,
                );
            }
        );
    } else if !simd_fused_multiply_screen_row(left_bytes, right_bytes, &mut output) {
        return Ok(None);
    }

    #[cfg(not(feature = "parallel"))]
    if !simd_fused_multiply_screen_row(left_bytes, right_bytes, &mut output) {
        return Ok(None);
    }
    let result =
        match channels {
            1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, output).ok_or_else(
                || PilError::InternalError("SIMD fused Chops buffer shape mismatch".into()),
            )?),
            2 => DynamicImage::ImageLumaA8(
                GrayAlphaImage::from_raw(width, height, output).ok_or_else(|| {
                    PilError::InternalError("SIMD fused Chops buffer shape mismatch".into())
                })?,
            ),
            3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, output).ok_or_else(
                || PilError::InternalError("SIMD fused Chops buffer shape mismatch".into()),
            )?),
            4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, output).ok_or_else(
                || PilError::InternalError("SIMD fused Chops buffer shape mismatch".into()),
            )?),
            _ => return Ok(None),
        };
    Ok(Some(preserve_mode(img, result)))
}

/// Reverse rows in native 8-bit storage without packing pixels into RGBA.
#[inline]
fn reverse_pixel_block(input: &[u8], channels: usize) -> Option<[u8; 16]> {
    let indices = match channels {
        1 => [15, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
        2 => [14, 15, 12, 13, 10, 11, 8, 9, 6, 7, 4, 5, 2, 3, 0, 1],
        4 => [12, 13, 14, 15, 8, 9, 10, 11, 4, 5, 6, 7, 0, 1, 2, 3],
        _ => return None,
    };
    let input = u8x16::new(<[u8; 16]>::try_from(input).ok()?);
    Some(input.swizzle_relaxed(u8x16::new(indices)).to_array())
}

fn mirror_native_bytes(bytes: &mut [u8], width: u32, height: u32, channels: usize) -> bool {
    let Some(row_len) = (width as usize).checked_mul(channels) else {
        return false;
    };
    let Some(total_len) = row_len.checked_mul(height as usize) else {
        return false;
    };
    let Some(rows) = bytes.get_mut(..total_len) else {
        return false;
    };
    for row in rows.chunks_exact_mut(row_len) {
        // A 16-byte lane can reverse complete L, LA, or RGBA pixel groups.
        // RGB is intentionally left on the scalar path: its three-byte
        // pixels straddle 16-byte lanes and a lane-local shuffle would either
        // reorder channels or require an extra cross-lane staging pass.
        if matches!(channels, 1 | 2 | 4) {
            let mut left = 0usize;
            let mut right = row_len;
            while right.saturating_sub(left) >= 32 {
                let Ok(left_block) = <[u8; 16]>::try_from(&row[left..left + 16]) else {
                    return false;
                };
                let right_start = right - 16;
                let Ok(right_block) = <[u8; 16]>::try_from(&row[right_start..right]) else {
                    return false;
                };
                let Some(left_reversed) = reverse_pixel_block(&right_block, channels) else {
                    return false;
                };
                let Some(right_reversed) = reverse_pixel_block(&left_block, channels) else {
                    return false;
                };
                row[left..left + 16].copy_from_slice(&left_reversed);
                row[right_start..right].copy_from_slice(&right_reversed);
                left += 16;
                right -= 16;
            }
            let remaining_pixels = (right - left) / channels;
            for x in 0..(remaining_pixels / 2) {
                let left_pixel = left + x * channels;
                let right_pixel = right - channels - x * channels;
                for channel in 0..channels {
                    row.swap(left_pixel + channel, right_pixel + channel);
                }
            }
            continue;
        }
        for x in 0..(width as usize / 2) {
            let left = x * channels;
            let right = row_len - channels - left;
            for channel in 0..channels {
                row.swap(left + channel, right + channel);
            }
        }
    }
    true
}

/// Fast native-layout mirror for ordinary 8-bit byte images.
fn native_mirror(img: &DynamicImage, mode: Option<&str>) -> Option<DynamicImage> {
    let mut result = img.clone();
    let mirrored = match &mut result {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            let (width, height) = image.dimensions();
            mirror_native_bytes(image, width, height, 1)
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            let (width, height) = image.dimensions();
            mirror_native_bytes(image, width, height, 2)
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB")) => {
            let (width, height) = image.dimensions();
            mirror_native_bytes(image, width, height, 3)
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            let (width, height) = image.dimensions();
            mirror_native_bytes(image, width, height, 4)
        }
        _ => return None,
    };
    mirrored.then_some(result)
}

/// Reorder a native byte image for one Pillow transpose method.
///
/// Geometry does not need an RGBA representation: each pixel is an opaque
/// byte group whose channels must move together. Keeping the operation in its
/// original layout avoids the pack/kernel/unpack round trip that dominated
/// the SIMD transpose benchmark for RGB and RGBA images.
fn native_transpose_bytes(
    bytes: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    method: TransposeMethod,
) -> Option<(Vec<u8>, u32, u32)> {
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let pixels = width.checked_mul(height)?;
    let total_bytes = pixels.checked_mul(channels)?;
    let source = bytes.get(..total_bytes)?;
    let (out_width, out_height) = match method {
        TransposeMethod::FlipLeftRight
        | TransposeMethod::FlipTopBottom
        | TransposeMethod::Rotate180 => (width, height),
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
    };
    let mut output = vec![0u8; total_bytes];

    match method {
        TransposeMethod::FlipLeftRight => {
            output.copy_from_slice(source);
            if !mirror_native_bytes(&mut output, width as u32, height as u32, channels) {
                return None;
            }
        }
        TransposeMethod::FlipTopBottom => {
            let row_bytes = width.checked_mul(channels)?;
            for y in 0..height {
                let source_start = (height - 1 - y).checked_mul(row_bytes)?;
                let output_start = y.checked_mul(row_bytes)?;
                output[output_start..output_start + row_bytes]
                    .copy_from_slice(&source[source_start..source_start + row_bytes]);
            }
        }
        TransposeMethod::Rotate180 => {
            for output_pixel in 0..pixels {
                let source_pixel = pixels - 1 - output_pixel;
                let output_start = output_pixel.checked_mul(channels)?;
                let source_start = source_pixel.checked_mul(channels)?;
                output[output_start..output_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
        method => {
            // Validate the row strides once. The previous coordinate-by-
            // coordinate checked arithmetic made the common RGB/RGBA rotate
            // path pay several overflow checks for every pixel, even though
            // the complete buffer size had already been checked above. A
            // transpose keeps one source column fixed for each output row, so
            // iterating source rows also removes the inner-loop multiplies.
            let source_row_bytes = width.checked_mul(channels)?;
            let output_row_bytes = out_width.checked_mul(channels)?;
            let write_output_rows_serial = |output: &mut [u8]| -> Option<()> {
                let source_rows = source.chunks_exact(source_row_bytes);
                let mut output_rows = output.chunks_exact_mut(output_row_bytes);
                for output_row in 0..out_height {
                    let (source_x, reverse_source_y) = match method {
                        TransposeMethod::Rotate90 => (width - 1 - output_row, false),
                        TransposeMethod::Rotate270 => (output_row, true),
                        TransposeMethod::Transpose => (output_row, false),
                        TransposeMethod::Transverse => (width - 1 - output_row, true),
                        _ => unreachable!("same-dimension transpose handled above"),
                    };
                    let source_x_offset = source_x.checked_mul(channels)?;
                    let output_row_slice = output_rows.next()?;
                    if reverse_source_y {
                        for (output_pixel, source_row) in output_row_slice
                            .chunks_exact_mut(channels)
                            .zip(source_rows.clone().rev())
                        {
                            let source_pixel =
                                source_row.get(source_x_offset..source_x_offset + channels)?;
                            output_pixel.copy_from_slice(source_pixel);
                        }
                    } else {
                        for (output_pixel, source_row) in output_row_slice
                            .chunks_exact_mut(channels)
                            .zip(source_rows.clone())
                        {
                            let source_pixel =
                                source_row.get(source_x_offset..source_x_offset + channels)?;
                            output_pixel.copy_from_slice(source_pixel);
                        }
                    }
                }
                Some(())
            };
            #[cfg(feature = "parallel")]
            if pixels >= 256 * 1024 {
                crate::par_rows_mut!(
                    &mut output,
                    output_row_bytes,
                    out_height,
                    |_row_start, _row_end, output_row, output_row_slice| {
                        let (source_x, reverse_source_y) = match method {
                            TransposeMethod::Rotate90 => (width - 1 - output_row as usize, false),
                            TransposeMethod::Rotate270 => (output_row as usize, true),
                            TransposeMethod::Transpose => (output_row as usize, false),
                            TransposeMethod::Transverse => (width - 1 - output_row as usize, true),
                            _ => unreachable!("same-dimension transpose handled above"),
                        };
                        let source_x_offset = source_x * channels;
                        if reverse_source_y {
                            for (output_pixel, source_row) in output_row_slice
                                .chunks_exact_mut(channels)
                                .zip(source.chunks_exact(source_row_bytes).rev())
                            {
                                let source_pixel =
                                    &source_row[source_x_offset..source_x_offset + channels];
                                output_pixel.copy_from_slice(source_pixel);
                            }
                        } else {
                            for (output_pixel, source_row) in output_row_slice
                                .chunks_exact_mut(channels)
                                .zip(source.chunks_exact(source_row_bytes))
                            {
                                let source_pixel =
                                    &source_row[source_x_offset..source_x_offset + channels];
                                output_pixel.copy_from_slice(source_pixel);
                            }
                        }
                    }
                );
            } else {
                write_output_rows_serial(&mut output)?;
            }
            #[cfg(not(feature = "parallel"))]
            write_output_rows_serial(&mut output)?;
        }
    }

    Some((
        output,
        u32::try_from(out_width).ok()?,
        u32::try_from(out_height).ok()?,
    ))
}

/// Apply transpose while retaining the native 8-bit DynamicImage variant.
fn native_transpose(
    img: &DynamicImage,
    mode: Option<&str>,
    method: TransposeMethod,
) -> Option<DynamicImage> {
    let channels = match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => 1,
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => 2,
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => 3,
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "RGBa" | "CMYK")) => 4,
        _ => return None,
    };
    let (bytes, width, height) =
        native_transpose_bytes(img.as_bytes(), img.width(), img.height(), channels, method)?;
    match img {
        DynamicImage::ImageLuma8(_) => Some(DynamicImage::ImageLuma8(GrayImage::from_raw(
            width, height, bytes,
        )?)),
        DynamicImage::ImageLumaA8(_) => Some(DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(
            width, height, bytes,
        )?)),
        DynamicImage::ImageRgb8(_) => Some(DynamicImage::ImageRgb8(RgbImage::from_raw(
            width, height, bytes,
        )?)),
        DynamicImage::ImageRgba8(_) => Some(DynamicImage::ImageRgba8(RgbaImage::from_raw(
            width, height, bytes,
        )?)),
        _ => None,
    }
}

// ── Helper: DynamicImage ↔ packed u32 ─────────────────────────────────

/// Extract packed u32 RGBA pixels from a DynamicImage.
fn pixels_from_dynimg(img: &DynamicImage) -> Vec<u32> {
    img.to_rgba8()
        .pixels()
        .map(|p| {
            (p[0] as u32) | ((p[1] as u32) << 8) | ((p[2] as u32) << 16) | ((p[3] as u32) << 24)
        })
        .collect()
}

/// Reconstruct a DynamicImage from packed u32 RGBA pixels.
fn dynimg_from_rgba(pixels: Vec<u32>, w: u32, h: u32) -> Result<DynamicImage, PilError> {
    let rgba_bytes: Vec<u8> = pixels
        .iter()
        .flat_map(|&p| {
            vec![
                (p & 0xFF) as u8,
                ((p >> 8) & 0xFF) as u8,
                ((p >> 16) & 0xFF) as u8,
                ((p >> 24) & 0xFF) as u8,
            ]
        })
        .collect();
    RgbaImage::from_raw(w, h, rgba_bytes)
        .map(DynamicImage::ImageRgba8)
        .ok_or_else(|| PilError::InternalError("SIMD RGBA buffer shape mismatch".to_string()))
}

/// Compute the rounded `ImageStat.mean` midpoint used by Pillow Contrast.
fn rounded_pil_mean(img: &DynamicImage) -> Result<f64, PilError> {
    let gray = crate::color::pil_grayscale(img)?;
    let mut sum = 0u64;
    let mut count = 0u64;
    for pixel in gray.pixels() {
        sum += pixel[0] as u64;
        count += 1;
    }
    if count == 0 {
        Ok(0.0)
    } else {
        Ok(((sum as f64 / count as f64) + 0.5) as u8 as f64)
    }
}

/// Reconstruct the logical sample layout used by a mode-preserving mutator.
fn dynimg_from_pixel_mode(
    pixels: Vec<u32>,
    w: u32,
    h: u32,
    mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    match mode {
        PixelMode::L | PixelMode::P | PixelMode::Mode1 => {
            let bytes = pixels.iter().map(|pixel| (*pixel & 0xFF) as u8).collect();
            GrayImage::from_raw(w, h, bytes)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::InternalError("SIMD L buffer shape mismatch".to_string()))
        }
        PixelMode::LA | PixelMode::PA => {
            let bytes = pixels
                .iter()
                .flat_map(|pixel| [(*pixel & 0xFF) as u8, ((*pixel >> 24) & 0xFF) as u8])
                .collect();
            GrayAlphaImage::from_raw(w, h, bytes)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| PilError::InternalError("SIMD LA buffer shape mismatch".to_string()))
        }
        PixelMode::RGB | PixelMode::YCbCr | PixelMode::HSV => {
            let bytes = pixels
                .iter()
                .flat_map(|pixel| {
                    [
                        (*pixel & 0xFF) as u8,
                        ((*pixel >> 8) & 0xFF) as u8,
                        ((*pixel >> 16) & 0xFF) as u8,
                    ]
                })
                .collect();
            RgbImage::from_raw(w, h, bytes)
                .map(DynamicImage::ImageRgb8)
                .ok_or_else(|| {
                    PilError::InternalError("SIMD RGB buffer shape mismatch".to_string())
                })
        }
        PixelMode::RGBA | PixelMode::CMYK | PixelMode::I | PixelMode::F => {
            dynimg_from_rgba(pixels, w, h)
        }
    }
}

/// Reconstruct the promoted result of `Image.putalpha`.
fn dynimg_from_put_alpha(
    pixels: Vec<u32>,
    w: u32,
    h: u32,
    mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    match mode {
        PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => {
            dynimg_from_pixel_mode(pixels, w, h, PixelMode::LA)
        }
        _ => dynimg_from_rgba(pixels, w, h),
    }
}

/// Materialize an Arc<Image> → DynamicImage.
fn arc_to_dynimg(arc: &Arc<Image>) -> Result<DynamicImage, PilError> {
    arc.materialize_for_ops()
}

/// Extract packed u32 pixels from an Arc<Image>.
fn pixels_from_arc(arc: &Arc<Image>) -> Result<Vec<u32>, PilError> {
    let img = arc_to_dynimg(arc)?;
    Ok(pixels_from_dynimg(&img))
}

/// Extract an operand in the sample domain required by ImageChops.
///
/// The ordinary materialization helper expands palette images for color
/// operations. Pillow's Chops C kernels instead combine raw P/PA samples, so
/// indexed Chops operands must stay in their native one- or two-byte layout.
fn pixels_from_arc_for_chops(arc: &Arc<Image>, mode: Option<&str>) -> Result<Vec<u32>, PilError> {
    let img = if matches!(mode, Some("P" | "PA")) {
        arc.materialize_indices()?
    } else {
        arc.materialize_for_ops()?
    };
    Ok(pixels_from_dynimg(&img))
}

fn materialize_chops_operand(
    arc: &Arc<Image>,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if matches!(mode, Some("P" | "PA")) {
        arc.materialize_indices()
    } else {
        arc.materialize_for_ops()
    }
}

fn simd_native_chops_or_packed(
    img: &DynamicImage,
    other: &Arc<Image>,
    mode: Option<&str>,
    native: fn(&DynamicImage, &DynamicImage, Option<&str>) -> Option<DynamicImage>,
    scalar: fn(&mut [u32], u32, &[u32]),
) -> Result<DynamicImage, PilError> {
    let other_img = materialize_chops_operand(other, mode)?;
    if let Some(result) = native(img, &other_img, mode) {
        return Ok(result);
    }

    let (w, h) = img.dimensions();
    // Sharpness is the one packed path that treats CMYK's fourth byte as a
    // real sample. Other SIMD adapters operate on the RGBA promotion of CMYK
    // and must retain the ordinary four-channel code (3).
    let mode_code = if mode == Some("CMYK") {
        4
    } else {
        mode_to_u32(img, mode)
    };
    let mut pixels = pixels_from_dynimg(img);
    let other_pixels = pixels_from_dynimg(&other_img);
    scalar(&mut pixels, mode_code, &other_pixels);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section A: Simple single-image ops (no extra params beyond mode)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_invert(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(result) = native_invert(img, mode, false) {
        return Ok(result);
    }
    let mode_code = mode_to_u32(img, mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::invert(&mut pixels, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_grayscale(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode_code = mode_to_u32(img, mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::grayscale(&mut pixels, mode_code);
    let luma = pixels.into_iter().map(|pixel| pixel as u8).collect();
    GrayImage::from_raw(w, h, luma)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD grayscale buffer shape mismatch".to_string()))
}

pub fn simd_duplicate(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let mode_code = mode_to_u32(img, mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::duplicate(&mut pixels, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_invert_chops(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(result) = native_invert(img, mode, true) {
        return Ok(result);
    }
    let mode_code = mode_to_u32(img, mode);
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::invert_chops(&mut pixels, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section B: Single-image with extra params (solarize, posterize, ...)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_solarize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Solarize { threshold } = op {
        if let Some(result) = native_byte_transform(img, mode, |input| {
            input
                .simd_ge(u8x16::splat(*threshold))
                .select(u8x16::splat(u8::MAX) - input, input)
        }) {
            return Ok(result);
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Solarize { threshold } = op {
        super::scalar::solarize(&mut pixels, mode_code, *threshold);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_posterize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Posterize { bits } = op {
        if *bits <= 8 {
            let shift = 8 - *bits as u32;
            if let Some(result) =
                native_byte_transform(img, mode, |input| (input >> shift) << shift)
            {
                return Ok(result);
            }
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Posterize { bits } = op {
        super::scalar::posterize(&mut pixels, mode_code, *bits as u32);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_brightness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Brightness { factor } = op {
        // The packed scalar adapter intentionally quantizes the public factor
        // to the same fixed-point domain as the SIMD operation.  Build that
        // exact 256-entry map once and apply it in native L/LA/RGB/RGBA byte
        // storage; this avoids the full RGBA pack/unpack round trip for the
        // common image modes.  CMYK remains on the packed path because its K
        // lane is an active channel rather than alpha.
        let factor_fp = (*factor * 1000.0) as u32;
        let lut: Vec<u8> = (0u32..=255)
            .map(|value| ((value as u64 * factor_fp as u64) / 1000).min(255) as u8)
            .collect();
        if let Some(tables) = native_lut_tables(&lut) {
            if let Some(result) =
                native_byte_transform(img, mode, |input| native_lut_chunk(input, &tables))
            {
                return Ok(result);
            }
        }
    }
    let (w, h) = img.dimensions();
    // CMYK stores K in the packed alpha lane, but it is an active sample for
    // Brightness rather than an alpha channel.
    let mode_code = if mode == Some("CMYK") {
        4
    } else {
        mode_to_u32(img, mode)
    };
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Brightness { factor } = op {
        super::scalar::brightness(&mut pixels, mode_code, (factor * 1000.0) as u32);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_contrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if mode == Some("CMYK") {
        if let PipelineOp::Contrast { factor } = op {
            // The packed SIMD contrast primitive has no CMYK→L midpoint and
            // cannot represent Pillow's CMYK degenerate K channel.
            return crate::compute::pool_cpu::ops::enhance::op_enhance_contrast(img, *factor, mode);
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Contrast { factor } = op {
        let mean = rounded_pil_mean(img)?;
        super::scalar::contrast(&mut pixels, mode_code, *factor, mean);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_color_saturation(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if mode == Some("CMYK") {
        if let PipelineOp::ColorSaturation { factor } = op {
            // CMYK Color requires Pillow's CMYK→L→CMYK degenerate conversion,
            // which is outside the RGB/LA packed SIMD saturation domain.
            return crate::compute::pool_cpu::ops::enhance::op_enhance_color_saturation(
                img, *factor, mode,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::ColorSaturation { factor } = op {
        super::scalar::color_saturation(&mut pixels, mode_code, (factor * 1000.0) as u32);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_sharpness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    // Sharpness processes CMYK's promoted K byte as a fourth sample. Other
    // packed adapters intentionally keep CMYK in the ordinary RGBA code.
    let mode_code = if mode == Some("CMYK") {
        4
    } else {
        mode_to_u32(img, mode)
    };
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Sharpness { factor } = op {
        super::scalar::sharpness(&mut pixels, w, h, mode_code, (factor * 1000.0) as u32);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_colorize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Colorize {
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    } = op
    {
        let lut = crate::compute::pool_cpu::ops::imageops::colorize_lut(
            black,
            white,
            *mid,
            *blackpoint,
            *midpoint,
            *whitepoint,
        );
        super::scalar::colorize(&mut pixels, mode_code, &lut);
    }
    // Pillow's ImageOps.colorize always promotes its L input to RGB. Keeping
    // the packed SIMD result as RGBA leaks the implementation storage type
    // into the public result and breaks exact mode/byte parity.
    dynimg_from_pixel_mode(pixels, w, h, PixelMode::RGB)
}

pub fn simd_constant(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let area = u64::from(w) * u64::from(h);
    if area >= 32 * 32 {
        if let PipelineOp::Constant { value } = op {
            // ImageChops.constant ignores the source samples and always returns a
            // one-band L image. Constructing that native result directly avoids
            // an unnecessary RGBA expansion and a packed scalar traversal.
            return Ok(DynamicImage::ImageLuma8(GrayImage::from_pixel(
                w,
                h,
                Luma([*value]),
            )));
        }
    }
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Constant { value } = op {
        let packed =
            (*value as u32) | ((*value as u32) << 8) | ((*value as u32) << 16) | 0xFF00_0000;
        super::scalar::constant(&mut pixels, mode_code, packed);
    }
    // ImageChops.constant always allocates a one-band L image; it does not
    // preserve the source mode.
    dynimg_from_pixel_mode(pixels, w, h, PixelMode::L)
}

pub fn simd_offset(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Offset { x, y } = op {
        super::scalar::offset(&mut pixels, w, h, mode_code, *x, *y);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section C: Spatial single-image (flip, mirror, equalize, autocontrast)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_flip(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // ImageOps.flip is the vertical half of transpose. Reuse the native byte
    // mover so ordinary L/LA/RGB/RGBA images do not pay the packed-RGBA
    // conversion and reconstruction cost used by the scalar fallback.
    if let Some(result) = native_transpose(img, mode, TransposeMethod::FlipTopBottom) {
        return Ok(result);
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::flip(&mut pixels, w, h, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_mirror(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(result) = native_mirror(img, mode) {
        return Ok(result);
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::mirror(&mut pixels, w, h, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_equalize(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // RGB equalization is a histogram pass followed by a byte LUT.  The
    // shared CPU implementation already performs both passes in native RGB
    // storage (and parallelizes independent rows); sending the same image
    // through packed RGBA lanes here only adds two full-frame conversions.
    // Restrict this delegation to RGB, whose channel contract is identical;
    // LA/RGBA and palette/typed modes retain the established SIMD adapter.
    if matches!(img, DynamicImage::ImageRgb8(_)) && matches!(mode, None | Some("RGB")) {
        return crate::compute::pool_cpu::ops::imageops::op_equalize(img);
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    super::scalar::equalize(&mut pixels, w, h, mode_code);
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_autocontrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // As with Equalize, keep ordinary RGB in its native byte layout.  The
    // CPU helper is the exact histogram/LUT implementation used by the
    // public operation; only the packed representation is avoided here.
    if matches!(img, DynamicImage::ImageRgb8(_)) && matches!(mode, None | Some("RGB")) {
        if let PipelineOp::Autocontrast { cutoff, mask } = op {
            return crate::compute::pool_cpu::ops::imageops::op_autocontrast(
                img,
                *cutoff as f64,
                mask.as_ref(),
            );
        }
        return Err(PilError::ValueError("expected Autocontrast op".into()));
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Autocontrast { cutoff, mask } = op {
        if mask.is_some() {
            return crate::compute::pool_cpu::ops::imageops::op_autocontrast(
                img,
                *cutoff as f64,
                mask.as_ref(),
            );
        }
        super::scalar::autocontrast(&mut pixels, w, h, mode_code, *cutoff as u32);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section D: Filter/window ops (median, max, min, rank, conv, blur)
// ═══════════════════════════════════════════════════════════════════════

/// Avoid the packed RGBA adapter for large ordinary byte images.
///
/// The exact CPU rank implementation already works over the source's native
/// channel layout and uses row-disjoint output plus a sliding histogram for
/// large byte windows.  Packing those same images into `u32` lanes before a
/// scalar window scan makes the SIMD-labelled route slower without providing
/// architecture-specific vector work.  Keep small images and explicit/typed
/// modes on the existing packed path until a native SIMD window kernel exists.
fn use_native_byte_rank_path(img: &DynamicImage, mode: Option<&str>) -> bool {
    if mode.is_some() {
        return false;
    }
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    pixel_count > 64 * 64
        && matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        )
}

/// The packed convolution adapter is scalar and widens every native byte
/// image to RGBA before evaluating its window. For large ordinary byte
/// layouts, use the native row kernels below so SIMD does not pay a conversion
/// boundary. The 3x3 RGBA lane is still routed through the exact CPU kernel at
/// its current crossover because the four-channel vector setup is slower on
/// the maintained arm64 release benchmark.
fn use_native_byte_convolution_path(img: &DynamicImage, mode: Option<&str>) -> bool {
    if mode.is_some() {
        return false;
    }
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    pixel_count > 64 * 64
        && matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        )
}

/// Evaluate eight output samples of Pillow's 3x3 byte convolution in parallel.
///
/// The scalar CPU implementation starts each three-tap row with the middle
/// product and then uses fused multiply-adds for the left and right products.
/// Keep that order here so vector hardware does not change the observable
/// truncation boundary.  The final lane is clamped to the last interior pixel
/// when a row does not divide evenly by eight; callers only store valid lanes.
#[inline]
fn native_filter_3x3_vector(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut left = [0.0f32; 8];
        let mut middle = [0.0f32; 8];
        let mut right = [0.0f32; 8];
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 2);
            let row = (y as isize + dy) as usize * width;
            let left_index = (row + x - 1) * channels;
            let middle_index = (row + x) * channels;
            let right_index = (row + x + 1) * channels;
            left[lane] = raw[left_index + channel] as f32;
            middle[lane] = raw[middle_index + channel] as f32;
            right[lane] = raw[right_index + channel] as f32;
        }
        let sum = f32x8::from(middle) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(left).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        f32x8::from(right).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(1, 0);
    total += row(0, 3);
    total += row(-1, 6);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Apply the exact native-byte 3x3 convolution with eight-wide vector lanes.
/// Borders retain the source bytes, matching the CPU implementation.
fn native_filter_3x3_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) {
    if !(1..=4).contains(&channels) || width < 3 || height < 3 {
        return;
    }
    let row_stride = width * channels;
    let apply_row = |y: usize, row: &mut [u8]| {
        for channel in 0..channels {
            let mut x = 1usize;
            while x + 7 < width - 1 {
                let values = native_filter_3x3_vector(
                    raw,
                    width,
                    channels,
                    channel,
                    y,
                    x,
                    kernel,
                    rounding_bias,
                );
                for (lane, value) in values.into_iter().enumerate() {
                    row[(x + lane) * channels + channel] = value;
                }
                x += 8;
            }
            while x < width - 1 {
                let value = native_filter_3x3_vector(
                    raw,
                    width,
                    channels,
                    channel,
                    y,
                    x,
                    kernel,
                    rounding_bias,
                )[0];
                row[x * channels + channel] = value;
                x += 1;
            }
        }
    };

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(out, row_stride, height, |_row_start, _row_end, y, row| {
        let y = y as usize;
        if (1..height - 1).contains(&y) {
            apply_row(y, row);
        }
    });
    #[cfg(not(feature = "parallel"))]
    for y in 1..height - 1 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }
}

/// Evaluate eight output samples of Pillow's 5x5 byte convolution in parallel.
/// The five row sums use the same middle-first, left-to-right FMA order as the
/// exact CPU implementation.
#[inline]
fn native_filter_5x5_vector(
    raw: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [u8; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut samples = [[0.0f32; 8]; 5];
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 3);
            let row = (y as isize + dy) as usize * width;
            for tap in 0..5 {
                samples[tap][lane] = raw[(row + x + tap - 2) * channels + channel] as f32;
            }
        }
        let sum = f32x8::from(samples[1]) * f32x8::splat(kernel[kernel_start + 1]);
        let sum = f32x8::from(samples[0]).mul_add(f32x8::splat(kernel[kernel_start]), sum);
        let sum = f32x8::from(samples[2]).mul_add(f32x8::splat(kernel[kernel_start + 2]), sum);
        let sum = f32x8::from(samples[3]).mul_add(f32x8::splat(kernel[kernel_start + 3]), sum);
        f32x8::from(samples[4]).mul_add(f32x8::splat(kernel[kernel_start + 4]), sum)
    };

    let mut total = f32x8::splat(rounding_bias);
    total += row(2, 0);
    total += row(1, 5);
    total += row(0, 10);
    total += row(-1, 15);
    total += row(-2, 20);
    let values = total.to_array();
    std::array::from_fn(|lane| {
        let value = values[lane];
        if value <= 0.0 {
            0
        } else if value >= 255.0 {
            255
        } else {
            value as u8
        }
    })
}

/// Apply the exact native-byte 5x5 convolution with eight-wide vector lanes.
fn native_filter_5x5_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    if !(1..=4).contains(&channels) || width < 5 || height < 5 {
        return;
    }
    let row_stride = width * channels;
    let apply_row = |y: usize, row: &mut [u8]| {
        for channel in 0..channels {
            let mut x = 2usize;
            while x + 7 < width - 2 {
                let values = native_filter_5x5_vector(
                    raw,
                    width,
                    channels,
                    channel,
                    y,
                    x,
                    kernel,
                    rounding_bias,
                );
                for (lane, value) in values.into_iter().enumerate() {
                    row[(x + lane) * channels + channel] = value;
                }
                x += 8;
            }
            while x < width - 2 {
                let value = native_filter_5x5_vector(
                    raw,
                    width,
                    channels,
                    channel,
                    y,
                    x,
                    kernel,
                    rounding_bias,
                )[0];
                row[x * channels + channel] = value;
                x += 1;
            }
        }
    };

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(out, row_stride, height, |_row_start, _row_end, y, row| {
        let y = y as usize;
        if (2..height - 2).contains(&y) {
            apply_row(y, row);
        }
    });
    #[cfg(not(feature = "parallel"))]
    for y in 2..height - 2 {
        let row_start = y * row_stride;
        apply_row(y, &mut out[row_start..row_start + row_stride]);
    }
}

pub fn simd_median_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // F-mode is already represented as native scalar samples at this
    // boundary.  Packing it through the byte-oriented SIMD adapter would
    // convert the sample domain before the window operation; use the exact
    // shared implementation instead.
    if mode == Some("F") {
        if let PipelineOp::MedianFilter { size } = op {
            return crate::compute::pool_cpu::ops::filter::execute_median_filter_with_mode(
                img, *size, mode,
            );
        }
    }
    if use_native_byte_rank_path(img, mode) {
        if let PipelineOp::MedianFilter { size } = op {
            return crate::compute::pool_cpu::ops::filter::execute_median_filter_with_mode(
                img, *size, mode,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::MedianFilter { size } = op {
        super::scalar::median_filter(&mut pixels, w, h, mode_code, *size);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_max_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if mode == Some("F") {
        if let PipelineOp::MaxFilter { size } = op {
            return crate::compute::pool_cpu::ops::filter::execute_max_filter_with_mode(
                img, *size, mode,
            );
        }
    }
    if use_native_byte_rank_path(img, mode) {
        if let PipelineOp::MaxFilter { size } = op {
            return crate::compute::pool_cpu::ops::filter::execute_max_filter_with_mode(
                img, *size, mode,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::MaxFilter { size } = op {
        super::scalar::max_filter(&mut pixels, w, h, mode_code, *size);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_min_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if mode == Some("F") {
        if let PipelineOp::MinFilter { size } = op {
            return crate::compute::pool_cpu::ops::filter::execute_min_filter_with_mode(
                img, *size, mode,
            );
        }
    }
    if use_native_byte_rank_path(img, mode) {
        if let PipelineOp::MinFilter { size } = op {
            return crate::compute::pool_cpu::ops::filter::execute_min_filter_with_mode(
                img, *size, mode,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::MinFilter { size } = op {
        super::scalar::min_filter(&mut pixels, w, h, mode_code, *size);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_rank_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if mode == Some("F") {
        if let PipelineOp::RankFilter { size, rank } = op {
            return crate::compute::pool_cpu::ops::filter::execute_rank_filter_with_mode(
                img, *size, *rank, mode,
            );
        }
    }
    if use_native_byte_rank_path(img, mode) {
        if let PipelineOp::RankFilter { size, rank } = op {
            return crate::compute::pool_cpu::ops::filter::execute_rank_filter_with_mode(
                img, *size, *rank, mode,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::RankFilter { size, rank } = op {
        super::scalar::rank_filter(&mut pixels, w, h, mode_code, *size, *rank);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_filter_3x3(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // The packed SIMD representation is not a native I-mode layout: widening
    // signed 32-bit pixels into RGBA words is both slower and needlessly
    // allocates for this exact scalar contract.  Keep the requested SIMD lane
    // honest by using the optimized native I-mode implementation until a
    // signed-lane convolution kernel exists.
    if mode == Some("I") {
        if let PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } = op
        {
            return crate::compute::pool_cpu::ops::filter::execute_filter3x3(
                img, kernel, *scale, *offset, mode,
            );
        }
    }
    if mode.is_none()
        && matches!(img, DynamicImage::ImageRgba8(_))
        && (img.width() as usize).saturating_mul(img.height() as usize) > 64 * 64
    {
        if let PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } = op
        {
            // The native vector path is retained for L/LA/RGB, where the
            // current release receipt shows a crossover. RGBA 3x3 remains
            // exact CPU fallback until its four-channel lane earns a win.
            crate::compute::record_pipeline_backend_fallback(
                "SIMD 3x3 RGBA: exact CPU crossover fallback",
            );
            return crate::compute::pool_cpu::ops::filter::execute_filter3x3(
                img, kernel, *scale, *offset, mode,
            );
        }
    }
    #[cfg(target_arch = "aarch64")]
    if mode.is_none()
        && matches!(img, DynamicImage::ImageLumaA8(_))
        && (img.width() as usize).saturating_mul(img.height() as usize) >= 512 * 512
    {
        if let PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } = op
        {
            // The arm64 release crossover matrix is SIMD-positive at 256²,
            // but the native LA row lane is marginally slower at 512² and
            // slower again at 1024×768. Keep the exact CPU implementation for
            // the larger layouts and leave the small-image SIMD path intact.
            crate::compute::record_pipeline_backend_fallback(
                "SIMD 3x3 LA: exact CPU crossover fallback",
            );
            return crate::compute::pool_cpu::ops::filter::execute_filter3x3(
                img, kernel, *scale, *offset, mode,
            );
        }
    }
    if use_native_byte_convolution_path(img, mode) {
        if let PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } = op
        {
            let channels = img.color().channel_count() as usize;
            let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
            let mut output = img.as_bytes().to_vec();
            native_filter_3x3_rows(
                img.as_bytes(),
                &mut output,
                img.width() as usize,
                img.height() as usize,
                channels,
                &normalized_kernel,
                *offset as f32 + 0.5,
            );
            return crate::image_utils::raw_bytes_to_image(
                img.width(),
                img.height(),
                output,
                channels,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Filter3x3 {
        kernel,
        scale,
        offset,
    } = op
    {
        super::scalar::filter_3x3(&mut pixels, w, h, mode_code, kernel, *scale, *offset);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_filter_5x5(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // See the 3x3 I-mode path above.  A packed-byte emulation would make the
    // SIMD request slower than the exact native implementation and would not
    // constitute signed-lane SIMD acceleration.
    if mode == Some("I") {
        if let PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } = op
        {
            return crate::compute::pool_cpu::ops::filter::execute_filter5x5(
                img, kernel, *scale, *offset, mode,
            );
        }
    }
    if use_native_byte_convolution_path(img, mode) {
        if let PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } = op
        {
            let channels = img.color().channel_count() as usize;
            let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
            let mut output = img.as_bytes().to_vec();
            native_filter_5x5_rows(
                img.as_bytes(),
                &mut output,
                img.width() as usize,
                img.height() as usize,
                channels,
                &normalized_kernel,
                *offset as f32 + 0.5,
            );
            return crate::image_utils::raw_bytes_to_image(
                img.width(),
                img.height(),
                output,
                channels,
            );
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Filter5x5 {
        kernel,
        scale,
        offset,
    } = op
    {
        super::scalar::filter_5x5(&mut pixels, w, h, mode_code, kernel, *scale, *offset);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

const SIMD_BOX_BLUR_SCALE: u32 = 1 << 24;
const SIMD_BOX_BLUR_BIAS: u32 = 1 << 23;

#[derive(Clone, Copy)]
enum SimdBlurRegion {
    Leading,
    Middle,
    Trailing,
    Clamped,
}

#[inline(always)]
fn simd_blur_block(
    source: &[u8],
    destination: &mut [u8],
    accumulator: &mut [u32; 4],
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
    last: usize,
    output_start: usize,
    output_count: usize,
    region: SimdBlurRegion,
) {
    debug_assert!(output_count <= 8);
    debug_assert!(element_width <= accumulator.len());

    // The rolling recurrence remains scalar because each output accumulator
    // depends on the preceding output.  Its fixed-point multiply, fractional
    // edge term, bias, and pack are independent across eight output pixels,
    // so keep that exact recurrence and vectorize the arithmetic tail. This
    // avoids the radius-sized per-pixel loop without changing Pillow's u32
    // wrapping behavior or its 24-bit rounding boundary.
    let mut accumulated = [[0u32; 8]; 4];
    let mut fractional = [[0u32; 8]; 4];
    for lane in 0..output_count {
        let output = output_start + lane;
        let (subtract, add, far_left, far_right) = match region {
            SimdBlurRegion::Leading => (0, output + radius, 0, output + radius + 1),
            SimdBlurRegion::Middle => (
                output - radius - 1,
                output + radius,
                output - radius - 1,
                output + radius + 1,
            ),
            SimdBlurRegion::Trailing => (output - radius - 1, last, output - radius - 1, last),
            SimdBlurRegion::Clamped => (0, last, 0, last),
        };
        let subtract_base = subtract * element_width;
        let add_base = add * element_width;
        let far_left_base = far_left * element_width;
        let far_right_base = far_right * element_width;
        for component in 0..element_width {
            accumulator[component] = accumulator[component]
                .wrapping_sub(source[subtract_base + component] as u32)
                .wrapping_add(source[add_base + component] as u32);
            accumulated[component][lane] = accumulator[component];
            fractional[component][lane] = (source[far_left_base + component] as u32
                + source[far_right_base + component] as u32)
                .wrapping_mul(fractional_weight);
        }
    }

    for component in 0..element_width {
        let bulk = u32x8::new(accumulated[component]) * u32x8::splat(whole_weight)
            + u32x8::new(fractional[component]);
        let values = (bulk + u32x8::splat(SIMD_BOX_BLUR_BIAS) >> 24u32).to_array();
        for lane in 0..output_count {
            destination[(output_start + lane) * element_width + component] = values[lane] as u8;
        }
    }
}

/// Blur one contiguous native-byte line with Pillow's fixed-point recurrence.
///
/// The edge-region split mirrors `src/libImaging/BoxBlur.c::ImagingLineBoxBlur`
/// exactly. SIMD only changes the independent fixed-point output arithmetic;
/// sample entry/removal and all border indices remain in the scalar order used
/// by the reference CPU implementation.
fn simd_blur_line(
    source: &[u8],
    destination: &mut [u8],
    line_length: usize,
    element_width: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    debug_assert!(line_length > 0);
    debug_assert!((1..=4).contains(&element_width));
    debug_assert_eq!(source.len(), line_length * element_width);
    debug_assert_eq!(destination.len(), source.len());

    let last = line_length - 1;
    let edge_a = (radius + 1).min(line_length);
    let edge_b = line_length.saturating_sub(radius + 1);
    let mut accumulator = [0u32; 4];

    for component in 0..element_width {
        accumulator[component] = (source[component] as u32).wrapping_mul((radius + 1) as u32);
    }
    for position in 0..edge_a.saturating_sub(1) {
        let base = position * element_width;
        for component in 0..element_width {
            accumulator[component] =
                accumulator[component].wrapping_add(source[base + component] as u32);
        }
    }
    let last_count = radius.saturating_add(1).saturating_sub(edge_a);
    let last_base = last * element_width;
    for component in 0..element_width {
        accumulator[component] = accumulator[component]
            .wrapping_add((source[last_base + component] as u32).wrapping_mul(last_count as u32));
    }

    let mut apply_region = |start: usize, end: usize, region: SimdBlurRegion| {
        let mut output = start;
        while output < end {
            let output_count = (end - output).min(8);
            simd_blur_block(
                source,
                destination,
                &mut accumulator,
                element_width,
                radius,
                whole_weight,
                fractional_weight,
                last,
                output,
                output_count,
                region,
            );
            output += output_count;
        }
    };

    if edge_a <= edge_b {
        apply_region(0, edge_a, SimdBlurRegion::Leading);
        apply_region(edge_a, edge_b, SimdBlurRegion::Middle);
        apply_region(edge_b, line_length, SimdBlurRegion::Trailing);
    } else {
        // When the radius overlaps both edges, the center region is clamped
        // to the two endpoints. Its input indices remain valid even when the
        // radius is larger than the line itself.
        apply_region(0, edge_b, SimdBlurRegion::Leading);
        apply_region(edge_b, edge_a, SimdBlurRegion::Clamped);
        apply_region(edge_a, line_length, SimdBlurRegion::Trailing);
    }
}

#[inline]
fn simd_blur_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    simd_blur_line(
        source,
        destination,
        width,
        channels,
        radius,
        whole_weight,
        fractional_weight,
    );
}

fn simd_blur_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    radius: usize,
    whole_weight: u32,
    fractional_weight: u32,
) {
    let dimensions = CheckedDims::new(width as u32, height as u32, channels as u8)
        .expect("native SIMD blur dimensions were validated at the adapter boundary");
    let row_stride = dimensions.row_stride();
    debug_assert_eq!(source.len(), dimensions.total_bytes());
    debug_assert_eq!(destination.len(), dimensions.total_bytes());

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        destination,
        row_stride,
        height,
        |row_start, row_end, _y, row| {
            simd_blur_row(
                &source[row_start..row_end],
                row,
                width,
                channels,
                radius,
                whole_weight,
                fractional_weight,
            );
        }
    );
    #[cfg(not(feature = "parallel"))]
    for row_index in 0..height {
        let row_start = row_index * row_stride;
        simd_blur_row(
            &source[row_start..row_start + row_stride],
            &mut destination[row_start..row_start + row_stride],
            width,
            channels,
            radius,
            whole_weight,
            fractional_weight,
        );
    }
}

fn simd_transpose_interleaved_rows(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
) {
    let source_dimensions = CheckedDims::new(width as u32, height as u32, channels as u8)
        .expect("native SIMD blur dimensions were validated at the adapter boundary");
    let destination_dimensions = CheckedDims::new(height as u32, width as u32, channels as u8)
        .expect("native SIMD blur dimensions were validated at the adapter boundary");
    let source_row_stride = source_dimensions.row_stride();
    let destination_row_stride = destination_dimensions.row_stride();
    debug_assert_eq!(source.len(), source_dimensions.total_bytes());
    debug_assert_eq!(destination.len(), destination_dimensions.total_bytes());

    #[cfg(feature = "parallel")]
    crate::par_rows_mut!(
        destination,
        destination_row_stride,
        width,
        |row_start, row_end, x, row| {
            let _ = (row_start, row_end);
            let x = x as usize;
            for y in 0..height {
                let source_start = y * source_row_stride + x * channels;
                let destination_start = y * channels;
                row[destination_start..destination_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            }
        }
    );
    #[cfg(not(feature = "parallel"))]
    for x in 0..width {
        let destination_start = x * destination_row_stride;
        for y in 0..height {
            let source_start = y * source_row_stride + x * channels;
            let output_start = destination_start + y * channels;
            destination[output_start..output_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        }
    }
}

fn simd_pil_box_blur(
    img: &DynamicImage,
    radius: f32,
    passes: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new(img.width(), img.height(), channels as u8)?;
    if img.as_bytes().len() != dimensions.total_bytes() || radius <= 0.0 || passes == 0 {
        return Ok(img.clone());
    }
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let integer_radius = radius as i32 as usize;
    let window_pixels = (2 * integer_radius + 1) as u32;
    let whole_weight = (SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
    let fractional_weight =
        SIMD_BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(whole_weight)) / 2;

    let mut work = img.as_bytes().to_vec();
    let mut scratch = dimensions.alloc_buffer();
    for _ in 0..passes {
        simd_blur_rows(
            &work,
            &mut scratch,
            width,
            height,
            channels,
            integer_radius,
            whole_weight,
            fractional_weight,
        );
        std::mem::swap(&mut work, &mut scratch);
    }

    // Pillow performs all horizontal passes before transposing for the
    // vertical passes. Keeping the transposed representation here gives the
    // SIMD row helper the same contiguous access pattern in both directions.
    simd_transpose_interleaved_rows(&work, &mut scratch, width, height, channels);
    for pass in 0..passes {
        simd_blur_rows(
            &scratch,
            &mut work,
            height,
            width,
            channels,
            integer_radius,
            whole_weight,
            fractional_weight,
        );
        // Keep the final pass result in `work` for the restoring transpose.
        // Swapping after that pass would make odd-pass workloads read the
        // pre-blur transposed buffer and silently drop the vertical blur.
        if pass + 1 < passes {
            std::mem::swap(&mut work, &mut scratch);
        }
    }
    simd_transpose_interleaved_rows(&work, &mut scratch, height, width, channels);

    let result = crate::image_utils::raw_bytes_to_image(
        dimensions.width,
        dimensions.height,
        scratch,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

fn simd_native_blur_channels(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    let channels = native_byte_layout(img, mode)?;
    (1..=4).contains(&channels).then_some(channels)
}

pub fn simd_box_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::BoxBlur { radius } = op {
        if let Some(channels) = simd_native_blur_channels(img, mode) {
            let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
            if pixel_count > 64 * 64 {
                return simd_pil_box_blur(img, *radius as f32, 1, channels);
            }
        }
        let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
        if pixel_count <= 64 * 64 {
            let mode_code = mode_to_u32(img, mode);
            let mut pixels = pixels_from_dynimg(img);
            super::scalar::box_blur(&mut pixels, img.width(), img.height(), mode_code, *radius);
            return Ok(preserve_mode(
                img,
                dynimg_from_rgba(pixels, img.width(), img.height())?,
            ));
        }
        // The packed fallback used a radius-sized nested loop for every
        // output sample. That made the SIMD adapter asymptotically worse than
        // the CPU backend's exact rolling-window implementation, especially
        // for the public 1024x768 radius-4 workload. Reuse the same
        // parity-locked fixed-point recurrence here until the native packed
        // SIMD rolling kernel exists; this keeps the SIMD route's mode/layout
        // handling exact and removes the RGBA pack/unpack boundary.
        return crate::compute::pool_cpu::ops::filter::execute_box_blur(img, *radius);
    }
    Err(PilError::ValueError("expected BoxBlur op".into()))
}

pub fn simd_gaussian_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::GaussianBlur { sigma } = op {
        if let Some(channels) = simd_native_blur_channels(img, mode) {
            let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
            if *sigma > 0.0 && pixel_count > 64 * 64 {
                let passes = 3.0f64;
                let sigma2 = *sigma as f64 * *sigma as f64 / passes;
                let l_val = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
                let l = l_val.floor();
                let l1 = l + 1.0;
                let a_num = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
                let a_den = 6.0 * (sigma2 - l1 * l1);
                let blur_radius = (l + a_num / a_den) as f32;
                return simd_pil_box_blur(img, blur_radius, 3, channels);
            }
        }
        // The shared CPU implementation retains Pillow's fractional box-blur
        // radius and 24-bit accumulator. The packed SIMD approximation rounds
        // that radius to an integer, which first diverges in UnsharpMask's
        // nonuniform threshold cases.
        return crate::compute::pool_cpu::ops::filter::execute_gaussian_blur(img, *sigma);
    }
    Err(PilError::ValueError("expected GaussianBlur op".into()))
}

// ═══════════════════════════════════════════════════════════════════════
// Section E: Dual-image per-pixel ops (Add, Subtract, Multiply, ...)
// ═══════════════════════════════════════════════════════════════════════

macro_rules! dual_op_adapter {
    ($name:ident, $variant:ident, $scalar_fn:path) => {
        pub fn $name(
            img: &DynamicImage,
            op: &PipelineOp,
            mode: Option<&str>,
        ) -> Result<DynamicImage, PilError> {
            let (w, h) = img.dimensions();
            let mode_code = mode_to_u32(img, mode);
            let mut pixels = pixels_from_dynimg(img);
            if let PipelineOp::$variant { other } = op {
                let other_pixels = pixels_from_arc_for_chops(other, mode)?;
                $scalar_fn(&mut pixels, mode_code, &other_pixels);
            }
            Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
        }
    };
}

macro_rules! native_dual_op_adapter {
    ($name:ident, $variant:ident, $native:path, $scalar:path) => {
        pub fn $name(
            img: &DynamicImage,
            op: &PipelineOp,
            mode: Option<&str>,
        ) -> Result<DynamicImage, PilError> {
            if let PipelineOp::$variant { other } = op {
                return simd_native_chops_or_packed(img, other, mode, $native, $scalar);
            }
            Ok(img.clone())
        }
    };
}

pub fn simd_multiply(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Multiply { other } = op {
        return simd_native_chops_or_packed(
            img,
            other,
            mode,
            |image, operand, current_mode| native_chops_blend(image, operand, current_mode, false),
            super::scalar::multiply,
        );
    }
    Ok(img.clone())
}

pub fn simd_screen(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Screen { other } = op {
        return simd_native_chops_or_packed(
            img,
            other,
            mode,
            |image, operand, current_mode| native_chops_blend(image, operand, current_mode, true),
            super::scalar::screen,
        );
    }
    Ok(img.clone())
}

native_dual_op_adapter!(
    simd_darker,
    Darker,
    native_chops_darker,
    super::scalar::darker
);
native_dual_op_adapter!(
    simd_lighter,
    Lighter,
    native_chops_lighter,
    super::scalar::lighter
);
native_dual_op_adapter!(
    simd_difference,
    Difference,
    native_chops_difference,
    super::scalar::difference
);
native_dual_op_adapter!(
    simd_add_modulo,
    AddModulo,
    native_chops_add_modulo,
    super::scalar::add_modulo
);
native_dual_op_adapter!(
    simd_subtract_modulo,
    SubtractModulo,
    native_chops_subtract_modulo,
    super::scalar::subtract_modulo
);
native_dual_op_adapter!(
    simd_logical_and,
    LogicalAnd,
    native_chops_logical_and,
    super::scalar::logical_and
);
native_dual_op_adapter!(
    simd_logical_or,
    LogicalOr,
    native_chops_logical_or,
    super::scalar::logical_or
);
native_dual_op_adapter!(
    simd_logical_xor,
    LogicalXor,
    native_chops_logical_xor,
    super::scalar::logical_xor
);
dual_op_adapter!(simd_overlay, Overlay, super::scalar::overlay);
dual_op_adapter!(simd_hard_light, HardLight, super::scalar::hard_light);
dual_op_adapter!(simd_soft_light, SoftLight, super::scalar::soft_light);

pub fn simd_add(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Add {
        other,
        scale,
        offset,
    } = op
    {
        let other_img = materialize_chops_operand(other, mode)?;
        // With Pillow's default parameters the exact scalar formula is
        // clamp(left + right, 0, 255).  Keep ordinary byte images in their
        // native interleaved layout so this path avoids the packed RGBA
        // conversion that otherwise dominates large ImageChops.add calls.
        if *scale == 1.0 && *offset == 0.0 {
            if let Some(result) = native_chops_add_clamped(img, &other_img, mode) {
                return Ok(result);
            }
        }
        let (w, h) = img.dimensions();
        let mode_code = mode_to_u32(img, mode);
        let mut pixels = pixels_from_dynimg(img);
        let other_pixels = pixels_from_dynimg(&other_img);
        super::scalar::add(
            &mut pixels,
            mode_code,
            &other_pixels,
            *scale as f32,
            *offset as f32,
        );
        return Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?));
    }
    Err(PilError::ValueError("expected Add op".into()))
}

pub fn simd_subtract(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Subtract {
        other,
        scale,
        offset,
    } = op
    {
        let other_img = materialize_chops_operand(other, mode)?;
        // For the default parameters Pillow's formula is the exact unsigned
        // saturating subtraction.  As with Add, use native bytes only when
        // the representation and public mode are an exact match.
        if *scale == 1.0 && *offset == 0.0 {
            if let Some(result) = native_chops_subtract_clamped(img, &other_img, mode) {
                return Ok(result);
            }
        }
        let (w, h) = img.dimensions();
        let mode_code = mode_to_u32(img, mode);
        let mut pixels = pixels_from_dynimg(img);
        let other_pixels = pixels_from_dynimg(&other_img);
        super::scalar::subtract(
            &mut pixels,
            mode_code,
            &other_pixels,
            *scale as f32,
            *offset as f32,
        );
        return Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?));
    }
    Err(PilError::ValueError("expected Subtract op".into()))
}

pub fn simd_blend_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::BlendModule { other, alpha } = op {
        let other_pixels = pixels_from_arc(other)?;
        super::scalar::blend_module(&mut pixels, mode_code, &other_pixels, *alpha);
    }
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_composite_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::CompositeModule {
        other,
        mask,
        mask_alpha,
    } = op
    {
        let other_img = if mode == Some("P") {
            other.materialize_indices()?
        } else {
            other.materialize_for_ops()?
        };
        let mask_img = mask.materialize_for_ops()?;
        let (other_w, other_h) = other_img.dimensions();
        let (mask_w, mask_h) = mask_img.dimensions();
        let other_pixels = pixels_from_dynimg(&other_img);
        let mask_pixels = pixels_from_dynimg(&mask_img);
        let result = super::scalar::composite_module(
            &pixels,
            w,
            h,
            mode_code,
            &other_pixels,
            other_w,
            other_h,
            &mask_pixels,
            mask_w,
            mask_h,
            *mask_alpha,
        );
        return Ok(preserve_mode(
            &other_img,
            dynimg_from_rgba(result, other_w, other_h)?,
        ));
    }
    Err(PilError::ValueError(
        "expected CompositeModule op".to_owned(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Section F: Ops that change dimensions (return new pixel buffer)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_transpose(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Transpose { method } = op {
        if let Some(result) = native_transpose(img, mode, method.clone()) {
            return Ok(result);
        }
    }
    if uses_native_scalar_mode(img, mode) {
        if let PipelineOp::Transpose { method } = op {
            return crate::compute::pool_cpu::ops::geometry::execute_transpose(img, method);
        }
        return Err(PilError::ValueError("expected Transpose op".into()));
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let method_code: u32 = match op {
        PipelineOp::Transpose { method } => match method {
            TransposeMethod::FlipLeftRight => 0,
            TransposeMethod::FlipTopBottom => 1,
            TransposeMethod::Rotate90 => 2,
            TransposeMethod::Rotate180 => 3,
            TransposeMethod::Rotate270 => 4,
            TransposeMethod::Transpose => 5,
            TransposeMethod::Transverse => 6,
        },
        _ => return Err(PilError::ValueError("expected Transpose op".into())),
    };
    // scalar::transpose modifies pixels in-place for ops 0,1,3 and returns new buffer
    // for ops 2,4,5,6. Pass the actual pixel buffer so in-place ops work correctly.
    let mut pixels = pixels_from_dynimg(img);
    let (result, nw, nh) = super::scalar::transpose(&mut pixels, w, h, mode_code, method_code);
    let final_pixels = if result.is_empty() { pixels } else { result };
    Ok(preserve_mode(img, dynimg_from_rgba(final_pixels, nw, nh)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section G: New-buffer ops with PipelineOp dispatch
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_resize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Resize {
        w: dst_w,
        h: dst_h,
        filter,
    } = op
    {
        if uses_native_scalar_mode(img, mode)
            || !simd_resize_filter_supported(filter)
            || matches!(filter, ResampleFilter::Bilinear)
            || matches!(mode, Some("P" | "PA"))
            || mode == Some("RGBa")
            || native_byte_layout(img, mode).is_some()
        {
            return crate::compute::pool_cpu::ops::geometry::execute_resize(
                img, *dst_w, *dst_h, filter, mode,
            );
        }
        let pixels = pixels_from_dynimg(img);
        let mode_code = dynimg_mode(img);
        let f = filter_to_u32(filter);
        let (result, new_w, new_h) =
            super::scalar::resize(&pixels, w, h, *dst_w, *dst_h, mode_code, f);
        // The packed RGBA buffer is an internal SIMD representation. Pillow's
        // resize family preserves the logical source mode at this boundary.
        Ok(preserve_mode(img, dynimg_from_rgba(result, new_w, new_h)?))
    } else {
        Err(PilError::ValueError("expected Resize op".into()))
    }
}

pub fn simd_thumbnail(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Thumbnail {
        w: dw,
        h: dh,
        filter,
    } = op
    {
        if uses_native_scalar_mode(img, mode) || !matches!(filter, ResampleFilter::Nearest) {
            return crate::compute::pool_cpu::ops::geometry::execute_thumbnail(
                img, *dw, *dh, filter, mode,
            );
        }
        let pixels = pixels_from_dynimg(img);
        let mode_code = dynimg_mode(img);
        let filter_code = if matches!(mode, Some("P" | "PA")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let (result, nw, nh) =
            super::scalar::thumbnail(&pixels, w, h, mode_code, *dw, *dh, filter_code);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Thumbnail op".into()))
    }
}

pub fn simd_contain(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Contain {
        w: dw,
        h: dh,
        filter,
    } = op
    {
        // The packed sizing kernel is still an approximate bilinear path;
        // native byte layouts have a coefficient-exact CPU implementation.
        // Keep the SIMD label from changing public pixels until the packed
        // ImageOps coefficient tables are ported exactly.
        if uses_native_scalar_mode(img, mode)
            || native_byte_layout(img, mode).is_some()
            || !simd_resize_filter_supported(filter)
        {
            return crate::compute::pool_cpu::ops::imageops::op_contain(
                img, *dw, *dh, *filter, mode,
            );
        }
        let mode_code = dynimg_mode(img);
        let filter_code = if matches!(mode, Some("P" | "PA")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let (result, nw, nh) =
            super::scalar::contain(&pixels, w, h, mode_code, *dw, *dh, filter_code);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Contain op".into()))
    }
}

pub fn simd_cover(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Cover {
        w: dw,
        h: dh,
        filter,
    } = op
    {
        // See `simd_contain`: the packed crop/resize approximation is not a
        // public parity implementation for ordinary native byte images.
        if uses_native_scalar_mode(img, mode)
            || native_byte_layout(img, mode).is_some()
            || !simd_resize_filter_supported(filter)
        {
            return crate::compute::pool_cpu::ops::imageops::op_cover(img, *dw, *dh, *filter, mode);
        }
        let mode_code = dynimg_mode(img);
        let filter_code = if matches!(mode, Some("P" | "PA")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let (result, nw, nh) =
            super::scalar::cover(&pixels, w, h, mode_code, *dw, *dh, filter_code);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Cover op".into()))
    }
}

pub fn simd_fit(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Fit {
        w: dw,
        h: dh,
        filter,
        bleed,
        centering,
        ..
    } = op
    {
        // RGBa stores premultiplied channels. Keep it on the exact native
        // implementation instead of treating its packed bytes as straight
        // RGBA samples in the SIMD fit kernel.
        if uses_native_scalar_mode(img, mode)
            || !simd_resize_filter_supported(filter)
            || mode == Some("RGBa")
            // The CPU boxed resize premultiplies straight alpha before
            // filtering and unpremultiplies afterward; the packed SIMD fit
            // kernel currently samples RGBA/LA channels independently. These
            // ordinary modes are represented directly by DynamicImage, so
            // their explicit mode tag is often None.
            || matches!(mode, Some("RGBA" | "LA"))
            || matches!(
                img,
                DynamicImage::ImageRgba8(_) | DynamicImage::ImageLumaA8(_)
            )
        {
            return crate::compute::pool_cpu::ops::imageops::op_fit(
                img, *dw, *dh, *filter, *bleed, *centering, mode,
            );
        }
        let mode_code = dynimg_mode(img);
        let filter_code = if matches!(mode, Some("P" | "PA")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let (result, nw, nh) = super::scalar::fit(
            &pixels,
            w,
            h,
            mode_code,
            *dw,
            *dh,
            *bleed as f32,
            (centering.0 as f32, centering.1 as f32),
            filter_code,
        );
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Fit op".into()))
    }
}

pub fn simd_scale(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Scale { factor, filter } = op {
        // Native byte layouts already have an exact representation-aware
        // resize implementation.  Packing them into RGBA before scaling
        // adds a full-frame conversion and makes the SIMD-labelled route
        // slower without adding vector work; keep the source layout intact.
        if uses_native_scalar_mode(img, mode)
            || !simd_resize_filter_supported(filter)
            || native_byte_layout(img, mode).is_some()
        {
            return crate::compute::pool_cpu::ops::imageops::op_scale(img, *factor, *filter, mode);
        }
        let pixels = pixels_from_dynimg(img);
        let mode_code = dynimg_mode(img);
        let filter_code = if matches!(mode, Some("P" | "PA")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let (result, nw, nh) = super::scalar::scale(&pixels, w, h, mode_code, *factor, filter_code);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Scale op".into()))
    }
}

pub fn simd_pad(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Pad {
        w: dw,
        h: dh,
        filter,
        color,
        centering,
    } = op
    {
        // Pad contains the same packed resize approximation; use the exact
        // native path for ordinary byte layouts until its coefficient tables
        // are ported to SIMD.
        if uses_native_scalar_mode(img, mode)
            || native_byte_layout(img, mode).is_some()
            || !simd_resize_filter_supported(filter)
        {
            return crate::compute::pool_cpu::ops::imageops::op_pad(
                img, *dw, *dh, *filter, *color, *centering, mode,
            );
        }
        let filter_code = if matches!(mode, Some("P" | "PA")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let fill = match color {
            Some(c) => pack_rgba(*c),
            None if mode_code == 1 || mode_code == 3 => 0,
            None => 0xFF00_0000u32,
        };
        let (result, nw, nh) = super::scalar::pad(
            &pixels,
            w,
            h,
            mode_code,
            *dw,
            *dh,
            filter_code,
            centering.0,
            centering.1,
            fill,
        );
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Pad op".into()))
    }
}

pub fn simd_expand(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Expand { border, fill } = op {
        let fill_rgba = pack_rgba(*fill);
        let (result, nw, nh) = super::scalar::expand(&pixels, w, h, mode_code, *border, fill_rgba);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Expand op".into()))
    }
}

pub fn simd_crop_border(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // CropBorder is a contiguous byte movement, not an arithmetic SIMD
    // workload.  On ordinary native layouts, avoid widening every source
    // pixel to the packed RGBA representation only to copy it back out.  The
    // CPU implementation uses the image crate's native crop and preserves
    // the exact public error/zero-sized-border behavior; reusing it here also
    // keeps typed samples out of the lossy RGBA adapter.
    if uses_native_scalar_mode(img, mode) || native_byte_layout(img, mode).is_some() {
        if let PipelineOp::CropBorder { border } = op {
            let (w, h) = img.dimensions();
            // Avoid the packed adapter's representation conversion while
            // preserving ImageOps.crop's public error text.  The checked
            // half-size form is equivalent to Pillow's `2 * border > size`
            // rule without allowing a u32 multiplication to wrap.
            if *border > w / 2 || *border > h / 2 {
                return Err(PilError::ValueError(
                    "Coordinate 'right' is less than 'left'".into(),
                ));
            }
            return crate::compute::pool_cpu::ops::geometry::execute_crop(
                img,
                *border,
                *border,
                w - *border,
                h - *border,
            );
        }
        return Err(PilError::ValueError("expected CropBorder op".into()));
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::CropBorder { border } = op {
        let (result, nw, nh) = super::scalar::crop_border(&pixels, w, h, mode_code, *border);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected CropBorder op".into()))
    }
}

pub fn simd_crop(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // Crop is a byte movement, not an arithmetic SIMD workload.  The shared
    // native implementation copies rows directly for all ordinary byte
    // layouts (and handles typed samples without widening), whereas the
    // packed adapter would first expand every pixel to RGBA and then scan it
    // one scalar pixel at a time.  Reuse the exact native implementation for
    // both representations; unsupported/typed layouts remain on its own
    // representation-aware path as well.
    if uses_native_scalar_mode(img, mode) || native_byte_layout(img, mode).is_some() {
        if let PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } = op
        {
            return crate::compute::pool_cpu::ops::geometry::execute_crop(
                img, *left, *top, *right, *bottom,
            );
        }
        return Err(PilError::ValueError("expected Crop op".into()));
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Crop {
        left,
        top,
        right,
        bottom,
    } = op
    {
        let (result, nw, nh) =
            super::scalar::crop(&pixels, w, h, mode_code, *left, *top, *right, *bottom);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Crop op".into()))
    }
}

pub fn simd_rotate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Rotate {
        angle,
        expand,
        fill,
        center,
        translate,
        nearest,
    } = op
    {
        // Bilinear LA/RGBA remains in the packed kernel; it performs the same
        // premultiplied alpha round-trip as the exact affine path below.
        if uses_native_scalar_mode(img, mode) || matches!(mode, Some("1" | "P" | "PA" | "RGBa")) {
            return crate::compute::pool_cpu::ops::geometry::execute_rotate(
                img, *angle, *expand, *fill, *center, *translate, *nearest, mode,
            );
        }
        let mode_code = mode_to_u32(img, mode);
        let pixels = pixels_from_dynimg(img);
        let fill_rgba = match fill {
            Some(c) => pack_rgba(*c),
            None => 0u32,
        };
        let (result, nw, nh) =
            super::scalar::rotate(&pixels, w, h, mode_code, *angle, *expand, fill_rgba);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Rotate op".into()))
    }
}

pub fn simd_reduce(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // Reduce has the same shape as the CPU's exact native implementation:
    // each output row owns disjoint source blocks and uses Pillow's fixed
    // point rounding, including partial edge blocks and alpha premultiplying.
    // Avoid the packed RGBA adapter for native byte and typed scalar images;
    // there is no vector arithmetic here to justify the representation copy.
    if uses_native_scalar_mode(img, mode) || native_byte_layout(img, mode).is_some() {
        if let PipelineOp::Reduce { x_factor, y_factor } = op {
            return crate::compute::pool_cpu::ops::geometry::execute_reduce(
                img, *x_factor, *y_factor,
            );
        }
        return Err(PilError::ValueError("expected Reduce op".into()));
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::Reduce { x_factor, y_factor } = op {
        let (result, nw, nh) =
            super::scalar::reduce(&pixels, w, h, mode_code, *x_factor, *y_factor);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Reduce op".into()))
    }
}

pub fn simd_convert(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Convert {
        mode: cm, ..
    } = op
    {
        // The packed SIMD converter only represents byte L/LA/RGB/RGBA/CMYK
        // samples. Keep scalar-mode and color-space conversions on the shared
        // pure-Rust converter instead of returning an RGBA-shaped result for a
        // public HSV, YCbCr, I, or F image.
        if matches!(
            cm,
            ColorMode::HSV
                | ColorMode::YCbCr
                | ColorMode::I
                | ColorMode::F
                | ColorMode::P
                | ColorMode::Mode1
        ) {
            return crate::compute::pool_cpu::ops::color::op_convert(
                img,
                cm,
            );
        }
        if matches!(cm, ColorMode::CMYK)
            && matches!(
                img,
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
            )
        {
            if let Some(result) = native_luma_to_cmyk(img) {
                return Ok(result);
            }
        }
        // For ordinary native byte layouts, the exact converter already
        // operates on the source representation directly.  Avoid widening
        // L/LA/RGB/RGBA/CMYK to packed RGBA before a byte-to-byte conversion;
        // that conversion cost dominates terminal-read pipelines and adds no
        // architecture-specific SIMD work.
        // `mode` is the pipeline's destination tag for Convert, so inspect
        // the concrete source layout without applying that destination tag.
        if native_byte_layout(img, None).is_some()
            && matches!(
                cm,
                ColorMode::L | ColorMode::LA | ColorMode::RGB | ColorMode::RGBA | ColorMode::CMYK
            )
        {
            return crate::compute::pool_cpu::ops::color::op_convert(
                img,
                cm,
            );
        }
        let src_mode = dynimg_mode(img);
        let pixels = pixels_from_dynimg(img);
        let target_mode = color_mode_to_u32(cm);
        let (result, _nw, _nh) = super::scalar::convert(&pixels, w, h, src_mode, target_mode);
        // `convert` returns packed RGBA storage for every logical target. The
        // public result must retain the target mode, not the storage mode.
        let output_mode = match target_mode {
            0 => PixelMode::L,
            1 => PixelMode::LA,
            2 => PixelMode::RGB,
            4 => PixelMode::CMYK,
            _ => PixelMode::RGBA,
        };
        dynimg_from_pixel_mode(result, w, h, output_mode)
    } else {
        Err(PilError::ValueError("expected Convert op".into()))
    }
}

pub fn simd_remap_palette(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let pixels = pixels_from_dynimg(img);
    if let PipelineOp::RemapPalette { dest_map } = op {
        let mut inverse = [0u8; 256];
        for (new_index, &old_index) in dest_map.iter().take(256).enumerate() {
            inverse[usize::from(old_index)] = new_index as u8;
        }
        let result = super::scalar::remap_palette(&pixels, mode_code, &inverse);
        Ok(preserve_mode(img, dynimg_from_rgba(result, w, h)?))
    } else {
        Err(PilError::ValueError("expected RemapPalette op".into()))
    }
}

pub fn simd_transform(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    if let PipelineOp::Transform {
        w: dw,
        h: dh,
        method,
        data,
        filter,
        fill,
        palette_fill,
        ..
    } = op
    {
        let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
        // Keep explicit premultiplied RGBa, non-affine methods, and native
        // scalar modes on the exact CPU implementation. LA/RGBA are supported
        // by the SIMD scalar kernel and must not be redirected by storage type.
        if mode == Some("RGBa")
            || !matches!(method, TransformMethod::Affine)
            || uses_native_scalar_mode(img, mode)
        {
            return crate::compute::pool_cpu::ops::effects::op_transform(
                img,
                *dw,
                *dh,
                method,
                data,
                filter,
                resolved_fill,
                mode,
            );
        }
        let mode_code = transform_mode_to_u32(img, mode);
        let pixels = pixels_from_dynimg(img);
        let matrix: [f64; 8] = {
            let mut arr = [0.0f64; 8];
            let len = data.len().min(8);
            arr[..len].copy_from_slice(&data[..len]);
            arr
        };
        // Pillow keeps palette/index images on nearest-neighbor sampling even
        // when a different public resampling filter is requested. Preserve
        // the CPU path's mode-specific behavior before entering the packed
        // SIMD transform kernel; interpolating palette indices would produce
        // invalid colors rather than a filtered image.
        let f = if matches!(mode, Some("1" | "P")) {
            0
        } else {
            filter_to_u32(filter)
        };
        let fill_rgba = match resolved_fill {
            Some((r, g, b, a)) => {
                // The packed kernel always receives RGBA-shaped samples. PIL
                // stores LA fills as (gray, alpha), so duplicate gray into
                // the RGB lanes and carry alpha in the packed high byte.
                let canonical = match mode_code {
                    0 => (r, r, r, 255),
                    1 => (r, r, r, g),
                    _ => (r, g, b, a),
                };
                pack_rgba(canonical)
            }
            None => 0u32,
        };
        let (result, nw, nh) =
            super::scalar::transform(&pixels, w, h, mode_code, *dw, *dh, &matrix, f, fill_rgba);
        Ok(preserve_mode(img, dynimg_from_rgba(result, nw, nh)?))
    } else {
        Err(PilError::ValueError("expected Transform op".into()))
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section H: Special/mutating ops
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_put_pixel(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some(result) = native_put_pixel(img, op, mode) {
        return Ok(result);
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PutPixel { x, y, color, .. } = op {
        let packed = pack_rgba(*color);
        super::scalar::put_pixel(&mut pixels, w, mode_code, *x, *y, packed);
    }
    // `PutPixel` is mode-preserving in Pillow. Rebuilding every result as
    // RGBA changes the logical mode of an L/LA/RGB pipeline when no explicit
    // mode tag is present, so a following mode-sensitive operation such as
    // ImageOps.colorize observes RGBA and raises instead of receiving L.
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

pub fn simd_put_data(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PutData { data, mode } = op {
        super::scalar::put_data(&mut pixels, mode.code(), data);
        return dynimg_from_pixel_mode(pixels, w, h, *mode);
    }
    Err(PilError::ValueError("expected PutData op".into()))
}

pub fn simd_put_alpha(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::PutAlpha { alpha, mode } = op {
        super::scalar::put_alpha(&mut pixels, mode.code(), *alpha);
        return dynimg_from_put_alpha(pixels, w, h, *mode);
    }
    Err(PilError::ValueError("expected PutAlpha op".into()))
}

pub fn simd_eval(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::Eval { lut } = op {
        // Public Image.point is represented as Eval after the binding
        // validates the LUT.  Keep ordinary native L/RGB images in their
        // byte layout instead of widening them to packed RGBA for a single
        // lookup-table traversal.  Typed, palette, and alpha-sensitive
        // layouts retain the exact packed fallback below.
        if let Some(result) = native_point_lut(img, mode, lut) {
            return Ok(result);
        }
    }
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Eval { lut } = op {
        let lut_arr: [u8; 1024] = {
            let mut arr = [0u8; 1024];
            let len = lut.len().min(1024);
            arr[..len].copy_from_slice(&lut[..len]);
            arr
        };
        super::scalar::eval(&mut pixels, mode_code, &lut_arr);
    }
    Ok(crate::image::preserve_mode(
        img,
        dynimg_from_rgba(pixels, w, h)?,
    ))
}

pub fn simd_paste(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if uses_native_scalar_mode(img, mode) {
        if let PipelineOp::Paste {
            source,
            x,
            y,
            mask,
            mask_alpha,
            ..
        } = op
        {
            return crate::compute::pool_cpu::ops::effects::op_paste(
                img,
                source,
                i64::from(*x),
                i64::from(*y),
                mask,
                *mask_alpha,
                mode,
            );
        }
        return Err(PilError::ValueError("expected Paste op".into()));
    }
    let (w, h) = img.dimensions();
    let mode_code = if mode == Some("P") {
        0
    } else if mode == Some("PA") {
        1
    } else if mode.is_some() {
        mode_to_u32(img, mode)
    } else {
        dynimg_mode(img)
    };
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Paste {
        source,
        x,
        y,
        w: _,
        h: _,
        mask,
        mask_alpha,
    } = op
    {
        let src_img = if matches!(mode, Some("P" | "PA")) {
            source.materialize_indices()?
        } else {
            arc_to_dynimg(source)?
        };
        let (src_w, src_h) = src_img.dimensions();
        let src_pixels = pixels_from_dynimg(&src_img);
        let mask_pixels: Option<Vec<u32>> = match mask {
            Some(m) => Some(pixels_from_arc(m)?),
            None => None,
        };
        super::scalar::paste(
            &mut pixels,
            w,
            h,
            mode_code,
            &src_pixels,
            src_w,
            src_h,
            *x,
            *y,
            mask_pixels.as_deref(),
            *mask_alpha,
        );
    }
    Ok(crate::image::preserve_mode(
        img,
        dynimg_from_rgba(pixels, w, h)?,
    ))
}

pub fn simd_alpha_composite(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mode_code = mode_to_u32(img, mode);
    let PipelineOp::AlphaComposite { source, dest, src } = op else {
        return Err(PilError::ValueError(
            "expected AlphaComposite op".to_string(),
        ));
    };

    #[cfg(target_arch = "aarch64")]
    if *dest == (0, 0)
        && *src == (0, 0)
        && matches!(
            img,
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        )
        && matches!(mode, None | Some("LA" | "RGBA"))
    {
        // On the arm64 release host the exact native SIMD alpha loop is
        // slower than the CPU row implementation at both 256² and
        // 1024×768. Keep the public SIMD result exact and make the
        // measured crossover explicit until an architecture-specific
        // integer alpha kernel earns a win.
        crate::compute::record_pipeline_backend_fallback(
            "SIMD AlphaComposite: exact CPU crossover fallback",
        );
        return crate::compute::pool_cpu::ops::effects::op_alpha_composite(img, source);
    }

    let mut pixels = pixels_from_dynimg(img);
    let src_img = arc_to_dynimg(source)?;
    if let Some(result) = native_alpha_composite(img, &src_img, mode, *dest, *src) {
        return Ok(result);
    }
    let (src_w, src_h) = src_img.dimensions();
    let src_pixels = pixels_from_dynimg(&src_img);
    super::scalar::alpha_composite(
        &mut pixels,
        w,
        h,
        mode_code,
        &src_pixels,
        src_w,
        src_h,
        dest.0,
        dest.1,
        src.0,
        src.1,
    );
    Ok(preserve_mode(img, dynimg_from_rgba(pixels, w, h)?))
}

// ═══════════════════════════════════════════════════════════════════════
// Section I: Merge — multi-image band composition
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_merge(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut pixels = pixels_from_dynimg(img);
    if let PipelineOp::Merge {
        mode: merge_mode,
        bands,
    } = op
    {
        // The registry `mode` argument carries the source image's legacy mode
        // tag and is `None` for ordinary Image.merge calls. The operation's
        // ColorMode is the authoritative output mode and also determines how
        // many bands are valid. Using the registry tag here made RGB merges
        // fall through to the RGBA path and index a fourth (nonexistent) band.
        let (mode_code, output_mode) = match merge_mode {
            ColorMode::L => (0, PixelMode::L),
            ColorMode::LA => (1, PixelMode::LA),
            ColorMode::RGB => (2, PixelMode::RGB),
            ColorMode::RGBA => (3, PixelMode::RGBA),
            // CMYK is stored in the packed four-byte representation used by
            // the RGBA SIMD lane, but retains its logical mode at the Image
            // layer through the existing explicit-mode tag.
            ColorMode::CMYK => (3, PixelMode::CMYK),
            _ => {
                return Err(PilError::ValueError(
                    "SIMD merge: unsupported output mode".to_string(),
                ));
            }
        };
        // Pillow's ImagingMerge consumes every input as a single-byte band.
        // The current image is already that raw sample buffer, which matters
        // when the first band is a P image: materialize_for_ops() would expand
        // palette index 1 into its visible palette color before the merge.
        // Later P bands are rejected by the public validation, so only those
        // remaining inputs need ordinary operation materialization here.
        let mut band_pixels = vec![pixels.clone()];
        for band in bands.iter().skip(1) {
            let band_img = band.materialize_for_ops()?;
            band_pixels.push(pixels_from_dynimg(&band_img));
        }
        let expected_bands = match mode_code {
            0 => 1,
            1 => 2,
            2 => 3,
            3 => 4,
            _ => unreachable!(),
        };
        if band_pixels.len() != expected_bands
            || band_pixels.iter().any(|band| band.len() != pixels.len())
        {
            // Match Pillow's ImagingMerge contract and the CPU lane. This is
            // also the safe boundary before scalar merge indexes each band.
            return Err(PilError::ValueError("size mismatch".to_string()));
        }
        let band_refs: Vec<&[u32]> = band_pixels.iter().map(|v| v.as_slice()).collect();
        super::scalar::merge(&mut pixels, mode_code, &band_refs);
        return dynimg_from_pixel_mode(pixels, w, h, output_mode);
    }
    Err(PilError::ValueError("expected Merge op".to_string()))
}
