use std::borrow::Cow;

use super::conversion::muldiv255;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::DynamicImage;

#[cfg(feature = "parallel")]
const CMYK_TO_BINARY_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

#[inline]
/// Converts one RGB pixel to a Pillow-compatible `L` value.
///
/// PIL uses: Y = (19595*R + 38470*G + 7471*B + 32768) >> 16
/// This matches PIL pixel-for-pixel. Decimal approximation (299/587/114)
/// differs for ~3/10000 pixels due to rounding in different directions.
pub fn rgb_to_luma_u8(r: u8, g: u8, b: u8) -> u8 {
    (((19595u32 * r as u32 + 38470u32 * g as u32 + 7471u32 * b as u32 + 32768) >> 16) & 0xFF) as u8
}

// The x86 scalar path sums precomputed exact contributions so each pixel uses
// small, cache-resident lookups instead of variable coefficient multiplies.
#[cfg(any(test, target_arch = "x86_64"))]
const fn grayscale_contribution_table(coefficient: u32) -> [u32; 256] {
    let mut table = [0; 256];
    let mut sample = 0;
    while sample < table.len() {
        table[sample] = sample as u32 * coefficient;
        sample += 1;
    }
    table
}

#[cfg(any(test, target_arch = "x86_64"))]
const GRAYSCALE_RED_CONTRIBUTIONS: [u32; 256] = grayscale_contribution_table(19595);
#[cfg(any(test, target_arch = "x86_64"))]
const GRAYSCALE_GREEN_CONTRIBUTIONS: [u32; 256] = grayscale_contribution_table(38470);
#[cfg(any(test, target_arch = "x86_64"))]
const GRAYSCALE_BLUE_CONTRIBUTIONS: [u32; 256] = grayscale_contribution_table(7471);

#[cfg(test)]
#[path = "../color_grayscale_allocation_tests.rs"]
mod la_conversion_allocation_receipts;

/// Converts an image to Pillow-compatible `L` grayscale.
///
/// The conversion uses Pillow's rounded BT.601 fixed-point formula, not the
/// `image` crate's sRGB luminance weights. The returned image has the same
/// dimensions as `img` and one byte per pixel.
pub fn pil_grayscale(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    pil_grayscale_inner(img, true)
}

/// Converts an image to grayscale with Pillow's mode `"1"` truncation rule.
///
/// `Image.convert("1")` uses the BT.601 coefficients without the rounding bias
/// used by `"L"` conversion. The returned buffer is still an 8-bit grayscale
/// image; callers perform the final binary thresholding step.
pub fn pil_grayscale_truncate(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    pil_grayscale_inner(img, false)
}

fn pil_grayscale_inner(
    img: &DynamicImage,
    round: bool,
) -> Result<crate::raster::GrayImage, PilError> {
    let (w, h) = (img.width(), img.height());
    // Pillow's ImageOps.grayscale preserves valid empty images.  The output
    // has no samples to allocate or process, so use the explicit empty-image
    // allowance rather than treating this as an invalid allocation.
    let dims = CheckedDims::new_allow_empty(w, h, 1)?;
    // PIL-identical 16-bit fixed-point BT.601:
    // Y = (19595*R + 38470*G + 7471*B + [32768]) >> 16
    let rounding = if round { 32768u32 } else { 0u32 };
    // Borrow byte storage in its native layout. Expanding L/LA or stripping
    // RGBA through to_rgb8 allocates an entire extra image; even RGB clones.
    let gray = match img {
        DynamicImage::ImageLuma8(source) => source.as_raw().to_vec(),
        DynamicImage::ImageLumaA8(source) => source
            .as_raw()
            .chunks_exact(2)
            .map(|pixel| pixel[0])
            .collect(),
        DynamicImage::ImageRgb8(source) => {
            grayscale_rgb_bytes::<3>(source.as_raw(), dims, rounding)
        }
        DynamicImage::ImageRgba8(source) => {
            grayscale_rgb_bytes::<4>(source.as_raw(), dims, rounding)
        }
        _ => {
            let rgb = img.to_rgb8();
            if rgb.as_raw().capacity() != 0 {
                crate::compute::record_pipeline_allocation(rgb.as_raw().capacity());
            }
            grayscale_rgb_bytes::<3>(rgb.as_raw(), dims, rounding)
        }
    };
    // Every branch owns one grayscale Vec, including native L/LA copies.
    // Record its actual capacity here so RGB paths are counted once and empty
    // buffers do not manufacture an allocation. The fallback's RGB temporary
    // is a separate allocation recorded at its creation above.
    if gray.capacity() != 0 {
        crate::compute::record_pipeline_allocation(gray.capacity());
    }

    crate::raster::GrayImage::from_raw(w, h, gray)
        .ok_or_else(|| PilError::InternalError("pil_grayscale buffer mismatch".to_string()))
}

pub(crate) fn grayscale_rgb_bytes<const CHANNELS: usize>(
    source: &[u8],
    dims: CheckedDims,
    rounding: u32,
) -> Vec<u8> {
    let expected_pixels = dims.total_pixels();
    let mut gray: Vec<u8> = source
        .chunks_exact(CHANNELS)
        .take(expected_pixels)
        .map(|pixel| grayscale_rgb_pixel(pixel, rounding))
        .collect();
    // Valid image buffers yield every pixel and avoid zero-initializing the
    // output before overwriting it. Keep the former zero-fill behavior if an
    // internal caller supplies a truncated buffer.
    gray.resize(expected_pixels, 0);
    gray
}

#[inline(always)]
pub(crate) fn grayscale_rgb_pixel(pixel: &[u8], rounding: u32) -> u8 {
    #[cfg(target_arch = "x86_64")]
    {
        grayscale_rgb_pixel_lookup(pixel, rounding)
    }
    #[cfg(target_arch = "aarch64")]
    if rounding == 32768 {
        let red = u16::from(pixel[0]);
        let green = u16::from(pixel[1]);
        let blue = u16::from(pixel[2]);
        // Split the exact Pillow coefficients at bit 8:
        // 19595=77*256-117, 38470=150*256+70, 7471=29*256+47.
        // With the rounding bias, residual is 2,933..62,603; base is at most
        // 65,280 and base plus carry at most 65,524, so u16 is exact.
        let base = red * 77 + green * 150 + blue * 29;
        let residual = green * 70 + blue * 47 + 32768 - red * 117;
        return ((base + (residual >> 8)) >> 8) as u8;
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        ((19595 * u32::from(pixel[0])
            + 38470 * u32::from(pixel[1])
            + 7471 * u32::from(pixel[2])
            + rounding)
            >> 16) as u8
    }
}

#[cfg(any(test, target_arch = "x86_64"))]
#[inline(always)]
pub(super) fn grayscale_rgb_pixel_lookup(pixel: &[u8], rounding: u32) -> u8 {
    let sum = GRAYSCALE_RED_CONTRIBUTIONS[usize::from(pixel[0])]
        + GRAYSCALE_GREEN_CONTRIBUTIONS[usize::from(pixel[1])]
        + GRAYSCALE_BLUE_CONTRIBUTIONS[usize::from(pixel[2])]
        + rounding;
    (sum >> 16) as u8
}

#[cfg(test)]
#[inline(always)]
pub(super) fn grayscale_rgb_pixel_delta(pixel: &[u8], rounding: u32) -> u8 {
    let green = i32::from(pixel[1]);
    // The BT.601 coefficients sum to 65536, so G is an exact base and the
    // correction needs only R/G and B/G differences:
    // Y = G + (((R-G)*19595 + (B-G)*7471 + rounding) >> 16).
    let correction = (i32::from(pixel[0]) - green) * 19595
        + (i32::from(pixel[2]) - green) * 7471
        + rounding as i32;
    (green + (correction >> 16)) as u8
}

#[inline(always)]
pub(crate) fn cmyk_muldiv255_byte(channel: u8, ink: u8) -> u8 {
    // For byte inputs, t <= 65,153 and t + (t >> 8) <= 65,407, so Pillow's
    // exact MULDIV255 identity fits entirely in u16.
    let t = u16::from(channel) * u16::from(ink) + 128;
    (((t >> 8) + t) >> 8) as u8
}

/// Converts a CMYK image to Pillow-compatible grayscale.
///
/// The input is stored as RGBA where channels mean `C`, `M`, `Y`, and `K`.
/// Output dimensions match the input and each output pixel is an `L` byte.
pub fn cmyk_to_grayscale(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    use crate::compute::pool_cpu::ops::grayscale::cmyk8::default::grayscale_impl;
    let source = match img {
        DynamicImage::ImageCmyk8(source) | DynamicImage::ImageRgba8(source) => Cow::Borrowed(source),
        _ => Cow::Owned(img.to_rgba8()),
    };
    grayscale_impl::execute(grayscale_impl::prepare(source.as_ref())?)
}

/// Converts CMYK storage to Pillow's truncated binary-conversion luminance.
///
/// Pillow's `convert("1")` path uses the integer `299/587/114` luma formula
/// after expanding each CMYK sample to RGB. This is intentionally different
/// from ordinary `L` conversion's rounded fixed-point formula because the
/// difference changes threshold decisions near 128. Fuse both stages so the
/// intermediate RGB frames are never materialized.
#[inline]
fn cmyk_pixel_to_truncated_luma(pixel: &[u8]) -> u8 {
    let ink = 255u32 - u32::from(pixel[3]);
    let red = (ink - muldiv255(u32::from(pixel[0]), ink)) as u8;
    let green = (ink - muldiv255(u32::from(pixel[1]), ink)) as u8;
    let blue = (ink - muldiv255(u32::from(pixel[2]), ink)) as u8;
    let value =
        (299u32 * u32::from(red) + 587u32 * u32::from(green) + 114u32 * u32::from(blue)) / 1000;
    value.min(255) as u8
}

#[inline]
fn cmyk_to_binary_row(source: &[u8], output: &mut [u8]) {
    for (value, pixel) in output.iter_mut().zip(source.chunks_exact(4)) {
        *value = if cmyk_pixel_to_truncated_luma(pixel) >= 128 {
            255
        } else {
            0
        };
    }
}

pub fn cmyk_to_grayscale_truncate(
    img: &DynamicImage,
) -> Result<crate::raster::GrayImage, PilError> {
    let (w, h) = (img.width(), img.height());
    let cmyk = match img {
        DynamicImage::ImageRgba8(image) => Cow::Borrowed(image.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let dims = CheckedDims::new(w, h, 1)?;
    let expected_source_bytes = dims
        .total_pixels()
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("CMYK binary conversion size overflow".into()))?;
    if cmyk.len() != expected_source_bytes {
        return Err(PilError::InternalError(
            "CMYK binary conversion source buffer mismatch".into(),
        ));
    }
    let mut gray = dims.alloc_buffer();
    for (index, pixel) in cmyk.chunks_exact(4).enumerate() {
        gray[index] = cmyk_pixel_to_truncated_luma(pixel);
    }
    crate::raster::GrayImage::from_raw(w, h, gray)
        .ok_or_else(|| PilError::InternalError("cmyk grayscale buffer mismatch".to_string()))
}

/// Converts CMYK storage directly to Pillow's no-dither binary output.
///
/// This preserves the exact truncated luminance and `>= 128` threshold while
/// avoiding a grayscale intermediate and a second full-frame pass.
pub fn cmyk_to_binary_truncate(img: &DynamicImage) -> Result<crate::raster::GrayImage, PilError> {
    let (w, h) = (img.width(), img.height());
    let cmyk = match img {
        DynamicImage::ImageRgba8(image) => Cow::Borrowed(image.as_raw().as_slice()),
        _ => Cow::Owned(img.to_rgba8().into_raw()),
    };
    let dims = CheckedDims::new(w, h, 1)?;
    let expected_source_bytes = dims
        .total_pixels()
        .checked_mul(4)
        .ok_or_else(|| PilError::InternalError("CMYK binary conversion size overflow".into()))?;
    if cmyk.len() != expected_source_bytes {
        return Err(PilError::InternalError(
            "CMYK binary conversion source buffer mismatch".into(),
        ));
    }
    let mut output = dims.alloc_buffer();
    let source_stride = (w as usize) * 4;
    let output_stride = w as usize;
    #[cfg(feature = "parallel")]
    if dims.total_pixels() >= CMYK_TO_BINARY_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            &mut output,
            output_stride,
            h as usize,
            |_row_start, _row_end, y, row| {
                let source_start = y as usize * source_stride;
                cmyk_to_binary_row(&cmyk[source_start..source_start + source_stride], row);
            }
        );
        return crate::raster::GrayImage::from_raw(w, h, output)
            .ok_or_else(|| PilError::InternalError("cmyk binary buffer mismatch".to_string()));
    }
    for (source, output) in cmyk
        .chunks_exact(source_stride)
        .zip(output.chunks_exact_mut(output_stride))
    {
        cmyk_to_binary_row(source, output);
    }
    crate::raster::GrayImage::from_raw(w, h, output)
        .ok_or_else(|| PilError::InternalError("cmyk binary buffer mismatch".to_string()))
}
/// Converts an image to Pillow-compatible `LA`.
///
/// The luma channel uses [`pil_grayscale`]. The alpha channel is set to fully
/// opaque (`255`) for every pixel.
pub fn pil_grayscale_alpha(img: &DynamicImage) -> Result<crate::raster::GrayAlphaImage, PilError> {
    let gray = pil_grayscale(img)?;
    let (w, h) = gray.dimensions();
    let mut ga = crate::raster::GrayAlphaImage::new(w, h);
    for (gap, gp) in ga.pixels_mut().zip(gray.pixels()) {
        gap[0] = gp[0];
        gap[1] = 255;
    }
    Ok(ga)
}
