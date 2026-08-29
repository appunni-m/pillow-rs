//! SIMD adapter wrappers for the registry's `SimdOpFn` signature.
//!
//! Each admitted adapter validates the concrete image layout and parameters
//! before execution, then keeps the hot pixel/data path in a native byte
//! kernel. Scalar work is limited to control/validation and unavoidable edge
//! or tail handling; unsupported contracts return a capability error.

use crate::checked_dims::CheckedDims;
use crate::compute::pool_cpu::ops::draw::{
    arc_clip_state, chord_clip_state, chord_line_clip_state, for_each_clipped_ellipse_span,
    for_each_ellipse_span, normalize_angles, pie_clip_state, pie_side_clip_state,
};
use crate::compute::pool_cpu::ops::geometry::resample_kernel;
use crate::draw::{for_each_bresenham_point, for_each_polygon_fill_span, wide_line_polygon_points};
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::ops::pil_resize::{
    filter_from_resample, luma16_resample_big_endian, luma16_resample_read,
    luma16_resample_write, precompute_coeffs, precompute_coeffs_boxed_for_filter,
    precompute_coeffs_f64, precompute_coeffs_f64_boxed, round_up, FilterCoeffs,
};
use crate::pipeline::{
    ColorMode, PipelineOp, PixelMode, ResampleFilter, TransformMethod, TransposeMethod,
};
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, ImageBuffer, Luma, RgbImage,
    RgbaImage,
};
use std::sync::{Arc, OnceLock};
use wide::{f32x8, f64x8, i16x8, i32x8, u8x16, u16x8, u16x16, u32x8};

fn native_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Return the native byte layout accepted by Pillow's filter kernels.
///
/// Filter.c operates on the stored samples rather than interpreting the
/// fourth byte as alpha.  That makes the filter contract broader than the
/// ordinary RGBA arithmetic adapters: `1`, CMYK, and PA stay in their native
/// packed layouts.  Palette-only `P` is intentionally excluded because
/// Pillow rejects filtering a palette image.
fn native_filter_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX" | "CMYK")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_extract_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_typed_filter_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    matches!(img, DynamicImage::ImageRgba8(_))
        .then_some(())
        .filter(|_| matches!(mode, Some("I" | "F")))
        .map(|_| 4)
}

/// Return the native byte layout accepted by rotation. Indexed images remain
/// raw sample planes during Pillow rotation: `1`/`P` are one-byte nearest
/// data and `PA` is a two-byte index/alpha plane. CMYK, RGBa, and RGBX also
/// remain raw four-byte samples; only straight-alpha `LA`/`RGBA` gets
/// alpha-aware interpolation. These modes must not use the narrower
/// ordinary-byte helper just because their storage is backed by the same Rust
/// image type.
fn native_rotate_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, Some("1" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if mode == Some("PA") => Some(2),
        DynamicImage::ImageRgba8(_)
            if matches!(mode, Some("CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => native_byte_layout(img, mode),
    }
}

/// Return the native byte layout and whether the final channel is alpha for
/// `Image.reduce`. CMYK is four stored color samples, not RGBA; HSV and YCbCr
/// are three independent native samples. Keeping this distinction in the
/// admission contract prevents the reduction kernel from premultiplying a
/// color channel that only happens to occupy the fourth byte.
fn native_reduce_layout(img: &DynamicImage, mode: Option<&str>) -> Option<(usize, bool)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, false)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, true)),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some((3, false))
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, true)),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, false)),
        _ => None,
    }
}

/// Return the native byte layout accepted by brightness blending.
///
/// HSV and YCbCr are physically stored as three-byte planes, while CMYK is
/// stored in `Rgba8` with its fourth byte representing K rather than alpha.
/// Brightness blends every sample with zero, so these layouts can use the
/// same byte-wise SIMD LUT without a color-space conversion or alpha rule.
fn native_brightness_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        // HSV and YCbCr are also three-byte native planes. Brightness is a
        // blend with the same-mode black image, so each stored sample follows
        // the same byte LUT without a color-space conversion.
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "CMYK")) => Some(4),
        _ => None,
    }
}

/// Return the native layout and active-channel count used by ImageEnhance.
///
/// Contrast and color saturation preserve alpha in `LA`/`RGBA`, while CMYK's
/// fourth byte is the K sample rather than an alpha byte.  Keeping that
/// distinction at the admission boundary prevents the vector kernel from
/// accidentally treating packed RGBA storage as a color contract.
fn native_enhance_layout(img: &DynamicImage, mode: Option<&str>) -> Option<(usize, usize)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, 1)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, 1)),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

/// Return the native layout and active-channel count used by Sharpness.
///
/// Pillow applies Sharpness to RGB samples and preserves alpha for LA/RGBA;
/// CMYK is different because all four stored bytes are color samples. Keeping
/// that distinction here lets the neighborhood kernel operate in-place on the
/// native interleaved buffer without widening through packed RGBA storage.
fn native_sharpness_layout(img: &DynamicImage, mode: Option<&str>) -> Option<(usize, usize)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, 1)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, 1)),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

fn native_merge_channels(mode: &ColorMode) -> Option<usize> {
    match mode {
        ColorMode::L | ColorMode::Mode1 => Some(1),
        ColorMode::LA => Some(2),
        ColorMode::RGB => Some(3),
        ColorMode::RGBA | ColorMode::CMYK => Some(4),
        _ => None,
    }
}

fn native_merge_band_contract(
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
) -> bool {
    // Pillow preserves the raw palette indices when a P band is first in a
    // multi-band merge. It is the only non-L band accepted by Image.merge;
    // later bands must still be L, and a single-band L merge cannot use P.
    let palette_first = mode == Some("P")
        && !matches!(target_mode, ColorMode::L | ColorMode::Mode1)
        && bands
            .first()
            .is_some_and(|band| band.mode().ok().is_some_and(|band_mode| band_mode == "P"));
    if !matches!(mode, None | Some("L")) && !palette_first {
        return false;
    }
    bands.iter().enumerate().all(|(index, band)| {
        let Ok(band_mode) = band.mode() else {
            return false;
        };
        band_mode == "L" || (palette_first && index == 0 && band_mode == "P")
    })
}

fn native_merge_contract_for_image(
    img: &DynamicImage,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
) -> Option<(usize, usize)> {
    let channels = native_merge_channels(target_mode)?;
    if !matches!(img, DynamicImage::ImageLuma8(_)) || bands.len() != channels {
        return None;
    }
    if !native_merge_band_contract(target_mode, bands, mode) {
        return None;
    }
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if pixels.checked_mul(channels).is_none() || img.as_bytes().len() != pixels {
        return None;
    }
    if !bands
        .iter()
        .all(|band| band.size().ok() == Some(img.dimensions()))
    {
        return None;
    }
    Some((channels, pixels))
}

fn native_merge_contract_for_shape(
    shape: SimdImageShape,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
) -> Option<(usize, usize)> {
    let channels = native_merge_channels(target_mode)?;
    if shape.layout != SimdLayout::Luma8 || bands.len() != channels {
        return None;
    }
    if !native_merge_band_contract(target_mode, bands, mode) {
        return None;
    }
    let pixels = (shape.width as usize).checked_mul(shape.height as usize)?;
    if pixels.checked_mul(channels).is_none() {
        return None;
    }
    if !bands
        .iter()
        .all(|band| band.size().ok() == Some((shape.width, shape.height)))
    {
        return None;
    }
    Some((channels, pixels))
}

fn native_expand_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_)
            if matches!(mode, None | Some("1" | "L" | "P")) =>
        {
            Some(1)
        }
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_expand_contract_for_image(
    img: &DynamicImage,
    border: u32,
    mode: Option<&str>,
) -> Option<(usize, u32, u32)> {
    let channels = native_expand_layout(img, mode)?;
    let border_twice = border.checked_mul(2)?;
    let width = img.width().checked_add(border_twice)?;
    let height = img.height().checked_add(border_twice)?;
    let source_len = (img.width() as usize)
        .checked_mul(img.height() as usize)?
        .checked_mul(channels)?;
    let output_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)?;
    if img.width() == 0
        || img.height() == 0
        || output_len < 16
        || img.as_bytes().len() != source_len
    {
        return None;
    }
    Some((channels, width, height))
}

fn native_expand_contract_for_shape(
    shape: SimdImageShape,
    border: u32,
    mode: Option<&str>,
) -> Option<(usize, u32, u32)> {
    let channels = match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => 1,
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => 2,
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => 3,
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) => 4,
        _ => return None,
    };
    let border_twice = border.checked_mul(2)?;
    let width = shape.width.checked_add(border_twice)?;
    let height = shape.height.checked_add(border_twice)?;
    let output_len = (width as usize)
        .checked_mul(height as usize)?
        .checked_mul(channels)?;
    if shape.width == 0 || shape.height == 0 || output_len < 16 {
        return None;
    }
    Some((channels, width, height))
}

fn native_autocontrast_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        _ => None,
    }
}

/// Return the native source layout accepted by ImageOps.grayscale.
///
/// Grayscale ignores alpha and produces a new `L` image.  RGBX is admitted
/// because its fourth byte is padding, while palette, color-space, CMYK, and
/// typed modes are rejected until their mode-specific conversions can stay
/// native rather than widening through a packed scalar representation.
fn native_grayscale_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "RGBX")) => Some(4),
        _ => None,
    }
}

fn native_grayscale_supported_for_image(img: &DynamicImage, mode: Option<&str>) -> bool {
    let Some(channels) = native_grayscale_layout(img, mode) else {
        return false;
    };
    let Some(pixels) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    pixels != 0
        && pixels
            .checked_mul(channels)
            .is_some_and(|bytes| img.as_bytes().len() == bytes)
}

fn autocontrast_mask_supported(
    width: u32,
    height: u32,
    mask: Option<&Arc<Image>>,
) -> bool {
    let Some(mask) = mask else {
        return true;
    };
    matches!(mask.mode().ok().as_deref(), Some("1") | Some("L"))
        && mask.size().ok() == Some((width, height))
}

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
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "CMYK")) => Some(4),
        _ => None,
    }
}

/// Return the native byte layout accepted by drawing operations.
///
/// Drawing writes samples, rather than interpreting them as colors. That
/// allows the RGB-family logical modes backed by `Rgb8` and the CMYK-family
/// mode backed by `Rgba8` to stay in their native interleaved storage. An
/// explicit RGBA drawing context over an RGB image is also valid: the RGB
/// kernel blends the supplied alpha into the three destination samples.
fn native_draw_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_)
            if matches!(mode, None | Some("1" | "L" | "P")) =>
        {
            Some(1)
        }
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "RGBA" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Return a native interleaved byte layout for operations whose data plane is
/// memory movement rather than color arithmetic. Indexed and color-space
/// tags are included here because their stored samples are still raw bytes;
/// callers that interpret alpha or color arithmetic must use the narrower
/// helpers above instead.
fn native_copy_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            ) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Return whether an `I;16*` image can use the transpose byte kernel without
/// widening its samples into a packed color representation. This is kept
/// separate from `native_copy_layout`: other memory operations still need
/// their own two-byte sample contracts.
fn native_luma16_transpose_layout(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

/// Return the native channel stride for ImageChops operations.
///
/// Chops treats every stored byte as an active sample. In particular, the
/// fourth byte of a CMYK image is K, not alpha, so it must be admitted to the
/// same four-byte vector kernel as RGBA without going through packed-mode
/// interpretation. HSV/YCbCr and RGBa/RGBX likewise retain their raw byte
/// layout at this boundary; ImageChops does not reinterpret those samples as
/// colors or alpha.
fn native_chops_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Return the native byte count for one of Pillow's `Image.blend` mode
/// families. Unlike ImageChops, module-level blend accepts the three-byte
/// HSV/YCbCr family and the four-byte RGBa/RGBX family as raw native samples.
fn native_module_blend_mode_channels(mode: &str) -> Option<usize> {
    match mode {
        "L" => Some(1),
        "LA" => Some(2),
        "RGB" | "HSV" | "YCbCr" => Some(3),
        "RGBA" | "CMYK" | "RGBa" | "RGBX" => Some(4),
        _ => None,
    }
}

/// Validate a concrete image against the logical mode supplied at an
/// operation boundary and return its native interleaved byte count.
fn native_module_blend_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    let channels = match mode {
        Some(mode) => native_module_blend_mode_channels(mode)?,
        None => match img {
            DynamicImage::ImageLuma8(_) => 1,
            DynamicImage::ImageLumaA8(_) => 2,
            DynamicImage::ImageRgb8(_) => 3,
            DynamicImage::ImageRgba8(_) => 4,
            _ => return None,
        },
    };
    match (img, channels) {
        (DynamicImage::ImageLuma8(_), 1)
        | (DynamicImage::ImageLumaA8(_), 2)
        | (DynamicImage::ImageRgb8(_), 3)
        | (DynamicImage::ImageRgba8(_), 4) => Some(channels),
        _ => None,
    }
}

fn native_module_blend_pair_channels(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    other_mode: Option<&str>,
) -> Option<usize> {
    let channels = native_module_blend_layout(img, mode)?;
    (native_module_blend_layout(other, other_mode) == Some(channels)
        && img.dimensions() == other.dimensions())
    .then_some(channels)
}

fn native_module_blend_data_supported(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    let actual_len = img.as_bytes().len();
    actual_len == expected_len && (actual_len == 0 || actual_len >= 8)
}

/// Return whether a row-oriented byte kernel has at least one complete
/// sixteen-byte vector block per row.  A large image made of very narrow rows
/// can have many total bytes while still exercising only scalar tails; those
/// inputs must not be advertised as SIMD-capable.
fn has_vectorized_byte_rows(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0
        && img
            .width()
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

/// Return whether an operation whose formula is independent for every stored
/// byte has at least one complete vector in the whole image.  Row boundaries
/// are not semantic boundaries for Chops, so narrow rows may be concatenated
/// into one vector stream without changing the result.
fn has_vectorized_flat_bytes(img: &DynamicImage, channels: usize) -> bool {
    if img.width() == 0 || img.height() == 0 {
        return false;
    }
    img.width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|total_bytes| {
            total_bytes >= 16 && img.as_bytes().len() == total_bytes as usize
        })
}

/// A zero-pixel native image is a valid Chops data path even though it has no
/// vector block to execute. Returning its already-typed buffer is the native
/// no-work equivalent of Pillow's empty output; it must not be reported as a
/// scalar or arithmetic vector kernel.
fn has_empty_native_bytes(img: &DynamicImage, channels: usize) -> bool {
    (img.width() == 0 || img.height() == 0)
        && img
            .width()
            .checked_mul(img.height())
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            == Some(0)
        && img.as_bytes().is_empty()
}

/// Offset copies wrapped rows through vector loads and lane shuffles. Rows
/// narrower than one vector can still be batched across row boundaries; the
/// capability gate therefore requires one complete vector in the image, not
/// in every individual row.
fn has_vectorized_offset_rows(img: &DynamicImage, channels: usize, xoffset: i32) -> bool {
    if img.width() == 0 || img.height() == 0 {
        return false;
    }
    let Some(row_bytes) = img.width().checked_mul(channels as u32) else {
        return false;
    };
    let _ = xoffset;
    row_bytes
        .checked_mul(img.height())
        .is_some_and(|total_bytes| total_bytes >= 16)
}

/// Pillow's `I;16*` ImageChops offset path is byte-oriented: the native
/// implementation copies only the first `width` bytes of each `width * 2`
/// byte row, leaving the second half zero-filled. Admit that contract only
/// when those copied bytes contain a complete SIMD block. A 2-byte sample
/// image that is smaller than one vector remains an explicit capability miss;
/// padding it into a register would make the entire data path scalar.
fn has_vectorized_luma16_offset(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && (has_empty_native_bytes(img, 2)
            || (img.width() >= 16 && img.height() != 0))
}

/// Mirror uses one padded vector group for a partial final pixel group, so
/// even narrow rows can execute the native reverse kernel. RGB still needs
/// three vectors for one complete sixteen-pixel group, while the other layouts
/// use one vector group sized to their channel stride.
fn has_vectorized_mirror_rows(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0 && img.width() != 0 && matches!(channels, 1..=4)
}

/// A masked final block is still a real SIMD data path. Use this admission
/// predicate for byte-wise kernels whose operation is independent per sample;
/// it permits narrow rows without routing the whole operation to CPU merely
/// because the row has fewer than sixteen bytes.
fn has_nonempty_byte_data(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0
        && img.width() != 0
        && img
            .width()
            .checked_mul(img.height())
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

/// The enhancement kernels consume eight independent byte samples per
/// `f64x8` block.  A partial final block is handled by the same padded vector
/// kernel, but at least one complete block is required for strict SIMD.
fn has_vectorized_float_bytes(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    expected >= 8 && img.as_bytes().len() == expected
}

/// Sharpness needs one complete eight-pixel interior block for its 3x3
/// neighborhood pass. Border pixels are retained from the source, and a
/// scalar tail is allowed after the vector block, but a width with no interior
/// vector block would make the entire explicit SIMD request scalar-only.
fn has_vectorized_sharpness_bytes(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.width() >= 10
        && img.height() >= 3
        && expected != 0
        && img.as_bytes().len() == expected
}

/// Return whether an identity operation can use at least one complete native
/// vector copy.  Zero-radius blur is a valid Pillow no-op, but it must not be
/// admitted as SIMD merely because the image has byte storage: a one-byte
/// scalar-only copy is still an unsupported strict-SIMD data path.
fn has_vectorized_native_identity_copy(img: &DynamicImage, mode: Option<&str>) -> bool {
    let Some(channels) = native_byte_layout(img, mode) else {
        return false;
    };
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    expected_len >= 16 && img.as_bytes().len() == expected_len
}

/// The affine Chops kernel consumes eight byte samples per `f64x8` block.
/// Keep its admission threshold separate from `u8x16` byte kernels so narrow
/// rows still execute a genuine vector data path instead of being needlessly
/// routed to CPU.
fn has_affine_vector_rows(img: &DynamicImage, channels: usize) -> bool {
    has_empty_native_bytes(img, channels)
        || (img.height() != 0
            && img
                .width()
                .checked_mul(channels as u32)
                .is_some_and(|row_bytes| row_bytes >= 8))
}

/// Multiply and Screen consume eight independent byte samples in an
/// `f64x8` block. Keep their admission threshold separate from the
/// sixteen-byte bytewise kernels so an 8-pixel `L` row or a 4-pixel `LA` row
/// still takes a real vector data path.
fn has_blend_vector_rows(img: &DynamicImage, channels: usize) -> bool {
    has_affine_vector_rows(img, channels)
}

fn has_visible_draw_rectangle(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    outline_width: u32,
) -> bool {
    if !valid_draw_rectangle(width, height, x0, y0, x1, y1) {
        return false;
    }
    let has_ink = fill.is_some() || (outline.is_some() && outline_width != 0);
    has_ink
        && i64::from(x1) >= 0
        && i64::from(y1) >= 0
        && i64::from(x0) < i64::from(width)
        && i64::from(y0) < i64::from(height)
}

fn valid_draw_rectangle(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> bool {
    width != 0 && height != 0 && x1 >= x0 && y1 >= y0
}

#[inline]
fn record_native_row_work(width: usize, height: usize, channels: usize) {
    let row_bytes = width.saturating_mul(channels);
    let vector_blocks = (row_bytes / 16).saturating_mul(height);
    let scalar_tail = (row_bytes % 16).saturating_mul(height);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }
}

fn valid_convolution_parameters(kernel: &[f32], scale: f32) -> bool {
    scale.is_finite() && scale != 0.0 && kernel.iter().all(|value| value.is_finite())
}

fn put_alpha_data_shape(
    img: &DynamicImage,
    mask: &DynamicImage,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> Option<(usize, usize, usize, bool)> {
    if !matches!(mask, DynamicImage::ImageLuma8(_)) || img.dimensions() != mask.dimensions() {
        return None;
    }
    let (source_channels, output_channels, pixels_per_vector, cmyk_source) = match alpha_mode {
        PixelMode::L | PixelMode::P => (1, 2, 8, false),
        PixelMode::LA | PixelMode::PA => (2, 2, 8, false),
        PixelMode::RGB => (3, 4, 4, false),
        PixelMode::RGBA => (4, 4, 4, false),
        // CMYK is stored as four native bytes, but Pillow promotes it to
        // RGBA while inserting an image-backed alpha channel. The data-plane
        // kernel handles that conversion with vector arithmetic below.
        PixelMode::CMYK => (4, 4, 4, true),
        PixelMode::Mode1
        | PixelMode::YCbCr
        | PixelMode::HSV
        | PixelMode::I
        | PixelMode::F => return None,
    };
    let mode_matches = match alpha_mode {
        // `Image.putalpha_data` queues a P source with PixelMode::P but
        // `Image::push_op` promotes its resulting logical tag to PA before
        // execution. Accept both names at this byte-layout boundary.
        PixelMode::P => matches!(mode, Some("P") | Some("PA")),
        PixelMode::PA => mode == Some("PA"),
        PixelMode::L => matches!(mode, None | Some("L")),
        PixelMode::LA => matches!(mode, None | Some("LA")),
        PixelMode::RGB => matches!(mode, None | Some("RGB")),
        PixelMode::RGBA => matches!(mode, None | Some("RGBA" | "RGBX")),
        // `push_op` clears the CMYK tag because the result is promoted to
        // ordinary RGBA. The operation's `alpha_mode` remains the source of
        // truth, so a missing execution tag is valid here.
        PixelMode::CMYK => matches!(mode, None | Some("CMYK")),
        _ => false,
    };
    if !mode_matches {
        return None;
    }
    let source_matches = match (alpha_mode, img) {
        (PixelMode::L | PixelMode::P, DynamicImage::ImageLuma8(_)) => true,
        (PixelMode::LA | PixelMode::PA, DynamicImage::ImageLumaA8(_)) => true,
        (PixelMode::RGB, DynamicImage::ImageRgb8(_)) => true,
        (PixelMode::RGBA | PixelMode::CMYK, DynamicImage::ImageRgba8(_)) => true,
        _ => false,
    };
    source_matches.then_some((
        source_channels,
        output_channels,
        pixels_per_vector,
        cmyk_source,
    ))
}

fn put_alpha_shape(
    img: &DynamicImage,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> Option<(usize, usize, usize, bool)> {
    let (source_channels, output_channels, pixels_per_vector, cmyk_source) = match alpha_mode {
        PixelMode::L | PixelMode::P => (1, 2, 8, false),
        PixelMode::LA | PixelMode::PA => (2, 2, 8, false),
        PixelMode::RGB => (3, 4, 4, false),
        PixelMode::RGBA => (4, 4, 4, false),
        PixelMode::CMYK => (4, 4, 4, true),
        PixelMode::Mode1
        | PixelMode::YCbCr
        | PixelMode::HSV
        | PixelMode::I
        | PixelMode::F => return None,
    };
    let mode_matches = match alpha_mode {
        // A paletted source is represented by one-byte indices before the
        // operation and by a two-byte PA sample layout after it.
        PixelMode::P => matches!(mode, Some("P") | Some("PA")),
        PixelMode::PA => mode == Some("PA"),
        PixelMode::L => matches!(mode, None | Some("L")),
        PixelMode::LA => matches!(mode, None | Some("LA")),
        PixelMode::RGB => matches!(mode, None | Some("RGB")),
        PixelMode::RGBA => matches!(mode, None | Some("RGBA" | "RGBX")),
        // putalpha promotes CMYK storage to ordinary RGBA and clears the
        // logical CMYK tag at the Image boundary.
        PixelMode::CMYK => matches!(mode, None | Some("CMYK")),
        _ => false,
    };
    if !mode_matches {
        return None;
    }
    let source_matches = match (alpha_mode, img) {
        (PixelMode::L | PixelMode::P, DynamicImage::ImageLuma8(_)) => true,
        (PixelMode::LA | PixelMode::PA, DynamicImage::ImageLumaA8(_)) => true,
        (PixelMode::RGB, DynamicImage::ImageRgb8(_)) => true,
        (PixelMode::RGBA | PixelMode::CMYK, DynamicImage::ImageRgba8(_)) => true,
        _ => false,
    };
    source_matches.then_some((
        source_channels,
        output_channels,
        pixels_per_vector,
        cmyk_source,
    ))
}

fn logical_byte_channels(mode: &str) -> Option<usize> {
    match mode {
        "1" | "L" | "P" => Some(1),
        "LA" | "PA" => Some(2),
        "RGB" | "HSV" | "YCbCr" => Some(3),
        "RGBA" | "CMYK" | "RGBa" | "RGBX" => Some(4),
        _ => None,
    }
}

/// Check the secondary image contract without decoding its pixels. Public
/// Chops constructors already validate these fields, but repeating the
/// cheap shape/layout check at SIMD preflight prevents a malformed internal
/// pipeline from reaching an adapter that would discover the mismatch after
/// execution began. `mode()` and `size()` use retained headers or pipeline
/// metadata whenever possible; they do not force a byte-buffer materialize for
/// a loaded secondary image.
fn simd_chops_operands_supported(
    img: &DynamicImage,
    other: &Image,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_chops_layout(img, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    let Ok(other_size) = other.size() else {
        return false;
    };
    logical_byte_channels(&other_mode)
        .is_some_and(|other_channels| channels == other_channels)
        && img.dimensions() == other_size
}

/// Check the module-level blend contract without materializing the secondary
/// image. Pillow validates the mode family and dimensions before touching
/// pixels; only a non-empty byte buffer needs the SIMD block-size gate.
fn simd_module_blend_supported(
    img: &DynamicImage,
    other: &Image,
    mode: Option<&str>,
    alpha: f64,
) -> bool {
    if !alpha.is_finite() {
        return false;
    }
    let Some(channels) = native_module_blend_layout(img, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    if native_module_blend_mode_channels(&other_mode) != Some(channels)
        || other.size().ok() != Some(img.dimensions())
    {
        return false;
    }
    native_module_blend_data_supported(img, channels)
}

#[derive(Clone, Copy)]
struct NativePasteLayout {
    channels: usize,
    mode: &'static str,
}

#[derive(Clone, Copy)]
struct NativePasteMaskLayout {
    channels: usize,
    value_index: usize,
}

#[derive(Clone, Copy)]
struct NativePasteRegion {
    source_left: usize,
    source_top: usize,
    destination_left: usize,
    destination_top: usize,
    width: usize,
    height: usize,
}

#[derive(Clone, Copy)]
struct NativePastePlan {
    layout: NativePasteLayout,
    mask: Option<NativePasteMaskLayout>,
    region: NativePasteRegion,
    source_width: usize,
    source_height: usize,
}

/// Return the native byte contract for a destination/source pair.
///
/// `CMYK` is physically represented by the four bytes of `Rgba8` in the
/// raster layer, but Paste treats all four samples as ordinary channels. It
/// therefore shares the same byte kernel without interpreting the fourth
/// sample as alpha. Indexed, typed, premultiplied, and color-space-tagged
/// modes also use this kernel when the source has the same logical mode and
/// no mask is present: in that case Paste moves native samples rather than
/// interpreting their color meaning. Masked variants remain restricted to
/// the modes whose alpha blending contract is implemented below.
fn native_paste_layout(img: &DynamicImage, mode: Option<&str>) -> Option<NativePasteLayout> {
    match (img, mode) {
        (DynamicImage::ImageLuma8(_), None | Some("L")) => Some(NativePasteLayout {
            channels: 1,
            mode: "L",
        }),
        (DynamicImage::ImageLuma8(_), Some("1")) => Some(NativePasteLayout {
            channels: 1,
            mode: "1",
        }),
        (DynamicImage::ImageLuma8(_), Some("P")) => Some(NativePasteLayout {
            channels: 1,
            mode: "P",
        }),
        (DynamicImage::ImageLumaA8(_), None | Some("LA")) => Some(NativePasteLayout {
            channels: 2,
            mode: "LA",
        }),
        (DynamicImage::ImageLumaA8(_), Some("PA")) => Some(NativePasteLayout {
            channels: 2,
            mode: "PA",
        }),
        (DynamicImage::ImageRgb8(_), None | Some("RGB")) => Some(NativePasteLayout {
            channels: 3,
            mode: "RGB",
        }),
        (DynamicImage::ImageRgb8(_), Some("HSV")) => Some(NativePasteLayout {
            channels: 3,
            mode: "HSV",
        }),
        (DynamicImage::ImageRgb8(_), Some("YCbCr")) => Some(NativePasteLayout {
            channels: 3,
            mode: "YCbCr",
        }),
        (DynamicImage::ImageRgba8(_), None | Some("RGBA")) => Some(NativePasteLayout {
            channels: 4,
            mode: "RGBA",
        }),
        (DynamicImage::ImageRgba8(_), Some("CMYK")) => Some(NativePasteLayout {
            channels: 4,
            mode: "CMYK",
        }),
        (DynamicImage::ImageRgba8(_), Some("RGBa")) => Some(NativePasteLayout {
            channels: 4,
            mode: "RGBa",
        }),
        (DynamicImage::ImageRgba8(_), Some("RGBX")) => Some(NativePasteLayout {
            channels: 4,
            mode: "RGBX",
        }),
        (DynamicImage::ImageRgba8(_), Some("I")) => Some(NativePasteLayout {
            channels: 4,
            mode: "I",
        }),
        (DynamicImage::ImageRgba8(_), Some("F")) => Some(NativePasteLayout {
            channels: 4,
            mode: "F",
        }),
        _ => None,
    }
}

fn native_paste_shape_layout(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
    match (shape.layout, mode) {
        (SimdLayout::Luma8, None | Some("L")) => Some(NativePasteLayout {
            channels: 1,
            mode: "L",
        }),
        (SimdLayout::Luma8, Some("1")) => Some(NativePasteLayout {
            channels: 1,
            mode: "1",
        }),
        (SimdLayout::Luma8, Some("P")) => Some(NativePasteLayout {
            channels: 1,
            mode: "P",
        }),
        (SimdLayout::LumaA8, None | Some("LA")) => Some(NativePasteLayout {
            channels: 2,
            mode: "LA",
        }),
        (SimdLayout::LumaA8, Some("PA")) => Some(NativePasteLayout {
            channels: 2,
            mode: "PA",
        }),
        (SimdLayout::Rgb8, None | Some("RGB")) => Some(NativePasteLayout {
            channels: 3,
            mode: "RGB",
        }),
        (SimdLayout::Rgb8, Some("HSV")) => Some(NativePasteLayout {
            channels: 3,
            mode: "HSV",
        }),
        (SimdLayout::Rgb8, Some("YCbCr")) => Some(NativePasteLayout {
            channels: 3,
            mode: "YCbCr",
        }),
        (SimdLayout::Rgba8, None | Some("RGBA")) => Some(NativePasteLayout {
            channels: 4,
            mode: "RGBA",
        }),
        (SimdLayout::Rgba8, Some("CMYK")) => Some(NativePasteLayout {
            channels: 4,
            mode: "CMYK",
        }),
        (SimdLayout::Rgba8, Some("RGBa")) => Some(NativePasteLayout {
            channels: 4,
            mode: "RGBa",
        }),
        (SimdLayout::Rgba8, Some("RGBX")) => Some(NativePasteLayout {
            channels: 4,
            mode: "RGBX",
        }),
        (SimdLayout::Rgba8, Some("I")) => Some(NativePasteLayout {
            channels: 4,
            mode: "I",
        }),
        (SimdLayout::Rgba8, Some("F")) => Some(NativePasteLayout {
            channels: 4,
            mode: "F",
        }),
        _ => None,
    }
}

fn native_paste_mask_layout(mode: &str, mask_alpha: bool) -> Option<NativePasteMaskLayout> {
    match (mode, mask_alpha) {
        ("1" | "L", false) => Some(NativePasteMaskLayout {
            channels: 1,
            value_index: 0,
        }),
        ("LA", true) => Some(NativePasteMaskLayout {
            channels: 2,
            value_index: 1,
        }),
        ("RGBA" | "RGBa", true) => Some(NativePasteMaskLayout {
            channels: 4,
            value_index: 3,
        }),
        _ => None,
    }
}

fn native_paste_region(
    destination_width: u32,
    destination_height: u32,
    source_width: u32,
    source_height: u32,
    x: i32,
    y: i32,
) -> Option<NativePasteRegion> {
    let x = i64::from(x);
    let y = i64::from(y);
    let source_width = i64::from(source_width);
    let source_height = i64::from(source_height);
    let destination_width = i64::from(destination_width);
    let destination_height = i64::from(destination_height);

    // `x` and `y` originate as i32 values, so negating them in i64 cannot
    // overflow. Clamping before the subtraction mirrors Paste.c's clipped
    // source/destination rectangles and handles coordinates outside either
    // image without creating a wrapped usize.
    let source_left = (-x).max(0).min(source_width) as usize;
    let source_top = (-y).max(0).min(source_height) as usize;
    let destination_left = x.max(0).min(destination_width) as usize;
    let destination_top = y.max(0).min(destination_height) as usize;
    let width = (source_width as usize)
        .saturating_sub(source_left)
        .min((destination_width as usize).saturating_sub(destination_left));
    let height = (source_height as usize)
        .saturating_sub(source_top)
        .min((destination_height as usize).saturating_sub(destination_top));
    (width != 0 && height != 0).then_some(NativePasteRegion {
        source_left,
        source_top,
        destination_left,
        destination_top,
        width,
        height,
    })
}

/// Overlay and HardLight use eight `u32` lanes, so their minimum useful
/// frame is one eight-byte block rather than one sixteen-byte byte-vector.
fn has_vectorized_lut_bytes(img: &DynamicImage, channels: usize) -> bool {
    if img.width() == 0 || img.height() == 0 {
        return false;
    }
    img.width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|total_bytes| {
            total_bytes >= 8 && img.as_bytes().len() == total_bytes as usize
        })
}

fn native_paste_plan_from_layout(
    destination_width: u32,
    destination_height: u32,
    layout: NativePasteLayout,
    source: &Image,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mask_alpha: bool,
) -> Option<NativePastePlan> {
    let source_width = u32::try_from(width).ok()?;
    let source_height = u32::try_from(height).ok()?;
    if source.mode().ok()?.as_str() != layout.mode
        || source.size().ok()? != (source_width, source_height)
    {
        return None;
    }
    let mask_layout = match mask {
        Some(mask) => {
            // These modes can be copied byte-for-byte, but their masked
            // Pillow contracts are not the same as L/LA/RGB/RGBA blending.
            // Reject them before execution rather than silently applying an
            // RGBA-shaped alpha formula to indexed, typed, or tagged data.
            if !matches!(layout.mode, "L" | "LA" | "RGB" | "RGBA" | "CMYK") {
                return None;
            }
            let mask_mode = mask.mode().ok()?;
            let mask_layout = native_paste_mask_layout(&mask_mode, mask_alpha)?;
            if mask.size().ok()? != (source_width, source_height) {
                return None;
            }
            Some(mask_layout)
        }
        None => None,
    };
    let region = native_paste_region(
        destination_width,
        destination_height,
        source_width,
        source_height,
        x,
        y,
    )?;
    let row_bytes = region.width.checked_mul(layout.channels)?;
    // An unmasked paste is a byte copy, so even a short row can use the
    // padded u8x16 tail path below.  Requiring a complete 16-byte row here
    // made valid padded Image.crop pipelines report SIMD unsupported for
    // small clipped regions.  Masked blending retains its eight-byte
    // minimum because its narrow vector kernel has a different contract.
    if mask.is_some() && row_bytes < 8 {
        return None;
    }
    Some(NativePastePlan {
        layout,
        mask: mask_layout,
        region,
        source_width: source_width as usize,
        source_height: source_height as usize,
    })
}

fn native_paste_plan_for_image(
    img: &DynamicImage,
    source: &Arc<Image>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Option<NativePastePlan> {
    let layout = native_paste_layout(img, mode)?;
    native_paste_plan_from_layout(
        img.width(),
        img.height(),
        layout,
        source,
        x,
        y,
        width,
        height,
        mask,
        mask_alpha,
    )
}

fn native_paste_plan_for_shape(
    shape: SimdImageShape,
    source: &Arc<Image>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Option<NativePastePlan> {
    let layout = native_paste_shape_layout(shape, mode)?;
    native_paste_plan_from_layout(
        shape.width,
        shape.height,
        layout,
        source,
        x,
        y,
        width,
        height,
        mask,
        mask_alpha,
    )
}

/// Build the native byte contract for `Image.composite` and
/// `ImageChops.composite`.
///
/// Composite is a paste at `(0, 0)` whose destination is the second image.
/// The public constructor has already converted the first image to that
/// destination mode, so this plan can keep both operands in their native
/// storage and share the masked blend kernel with Paste. Unlike the general
/// Paste planner, the composite contract also admits indexed `P`/`PA` output:
/// Pillow blends those stored samples rather than expanding the palette.
fn native_composite_plan_for_image(
    img: &DynamicImage,
    other: &Image,
    mask: &Arc<Image>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Option<NativePastePlan> {
    let layout = native_paste_layout(img, mode)?;
    let (destination_width, destination_height) = other.size().ok()?;
    if other.mode().ok()?.as_str() != layout.mode {
        return None;
    }
    let mask_mode = mask.mode().ok()?;
    let mask_layout = native_paste_mask_layout(&mask_mode, mask_alpha)?;
    if mask.size().ok()? != img.dimensions() {
        return None;
    }
    let region = native_paste_region(
        destination_width,
        destination_height,
        img.width(),
        img.height(),
        0,
        0,
    )?;
    Some(NativePastePlan {
        layout,
        mask: Some(mask_layout),
        region,
        source_width: img.width() as usize,
        source_height: img.height() as usize,
    })
}

fn native_composite_plan_for_shape(
    shape: SimdImageShape,
    other: &Image,
    mask: &Arc<Image>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Option<NativePastePlan> {
    let layout = native_paste_shape_layout(shape, mode)?;
    let (destination_width, destination_height) = other.size().ok()?;
    if other.mode().ok()?.as_str() != layout.mode {
        return None;
    }
    let mask_mode = mask.mode().ok()?;
    let mask_layout = native_paste_mask_layout(&mask_mode, mask_alpha)?;
    if mask.size().ok()? != (shape.width, shape.height) {
        return None;
    }
    let region = native_paste_region(
        destination_width,
        destination_height,
        shape.width,
        shape.height,
        0,
        0,
    )?;
    Some(NativePastePlan {
        layout,
        mask: Some(mask_layout),
        region,
        source_width: shape.width as usize,
        source_height: shape.height as usize,
    })
}

fn native_put_data_layout(
    img: &DynamicImage,
    data_mode: PixelMode,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
    let logical_mode = pixel_mode_name(data_mode);
    let layout = native_paste_layout(img, mode)
        .or_else(|| native_paste_layout(img, Some(logical_mode)))?;
    (layout.channels == data_mode.channels()).then_some(layout)
}

fn native_put_data_shape_layout(
    shape: SimdImageShape,
    data_mode: PixelMode,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
    let logical_mode = pixel_mode_name(data_mode);
    let layout = native_paste_shape_layout(shape, mode)
        .or_else(|| native_paste_shape_layout(shape, Some(logical_mode)))?;
    (layout.channels == data_mode.channels()).then_some(layout)
}

fn native_paste_actual_layout(image: &DynamicImage, layout: NativePasteLayout) -> bool {
    native_paste_layout(image, Some(layout.mode)).is_some_and(|actual| {
        actual.channels == layout.channels
            && image
                .width()
                .checked_mul(image.height())
                .and_then(|pixels| pixels.checked_mul(layout.channels as u32))
                .is_some_and(|bytes| image.as_bytes().len() == bytes as usize)
    })
}

fn native_paste_actual_mask_layout(
    image: &DynamicImage,
    layout: NativePasteMaskLayout,
) -> bool {
    let channels = match image {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return false,
    };
    channels == layout.channels
        && image
            .width()
            .checked_mul(image.height())
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| image.as_bytes().len() == bytes as usize)
}

/// Gather one mask value per stored source byte. `wide` has no portable
/// byte-gather primitive, so this address calculation is scalar control work;
/// the source/destination arithmetic remains in the vector kernel below.
#[inline]
fn native_paste_mask_block<const N: usize>(
    mask: &[u8],
    mask_row_stride: usize,
    source_y: usize,
    source_left: usize,
    byte_start: usize,
    source_channels: usize,
    mask_layout: NativePasteMaskLayout,
) -> [u8; N] {
    std::array::from_fn(|lane| {
        let source_byte = byte_start + lane;
        let source_pixel = source_byte / source_channels;
        mask[source_y * mask_row_stride
            + (source_left + source_pixel) * mask_layout.channels
            + mask_layout.value_index]
    })
}

#[inline]
fn native_paste_mask_block_active<const N: usize>(
    mask: &[u8],
    mask_row_stride: usize,
    source_y: usize,
    source_left: usize,
    byte_start: usize,
    active_bytes: usize,
    source_channels: usize,
    mask_layout: NativePasteMaskLayout,
) -> [u8; N] {
    std::array::from_fn(|lane| {
        let source_byte = byte_start + lane;
        if source_byte >= active_bytes {
            return 0;
        }
        let source_pixel = source_byte / source_channels;
        mask[source_y * mask_row_stride
            + (source_left + source_pixel) * mask_layout.channels
            + mask_layout.value_index]
    })
}

#[inline]
fn native_paste_blend_vector16(
    source: [u8; 16],
    destination: [u8; 16],
    mask: [u8; 16],
) -> [u8; 16] {
    let source = u16x16::from(u8x16::new(source));
    let destination = u16x16::from(u8x16::new(destination));
    let mask = u16x16::from(u8x16::new(mask));
    let weighted = source * mask
        + destination * (u16x16::splat(255) - mask)
        + u16x16::splat(127);
    simd_pack_u16x16(simd_div255(weighted)).to_array()
}

#[inline]
fn native_paste_blend_vector8(
    source: [u8; 8],
    destination: [u8; 8],
    mask: [u8; 8],
) -> [u8; 8] {
    let source = u16x8::new(source.map(u16::from));
    let destination = u16x8::new(destination.map(u16::from));
    let mask = u16x8::new(mask.map(u16::from));
    let weighted = source * mask
        + destination * (u16x8::splat(255) - mask)
        + u16x8::splat(127);
    simd_div255_u16x8(weighted).to_array().map(|value| value as u8)
}

fn native_paste_apply(
    destination: &mut [u8],
    source: &[u8],
    mask: Option<&[u8]>,
    destination_width: usize,
    destination_height: usize,
    plan: NativePastePlan,
    allow_short_masked_tail: bool,
) -> bool {
    let Some(destination_row_stride) = destination_width.checked_mul(plan.layout.channels) else {
        return false;
    };
    let Some(source_row_stride) = plan.source_width.checked_mul(plan.layout.channels) else {
        return false;
    };
    let Some(destination_bytes) = destination_row_stride.checked_mul(destination_height) else {
        return false;
    };
    let Some(source_bytes) = source_row_stride.checked_mul(plan.source_height) else {
        return false;
    };
    if destination.len() != destination_bytes || source.len() != source_bytes {
        return false;
    }

    let mask_layout = match (plan.mask, mask) {
        (Some(layout), Some(mask)) => {
            let Some(mask_row_stride) = plan.source_width.checked_mul(layout.channels) else {
                return false;
            };
            let Some(mask_bytes) = mask_row_stride.checked_mul(plan.source_height) else {
                return false;
            };
            if mask.len() != mask_bytes {
                return false;
            }
            Some((layout, mask_row_stride))
        }
        (None, None) => None,
        _ => return false,
    };

    let region = plan.region;
    let Some(region_row_bytes) = region.width.checked_mul(plan.layout.channels) else {
        return false;
    };
    if region.source_left.saturating_add(region.width) > plan.source_width
        || region.source_top.saturating_add(region.height) > plan.source_height
        || region.destination_left.saturating_add(region.width) > destination_width
        || region.destination_top.saturating_add(region.height) > destination_height
    {
        return false;
    }

    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in 0..region.height {
        let source_row_start = (region.source_top + row) * source_row_stride
            + region.source_left * plan.layout.channels;
        let destination_row_start = (region.destination_top + row) * destination_row_stride
            + region.destination_left * plan.layout.channels;
        let source_row = &source[source_row_start..source_row_start + region_row_bytes];
        let destination_row =
            &mut destination[destination_row_start..destination_row_start + region_row_bytes];
        match mask_layout {
            None => {
                let Some((blocks, tail)) = copy_native_bytes(source_row, destination_row) else {
                    return false;
                };
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
            Some((mask_layout, mask_row_stride)) => {
                let mask = mask.expect("a mask layout always has mask bytes");
                let vector_len = region_row_bytes / 16 * 16;
                for start in (0..vector_len).step_by(16) {
                    let source_block = <[u8; 16]>::try_from(&source_row[start..start + 16])
                        .expect("native Paste blend block has 16 bytes");
                    let destination_block =
                        <[u8; 16]>::try_from(&destination_row[start..start + 16])
                            .expect("native Paste destination block has 16 bytes");
                    let mask_block = native_paste_mask_block::<16>(
                        mask,
                        mask_row_stride,
                        region.source_top + row,
                        region.source_left,
                        start,
                        plan.layout.channels,
                        mask_layout,
                    );
                    destination_row[start..start + 16].copy_from_slice(
                        &native_paste_blend_vector16(
                            source_block,
                            destination_block,
                            mask_block,
                        ),
                    );
                    vector_blocks = vector_blocks.saturating_add(1);
                }
                let vector_len8 = region_row_bytes / 8 * 8;
                for start in (vector_len..vector_len8).step_by(8) {
                    let source_block = <[u8; 8]>::try_from(&source_row[start..start + 8])
                        .expect("native Paste blend block has 8 bytes");
                    let destination_block =
                        <[u8; 8]>::try_from(&destination_row[start..start + 8])
                            .expect("native Paste destination block has 8 bytes");
                    let mask_block = native_paste_mask_block::<8>(
                        mask,
                        mask_row_stride,
                        region.source_top + row,
                        region.source_left,
                        start,
                        plan.layout.channels,
                        mask_layout,
                    );
                    destination_row[start..start + 8]
                        .copy_from_slice(&native_paste_blend_vector8(
                            source_block,
                            destination_block,
                            mask_block,
                        ));
                    vector_blocks = vector_blocks.saturating_add(1);
                }
                let tail = region_row_bytes - vector_len8;
                if tail != 0 && allow_short_masked_tail {
                    // A partial row still uses the same vector blend. Pad
                    // only the inactive lanes so tiny composite rows (for
                    // example a 3-byte P row) never fall back to scalar pixel
                    // arithmetic or read past the source/mask row.
                    let mut source_block = [0u8; 8];
                    let mut destination_block = [0u8; 8];
                    source_block[..tail].copy_from_slice(&source_row[vector_len8..]);
                    destination_block[..tail]
                        .copy_from_slice(&destination_row[vector_len8..]);
                    let mask_block = native_paste_mask_block_active::<8>(
                        mask,
                        mask_row_stride,
                        region.source_top + row,
                        region.source_left,
                        vector_len8,
                        region_row_bytes,
                        plan.layout.channels,
                        mask_layout,
                    );
                    let blended = native_paste_blend_vector8(
                        source_block,
                        destination_block,
                        mask_block,
                    );
                    destination_row[vector_len8..].copy_from_slice(&blended[..tail]);
                    vector_blocks = vector_blocks.saturating_add(1);
                    scalar_tail = scalar_tail.saturating_add(tail as u64);
                } else {
                    for index in vector_len8..region_row_bytes {
                        let source_pixel = index / plan.layout.channels;
                        let mask_value = mask[(region.source_top + row) * mask_row_stride
                            + (region.source_left + source_pixel) * mask_layout.channels
                            + mask_layout.value_index];
                        let source_value = source_row[index];
                        let destination_value = destination_row[index];
                        let mask = u16::from(mask_value);
                        destination_row[index] = ((u16::from(source_value) * mask
                            + u16::from(destination_value) * (255 - mask)
                            + 127)
                            / 255) as u8;
                        scalar_tail = scalar_tail.saturating_add(1);
                    }
                }
            }
        }
    }

    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::compute::record_pipeline_operation_path(if plan.mask.is_some() {
        "vector"
    } else {
        "native-copy"
    });
    true
}

pub fn simd_paste(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Paste {
        source,
        x,
        y,
        w,
        h,
        mask,
        mask_alpha,
    } = op
    else {
        return Err(PilError::ValueError("expected Paste op".into()));
    };
    let plan = native_paste_plan_for_image(
        img,
        source,
        *x,
        *y,
        *w,
        *h,
        mask.as_ref(),
        *mask_alpha,
        mode,
    )
    .ok_or_else(|| simd_unsupported("Paste"))?;
    let source_image = source.materialized_shared()?;
    if !native_paste_actual_layout(source_image.as_ref(), plan.layout) {
        return Err(simd_unsupported("Paste"));
    }
    let mask_image = match mask {
        Some(mask) => {
            let mask_image = mask.materialized_shared()?;
            let Some(mask_layout) = plan.mask else {
                return Err(simd_unsupported("Paste"));
            };
            if !native_paste_actual_mask_layout(mask_image.as_ref(), mask_layout) {
                return Err(simd_unsupported("Paste"));
            }
            Some(mask_image)
        }
        None => None,
    };
    let mut output = img.as_bytes().to_vec();
    if !native_paste_apply(
        &mut output,
        source_image.as_bytes(),
        mask_image.as_ref().map(|image| image.as_bytes()),
        img.width() as usize,
        img.height() as usize,
        plan,
        false,
    ) {
        return Err(simd_unsupported("Paste"));
    }
    let result = crate::image_utils::raw_bytes_to_image(
        img.width(),
        img.height(),
        output,
        plan.layout.channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Composite two images through their native byte planes.
///
/// Pillow implements composite as a copy of `image2` followed by a masked
/// paste of `image1` at the origin. The output copy and the masked arithmetic
/// both stay on the host in native storage; no packed-RGBA conversion or CPU
/// adapter is used. A padded final vector handles narrow rows, which is
/// important for indexed images and small public inputs.
pub fn simd_composite_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::CompositeModule {
        other,
        mask,
        mask_alpha,
    } = op
    else {
        return Err(PilError::ValueError(
            "expected CompositeModule op".into(),
        ));
    };
    let plan = native_composite_plan_for_image(img, other, mask, *mask_alpha, mode)
        .ok_or_else(|| simd_unsupported("CompositeModule"))?;
    let destination = other.materialized_shared()?;
    if !native_paste_actual_layout(img, plan.layout)
        || !native_paste_actual_layout(destination.as_ref(), plan.layout)
        || destination.dimensions() != other.size()?
    {
        return Err(simd_unsupported("CompositeModule"));
    }
    let mask_image = mask.materialized_shared()?;
    let mask_layout = plan
        .mask
        .ok_or_else(|| simd_unsupported("CompositeModule"))?;
    if !native_paste_actual_mask_layout(mask_image.as_ref(), mask_layout)
        || mask_image.dimensions() != img.dimensions()
    {
        return Err(simd_unsupported("CompositeModule"));
    }

    // Composite starts from an image2 copy. Allocate the output once, then
    // copy the secondary image through the native vector type so this
    // required semantic copy is visible in execution telemetry.
    let mut output = vec![0u8; destination.as_bytes().len()];
    let (copy_blocks, copy_tail) = copy_native_bytes(destination.as_bytes(), &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD composite buffer shape mismatch".into()))?;
    if !native_paste_apply(
        &mut output,
        img.as_bytes(),
        Some(mask_image.as_bytes()),
        destination.width() as usize,
        destination.height() as usize,
        plan,
        true,
    ) {
        return Err(simd_unsupported("CompositeModule"));
    }
    crate::compute::record_pipeline_operation_vector_blocks(copy_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(copy_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        destination.width(),
        destination.height(),
        output,
        plan.layout.channels,
    )?;
    Ok(preserve_mode(destination.as_ref(), result))
}

/// Replace the canonical `putdata` prefix through the image's native byte
/// layout. The binding/core boundary has already normalized scalar and tuple
/// values into `data`; the SIMD adapter only performs the defined byte copy,
/// preserving untouched pixels when Pillow receives a short sequence.
pub fn simd_put_data(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutData {
        data,
        mode: data_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected PutData op".into()));
    };
    let layout = native_put_data_layout(img, *data_mode, mode)
        .ok_or_else(|| simd_unsupported("PutData"))?;
    if !native_paste_actual_layout(img, layout) {
        return Err(simd_unsupported("PutData"));
    }
    let expected_len = img
        .width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(layout.channels as u32))
        .map(|bytes| bytes as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutData byte count overflow".into()))?;
    if img.as_bytes().len() != expected_len {
        return Err(simd_unsupported("PutData"));
    }

    let mut output = vec![0u8; expected_len];
    let (mut vector_blocks, mut scalar_tail) = copy_native_bytes(img.as_bytes(), &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD PutData buffer shape mismatch".into()))?;
    let copy_len = data.len().min(expected_len);
    if copy_len != 0 {
        let (blocks, tail) = copy_native_bytes(&data[..copy_len], &mut output[..copy_len])
            .ok_or_else(|| PilError::InternalError("SIMD PutData prefix shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        img.width(),
        img.height(),
        output,
        layout.channels,
    )?;
    Ok(preserve_mode(img, result))
}

fn native_paste_in_place(
    img: &mut DynamicImage,
    source: &Arc<Image>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    let plan = native_paste_plan_for_image(
        img,
        source,
        x,
        y,
        width,
        height,
        mask,
        mask_alpha,
        mode,
    );
    let Some(plan) = plan else {
        return Ok(false);
    };
    let source_image = source.materialized_shared()?;
    if !native_paste_actual_layout(source_image.as_ref(), plan.layout) {
        return Ok(false);
    }
    let mask_image = match mask {
        Some(mask) => {
            let mask_image = mask.materialized_shared()?;
            let Some(mask_layout) = plan.mask else {
                return Ok(false);
            };
            if !native_paste_actual_mask_layout(mask_image.as_ref(), mask_layout) {
                return Ok(false);
            }
            Some(mask_image)
        }
        None => None,
    };
    let destination_width = img.width() as usize;
    let destination_height = img.height() as usize;
    let Some(output) = img.as_bytes_mut() else {
        return Ok(false);
    };
    Ok(native_paste_apply(
        output,
        source_image.as_bytes(),
        mask_image.as_ref().map(|image| image.as_bytes()),
        destination_width,
        destination_height,
        plan,
        false,
    ))
}

fn gaussian_blur_radius(sigma: f32) -> Option<f32> {
    if !sigma.is_finite() || sigma <= 0.0 {
        return None;
    }
    let sigma = f64::from(sigma);
    let sigma2 = sigma * sigma / 3.0;
    let l = ((12.0 * sigma2 + 1.0).sqrt() - 1.0) / 2.0;
    let l = l.floor();
    let l1 = l + 1.0;
    let denominator = 6.0 * (sigma2 - l1 * l1);
    if denominator == 0.0 {
        return None;
    }
    let numerator = (2.0 * l + 1.0) * (l * l1 - 3.0 * sigma2);
    let radius = (l + numerator / denominator) as f32;
    radius.is_finite().then_some(radius)
}

/// Return the native byte layout accepted by the all-channel inversion path.
fn native_invert_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Return the inclusive pixel span of a horizontal line after clipping it to
/// the destination.  Geometry is deliberately kept scalar: the SIMD kernel
/// below only owns the contiguous native-byte span once this control-plane
/// check has succeeded.
fn clipped_horizontal_span(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> Option<(usize, usize, usize)> {
    if width == 0 || height == 0 || y0 != y1 || y0 < 0 || y0 >= height as i32 {
        return None;
    }
    let last_x = i64::from(width) - 1;
    let start = i64::from(x0).min(i64::from(x1)).max(0);
    let end = i64::from(x0).max(i64::from(x1)).min(last_x);
    (start <= end).then_some((start as usize, end as usize, y0 as usize))
}

fn line_bounds_intersect(
    width: u32,
    height: u32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
) -> bool {
    let min_x = i64::from(x0.min(x1));
    let max_x = i64::from(x0.max(x1));
    let min_y = i64::from(y0.min(y1));
    let max_y = i64::from(y0.max(y1));
    width != 0
        && height != 0
        && max_x >= 0
        && max_y >= 0
        && min_x < i64::from(width)
        && min_y < i64::from(height)
}

fn has_visible_draw_point(width: u32, height: u32, points: &[(i32, i32)]) -> bool {
    points.iter().any(|&(x, y)| {
        x >= 0
            && y >= 0
            && i64::from(x) < i64::from(width)
            && i64::from(y) < i64::from(height)
    })
}

fn draw_line_source_byte(fill: (u8, u8, u8, u8), channels: usize, byte: usize) -> u8 {
    match channels {
        1 => fill.0,
        2 => match byte % 2 {
            0 => fill.0,
            _ => fill.3,
        },
        3 => match byte % 3 {
            0 => fill.0,
            1 => fill.1,
            _ => fill.2,
        },
        4 => match byte % 4 {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        },
        _ => 0,
    }
}

#[inline]
fn draw_line_blend_byte(source: u8, destination: u8, alpha: u8) -> u8 {
    let weighted = u16::from(source) * u16::from(alpha)
        + u16::from(destination) * u16::from(u8::MAX - alpha);
    ((u32::from(weighted) + 127) / 255) as u8
}

/// Apply one masked native-byte vector block for a horizontal line.
///
/// The mask makes the short-span case vectorized too: a 13-pixel L line in a
/// 16-byte row still executes one vector load/store while preserving the
/// neighbouring pixels.  RGB alpha blending uses the same exact rounded
/// formula as `NativeDrawCanvas`; the 16 lanes are byte channels, not packed
/// pixels, so a three-byte layout needs no RGBA widening.
#[inline]
fn draw_line_vector_block(
    row: &mut [u8],
    block_start: usize,
    target_start: usize,
    target_end: usize,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) {
    let input = <[u8; 16]>::try_from(&row[block_start..block_start + 16])
        .expect("draw line vector block has 16 bytes");
    let source = u8x16::new(std::array::from_fn(|lane| {
        draw_line_source_byte(fill, channels, block_start + lane)
    }));
    let transformed = if channels == 3 && alpha_blend_rgb {
        let destination = u16x16::from(u8x16::new(input));
        let source = u16x16::from(source);
        let alpha = u16x16::splat(u16::from(fill.3));
        let inverse_alpha = u16x16::splat(u16::from(u8::MAX - fill.3));
        let weighted = source * alpha + destination * inverse_alpha;
        simd_pack_u16x16(simd_div255(weighted + u16x16::splat(127)))
    } else {
        source
    };
    let mask = u8x16::new(std::array::from_fn(|lane| {
        let position = block_start + lane;
        if (target_start..target_end).contains(&position) {
            u8::MAX
        } else {
            0
        }
    }));
    let input = u8x16::new(input);
    let output = (transformed & mask) | (input & (u8x16::splat(u8::MAX) ^ mask));
    row[block_start..block_start + 16].copy_from_slice(&output.to_array());
}

fn simd_draw_line_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: (u8, u8, u8, u8),
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let Some(row_bytes) = (img.width() as usize).checked_mul(channels) else {
        return Ok(None);
    };
    // A real SIMD block must be available in the native row.  If the row is
    // narrower than one block, automatic routing selects CPU and strict SIMD
    // reports unsupported before this function is entered.
    if row_bytes < 16 {
        return Ok(None);
    }
    if width > 1 {
        let (image_width, image_height) = img.dimensions();
        let mut output = img.as_bytes().to_vec();
        let mut writer = SimdDrawSpanWriter {
            output: &mut output,
            image_width,
            image_height,
            channels,
            alpha_blend_rgb,
            vector_blocks: 0,
            scalar_tail: 0,
        };
        if let Some(points) = wide_line_polygon_points(x0, y0, x1, y1, width) {
            let mut first_error = None;
            for_each_polygon_fill_span(&points, image_width, image_height, |span_x0, span_x1, y| {
                if first_error.is_none()
                    && let Err(error) = writer.write(span_x0, y, span_x1, fill)
                {
                    first_error = Some(error);
                }
            });
            if let Some(error) = first_error {
                return Err(error);
            }
        } else {
            writer.write(x0, y0, x0, fill)?;
        }
        let vector_blocks = writer.vector_blocks;
        let scalar_tail = writer.scalar_tail;
        drop(writer);
        if vector_blocks == 0 {
            crate::compute::record_pipeline_operation_path("scalar-control");
            return Ok(Some(img.clone()));
        }
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels)
            .map(Some);
    }
    if y0 != y1 {
        return simd_draw_bresenham_native(
            img,
            x0,
            y0,
            x1,
            y1,
            channels,
            fill,
            alpha_blend_rgb,
        );
    }
    let Some((start_pixel, end_pixel, row_index)) =
        clipped_horizontal_span(img.width(), img.height(), x0, y0, x1, y1)
    else {
        return Ok(None);
    };
    let Some(target_start) = start_pixel.checked_mul(channels) else {
        return Ok(None);
    };
    let Some(target_end) = end_pixel
        .checked_add(1)
        .and_then(|pixel| pixel.checked_mul(channels))
    else {
        return Ok(None);
    };
    let Some(row_start) = row_index.checked_mul(row_bytes) else {
        return Ok(None);
    };
    let Some(row_end) = row_start.checked_add(row_bytes) else {
        return Ok(None);
    };
    let mut output = img.as_bytes().to_vec();
    let Some(row) = output.get_mut(row_start..row_end) else {
        return Ok(None);
    };

    let mut next = target_start;
    let mut vector_blocks = 0u64;
    while next.checked_add(16).is_some_and(|end| end <= target_end) {
        draw_line_vector_block(
            row,
            next,
            target_start,
            target_end,
            channels,
            fill,
            alpha_blend_rgb,
        );
        vector_blocks = vector_blocks.saturating_add(1);
        next += 16;
    }

    let mut scalar_tail = 0u64;
    if next < target_end {
        if vector_blocks == 0 {
            // Use a masked block for a span shorter than 16 bytes.  Select a
            // block wholly inside the row even when the span touches its end.
            let block_start = if target_start
                .checked_add(16)
                .is_some_and(|end| end <= row.len())
            {
                target_start
            } else {
                row.len() - 16
            };
            draw_line_vector_block(
                row,
                block_start,
                target_start,
                target_end,
                channels,
                fill,
                alpha_blend_rgb,
            );
            vector_blocks = 1;
        } else {
            for byte in next..target_end {
                let source = draw_line_source_byte(fill, channels, byte);
                row[byte] = if channels == 3 && alpha_blend_rgb {
                    draw_line_blend_byte(source, row[byte], fill.3)
                } else {
                    source
                };
                scalar_tail = scalar_tail.saturating_add(1);
            }
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .map(Some)
}

/// Fill one clipped horizontal span in a native drawing buffer.
///
/// The span bounds are scalar geometry output. Every complete sixteen-byte
/// block, including a masked block for a short span, is written by the same
/// native-byte vector kernel used by the line fast path. Only a trailing
/// partial block after a completed vector block is scalar.
fn simd_draw_span(
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    x0: i64,
    x1: i64,
    y: i64,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<(u64, u64), PilError> {
    if y < 0
        || y >= i64::from(image_height)
        || image_width == 0
        || x1 < 0
        || x0 >= i64::from(image_width)
    {
        return Ok((0, 0));
    }
    let start_pixel = x0.max(0) as usize;
    let end_pixel = x1.min(i64::from(image_width) - 1) as usize;
    if start_pixel > end_pixel {
        return Ok((0, 0));
    }
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    let row_start = (y as usize)
        .checked_mul(row_bytes)
        .ok_or_else(|| PilError::InternalError("SIMD draw row offset overflow".into()))?;
    let row_end = row_start
        .checked_add(row_bytes)
        .ok_or_else(|| PilError::InternalError("SIMD draw row end overflow".into()))?;
    let row = output
        .get_mut(row_start..row_end)
        .ok_or_else(|| PilError::InternalError("SIMD draw row buffer shape mismatch".into()))?;
    let target_start = start_pixel
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw span start overflow".into()))?;
    let target_end = end_pixel
        .checked_add(1)
        .and_then(|pixel| pixel.checked_mul(channels))
        .ok_or_else(|| PilError::InternalError("SIMD draw span end overflow".into()))?;

    let mut next = target_start;
    let mut vector_blocks = 0u64;
    while next
        .checked_add(16)
        .is_some_and(|block_end| block_end <= target_end)
    {
        draw_line_vector_block(
            row,
            next,
            target_start,
            target_end,
            channels,
            fill,
            alpha_blend_rgb,
        );
        vector_blocks = vector_blocks.saturating_add(1);
        next += 16;
    }

    let mut scalar_tail = 0u64;
    if next < target_end {
        if vector_blocks == 0 {
            let block_start = if target_start
                .checked_add(16)
                .is_some_and(|block_end| block_end <= row.len())
            {
                target_start
            } else {
                row.len().checked_sub(16).ok_or_else(|| {
                    PilError::InternalError("SIMD draw row has no vector block".into())
                })?
            };
            draw_line_vector_block(
                row,
                block_start,
                target_start,
                target_end,
                channels,
                fill,
                alpha_blend_rgb,
            );
            vector_blocks = 1;
        } else {
            for byte in next..target_end {
                let source = draw_line_source_byte(fill, channels, byte);
                row[byte] = if channels == 3 && alpha_blend_rgb {
                    draw_line_blend_byte(source, row[byte], fill.3)
                } else {
                    source
                };
                scalar_tail = scalar_tail.saturating_add(1);
            }
        }
    }
    Ok((vector_blocks, scalar_tail))
}

fn flush_simd_draw_span(
    span: &mut Option<(i64, i64, i64, i64)>,
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    vector_blocks: &mut u64,
    scalar_tail: &mut u64,
) -> Result<(), PilError> {
    let Some((x0, x1, _last_x, y)) = span.take() else {
        return Ok(());
    };
    let (vectors, tail) = simd_draw_span(
        output,
        image_width,
        image_height,
        x0,
        x1,
        y,
        channels,
        fill,
        alpha_blend_rgb,
    )?;
    *vector_blocks = vector_blocks.saturating_add(vectors);
    *scalar_tail = scalar_tail.saturating_add(tail);
    Ok(())
}

fn push_simd_draw_point(
    span: &mut Option<(i64, i64, i64, i64)>,
    x: i64,
    y: i64,
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    vector_blocks: &mut u64,
    scalar_tail: &mut u64,
) -> Result<(), PilError> {
    let extends_run = span.as_ref().is_some_and(|(_x0, _x1, last_x, run_y)| {
        *run_y == y && (x - *last_x).abs() == 1
    });
    if extends_run {
        if let Some((run_x0, run_x1, last_x, _run_y)) = span.as_mut() {
            *run_x0 = (*run_x0).min(x);
            *run_x1 = (*run_x1).max(x);
            *last_x = x;
        }
        return Ok(());
    }
    flush_simd_draw_span(
        span,
        output,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        vector_blocks,
        scalar_tail,
    )?;
    *span = Some((x, x, x, y));
    Ok(())
}

/// Draw a width-one Bresenham line with scalar geometry and native-byte SIMD
/// spans.  Consecutive pixels on the same raster row are coalesced into one
/// contiguous span; steep lines remain a sequence of one-pixel masked vector
/// stores rather than entering the scalar CPU draw adapter.
fn simd_draw_bresenham_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut span = None;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    let mut x = i64::from(x0);
    let mut y = i64::from(y0);
    let target_x = i64::from(x1);
    let target_y = i64::from(y1);
    let mut dx = target_x - x;
    let step_x = if dx < 0 {
        dx = -dx;
        -1
    } else {
        1
    };
    let mut dy = target_y - y;
    let step_y = if dy < 0 {
        dy = -dy;
        -1
    } else {
        1
    };

    if dx == 0 {
        for _ in 0..dy {
            push_simd_draw_point(
                &mut span,
                x,
                y,
                &mut output,
                image_width,
                image_height,
                channels,
                fill,
                alpha_blend_rgb,
                &mut vector_blocks,
                &mut scalar_tail,
            )?;
            y += step_y;
        }
    } else if dy == 0 {
        for _ in 0..dx {
            push_simd_draw_point(
                &mut span,
                x,
                y,
                &mut output,
                image_width,
                image_height,
                channels,
                fill,
                alpha_blend_rgb,
                &mut vector_blocks,
                &mut scalar_tail,
            )?;
            x += step_x;
        }
    } else if dx > dy {
        let steps = dx;
        dy += dy;
        let mut error = dy - dx;
        dx += dx;
        for _ in 0..steps {
            push_simd_draw_point(
                &mut span,
                x,
                y,
                &mut output,
                image_width,
                image_height,
                channels,
                fill,
                alpha_blend_rgb,
                &mut vector_blocks,
                &mut scalar_tail,
            )?;
            if error >= 0 {
                y += step_y;
                error -= dx;
            }
            error += dy;
            x += step_x;
        }
    } else {
        let steps = dy;
        dx += dx;
        let mut error = dx - dy;
        dy += dy;
        for _ in 0..steps {
            push_simd_draw_point(
                &mut span,
                x,
                y,
                &mut output,
                image_width,
                image_height,
                channels,
                fill,
                alpha_blend_rgb,
                &mut vector_blocks,
                &mut scalar_tail,
            )?;
            if error >= 0 {
                x += step_x;
                error -= dy;
            }
            error += dx;
            y += step_y;
        }
    }
    push_simd_draw_point(
        &mut span,
        target_x,
        target_y,
        &mut output,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        &mut vector_blocks,
        &mut scalar_tail,
    )?;
    flush_simd_draw_span(
        &mut span,
        &mut output,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        &mut vector_blocks,
        &mut scalar_tail,
    )?;

    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

/// Draw points through the native-byte masked vector kernel.  Adjacent points
/// on one row are coalesced so a point cloud that forms a span still gets wide
/// vector stores; isolated points use a masked vector block and never call the
/// scalar CPU drawing adapter.
fn simd_draw_points_native(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
    channels: usize,
) -> Result<Option<DynamicImage>, PilError> {
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if has_visible_draw_point(image_width, image_height, points) && row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut span = None;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for &(x, y) in points {
        push_simd_draw_point(
            &mut span,
            i64::from(x),
            i64::from(y),
            &mut output,
            image_width,
            image_height,
            channels,
            fill,
            alpha_blend_rgb,
            &mut vector_blocks,
            &mut scalar_tail,
        )?;
    }
    flush_simd_draw_span(
        &mut span,
        &mut output,
        image_width,
        image_height,
        channels,
        fill,
        alpha_blend_rgb,
        &mut vector_blocks,
        &mut scalar_tail,
    )?;

    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

/// Set one validated pixel through a masked native-byte vector store.  The
/// surrounding pixels are preserved, so this keeps `P`, `CMYK`, and the
/// ordinary byte layouts in their logical storage without packed RGBA
/// conversion.  Palette resolution and coordinate validation happen before
/// the pipeline operation is queued.
fn simd_put_pixel_native(
    img: &DynamicImage,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
    channels: usize,
) -> Result<Option<DynamicImage>, PilError> {
    if x >= img.width() || y >= img.height() {
        return Ok(None);
    }
    let row_bytes = (img.width() as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel row stride overflow".into()))?;
    let target_start = (y as usize)
        .checked_mul(row_bytes)
        .and_then(|row| row.checked_add((x as usize).checked_mul(channels)?))
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel offset overflow".into()))?;
    let target_end = target_start
        .checked_add(channels)
        .ok_or_else(|| PilError::InternalError("SIMD PutPixel end overflow".into()))?;
    let mut output = img.as_bytes().to_vec();
    if target_end > output.len() {
        return Ok(None);
    }
    // A single pixel can sit in a row narrower than one vector.  Use a
    // masked block over the flat native buffer instead of rejecting the
    // operation: bytes from the adjacent row are loaded and stored
    // unchanged, while only this pixel's channel lanes are enabled.  The
    // channel pattern is based on the global byte offset, so crossing a row
    // boundary does not change the logical sample mapping.
    if output.len() < 16 {
        let mut block = [0u8; 16];
        block[..output.len()].copy_from_slice(&output);
        draw_line_vector_block(&mut block, 0, target_start, target_end, channels, color, false);
        let output_len = output.len();
        output.copy_from_slice(&block[..output_len]);
    } else {
        let block_start = target_start.min(output.len() - 16);
        draw_line_vector_block(
            &mut output,
            block_start,
            target_start,
            target_end,
            channels,
            color,
            false,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(1);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels).map(Some)
}

fn simd_draw_vertical_segment(
    output: &mut [u8],
    image_width: u32,
    image_height: u32,
    x: i64,
    y0: i64,
    y1: i64,
    channels: usize,
    fill: (u8, u8, u8, u8),
    alpha_blend_rgb: bool,
) -> Result<(u64, u64), PilError> {
    if x < 0 || x >= i64::from(image_width) || image_height == 0 {
        return Ok((0, 0));
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    if y0 < y1 {
        let start = y0.max(0);
        let end = y1.min(i64::from(image_height));
        for y in start..end {
            let (vectors, tail) = simd_draw_span(
                output,
                image_width,
                image_height,
                x,
                x,
                y,
                channels,
                fill,
                alpha_blend_rgb,
            )?;
            vector_blocks = vector_blocks.saturating_add(vectors);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    } else if y0 > y1 {
        let mut y = y0.min(i64::from(image_height) - 1);
        let stop = y1.max(-1);
        while y > stop {
            let (vectors, tail) = simd_draw_span(
                output,
                image_width,
                image_height,
                x,
                x,
                y,
                channels,
                fill,
                alpha_blend_rgb,
            )?;
            vector_blocks = vector_blocks.saturating_add(vectors);
            scalar_tail = scalar_tail.saturating_add(tail);
            y -= 1;
        }
    }
    Ok((vector_blocks, scalar_tail))
}

fn simd_draw_rectangle_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    if image_width == 0
        || image_height == 0
        || x1 < x0
        || y1 < y0
        || !has_visible_draw_rectangle(
            image_width,
            image_height,
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
        )
    {
        return Ok(None);
    }
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD draw row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    if let Some(fill) = fill {
        let start = i64::from(y0).max(0);
        let end = i64::from(y1).min(i64::from(image_height) - 1);
        for y in start..=end {
            let (vectors, tail) = simd_draw_span(
                &mut output,
                image_width,
                image_height,
                i64::from(x0),
                i64::from(x1),
                y,
                channels,
                fill,
                alpha_blend_rgb,
            )?;
            vector_blocks = vector_blocks.saturating_add(vectors);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }

    if let Some(outline) = outline.filter(|_| width != 0) {
        let width = i64::from(width);
        for i in 0..width {
            for y in [i64::from(y0).saturating_add(i), i64::from(y1).saturating_sub(i)] {
                let (vectors, tail) = simd_draw_span(
                    &mut output,
                    image_width,
                    image_height,
                    i64::from(x0),
                    i64::from(x1),
                    y,
                    channels,
                    outline,
                    alpha_blend_rgb,
                )?;
                vector_blocks = vector_blocks.saturating_add(vectors);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
            for x in [
                i64::from(x1).saturating_sub(i),
                i64::from(x0).saturating_add(i),
            ] {
                let (vectors, tail) = simd_draw_vertical_segment(
                    &mut output,
                    image_width,
                    image_height,
                    x,
                    i64::from(y0).saturating_add(width),
                    i64::from(y1).saturating_sub(width).saturating_add(1),
                    channels,
                    outline,
                    alpha_blend_rgb,
                )?;
                vector_blocks = vector_blocks.saturating_add(vectors);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
        }
    }

    if vector_blocks == 0 {
        return Ok(None);
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

struct SimdDrawSpanWriter<'a> {
    output: &'a mut [u8],
    image_width: u32,
    image_height: u32,
    channels: usize,
    alpha_blend_rgb: bool,
    vector_blocks: u64,
    scalar_tail: u64,
}

impl SimdDrawSpanWriter<'_> {
    fn write(
        &mut self,
        x0: i32,
        y: i32,
        x1: i32,
        color: (u8, u8, u8, u8),
    ) -> Result<(), PilError> {
        let (vectors, tail) = simd_draw_span(
            self.output,
            self.image_width,
            self.image_height,
            i64::from(x0),
            i64::from(x1),
            i64::from(y),
            self.channels,
            color,
            self.alpha_blend_rgb,
        )?;
        self.vector_blocks = self.vector_blocks.saturating_add(vectors);
        self.scalar_tail = self.scalar_tail.saturating_add(tail);
        Ok(())
    }
}

fn simd_write_ellipse_spans(
    writer: &mut SimdDrawSpanWriter<'_>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    geometry_width: i32,
    color: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    let mut first_error = None;
    for_each_ellipse_span(x0, y0, x1, y1, geometry_width, |span_x0, span_y, span_x1| {
        if first_error.is_none()
            && let Err(error) = writer.write(span_x0, span_y, span_x1, color)
        {
            first_error = Some(error);
        }
    });
    first_error.map_or(Ok(()), Err)
}

fn simd_write_clipped_ellipse_spans(
    writer: &mut SimdDrawSpanWriter<'_>,
    x0: i32,
    y0: i32,
    a: i32,
    b: i32,
    state: crate::compute::pool_cpu::ops::draw::ClipEllipseState,
    color: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    let mut first_error = None;
    for_each_clipped_ellipse_span(x0, y0, a, b, state, |span_x0, span_y, span_x1| {
        if first_error.is_none()
            && let Err(error) = writer.write(span_x0, span_y, span_x1, color)
        {
            first_error = Some(error);
        }
    });
    first_error.map_or(Ok(()), Err)
}

/// Draw an ellipse using scalar Pillow-compatible scan conversion and native
/// byte SIMD span writes.  The geometry helper emits the same ordered spans
/// as the CPU canvas; this adapter owns the destination buffer and therefore
/// preserves repeated-span blending without entering a CPU pixel loop.
fn simd_draw_ellipse_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD ellipse row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }

    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };

    if let Some(fill) = fill {
        let geometry_width = x1
            .checked_sub(x0)
            .and_then(|a| y1.checked_sub(y0).map(|b| a.saturating_add(b)))
            .unwrap_or(i32::MAX);
        simd_write_ellipse_spans(&mut writer, x0, y0, x1, y1, geometry_width, fill)?;
    }
    if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
        simd_write_ellipse_spans(
            &mut writer,
            x0,
            y0,
            x1,
            y1,
            i32::try_from(width).unwrap_or(i32::MAX),
            outline,
        )?;
    }
    // Keep the small scalar callback above out of the generated data path if
    // no valid span was produced: this is a SIMD control-plane no-op, not an
    // implicit CPU fallback.
    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_draw_arc_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD arc row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let (start, end) = normalize_angles(start as f32, end as f32);
    if start + 360.0 == end {
        if let Some(fill) = fill {
            simd_write_ellipse_spans(
                &mut writer,
                x0,
                y0,
                x1,
                y1,
                i32::try_from(width).unwrap_or(i32::MAX),
                fill,
            )?;
        }
    } else if start != end
        && let (Some(a), Some(b)) = (x1.checked_sub(x0), y1.checked_sub(y0))
        && a >= 0
        && b >= 0
    {
        let state = arc_clip_state(a, b, i32::try_from(width).unwrap_or(i32::MAX), start, end);
        if let Some(fill) = fill {
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, fill)?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_draw_chord_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD chord row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let (start, end) = normalize_angles(start as f32, end as f32);
    let dimensions = x1
        .checked_sub(x0)
        .zip(y1.checked_sub(y0))
        .filter(|(a, b)| *a >= 0 && *b >= 0);
    if start + 360.0 == end {
        if let Some(fill) = fill {
            let geometry_width = x1
                .checked_sub(x0)
                .and_then(|a| y1.checked_sub(y0).map(|b| a.saturating_add(b)))
                .unwrap_or(i32::MAX);
            simd_write_ellipse_spans(&mut writer, x0, y0, x1, y1, geometry_width, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            simd_write_ellipse_spans(
                &mut writer,
                x0,
                y0,
                x1,
                y1,
                i32::try_from(width).unwrap_or(i32::MAX),
                outline,
            )?;
        }
    } else if start != end
        && let Some((a, b)) = dimensions
    {
        if let Some(fill) = fill {
            let state = chord_clip_state(a, b, a.saturating_add(b).saturating_add(1), start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            let width = i32::try_from(width).unwrap_or(i32::MAX);
            let line_state = chord_line_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, line_state, outline)?;
            let arc_state = chord_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, arc_state, outline)?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_draw_pieslice_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    start: f64,
    end: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD pieslice row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }
    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let (start, end) = normalize_angles(start as f32, end as f32);
    let dimensions = x1
        .checked_sub(x0)
        .zip(y1.checked_sub(y0))
        .filter(|(a, b)| *a >= 0 && *b >= 0);
    if start + 360.0 == end {
        if let Some(fill) = fill {
            let geometry_width = x1
                .checked_sub(x0)
                .and_then(|a| y1.checked_sub(y0).map(|b| a.saturating_add(b)))
                .unwrap_or(i32::MAX);
            simd_write_ellipse_spans(&mut writer, x0, y0, x1, y1, geometry_width, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            simd_write_ellipse_spans(
                &mut writer,
                x0,
                y0,
                x1,
                y1,
                i32::try_from(width).unwrap_or(i32::MAX),
                outline,
            )?;
        }
    } else if start != end
        && let Some((a, b)) = dimensions
    {
        if let Some(fill) = fill {
            let state = pie_clip_state(a, b, a.saturating_add(b), start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, fill)?;
        }
        if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
            let width = i32::try_from(width).unwrap_or(i32::MAX);
            for angle in [start, end] {
                let state = pie_side_clip_state(a, b, width, angle);
                simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, outline)?;
            }
            let center_x = ((f64::from(x0) + f64::from(x1) - f64::from(width)) / 2.0).round()
                as i32;
            let center_y = ((f64::from(y0) + f64::from(y1) - f64::from(width)) / 2.0).round()
                as i32;
            simd_write_ellipse_spans(
                &mut writer,
                center_x,
                center_y,
                center_x.saturating_add(width - 1),
                center_y.saturating_add(width - 1),
                width.saturating_mul(2).saturating_sub(2),
                outline,
            )?;
            let state = pie_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, x0, y0, a, b, state, outline)?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_write_rect_spans(
    writer: &mut SimdDrawSpanWriter<'_>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: (u8, u8, u8, u8),
) -> Result<(), PilError> {
    if y0 > y1 {
        return Ok(());
    }
    for y in i64::from(y0)..=i64::from(y1) {
        let y = y.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32;
        writer.write(x0, y, x1, color)?;
    }
    Ok(())
}

fn polygon_fill_intervals(
    points: &[(i32, i32)],
    image_width: u32,
    image_height: u32,
) -> std::collections::BTreeMap<i32, Vec<(i32, i32)>> {
    let mut rows = std::collections::BTreeMap::new();
    for_each_polygon_fill_span(points, image_width, image_height, |x0, x1, y| {
        rows.entry(y).or_insert_with(Vec::new).push((x0, x1));
    });
    for intervals in rows.values_mut() {
        intervals.sort_unstable();
        let mut merged: Vec<(i32, i32)> = Vec::with_capacity(intervals.len());
        for (x0, x1) in std::mem::take(intervals) {
            if let Some(last) = merged.last_mut()
                && x0 <= last.1.saturating_add(1)
            {
                last.1 = last.1.max(x1);
            } else {
                merged.push((x0, x1));
            }
        }
        *intervals = merged;
    }
    rows
}

fn simd_draw_polygon_native(
    img: &DynamicImage,
    points: &[(i32, i32)],
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD polygon row stride overflow".into()))?;
    if row_bytes < 16 {
        return Ok(None);
    }

    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };
    let mut first_error = None;
    if let Some(fill) = fill {
        for_each_polygon_fill_span(points, image_width, image_height, |x0, x1, y| {
            if first_error.is_none()
                && let Err(error) = writer.write(x0, y, x1, fill)
            {
                first_error = Some(error);
            }
        });
    }

    // Pillow masks a wide polygon outline against the filled polygon. Keep
    // that mask as compact row intervals rather than materializing a second
    // full-frame image. Stroke spans are still emitted in edge order, so
    // repeated writes and RGB alpha blending retain CPU ordering.
    if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
        let mask = polygon_fill_intervals(points, image_width, image_height);
        for index in 0..points.len() {
            let (x0, y0) = points[index];
            let (x1, y1) = points[(index + 1) % points.len()];
            if width == 1 {
                for_each_bresenham_point(x0, y0, x1, y1, |x, y| {
                    if first_error.is_none()
                        && let Err(error) = writer.write(x, y, x, outline)
                    {
                        first_error = Some(error);
                    }
                });
                continue;
            }

            let stroke_width = width.saturating_mul(2).saturating_sub(1);
            if let Some(stroke_points) = wide_line_polygon_points(x0, y0, x1, y1, stroke_width) {
                for_each_polygon_fill_span(
                    &stroke_points,
                    image_width,
                    image_height,
                    |span_x0, span_x1, y| {
                        if first_error.is_some() {
                            return;
                        }
                        if let Some(row) = mask.get(&y) {
                            for &(mask_x0, mask_x1) in row {
                                let clipped_x0 = span_x0.max(mask_x0);
                                let clipped_x1 = span_x1.min(mask_x1);
                                if clipped_x0 <= clipped_x1
                                    && let Err(error) =
                                        writer.write(clipped_x0, y, clipped_x1, outline)
                                {
                                    first_error = Some(error);
                                    break;
                                }
                            }
                        }
                    },
                );
            } else if first_error.is_none()
                && mask
                    .get(&y0)
                    .is_some_and(|row| row.iter().any(|&(left, right)| x0 >= left && x0 <= right))
                && let Err(error) = writer.write(x0, y0, x0, outline)
            {
                first_error = Some(error);
            }
        }
    }
    if let Some(error) = first_error {
        return Err(error);
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_draw_rounded_rect_native(
    img: &DynamicImage,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    radius: f64,
    fill: Option<(u8, u8, u8, u8)>,
    outline: Option<(u8, u8, u8, u8)>,
    width: u32,
    alpha_blend_rgb: bool,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_draw_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    let row_bytes = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("SIMD rounded rectangle row stride overflow".into()))?;
    if row_bytes < 16 || x1 < x0 || y1 < y0 || !radius.is_finite() || radius < 0.0 {
        return Ok(None);
    }

    let mut diameter = radius * 2.0;
    let full_x = diameter >= f64::from(x1 - x0 - 1);
    if full_x {
        diameter = f64::from(x1 - x0);
    }
    let full_y = full_x && diameter >= f64::from(y1 - y0 - 1);
    if full_y {
        diameter = f64::from(y1 - y0);
    }
    if full_x && full_y {
        return simd_draw_ellipse_native(
            img, x0, y0, x1, y1, fill, outline, width, alpha_blend_rgb, mode,
        );
    }
    if diameter == 0.0 {
        return simd_draw_rectangle_native(
            img, x0, y0, x1, y1, fill, outline, width, alpha_blend_rgb, mode,
        );
    }

    let diameter = diameter as i32;
    let radius = diameter / 2;
    let corners = if full_x {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 180.0_f32, 360.0_f32),
            (x0, y1 - diameter, x0 + diameter, y1, 0.0, 180.0),
        ]
    } else if full_y {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 90.0_f32, 270.0_f32),
            (x1 - diameter, y0, x1, y0 + diameter, 270.0, 90.0),
        ]
    } else {
        vec![
            (x0, y0, x0 + diameter, y0 + diameter, 180.0_f32, 270.0_f32),
            (x1 - diameter, y0, x1, y0 + diameter, 270.0, 360.0),
            (x1 - diameter, y1 - diameter, x1, y1, 0.0, 90.0),
            (x0, y1 - diameter, x0 + diameter, y1, 90.0, 180.0),
        ]
    };

    let mut output = img.as_bytes().to_vec();
    let mut writer = SimdDrawSpanWriter {
        output: &mut output,
        image_width,
        image_height,
        channels,
        alpha_blend_rgb,
        vector_blocks: 0,
        scalar_tail: 0,
    };

    if let Some(fill) = fill {
        for &(left, top, right, bottom, start, end) in &corners {
            let a = right.saturating_sub(left);
            let b = bottom.saturating_sub(top);
            let state = pie_clip_state(a, b, a.saturating_add(b), start, end);
            simd_write_clipped_ellipse_spans(&mut writer, left, top, a, b, state, fill)?;
        }
        if full_x {
            simd_write_rect_spans(
                &mut writer,
                x0,
                y0.saturating_add(radius + 1),
                x1,
                y1.saturating_sub(radius + 1),
                fill,
            )?;
        } else if x1 - radius - 1 >= x0 + radius + 1 {
            simd_write_rect_spans(
                &mut writer,
                x0.saturating_add(radius + 1),
                y0,
                x1.saturating_sub(radius + 1),
                y1,
                fill,
            )?;
        }
        if !full_x && !full_y {
            simd_write_rect_spans(
                &mut writer,
                x0,
                y0.saturating_add(radius + 1),
                x0.saturating_add(radius),
                y1.saturating_sub(radius + 1),
                fill,
            )?;
            simd_write_rect_spans(
                &mut writer,
                x1.saturating_sub(radius),
                y0.saturating_add(radius + 1),
                x1,
                y1.saturating_sub(radius + 1),
                fill,
            )?;
        }
    }

    if let Some(outline) = outline.filter(|color| Some(*color) != fill && width != 0) {
        let width = i32::try_from(width).unwrap_or(i32::MAX);
        for &(left, top, right, bottom, start, end) in &corners {
            let a = right.saturating_sub(left);
            let b = bottom.saturating_sub(top);
            let state = arc_clip_state(a, b, width, start, end);
            simd_write_clipped_ellipse_spans(&mut writer, left, top, a, b, state, outline)?;
        }
        if !full_x {
            simd_write_rect_spans(
                &mut writer,
                x0.saturating_add(radius + 1),
                y0,
                x1.saturating_sub(radius + 1),
                y0.saturating_add(width - 1),
                outline,
            )?;
            simd_write_rect_spans(
                &mut writer,
                x0.saturating_add(radius + 1),
                y1.saturating_sub(width - 1),
                x1.saturating_sub(radius + 1),
                y1,
                outline,
            )?;
        }
        if !full_y {
            simd_write_rect_spans(
                &mut writer,
                x0,
                y0.saturating_add(radius + 1),
                x0.saturating_add(width - 1),
                y1.saturating_sub(radius + 1),
                outline,
            )?;
            simd_write_rect_spans(
                &mut writer,
                x1.saturating_sub(width - 1),
                y0.saturating_add(radius + 1),
                x1,
                y1.saturating_sub(radius + 1),
                outline,
            )?;
        }
    }

    let vector_blocks = writer.vector_blocks;
    let scalar_tail = writer.scalar_tail;
    drop(writer);
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(Some(img.clone()));
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(image_width, image_height, output, channels).map(Some)
}

fn simd_unsupported(operation: &str) -> PilError {
    PilError::NotImplementedError(format!(
        "SIMD {operation} is unsupported for the validated image layout/parameters"
    ))
}

fn color3dlut_target_channels(mode: PixelMode) -> Option<usize> {
    match mode {
        PixelMode::RGB => Some(3),
        PixelMode::RGBA | PixelMode::CMYK => Some(4),
        _ => None,
    }
}

fn color3dlut_table_len(
    size: (u32, u32, u32),
    channels: u32,
    table_len: usize,
) -> Option<usize> {
    if !matches!(size.0, 2..=65)
        || !matches!(size.1, 2..=65)
        || !matches!(size.2, 2..=65)
        || !matches!(channels, 3 | 4)
    {
        return None;
    }
    let expected = (size.0 as usize)
        .checked_mul(size.1 as usize)?
        .checked_mul(size.2 as usize)?
        .checked_mul(channels as usize)?;
    (table_len == expected).then_some(expected)
}

fn color3dlut_source_channels_for_image(
    img: &DynamicImage,
    source_mode: PixelMode,
) -> Option<usize> {
    match (source_mode, img) {
        (PixelMode::RGB, DynamicImage::ImageRgb8(_)) => Some(3),
        (PixelMode::RGBA | PixelMode::CMYK, DynamicImage::ImageRgba8(_)) => Some(4),
        _ => None,
    }
}

fn color3dlut_source_channels_for_shape(
    shape: SimdImageShape,
    source_mode: PixelMode,
) -> Option<usize> {
    match (source_mode, shape.layout) {
        (PixelMode::RGB, SimdLayout::Rgb8) => Some(3),
        (PixelMode::RGBA | PixelMode::CMYK, SimdLayout::Rgba8) => Some(4),
        _ => None,
    }
}

fn color3dlut_supported_for_image(
    img: &DynamicImage,
    size: (u32, u32, u32),
    table_len: usize,
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    if !mode.is_none_or(|value| pixel_mode_name(source_mode) == value) {
        return false;
    }
    let Some(source_channels) = color3dlut_source_channels_for_image(img, source_mode) else {
        return false;
    };
    let Some(target_channels) = color3dlut_target_channels(target_mode) else {
        return false;
    };
    color3dlut_table_len(size, channels, table_len).is_some()
        && target_channels >= channels as usize
        && source_channels == source_mode.channels()
        && (img.width() as usize)
            .checked_mul(img.height() as usize)
            .and_then(|pixels| pixels.checked_mul(source_channels))
            .is_some_and(|expected| {
                expected == img.as_bytes().len() && expected / source_channels >= 8
            })
}

fn color3dlut_supported_for_shape(
    shape: SimdImageShape,
    size: (u32, u32, u32),
    table_len: usize,
    channels: u32,
    source_mode: PixelMode,
    target_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    if !mode.is_none_or(|value| pixel_mode_name(source_mode) == value) {
        return false;
    }
    let Some(source_channels) = color3dlut_source_channels_for_shape(shape, source_mode) else {
        return false;
    };
    let Some(target_channels) = color3dlut_target_channels(target_mode) else {
        return false;
    };
    color3dlut_table_len(size, channels, table_len).is_some()
        && target_channels >= channels as usize
        && source_channels == source_mode.channels()
        && (shape.width as usize)
            .checked_mul(shape.height as usize)
            .is_some_and(|pixels| pixels >= 8)
}

fn rotate_identity_contract(
    angle: f64,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> bool {
    angle.is_finite()
        && angle.rem_euclid(360.0) == 0.0
        && center.is_none_or(|(x, y)| x.is_finite() && y.is_finite())
        && translate.is_none_or(|(x, y)| x == 0.0 && y == 0.0)
}

#[derive(Clone, Copy)]
struct SimdRotateGeometry {
    affine: [f64; 6],
    width: u32,
    height: u32,
}

/// Build the same reverse affine matrix and expanded canvas as Pillow's
/// `Image.rotate` implementation.
///
/// Keeping this calculation separate from the sampler makes it the scalar
/// control plane: the vector kernel below only gathers native bytes and
/// stores complete output blocks after these coordinates have been fixed.
fn simd_rotate_geometry(
    width: u32,
    height: u32,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> Option<SimdRotateGeometry> {
    if !angle.is_finite()
        || center.is_some_and(|(x, y)| !x.is_finite() || !y.is_finite())
        || translate.is_some_and(|(x, y)| !x.is_finite() || !y.is_finite())
    {
        return None;
    }
    let sw = f64::from(width);
    let sh = f64::from(height);
    let radians = -angle.to_radians();
    let round_15 = |value: f64| {
        (value * 1_000_000_000_000_000.0).round() / 1_000_000_000_000_000.0
    };
    let affine_a = round_15(radians.cos());
    let affine_b = round_15(radians.sin());
    let affine_d = round_15(-radians.sin());
    let affine_e = affine_a;
    let (center_x, center_y) = center.unwrap_or((sw / 2.0, sh / 2.0));
    let (translate_x, translate_y) = translate.unwrap_or((0.0, 0.0));
    let mut affine_c = affine_a * (-center_x - translate_x)
        + affine_b * (-center_y - translate_y)
        + center_x;
    let mut affine_f = affine_d * (-center_x - translate_x)
        + affine_e * (-center_y - translate_y)
        + center_y;
    let transform = |x: f64, y: f64| {
        (
            affine_a * x + affine_b * y + affine_c,
            affine_d * x + affine_e * y + affine_f,
        )
    };
    let corners = [(0.0, 0.0), (sw, 0.0), (sw, sh), (0.0, sh)];
    let (mut min_x, mut min_y, mut max_x, mut max_y) =
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(corner_x, corner_y) in &corners {
        let (rotated_x, rotated_y) = transform(corner_x, corner_y);
        min_x = min_x.min(rotated_x);
        max_x = max_x.max(rotated_x);
        min_y = min_y.min(rotated_y);
        max_y = max_y.max(rotated_y);
    }
    let (destination_width, destination_height) = if expand {
        (
            max_x.ceil() - min_x.floor(),
            max_y.ceil() - min_y.floor(),
        )
    } else {
        (sw, sh)
    };
    if !destination_width.is_finite()
        || !destination_height.is_finite()
        || destination_width < 0.0
        || destination_height < 0.0
        || destination_width > f64::from(u32::MAX)
        || destination_height > f64::from(u32::MAX)
    {
        return None;
    }
    let destination_width = destination_width as u32;
    let destination_height = destination_height as u32;
    if expand {
        let shift_x = -(f64::from(destination_width) - sw) / 2.0;
        let shift_y = -(f64::from(destination_height) - sh) / 2.0;
        affine_c = affine_a * shift_x + affine_b * shift_y + affine_c;
        affine_f = affine_d * shift_x + affine_e * shift_y + affine_f;
    }
    Some(SimdRotateGeometry {
        affine: [affine_a, affine_b, affine_c, affine_d, affine_e, affine_f],
        width: destination_width,
        height: destination_height,
    })
}

/// `execute_rotate` has exact right-angle fast paths. Leave those operations
/// on their established implementation until a native transpose kernel is
/// selected explicitly; otherwise an affine sampler could be admitted for an
/// angle that Pillow intentionally snaps to a discrete rotation.
fn rotate_uses_discrete_fast_path(
    angle: f64,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> bool {
    rotate_discrete_fast_angle(angle, center, translate).is_some()
}

fn rotate_discrete_fast_angle(
    angle: f64,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
) -> Option<u32> {
    if center.is_some() || translate.is_some() || !angle.is_finite() {
        return None;
    }
    let degree = angle.round().rem_euclid(360.0);
    [90.0, 180.0, 270.0]
        .into_iter()
        .find(|fast_angle| (degree - fast_angle).abs() < 2.0)
        .map(|fast_angle| fast_angle as u32)
}

fn rotate_fill_sample(fill: (u8, u8, u8, u8), channels: usize, channel: usize) -> u8 {
    if channels == 2 && channel == 1 {
        fill.3
    } else {
        match channel {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        }
    }
}

fn rotate_nearest_supported_for_shape(
    width: u32,
    height: u32,
    channels: Option<usize>,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    nearest: bool,
) -> bool {
    let Some(channels) = channels else {
        return false;
    };
    if !nearest
        || width == 0
        || height == 0
        || rotate_uses_discrete_fast_path(angle, center, translate)
    {
        return false;
    }
    let Some(geometry) =
        simd_rotate_geometry(width, height, angle, expand, center, translate)
    else {
        return false;
    };
    geometry
        .width
        .checked_mul(geometry.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes >= SIMD_RANK_FILTER_LANES as u32)
}

fn rotate_bilinear_supported_for_shape(
    width: u32,
    height: u32,
    channels: Option<usize>,
    angle: f64,
    expand: bool,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    nearest: bool,
) -> bool {
    if nearest
        || channels.is_none()
        || width == 0
        || height == 0
        || rotate_uses_discrete_fast_path(angle, center, translate)
    {
        return false;
    }
    let Some(geometry) =
        simd_rotate_geometry(width, height, angle, expand, center, translate)
    else {
        return false;
    };
    geometry.width as usize >= SIMD_F64_LANES && geometry.height != 0
}

/// Check the contextual conditions for a real SIMD/native-copy operation.
///
/// The registry answers the cheap operation-only question.  This second
/// check runs after the source image is available. Returning `false` here is
/// important: an explicit SIMD request must fail before entering an adapter
/// that would use a scalar-only data path, while automatic routing can choose
/// CPU before any pixel work starts.
pub(crate) fn simd_supports_for_image(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    // The operation-level allow-list is the first admission gate. Some
    // scalar adapters remain in the registry for legacy ABI/workbench
    // compatibility, but they are never allowed to reach this contextual
    // layout check or an execution function under the SIMD backend.
    if !crate::compute::registry::simd_supports(op).unwrap_or(false) {
        return false;
    }
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    let native_byte_channels = native_byte_layout(img, mode);
    let native_filter_byte_channels = native_filter_byte_layout(img, mode);
    let native_typed_filter_channels = native_typed_filter_layout(img, mode);
    let native_rotate_channels = native_rotate_layout(img, mode);
    let native_copy = native_copy_layout(img, mode).is_some();
    let native_luma16_transpose = native_luma16_transpose_layout(img, mode);
    let native_chops = native_chops_layout(img, mode)
        .is_some_and(|channels| has_empty_native_bytes(img, channels)
            || has_vectorized_flat_bytes(img, channels));
    let lut_chops = native_chops_layout(img, mode)
        .is_some_and(|channels| has_empty_native_bytes(img, channels)
            || has_vectorized_lut_bytes(img, channels));
    let blend_chops = native_chops_layout(img, mode)
        .is_some_and(|channels| has_blend_vector_rows(img, channels));
    let affine_chops = native_chops_layout(img, mode)
        .is_some_and(|channels| has_affine_vector_rows(img, channels));
    match op {
        // These point operations have native byte kernels. Typed, indexed,
        // and explicitly mode-converted images are deliberately rejected
        // until their sample-domain kernels exist.
        PipelineOp::Invert | PipelineOp::InvertChops => native_invert_layout(img, mode)
            .is_some_and(|channels| has_empty_native_bytes(img, channels)
                || has_vectorized_byte_rows(img, channels)),
        PipelineOp::Resize { w, h, filter } => {
            native_resize_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Scale { factor, filter } => native_scale_dimensions(img.width(), img.height(), *factor)
            .is_some_and(|(w, h)| native_resize_supported_for_image(img, w, h, *filter, mode)),
        PipelineOp::Thumbnail { w, h, filter } => {
            native_thumbnail_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Contain { w, h, filter } => {
            native_contain_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Cover { w, h, filter } => {
            native_cover_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        } => native_fit_supported_for_image(
            img,
            *w,
            *h,
            *filter,
            *bleed,
            *centering,
            mode,
        ),
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => match method {
            TransformMethod::Affine => {
                native_affine_luma16_transform_supported_for_image(
                    img, *w, *h, method, data, mode,
                ) || native_affine_nearest_transform_supported_for_image(
                    img, *w, *h, method, data, *filter, mode,
                )
            }
            TransformMethod::Perspective | TransformMethod::Quad => {
                native_projective_nearest_transform_supported_for_image(
                    img,
                    *w,
                    *h,
                    method,
                    data,
                    *filter,
                    mode,
                )
            }
            TransformMethod::Mesh => native_mesh_transform_supported_for_image(
                img, *w, *h, data, *filter, mode,
            ),
        },
        PipelineOp::Pad { w, h, filter, .. } => {
            native_pad_supported_for_image(img, *w, *h, *filter, mode)
        }
        PipelineOp::Convert {
            mode: target,
            matrix,
            dither: _,
        } => native_convert_supported_for_image(
            img,
            target,
            matrix.as_deref(),
            mode,
        ),
        PipelineOp::Reduce {
            x_factor,
            y_factor,
        } => native_reduce_supported_for_image(img, *x_factor, *y_factor, mode),
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. } => {
            native_byte_channels.is_some_and(|channels| has_nonempty_byte_data(img, channels))
        }
        PipelineOp::Grayscale => native_grayscale_supported_for_image(img, mode),
        PipelineOp::Colorize { .. } => {
            matches!(img, DynamicImage::ImageLuma8(_))
                && matches!(mode, None | Some("L"))
                && img.width() != 0
                && img.height() != 0
                && (img.width() as usize)
                    .checked_mul(img.height() as usize)
                    .is_some_and(|pixels| img.as_bytes().len() == pixels)
        }
        PipelineOp::Brightness { factor } => native_brightness_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels))
            && factor.is_finite(),
        PipelineOp::Contrast { factor } | PipelineOp::ColorSaturation { factor } => {
            factor.is_finite()
                && native_enhance_layout(img, mode)
                    .is_some_and(|(channels, _)| has_vectorized_float_bytes(img, channels))
        }
        PipelineOp::Sharpness { factor } => {
            factor.is_finite()
                && native_sharpness_layout(img, mode)
                    .is_some_and(|(channels, _)| has_vectorized_sharpness_bytes(img, channels))
        }
        PipelineOp::Autocontrast { cutoff, mask } => {
            cutoff.is_finite()
                && native_autocontrast_layout(img, mode)
                    .is_some_and(|channels| has_vectorized_flat_bytes(img, channels))
                && autocontrast_mask_supported(
                    img.width(),
                    img.height(),
                    mask.as_ref(),
                )
        }
        PipelineOp::Equalize => native_autocontrast_layout(img, mode)
            .is_some_and(|channels| has_vectorized_flat_bytes(img, channels)),
        // Eval/PointOp keeps its interleaved native layout. The LUT lookup
        // itself is vectorized per band; scalar work only deinterleaves the
        // byte lanes because `wide` has no portable byte-gather primitive.
        PipelineOp::Eval { lut } => native_point_channels(img, mode)
            .is_some_and(|channels| lut.len() == 256 * channels && img.as_bytes().len() >= 16),
        PipelineOp::PutData { mode: data_mode, .. } => {
            native_put_data_layout(img, *data_mode, mode)
                .is_some_and(|layout| native_paste_actual_layout(img, layout))
        }
        PipelineOp::ExtractBand { index } => {
            native_extract_layout(img, mode).is_some_and(|channels| {
                usize::from(*index) < channels
                    && img
                        .width()
                        .checked_mul(img.height())
                        .and_then(|pixels| pixels.checked_mul(channels as u32))
                        .is_some_and(|bytes| img.as_bytes().len() == bytes as usize && bytes != 0)
            })
        }
        // These operations are native memory movement. They do not claim
        // arithmetic vectorization, but they never enter a CPU adapter.
        PipelineOp::Offset { x, .. } => {
            has_vectorized_luma16_offset(img, mode)
                || (native_copy
                    && native_copy_layout(img, mode)
                        .is_some_and(|channels| has_empty_native_bytes(img, channels)
                            || has_vectorized_offset_rows(img, channels, *x)))
        }
        PipelineOp::Flip => native_copy_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels)),
        PipelineOp::Mirror => native_copy_layout(img, mode)
            .is_some_and(|channels| has_vectorized_mirror_rows(img, channels)),
        PipelineOp::Transpose { .. } => {
            (native_copy || native_luma16_transpose) && pixel_count != 0
        }
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            native_copy
                && pixel_count != 0
                && *left < *right
                && *top < *bottom
                && *right <= img.width()
                && *bottom <= img.height()
        }
        PipelineOp::CropBorder { .. } => {
            // Let the adapter's scalar geometry check report Pillow's public
            // error for an oversized border. Capability preflight must not
            // replace that validation error with NotImplementedError.
            native_copy
        }
        PipelineOp::Expand { border, .. } => {
            native_expand_contract_for_image(img, *border, mode).is_some()
        }
        PipelineOp::Constant { .. } => img.width() != 0 && img.height() != 0,
        PipelineOp::Duplicate => native_copy_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels)),
        // The convolution kernels use wide vector blocks for the maintained
        // native-byte layout. Images narrower than one complete interior
        // vector remain automatic CPU work; image *area* is not a capability
        // condition because a small image can still contain a real vector
        // block.
        PipelineOp::Filter3x3 {
            kernel, scale, ..
        } => {
            valid_convolution_parameters(kernel, *scale)
                && (use_native_byte_convolution_path(img, mode, 1)
                    || (mode == Some("I")
                        && native_typed_filter_channels.is_some()
                        && use_native_i32_convolution_path(img, mode, 1))
                    || native_filter_identity_supported_for_image(img, mode, 1))
        }
        PipelineOp::Filter5x5 {
            kernel, scale, ..
        } => {
            valid_convolution_parameters(kernel, *scale)
                && (use_native_byte_convolution_path(img, mode, 2)
                    || (mode == Some("I")
                        && native_typed_filter_channels.is_some()
                        && use_native_i32_convolution_path(img, mode, 2))
                    || native_filter_identity_supported_for_image(img, mode, 2))
        }
        PipelineOp::BoxBlur { radius } => {
            if *radius == 0 {
                has_vectorized_native_identity_copy(img, mode)
            } else {
                native_byte_channels.is_some() && img.width() != 0 && img.height() != 0
            }
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            native_byte_channels.is_some()
                && *passes != 0
                && radius_x.is_finite()
                && radius_y.is_finite()
                && *radius_x >= 0.0
                && *radius_y >= 0.0
                && ((*radius_x != 0.0 || *radius_y != 0.0)
                    && img.width() != 0
                    && img.height() != 0
                    || (*radius_x == 0.0
                        && *radius_y == 0.0
                        && has_vectorized_native_identity_copy(img, mode)))
        }
        PipelineOp::GaussianBlur { sigma } => {
            if *sigma == 0.0 {
                has_vectorized_native_identity_copy(img, mode)
            } else {
                gaussian_blur_radius(sigma.abs()).is_some_and(|radius| {
                    native_byte_channels.is_some()
                        && radius > 0.0
                        && img.width() != 0
                        && img.height() != 0
                })
            }
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && img.width() != 0
                && img.height() != 0)
                || native_float_rank_supported_for_image(
                    img,
                    mode,
                    *size,
                    if matches!(op, PipelineOp::MaxFilter { .. }) {
                        size.saturating_mul(*size).saturating_sub(1)
                    } else {
                        0
                    },
                )
        }
        PipelineOp::MedianFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && *size <= 15
                && img.width() != 0
                && img.height() != 0)
                || native_float_rank_supported_for_image(
                    img,
                    mode,
                    *size,
                    size.saturating_mul(*size) / 2,
                )
        }
        PipelineOp::RankFilter { size, rank } => {
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && area <= 225
                && u64::from(*rank) < area
                && img.width() != 0
                && img.height() != 0)
                || native_float_rank_supported_for_image(img, mode, *size, *rank)
        }
        PipelineOp::PutAlpha { mode: alpha_mode, .. } => {
            let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
            put_alpha_shape(img, *alpha_mode, mode)
                .is_some_and(|(_, _, pixels_per_vector, _)| pixels >= pixels_per_vector)
        }
        PipelineOp::PutAlphaData { mask, mode: alpha_mode } => {
            let mask = mask.as_ref();
            let pixels = (img.width() as usize).saturating_mul(img.height() as usize);
            put_alpha_data_shape(img, mask, *alpha_mode, mode)
                .is_some_and(|(_, _, pixels_per_vector, _)| pixels >= pixels_per_vector)
        }
        PipelineOp::EffectNoise { sigma } => {
            sigma.is_finite()
                && img.height() != 0
                && (img.width() as usize).saturating_mul(img.height() as usize) != 0
        }
        PipelineOp::LinearGradient { mode } => matches!(
            mode,
            ColorMode::Mode1 | ColorMode::L | ColorMode::P | ColorMode::I | ColorMode::F
        ),
        PipelineOp::EffectSpread { distance } => {
            (*distance <= 1 || (img.width() == 1 && img.height() == 1))
                && native_copy_layout(img, mode).is_some_and(|channels| {
                    img.width()
                        .checked_mul(img.height())
                        .and_then(|pixels| pixels.checked_mul(channels as u32))
                        .is_some_and(|expected| expected != 0 && img.as_bytes().len() == expected as usize)
                })
        }
        PipelineOp::Color3DLut {
            size,
            table,
            channels,
            source_mode,
            target_mode,
        } => color3dlut_supported_for_image(
            img,
            *size,
            table.len(),
            *channels,
            *source_mode,
            *target_mode,
            mode,
        ),
        PipelineOp::Rotate {
            angle,
            expand,
            center,
            translate,
            nearest,
            ..
        } => {
            // Pillow's indexed modes always use nearest-neighbor sampling,
            // even when the public resample argument was omitted or named a
            // different filter. Keep that mode rule in the scalar
            // capability check so a P image reaches the native byte sampler.
            let nearest = *nearest || matches!(mode, Some("1" | "P"));
            if rotate_identity_contract(*angle, *center, *translate) {
                native_copy_layout(img, mode).is_some_and(|channels| {
                    img.width()
                        .checked_mul(img.height())
                        .and_then(|pixels| pixels.checked_mul(channels as u32))
                        .is_some_and(|expected| {
                            expected as usize >= 16 && img.as_bytes().len() == expected as usize
                        })
                })
            } else if rotate_uses_discrete_fast_path(*angle, *center, *translate) {
                native_rotate_channels.is_some() && pixel_count != 0
            } else {
                if nearest {
                    rotate_nearest_supported_for_shape(
                        img.width(),
                        img.height(),
                        native_rotate_channels,
                        *angle,
                        *expand,
                        *center,
                        *translate,
                        nearest,
                    )
                } else {
                    rotate_bilinear_supported_for_shape(
                        img.width(),
                        img.height(),
                        native_rotate_channels,
                        *angle,
                        *expand,
                        *center,
                        *translate,
                        nearest,
                    )
                }
            }
        }
        PipelineOp::DrawLine { x0, y0, x1, y1, .. } =>
            native_draw_layout(img, mode).is_some_and(|channels| {
            has_vectorized_byte_rows(img, channels)
                && line_bounds_intersect(img.width(), img.height(), *x0, *y0, *x1, *y1)
                && channels != 0
        }),
        PipelineOp::DrawPoint { points, .. } => native_draw_layout(img, mode).is_some_and(|channels| {
            (points.is_empty() || has_vectorized_byte_rows(img, channels))
                && (!has_visible_draw_point(img.width(), img.height(), points)
                    || has_vectorized_byte_rows(img, channels))
        }),
        PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. } => {
            native_draw_layout(img, mode)
                .is_some_and(|channels| has_vectorized_byte_rows(img, channels))
        }
        PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            ..
        } => {
            native_draw_layout(img, mode).is_some_and(|channels| {
                has_vectorized_byte_rows(img, channels)
                    && *x1 >= *x0
                    && *y1 >= *y0
                    && radius.is_finite()
                && *radius >= 0.0
            })
        }
        PipelineOp::DrawPolygon { .. } => {
            native_draw_layout(img, mode).is_some_and(|channels| {
                has_vectorized_byte_rows(img, channels)
            })
        }
        PipelineOp::PutPixel {
            x,
            y,
            palette_index: _,
            ..
        } => native_draw_layout(img, mode).is_some_and(|channels| {
            *x < img.width()
                && *y < img.height()
                && channels != 0
        }),
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            ..
        } => native_draw_layout(img, mode).is_some_and(|channels| {
            has_vectorized_byte_rows(img, channels)
                && valid_draw_rectangle(img.width(), img.height(), *x0, *y0, *x1, *y1)
                && (has_visible_draw_rectangle(
                    img.width(),
                    img.height(),
                    *x0,
                    *y0,
                    *x1,
                    *y1,
                    *fill,
                    *outline,
                    *width,
                ) || fill.is_none() && (outline.is_none() || *width == 0)
                    || i64::from(*x1) < 0
                    || i64::from(*y1) < 0
                    || i64::from(*x0) >= i64::from(img.width())
                    || i64::from(*y0) >= i64::from(img.height()))
        }),
        PipelineOp::AlphaComposite { source, dest, src } => {
            *dest == (0, 0)
                && *src == (0, 0)
                && simd_alpha_composite_operands_supported(img, source, mode)
        }
        PipelineOp::CompositeModule {
            other,
            mask,
            mask_alpha,
        } => native_composite_plan_for_image(img, other, mask, *mask_alpha, mode).is_some(),
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => native_paste_plan_for_image(
            img,
            source,
            *x,
            *y,
            *w,
            *h,
            mask.as_ref(),
            *mask_alpha,
            mode,
        )
        .is_some(),
        PipelineOp::Merge { mode: target_mode, bands } => {
            native_merge_contract_for_image(img, target_mode, bands, mode).is_some()
        }
        PipelineOp::BlendModule { other, alpha } => {
            simd_module_blend_supported(img, other, mode, *alpha)
        }
        // All-channel Chops kernels use native bytes, including CMYK's K
        // sample. Add/Subtract use the same native bytes for both their
        // default and scaled/offset affine formulas.
        PipelineOp::Multiply { other } | PipelineOp::Screen { other } => {
            blend_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other } => {
            native_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::Overlay { other } | PipelineOp::HardLight { other } => {
            lut_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::SoftLight { other } => {
            lut_chops && simd_chops_operands_supported(img, other, mode)
        }
        PipelineOp::Add {
            other,
            scale,
            offset,
        }
        | PipelineOp::Subtract {
            other,
            scale,
            offset,
        } => {
            affine_chops
                && scale.is_finite()
                && *scale != 0.0
                && offset.is_finite()
                && simd_chops_operands_supported(img, other, mode)
        }
        // Operations not listed above remain explicitly unsupported until their
        // data plane is vectorized or classified as native-copy.
        _ => false,
    }
}

fn alpha_composite_layout_mode(shape: SimdLayout) -> Option<&'static str> {
    match shape {
        SimdLayout::LumaA8 => Some("LA"),
        SimdLayout::Rgba8 => Some("RGBA"),
        _ => None,
    }
}

fn simd_alpha_composite_operands_supported(
    img: &DynamicImage,
    source: &Image,
    mode: Option<&str>,
) -> bool {
    let expected_mode = match img {
        DynamicImage::ImageLumaA8(_) => "LA",
        DynamicImage::ImageRgba8(_) => "RGBA",
        _ => return false,
    };
    if mode.is_some_and(|value| value != expected_mode) {
        return false;
    }
    let pixels_supported = img
        .width()
        .checked_mul(img.height())
        .is_some_and(|pixels| pixels == 0 || pixels >= 8);
    source.mode().ok().as_deref() == Some(expected_mode)
        && source.size().ok() == Some(img.dimensions())
        && pixels_supported
}

fn simd_alpha_composite_pixels_supported(width: u32, height: u32) -> bool {
    width
        .checked_mul(height)
        .is_some_and(|pixels| pixels == 0 || pixels >= 8)
}

fn simd_alpha_composite_shape_supported(
    shape: SimdImageShape,
    source: &Image,
    mode: Option<&str>,
) -> bool {
    let Some(expected_mode) = alpha_composite_layout_mode(shape.layout) else {
        return false;
    };
    mode.is_none_or(|value| value == expected_mode)
        && source.mode().ok().as_deref() == Some(expected_mode)
        && source.size().ok() == Some((shape.width, shape.height))
        && simd_alpha_composite_pixels_supported(shape.width, shape.height)
}

/// Whether an operation preserves the concrete pixel-buffer contract.
///
/// The automatic planner can keep adjacent operations in one backend segment
/// only while width, height, channel layout, and logical sample semantics are
/// unchanged. Operations not listed here are deliberately treated as a
/// segment boundary; a conservative boundary costs a dispatch opportunity but
/// cannot make a later capability check observe stale layout information.
pub(crate) fn preserves_native_contract(op: &PipelineOp) -> bool {
    matches!(
        op,
        PipelineOp::Filter3x3 { .. }
            | PipelineOp::Filter5x5 { .. }
            | PipelineOp::GaussianBlur { .. }
            | PipelineOp::BoxBlur { .. }
            | PipelineOp::BoxBlurXY { .. }
            | PipelineOp::MedianFilter { .. }
            | PipelineOp::MaxFilter { .. }
            | PipelineOp::MinFilter { .. }
            | PipelineOp::RankFilter { .. }
            | PipelineOp::Autocontrast { .. }
            | PipelineOp::Equalize
            | PipelineOp::Invert
            | PipelineOp::Flip
            | PipelineOp::Mirror
            | PipelineOp::Posterize { .. }
            | PipelineOp::Solarize { .. }
            | PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::Overlay { .. }
            | PipelineOp::HardLight { .. }
            | PipelineOp::SoftLight { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::Offset { .. }
            | PipelineOp::Duplicate
            | PipelineOp::InvertChops
            | PipelineOp::AlphaComposite { .. }
            | PipelineOp::Brightness { .. }
            | PipelineOp::Contrast { .. }
            | PipelineOp::ColorSaturation { .. }
            | PipelineOp::Sharpness { .. }
            | PipelineOp::EffectSpread { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::BlendModule { .. }
            | PipelineOp::Eval { .. }
            | PipelineOp::PutData { .. }
            | PipelineOp::PointOp { .. }
            | PipelineOp::PutPixel { .. }
            | PipelineOp::DrawLine { .. }
            | PipelineOp::DrawRectangle { .. }
            | PipelineOp::DrawRoundedRect { .. }
            | PipelineOp::DrawEllipse { .. }
            | PipelineOp::DrawCircle { .. }
            | PipelineOp::DrawPolygon { .. }
            | PipelineOp::DrawArc { .. }
            | PipelineOp::DrawChord { .. }
            | PipelineOp::DrawPieslice { .. }
            | PipelineOp::DrawPoint { .. }
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SimdLayout {
    Luma8,
    LumaA8,
    Rgb8,
    Rgba8,
    Luma16,
    Unsupported,
}

#[derive(Clone, Copy)]
struct SimdImageShape {
    width: u32,
    height: u32,
    layout: SimdLayout,
}

fn simd_shape(img: &DynamicImage) -> SimdImageShape {
    let layout = match img {
        DynamicImage::ImageLuma8(_) => SimdLayout::Luma8,
        DynamicImage::ImageLumaA8(_) => SimdLayout::LumaA8,
        DynamicImage::ImageRgb8(_) => SimdLayout::Rgb8,
        DynamicImage::ImageRgba8(_) => SimdLayout::Rgba8,
        DynamicImage::ImageLuma16(_) => SimdLayout::Luma16,
        _ => SimdLayout::Unsupported,
    };
    SimdImageShape {
        width: img.width(),
        height: img.height(),
        layout,
    }
}

fn shape_channels(shape: SimdImageShape) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 => Some(1),
        SimdLayout::LumaA8 => Some(2),
        SimdLayout::Rgb8 => Some(3),
        SimdLayout::Rgba8 => Some(4),
        SimdLayout::Luma16 | SimdLayout::Unsupported => None,
    }
}

fn shape_draw_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8
            if matches!(mode, None | Some("RGB" | "RGBA" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        SimdLayout::Rgba8
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")) =>
        {
            Some(4)
        }
        SimdLayout::Luma16 => None,
        _ => None,
    }
}

fn shape_mode_matches(shape: SimdImageShape, mode: Option<&str>) -> bool {
    match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F"))
        }
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    }
}

fn shape_native_byte_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("L")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB")),
        SimdLayout::Rgba8 => matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX")),
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_filter_byte_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX" | "CMYK"))
        }
        SimdLayout::Luma16 | SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_extract_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX"))
        }
        SimdLayout::Luma16 | SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_typed_filter_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    (shape.layout == SimdLayout::Rgba8 && matches!(mode, Some("I" | "F"))).then_some(4)
}

fn shape_native_i32_convolution_path(
    shape: SimdImageShape,
    mode: Option<&str>,
    border: usize,
) -> bool {
    shape_native_typed_filter_channels(shape, mode).is_some()
        && mode == Some("I")
        && shape.width as usize >= border.saturating_mul(2).saturating_add(1)
        && shape.height as usize > border.saturating_mul(2)
}

fn shape_native_filter_identity_supported(
    shape: SimdImageShape,
    mode: Option<&str>,
    border: usize,
) -> bool {
    let valid_layout = shape_native_filter_byte_channels(shape, mode).is_some()
        || (mode == Some("I") && shape_native_typed_filter_channels(shape, mode).is_some());
    let no_interior = shape.width as usize <= border.saturating_mul(2)
        || shape.height as usize <= border.saturating_mul(2);
    valid_layout && no_interior && shape.width != 0 && shape.height != 0
}

fn shape_native_float_rank_supported(
    shape: SimdImageShape,
    mode: Option<&str>,
    size: u32,
    rank: u32,
) -> bool {
    let area = u64::from(size).saturating_mul(u64::from(size));
    shape_native_typed_filter_channels(shape, mode).is_some()
        && mode == Some("F")
        && size != 0
        && size % 2 == 1
        && size <= SIMD_ORDER_STATISTIC_SORT_MAX_SIZE
        && u64::from(rank) < area
        && shape.width != 0
        && shape.height != 0
}

fn shape_native_rotate_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match (shape.layout, mode) {
        (SimdLayout::Luma8, Some("1" | "P")) => Some(1),
        (SimdLayout::LumaA8, Some("PA")) => Some(2),
        (SimdLayout::Rgba8, Some("CMYK" | "RGBa" | "RGBX")) => Some(4),
        _ => shape_native_byte_channels(shape, mode),
    }
}

fn shape_native_reduce_layout(shape: SimdImageShape, mode: Option<&str>) -> Option<(usize, bool)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, false)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, true)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some((3, false))
        }
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, true)),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, false)),
        _ => None,
    }
}

fn shape_native_brightness_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("L")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => matches!(mode, None | Some("RGBA" | "CMYK")),
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_enhance_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, usize)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, 1)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, 1)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

fn shape_native_sharpness_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, usize)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, 1)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, 1)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some((3, 3)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, 3)),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, 4)),
        _ => None,
    }
}

fn shape_native_autocontrast_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("L")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB")),
        SimdLayout::LumaA8
        | SimdLayout::Rgba8
        | SimdLayout::Luma16
        | SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_grayscale_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some(3),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "RGBX")) => Some(4),
        _ => None,
    }
}

fn shape_native_grayscale_supported(shape: SimdImageShape, mode: Option<&str>) -> bool {
    let Some(channels) = shape_native_grayscale_channels(shape, mode) else {
        return false;
    };
    shape.width != 0
        && shape.height != 0
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

fn shape_native_colorize_supported(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape.layout == SimdLayout::Luma8
        && matches!(mode, None | Some("L"))
        && shape.width != 0
        && shape.height != 0
}

fn shape_has_nonempty_byte_data(shape: SimdImageShape, channels: usize) -> bool {
    shape.width != 0
        && shape.height != 0
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

fn shape_has_vectorized_float_bytes(shape: SimdImageShape, channels: usize) -> bool {
    shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes >= 8)
}

fn shape_has_vectorized_sharpness_bytes(shape: SimdImageShape, channels: usize) -> bool {
    shape.width >= 10
        && shape.height >= 3
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

fn shape_has_vectorized_native_identity_copy(
    shape: SimdImageShape,
    channels: usize,
) -> bool {
    shape.width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes >= 16)
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

fn shape_native_invert_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB")),
        SimdLayout::Rgba8 => matches!(mode, None | Some("RGBA" | "CMYK")),
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_native_copy_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    shape_mode_matches(shape, mode).then_some(channels)
}

fn shape_luma16_transpose_layout(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

fn shape_native_identity_copy_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        SimdLayout::Rgba8
            if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "I" | "F")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn shape_native_chops_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX"))
        }
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    };
    valid.then_some(channels)
}

fn shape_has_empty_native_bytes(shape: SimdImageShape, channels: usize) -> bool {
    (shape.width == 0 || shape.height == 0)
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            == Some(0)
}

fn shape_has_vector_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape.height != 0
        && shape
            .width
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

fn shape_has_vectorized_flat_bytes(shape: SimdImageShape, channels: usize) -> bool {
    shape.width != 0
        && shape.height != 0
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|total_bytes| total_bytes >= 16)
}

fn shape_has_vectorized_lut_bytes(shape: SimdImageShape, channels: usize) -> bool {
    shape.width != 0
        && shape.height != 0
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|total_bytes| total_bytes >= 8)
}

fn shape_has_vectorized_offset_rows(
    shape: SimdImageShape,
    channels: usize,
    xoffset: i32,
) -> bool {
    if shape.width == 0 || shape.height == 0 {
        return false;
    }
    let Some(row_bytes) = shape.width.checked_mul(channels as u32) else {
        return false;
    };
    let _ = xoffset;
    row_bytes
        .checked_mul(shape.height)
        .is_some_and(|total_bytes| total_bytes >= 16)
}

fn shape_has_vectorized_luma16_offset(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && (shape_has_empty_native_bytes(shape, 2)
            || (shape.width >= 16 && shape.height != 0))
}

fn shape_has_vectorized_mirror_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape.height != 0 && shape.width != 0 && matches!(channels, 1..=4)
}

fn shape_has_affine_vector_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape_has_empty_native_bytes(shape, channels)
        || (shape.height != 0
            && shape
                .width
                .checked_mul(channels as u32)
                .is_some_and(|row_bytes| row_bytes >= 8))
}

fn shape_has_blend_vector_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape_has_affine_vector_rows(shape, channels)
}

fn shape_preserves_chops_operands(
    shape: SimdImageShape,
    other: &Arc<Image>,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = shape_native_chops_channels(shape, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    let Ok(other_size) = other.size() else {
        return false;
    };
    logical_byte_channels(&other_mode).is_some_and(|other_channels| {
        channels == other_channels
            && (shape.width, shape.height) == other_size
    })
}

fn shape_native_module_blend_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    match (shape.layout, mode) {
        (SimdLayout::Luma8, None | Some("L")) => Some(1),
        (SimdLayout::LumaA8, None | Some("LA")) => Some(2),
        (SimdLayout::Rgb8, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        (SimdLayout::Rgba8, None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX")) => Some(4),
        _ => None,
    }
}

fn shape_module_blend_supported(
    shape: SimdImageShape,
    other: &Arc<Image>,
    mode: Option<&str>,
    alpha: f64,
) -> bool {
    if !alpha.is_finite() {
        return false;
    }
    let Some(channels) = shape_native_module_blend_channels(shape, mode) else {
        return false;
    };
    let Ok(other_mode) = other.mode() else {
        return false;
    };
    if native_module_blend_mode_channels(&other_mode) != Some(channels)
        || other.size().ok() != Some((shape.width, shape.height))
    {
        return false;
    }
    let Some(total_bytes) = (shape.width as usize)
        .checked_mul(shape.height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    total_bytes == 0 || total_bytes >= 8
}

fn shape_put_alpha_supported(
    shape: SimdImageShape,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    let (source_layout, mode_matches) = match alpha_mode {
        PixelMode::L | PixelMode::P => (
            SimdLayout::Luma8,
            matches!(mode, None | Some("L" | "P" | "PA")),
        ),
        PixelMode::LA | PixelMode::PA => (
            SimdLayout::LumaA8,
            matches!(mode, None | Some("LA" | "PA")),
        ),
        PixelMode::RGB => (SimdLayout::Rgb8, matches!(mode, None | Some("RGB"))),
        PixelMode::RGBA => (
            SimdLayout::Rgba8,
            matches!(mode, None | Some("RGBA" | "RGBX")),
        ),
        PixelMode::CMYK => (SimdLayout::Rgba8, matches!(mode, None | Some("CMYK"))),
        _ => (SimdLayout::Unsupported, false),
    };
    mode_matches && shape.layout == source_layout
}

fn shape_put_alpha_data_supported(
    shape: SimdImageShape,
    mask: &DynamicImage,
    alpha_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    if !matches!(mask, DynamicImage::ImageLuma8(_))
        || (shape.width, shape.height) != mask.dimensions()
    {
        return false;
    }
    let (source_layout, mode_matches) = match alpha_mode {
        PixelMode::L | PixelMode::P => (
            SimdLayout::Luma8,
            matches!(mode, None | Some("L" | "P" | "PA")),
        ),
        PixelMode::LA | PixelMode::PA => (
            SimdLayout::LumaA8,
            matches!(mode, None | Some("LA" | "PA")),
        ),
        PixelMode::RGB => (SimdLayout::Rgb8, matches!(mode, None | Some("RGB"))),
        PixelMode::RGBA => (
            SimdLayout::Rgba8,
            matches!(mode, None | Some("RGBA" | "RGBX")),
        ),
        PixelMode::CMYK => (SimdLayout::Rgba8, matches!(mode, None | Some("CMYK"))),
        _ => (SimdLayout::Unsupported, false),
    };
    mode_matches && shape.layout == source_layout
}

fn shape_after_simd_op(shape: SimdImageShape, op: &PipelineOp) -> Option<SimdImageShape> {
    let mut next = shape;
    match op {
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            if *left >= *right
                || *top >= *bottom
                || *right > shape.width
                || *bottom > shape.height
            {
                return None;
            }
            next.width = *right - *left;
            next.height = *bottom - *top;
        }
        PipelineOp::CropBorder { border } => {
            let twice = border.saturating_mul(2);
            if twice > shape.width || twice > shape.height {
                return None;
            }
            next.width = shape.width - twice;
            next.height = shape.height - twice;
        }
        PipelineOp::Expand { border, .. } => {
            let twice = border.checked_mul(2)?;
            next.width = shape.width.checked_add(twice)?;
            next.height = shape.height.checked_add(twice)?;
        }
        PipelineOp::LinearGradient { mode } => {
            next.width = 256;
            next.height = 256;
            next.layout = match mode {
                ColorMode::Mode1 | ColorMode::L | ColorMode::P => SimdLayout::Luma8,
                ColorMode::I | ColorMode::F => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::Constant { .. } => next.layout = SimdLayout::Luma8,
        PipelineOp::Grayscale => next.layout = SimdLayout::Luma8,
        PipelineOp::Colorize { .. } => next.layout = SimdLayout::Rgb8,
        PipelineOp::Transpose { method } => {
            if matches!(
                method,
                TransposeMethod::Rotate90
                    | TransposeMethod::Rotate270
                    | TransposeMethod::Transpose
                    | TransposeMethod::Transverse
            ) {
                std::mem::swap(&mut next.width, &mut next.height);
            }
        }
        PipelineOp::ExtractBand { .. } => next.layout = SimdLayout::Luma8,
        PipelineOp::EffectNoise { .. } => next.layout = SimdLayout::Luma8,
        PipelineOp::Convert { mode, .. } => {
            next.layout = match mode {
                ColorMode::L => SimdLayout::Luma8,
                ColorMode::LA => SimdLayout::LumaA8,
                ColorMode::RGB | ColorMode::YCbCr | ColorMode::HSV => SimdLayout::Rgb8,
                ColorMode::RGBA | ColorMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::Reduce {
            x_factor,
            y_factor,
        } => {
            next.width = shape.width.div_ceil((*x_factor).max(1));
            next.height = shape.height.div_ceil((*y_factor).max(1));
        }
        PipelineOp::Resize { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::Scale { factor, .. } => {
            (next.width, next.height) =
                native_scale_dimensions(shape.width, shape.height, *factor)?;
        }
        PipelineOp::Thumbnail { w, h, .. } => {
            let (output_width, output_height) =
                native_thumbnail_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
        }
        PipelineOp::Contain { w, h, .. } => {
            let (output_width, output_height) =
                native_pad_contained_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
        }
        PipelineOp::Cover { w, h, .. } => {
            let (output_width, output_height) =
                native_cover_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
        }
        PipelineOp::Fit { w, h, .. } => {
            next.width = (*w).max(1);
            next.height = (*h).max(1);
        }
        PipelineOp::Transform { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::Pad { w, h, .. } => {
            next.width = *w;
            next.height = *h;
        }
        PipelineOp::Color3DLut { target_mode, .. } => {
            next.layout = match target_mode {
                PixelMode::RGB => SimdLayout::Rgb8,
                PixelMode::RGBA | PixelMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::Merge { mode, .. } => {
            next.layout = match mode {
                ColorMode::L | ColorMode::Mode1 => SimdLayout::Luma8,
                ColorMode::LA => SimdLayout::LumaA8,
                ColorMode::RGB => SimdLayout::Rgb8,
                ColorMode::RGBA | ColorMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::CompositeModule { other, .. } => {
            let output_mode = other.mode().ok()?;
            (next.width, next.height) = other.size().ok()?;
            next.layout = match output_mode.as_str() {
                "1" | "L" | "P" => SimdLayout::Luma8,
                "LA" | "PA" => SimdLayout::LumaA8,
                "RGB" | "HSV" | "YCbCr" => SimdLayout::Rgb8,
                "RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F" => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
            next.layout = match mode {
                PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => {
                    SimdLayout::LumaA8
                }
                PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => SimdLayout::Rgba8,
                _ => SimdLayout::Unsupported,
            };
        }
        _ => {}
    }
    Some(next)
}

fn color_mode_name(mode: &ColorMode) -> &'static str {
    match mode {
        ColorMode::L => "L",
        ColorMode::LA => "LA",
        ColorMode::RGB => "RGB",
        ColorMode::RGBA => "RGBA",
        ColorMode::CMYK => "CMYK",
        ColorMode::YCbCr => "YCbCr",
        ColorMode::HSV => "HSV",
        ColorMode::I => "I",
        ColorMode::F => "F",
        ColorMode::P => "P",
        ColorMode::Mode1 => "1",
    }
}

fn pixel_mode_name(mode: PixelMode) -> &'static str {
    match mode {
        PixelMode::L => "L",
        PixelMode::LA => "LA",
        PixelMode::RGB => "RGB",
        PixelMode::RGBA => "RGBA",
        PixelMode::P => "P",
        PixelMode::PA => "PA",
        PixelMode::CMYK => "CMYK",
        PixelMode::Mode1 => "1",
        PixelMode::YCbCr => "YCbCr",
        PixelMode::HSV => "HSV",
        PixelMode::I => "I",
        PixelMode::F => "F",
    }
}

fn concrete_simd_mode(img: &DynamicImage) -> Option<&'static str> {
    match img {
        DynamicImage::ImageLuma8(_) => Some("L"),
        DynamicImage::ImageLumaA8(_) => Some("LA"),
        DynamicImage::ImageRgb8(_) => Some("RGB"),
        DynamicImage::ImageRgba8(_) => Some("RGBA"),
        _ => None,
    }
}

fn operation_target_mode(op: &PipelineOp) -> Option<&'static str> {
    match op {
        PipelineOp::Convert { mode, .. } | PipelineOp::Merge { mode, .. } => {
            Some(color_mode_name(mode))
        }
        PipelineOp::Color3DLut { target_mode, .. }
        | PipelineOp::PutAlpha {
            mode: target_mode,
            ..
        }
        | PipelineOp::PutAlphaData {
            mode: target_mode,
            ..
        } => Some(pixel_mode_name(*target_mode)),
        PipelineOp::LinearGradient { mode } => Some(color_mode_name(mode)),
        PipelineOp::Grayscale | PipelineOp::EffectNoise { .. } => Some("L"),
        PipelineOp::Colorize { .. } => Some("RGB"),
        PipelineOp::Constant { .. } => Some("L"),
        PipelineOp::ExtractBand { .. } => Some("L"),
        _ => None,
    }
}

/// Resolve the logical mode at the first operation boundary.
///
/// Pipeline metadata carries the final output mode, while the concrete input
/// buffer still has the source layout. If a pipeline ends in a nonstandard
/// conversion (for example RGB -> CMYK), passing that final tag to the first
/// `PutPixel` or `Convert` operation would make a valid native source look
/// unsupported. Use the concrete source mode whenever the operation list
/// contains the final mode transition; preserving a nonstandard mode without
/// such a transition remains necessary for raw HSV/YCbCr/CMYK samples.
pub(crate) fn simd_initial_mode(
    img: &DynamicImage,
    ops: &[PipelineOp],
    final_mode: Option<&str>,
) -> Option<String> {
    let Some(final_mode) = final_mode else {
        return None;
    };
    // `PA` shares the two-byte `LumaA8` storage with `LA`, but its first byte
    // is a palette index rather than luma.  Keep the logical tag at this
    // boundary; replacing it with the concrete raster mode would make a
    // valid PA-only adapter look unsupported before execution begins.
    if final_mode == "PA" {
        return Some(final_mode.to_owned());
    }
    if let Some(PipelineOp::Color3DLut { source_mode, .. }) = ops.first() {
        return Some(pixel_mode_name(*source_mode).to_owned());
    }
    if ops
        .iter()
        .any(|op| operation_target_mode(op) == Some(final_mode))
    {
        return concrete_simd_mode(img)
            .map(str::to_owned)
            .or_else(|| Some(final_mode.to_owned()));
    }
    Some(final_mode.to_owned())
}

/// Advance the logical mode after one pipeline operation.
pub(crate) fn simd_mode_after_op(op: &PipelineOp, current: Option<&str>) -> Option<String> {
    if let PipelineOp::CompositeModule { other, .. } = op {
        return other.mode().ok();
    }
    if let Some(target) = operation_target_mode(op) {
        // Equalize expands indexed samples before its histogram operation.
        if matches!(op, PipelineOp::Equalize) {
            return Some(target.to_owned());
        }
        return Some(target.to_owned());
    }
    if matches!(op, PipelineOp::Equalize) && matches!(current, Some("P" | "PA")) {
        return Some("RGB".to_owned());
    }
    current.map(str::to_owned)
}

/// Find the first operation that would be unsupported for the actual
/// intermediate layout, without allocating or touching pixel data.
///
/// Strict SIMD uses this shape-only pass before entering the first adapter.
/// It closes the subtle gap where an early crop/extraction/alpha promotion
/// changes the layout enough to make a later operation unsupported.
pub(crate) fn first_unsupported_simd_op<'a>(
    img: &DynamicImage,
    ops: &'a [PipelineOp],
    mode: Option<&str>,
) -> Option<&'a PipelineOp> {
    let mut shape = simd_shape(img);
    let mut current_mode = simd_initial_mode(img, ops, mode);
    for op in ops {
        let op_mode = current_mode.as_deref();
        if !crate::compute::registry::simd_supports(op).unwrap_or(false) {
            return Some(op);
        }
        if !simd_supports_for_shape(shape, op, op_mode) {
            return Some(op);
        }
        shape = shape_after_simd_op(shape, op)?;
        current_mode = simd_mode_after_op(op, op_mode);
    }
    None
}

fn simd_supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let pixel_count = (shape.width as usize).saturating_mul(shape.height as usize);
    let native_byte_channels = shape_native_byte_channels(shape, mode);
    let native_filter_byte_channels = shape_native_filter_byte_channels(shape, mode);
    let native_copy = shape_native_copy_channels(shape, mode).is_some();
    let native_luma16_transpose = shape_luma16_transpose_layout(shape, mode);
    let native_chops = shape_native_chops_channels(shape, mode).is_some_and(|channels| {
        shape_has_empty_native_bytes(shape, channels)
            || shape_has_vectorized_flat_bytes(shape, channels)
    });
    let lut_chops = shape_native_chops_channels(shape, mode).is_some_and(|channels| {
        shape_has_empty_native_bytes(shape, channels)
            || shape_has_vectorized_lut_bytes(shape, channels)
    });
    let blend_chops = shape_native_chops_channels(shape, mode)
        .is_some_and(|channels| shape_has_blend_vector_rows(shape, channels));
    let affine_chops = shape_native_chops_channels(shape, mode)
        .is_some_and(|channels| shape_has_affine_vector_rows(shape, channels));
    match op {
        PipelineOp::Invert | PipelineOp::InvertChops => shape_native_invert_channels(shape, mode)
            .is_some_and(|channels| {
                shape_has_empty_native_bytes(shape, channels)
                    || shape_has_vector_rows(shape, channels)
            }),
        PipelineOp::Resize { w, h, filter } => {
            native_resize_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Scale { factor, filter } => native_scale_dimensions(shape.width, shape.height, *factor)
            .is_some_and(|(w, h)| native_resize_supported_for_shape(shape, w, h, *filter, mode)),
        PipelineOp::Thumbnail { w, h, filter } => {
            native_thumbnail_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Contain { w, h, filter } => {
            native_contain_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Cover { w, h, filter } => {
            native_cover_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        } => native_fit_supported_for_shape(
            shape,
            *w,
            *h,
            *filter,
            *bleed,
            *centering,
            mode,
        ),
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => match method {
            TransformMethod::Affine => {
                native_affine_luma16_transform_supported_for_shape(
                    shape, *w, *h, method, data, mode,
                ) || native_affine_nearest_transform_supported_for_shape(
                    shape, *w, *h, method, data, *filter, mode,
                )
            }
            TransformMethod::Perspective | TransformMethod::Quad => {
                native_projective_nearest_transform_supported_for_shape(
                    shape,
                    *w,
                    *h,
                    method,
                    data,
                    *filter,
                    mode,
                )
            }
            TransformMethod::Mesh => native_mesh_transform_supported_for_shape(
                shape, *w, *h, data, *filter, mode,
            ),
        },
        PipelineOp::Reduce {
            x_factor,
            y_factor,
        } => native_reduce_supported_for_shape(shape, *x_factor, *y_factor, mode),
        PipelineOp::Convert {
            mode: target,
            matrix,
            dither: _,
        } => native_convert_supported_for_shape(
            shape,
            target,
            matrix.as_deref(),
            mode,
        ),
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. } => {
            shape_native_byte_channels(shape, mode)
                .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels))
        }
        PipelineOp::Grayscale => shape_native_grayscale_supported(shape, mode),
        PipelineOp::Colorize { .. } => shape_native_colorize_supported(shape, mode),
        PipelineOp::Brightness { factor } => shape_native_brightness_channels(shape, mode)
            .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels))
            && factor.is_finite(),
        PipelineOp::Contrast { factor } | PipelineOp::ColorSaturation { factor } => {
            factor.is_finite()
                && shape_native_enhance_channels(shape, mode)
                    .is_some_and(|(channels, _)| shape_has_vectorized_float_bytes(shape, channels))
        }
        PipelineOp::Sharpness { factor } => {
            factor.is_finite()
                && shape_native_sharpness_channels(shape, mode).is_some_and(
                    |(channels, _)| shape_has_vectorized_sharpness_bytes(shape, channels),
                )
        }
        PipelineOp::Autocontrast { cutoff, mask } => {
            cutoff.is_finite()
                && shape_native_autocontrast_channels(shape, mode)
                    .is_some_and(|channels| shape_has_vectorized_flat_bytes(shape, channels))
                && autocontrast_mask_supported(shape.width, shape.height, mask.as_ref())
        }
        PipelineOp::Equalize => shape_native_autocontrast_channels(shape, mode)
            .is_some_and(|channels| shape_has_vectorized_flat_bytes(shape, channels)),
        PipelineOp::Eval { lut } | PipelineOp::PointOp { lut } => {
            shape_native_point_channels(shape, mode).is_some_and(|channels| {
                lut.len() == 256 * channels
                    && (shape.width as usize)
                        .saturating_mul(shape.height as usize)
                        .saturating_mul(channels)
                        >= 16
            })
        }
        PipelineOp::PutData { mode: data_mode, .. } => {
            native_put_data_shape_layout(shape, *data_mode, mode).is_some()
        }
        PipelineOp::ExtractBand { index } => {
            shape_native_extract_channels(shape, mode).is_some_and(|channels| {
                usize::from(*index) < channels
                    && pixel_count.saturating_mul(channels) != 0
            })
        }
        PipelineOp::Offset { x, .. } => {
            shape_has_vectorized_luma16_offset(shape, mode)
                || shape_native_copy_channels(shape, mode).is_some_and(|channels| {
                    shape_has_empty_native_bytes(shape, channels)
                        || shape_has_vectorized_offset_rows(shape, channels, *x)
                })
        }
        PipelineOp::Flip => shape_native_copy_channels(shape, mode)
            .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels)),
        PipelineOp::Mirror => shape_native_copy_channels(shape, mode)
            .is_some_and(|channels| shape_has_vectorized_mirror_rows(shape, channels)),
        PipelineOp::Transpose { .. } => {
            (native_copy || native_luma16_transpose) && pixel_count != 0
        }
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            native_copy
                && pixel_count != 0
                && *left < *right
                && *top < *bottom
                && *right <= shape.width
                && *bottom <= shape.height
        }
        PipelineOp::CropBorder { .. } => {
            // Invalid geometry is still a scalar control-plane concern; the
            // adapter must be allowed to return Pillow's ValueError instead
            // of strict SIMD masking it as an unsupported operation.
            native_copy
        }
        PipelineOp::Expand { border, .. } => {
            native_expand_contract_for_shape(shape, *border, mode).is_some()
        }
        PipelineOp::Pad { w, h, filter, .. } => {
            native_pad_supported_for_shape(shape, *w, *h, *filter, mode)
        }
        PipelineOp::Constant { .. } => shape.width != 0 && shape.height != 0,
        PipelineOp::Duplicate => shape_native_copy_channels(shape, mode).is_some_and(|channels| {
            shape.width != 0
                && shape.height != 0
                && shape
                    .width
                    .checked_mul(shape.height)
                    .and_then(|pixels| pixels.checked_mul(channels as u32))
                    .is_some_and(|bytes| bytes != 0)
        }),
        PipelineOp::Filter3x3 {
            kernel, scale, ..
        } => {
            valid_convolution_parameters(kernel, *scale)
                && (native_filter_byte_channels.is_some()
                    && shape.width as usize >= 3
                    && shape.height as usize > 2
                    || shape_native_i32_convolution_path(shape, mode, 1)
                    || shape_native_filter_identity_supported(shape, mode, 1))
        }
        PipelineOp::Filter5x5 {
            kernel, scale, ..
        } => {
            valid_convolution_parameters(kernel, *scale)
                && (native_filter_byte_channels.is_some()
                    && shape.width as usize >= 5
                    && shape.height as usize > 4
                    || shape_native_i32_convolution_path(shape, mode, 2)
                    || shape_native_filter_identity_supported(shape, mode, 2))
        }
        PipelineOp::BoxBlur { radius } => {
            if *radius == 0 {
                shape_native_byte_channels(shape, mode)
                    .is_some_and(|channels| {
                        shape_has_vectorized_native_identity_copy(shape, channels)
                    })
            } else {
                native_byte_channels.is_some() && shape.width != 0 && shape.height != 0
            }
        }
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            native_byte_channels.is_some()
                && *passes != 0
                && radius_x.is_finite()
                && radius_y.is_finite()
                && *radius_x >= 0.0
                && *radius_y >= 0.0
                && ((*radius_x != 0.0 || *radius_y != 0.0)
                    && shape.width != 0
                    && shape.height != 0
                    || (*radius_x == 0.0
                        && *radius_y == 0.0
                        && shape_native_byte_channels(shape, mode).is_some_and(|channels| {
                            shape_has_vectorized_native_identity_copy(shape, channels)
                        })))
        }
        PipelineOp::GaussianBlur { sigma } => {
            if *sigma == 0.0 {
                shape_native_byte_channels(shape, mode).is_some_and(|channels| {
                    shape_has_vectorized_native_identity_copy(shape, channels)
                })
            } else {
                gaussian_blur_radius(sigma.abs()).is_some_and(|radius| {
                    native_byte_channels.is_some()
                        && radius > 0.0
                        && shape.width != 0
                        && shape.height != 0
                })
            }
        }
        PipelineOp::MaxFilter { size } | PipelineOp::MinFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && shape.width != 0
                && shape.height != 0)
                || shape_native_float_rank_supported(
                    shape,
                    mode,
                    *size,
                    if matches!(op, PipelineOp::MaxFilter { .. }) {
                        size.saturating_mul(*size).saturating_sub(1)
                    } else {
                        0
                    },
                )
        }
        PipelineOp::MedianFilter { size } => {
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && *size <= 15
                && shape.width != 0
                && shape.height != 0)
                || shape_native_float_rank_supported(
                    shape,
                    mode,
                    *size,
                    size.saturating_mul(*size) / 2,
                )
        }
        PipelineOp::RankFilter { size, rank } => {
            let area = u64::from(*size).saturating_mul(u64::from(*size));
            (native_filter_byte_channels.is_some()
                && *size != 0
                && *size % 2 == 1
                && area <= 225
                && u64::from(*rank) < area
                && shape.width != 0
                && shape.height != 0)
                || shape_native_float_rank_supported(shape, mode, *size, *rank)
        }
        PipelineOp::PutAlpha { mode: alpha_mode, .. } => {
            let pixels_per_vector = match alpha_mode {
                PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => 8,
                PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => 4,
                _ => usize::MAX,
            };
            shape_put_alpha_supported(shape, *alpha_mode, mode)
                && pixel_count >= pixels_per_vector
        }
        PipelineOp::PutAlphaData { mask, mode: alpha_mode } => {
            shape_put_alpha_data_supported(shape, mask, *alpha_mode, mode)
                && pixel_count >= match alpha_mode {
                    PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA => 8,
                    PixelMode::RGB | PixelMode::RGBA | PixelMode::CMYK => 4,
                    _ => usize::MAX,
                }
        }
        PipelineOp::EffectNoise { sigma } => {
            sigma.is_finite()
                && shape.height != 0
                && (shape.width as usize).saturating_mul(shape.height as usize) != 0
        }
        PipelineOp::LinearGradient { mode } => matches!(
            mode,
            ColorMode::Mode1 | ColorMode::L | ColorMode::P | ColorMode::I | ColorMode::F
        ),
        PipelineOp::EffectSpread { distance } => {
            (*distance <= 1 || (shape.width == 1 && shape.height == 1))
                && shape_native_identity_copy_channels(shape, mode).is_some_and(|channels| {
                    (shape.width as usize)
                        .checked_mul(shape.height as usize)
                        .and_then(|pixels| pixels.checked_mul(channels))
                        .is_some_and(|bytes| bytes != 0)
                })
        }
        PipelineOp::Color3DLut {
            size,
            table,
            channels,
            source_mode,
            target_mode,
        } => color3dlut_supported_for_shape(
            shape,
            *size,
            table.len(),
            *channels,
            *source_mode,
            *target_mode,
            mode,
        ),
        PipelineOp::Rotate {
            angle,
            expand,
            center,
            translate,
            nearest,
            ..
        } => {
            let nearest = *nearest || matches!(mode, Some("1" | "P"));
            if rotate_identity_contract(*angle, *center, *translate) {
                shape_native_identity_copy_channels(shape, mode).is_some_and(|channels| {
                    (shape.width as usize)
                        .checked_mul(shape.height as usize)
                        .and_then(|pixels| pixels.checked_mul(channels))
                        .is_some_and(|bytes| bytes >= 16)
                })
            } else if rotate_uses_discrete_fast_path(*angle, *center, *translate) {
                shape_native_rotate_channels(shape, mode).is_some() && pixel_count != 0
            } else {
                if nearest {
                    rotate_nearest_supported_for_shape(
                        shape.width,
                        shape.height,
                        shape_native_rotate_channels(shape, mode),
                        *angle,
                        *expand,
                        *center,
                        *translate,
                        nearest,
                    )
                } else {
                    rotate_bilinear_supported_for_shape(
                        shape.width,
                        shape.height,
                        shape_native_rotate_channels(shape, mode),
                        *angle,
                        *expand,
                        *center,
                        *translate,
                        nearest,
                    )
                }
            }
        }
        PipelineOp::DrawLine { x0, y0, x1, y1, .. } =>
            shape_draw_channels(shape, mode).is_some_and(|channels| {
            shape_has_vector_rows(shape, channels)
                && line_bounds_intersect(shape.width, shape.height, *x0, *y0, *x1, *y1)
                && channels != 0
        }),
        PipelineOp::DrawPoint { points, .. } => shape_draw_channels(shape, mode).is_some_and(|channels| {
            (points.is_empty() || shape_has_vector_rows(shape, channels))
                && (!has_visible_draw_point(shape.width, shape.height, points)
                    || shape_has_vector_rows(shape, channels))
        }),
        PipelineOp::DrawEllipse { .. }
        | PipelineOp::DrawCircle { .. }
        | PipelineOp::DrawArc { .. }
        | PipelineOp::DrawChord { .. }
        | PipelineOp::DrawPieslice { .. } => {
            shape_draw_channels(shape, mode)
                .is_some_and(|channels| shape_has_vector_rows(shape, channels))
        }
        PipelineOp::DrawRoundedRect {
            x0,
            y0,
            x1,
            y1,
            radius,
            ..
        } => {
            shape_draw_channels(shape, mode).is_some_and(|channels| {
                shape_has_vector_rows(shape, channels)
                    && *x1 >= *x0
                    && *y1 >= *y0
                    && radius.is_finite()
                    && *radius >= 0.0
            })
        }
        PipelineOp::DrawPolygon { .. } => {
            shape_draw_channels(shape, mode).is_some_and(|channels| {
                shape_has_vector_rows(shape, channels)
            })
        }
        PipelineOp::PutPixel {
            x,
            y,
            palette_index: _,
            ..
        } => shape_draw_channels(shape, mode).is_some_and(|channels| {
            *x < shape.width
                && *y < shape.height
                && channels != 0
        }),
        PipelineOp::DrawRectangle {
            x0,
            y0,
            x1,
            y1,
            fill,
            outline,
            width,
            ..
        } => shape_draw_channels(shape, mode).is_some_and(|channels| {
            shape_has_vector_rows(shape, channels)
                && valid_draw_rectangle(shape.width, shape.height, *x0, *y0, *x1, *y1)
                && (has_visible_draw_rectangle(
                    shape.width,
                    shape.height,
                    *x0,
                    *y0,
                    *x1,
                    *y1,
                    *fill,
                    *outline,
                    *width,
                ) || fill.is_none() && (outline.is_none() || *width == 0)
                    || i64::from(*x1) < 0
                    || i64::from(*y1) < 0
                    || i64::from(*x0) >= i64::from(shape.width)
                    || i64::from(*y0) >= i64::from(shape.height))
        }),
        PipelineOp::AlphaComposite { source, dest, src } => {
            *dest == (0, 0)
                && *src == (0, 0)
                && simd_alpha_composite_shape_supported(shape, source, mode)
        }
        PipelineOp::CompositeModule {
            other,
            mask,
            mask_alpha,
        } => native_composite_plan_for_shape(shape, other, mask, *mask_alpha, mode).is_some(),
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => native_paste_plan_for_shape(
            shape,
            source,
            *x,
            *y,
            *w,
            *h,
            mask.as_ref(),
            *mask_alpha,
            mode,
        )
        .is_some(),
        PipelineOp::Merge { mode: target_mode, bands } => {
            native_merge_contract_for_shape(shape, target_mode, bands, mode).is_some()
        }
        PipelineOp::BlendModule { other, alpha } => {
            shape_module_blend_supported(shape, other, mode, *alpha)
        }
        PipelineOp::Multiply { other } | PipelineOp::Screen { other } => {
            blend_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other } => {
            native_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::Overlay { other } | PipelineOp::HardLight { other } => {
            lut_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::SoftLight { other } => {
            lut_chops && shape_preserves_chops_operands(shape, other, mode)
        }
        PipelineOp::Add {
            other,
            scale,
            offset,
        }
        | PipelineOp::Subtract {
            other,
            scale,
            offset,
        } => {
            affine_chops
                && scale.is_finite()
                && *scale != 0.0
                && offset.is_finite()
                && shape_preserves_chops_operands(shape, other, mode)
        }
        _ => false,
    }
}

// Native byte transforms always operate on L/LA/RGB/RGBA layouts. Keeping
// their active-channel masks in one table avoids duplicating an input-driven
// branch in every closure monomorphization of `native_byte_transform`.
const NATIVE_BYTE_ZERO_MASK: [u8; 16] = [0; 16];
const NATIVE_BYTE_ALL_MASK: [u8; 16] = [u8::MAX; 16];
const NATIVE_BYTE_LA_MASK: [u8; 16] = [
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
];
const NATIVE_BYTE_RGBA_MASK: [u8; 16] = [
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
];

const NATIVE_BYTE_ACTIVE_MASKS: [[u8; 16]; 5] = [
    NATIVE_BYTE_ZERO_MASK,
    NATIVE_BYTE_ALL_MASK,
    NATIVE_BYTE_LA_MASK,
    NATIVE_BYTE_ALL_MASK,
    NATIVE_BYTE_RGBA_MASK,
];

// `invert_alpha as usize` selects preserve-alpha (false) or invert-alpha
// (true). Native inversion dispatch supplies only channel counts 1..=4.
const NATIVE_BYTE_INVERT_MASKS: [[&[u8; 16]; 2]; 5] = [
    [&NATIVE_BYTE_ZERO_MASK, &NATIVE_BYTE_ZERO_MASK],
    [&NATIVE_BYTE_ALL_MASK, &NATIVE_BYTE_ALL_MASK],
    [&NATIVE_BYTE_LA_MASK, &NATIVE_BYTE_ALL_MASK],
    [&NATIVE_BYTE_ALL_MASK, &NATIVE_BYTE_ALL_MASK],
    [&NATIVE_BYTE_RGBA_MASK, &NATIVE_BYTE_ALL_MASK],
];

#[inline]
fn native_byte_transform_bytes<F>(bytes: &mut [u8], channels: usize, transform: &F)
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
        let input = u8x16::new(
            <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks"),
        );
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        chunk.copy_from_slice(&output.to_array());
    }
    let remainder = chunks.into_remainder();
    if !remainder.is_empty() {
        // A short row/image still has a genuine vector data path: copy only
        // the valid bytes into a stack-padded block, run the same SIMD kernel,
        // then commit only those lanes. This avoids pretending that a scalar
        // per-byte tail is SIMD while also avoiding an out-of-bounds load.
        let mut padded = [0u8; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        let input = u8x16::new(padded);
        let transformed = transform(input);
        let output = (transformed & active_vector) | (input & inactive);
        remainder.copy_from_slice(&output.to_array()[..remainder.len()]);
    }
}

#[inline]
fn native_all_channel_transform_bytes<F>(bytes: &mut [u8], transform: &F)
where
    F: Fn(u8x16) -> u8x16,
{
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = u8x16::new(
            <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks"),
        );
        chunk.copy_from_slice(&transform(input).to_array());
    }
    let remainder = chunks.into_remainder();
    if !remainder.is_empty() {
        let mut padded = [0u8; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        remainder.copy_from_slice(&transform(u8x16::new(padded)).to_array()[..remainder.len()]);
    }
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
    let Some(channels) = native_byte_layout(img, mode) else {
        return None;
    };
    if !has_nonempty_byte_data(img, channels) {
        return None;
    }
    let result = match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 1, &transform);
            Some(DynamicImage::ImageLuma8(result))
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 2, &transform);
            Some(DynamicImage::ImageLumaA8(result))
        }
        DynamicImage::ImageRgb8(image)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 3, &transform);
            Some(DynamicImage::ImageRgb8(result))
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 4, &transform);
            Some(DynamicImage::ImageRgba8(result))
        }
        _ => None,
    }?;
    let vector_blocks = img.as_bytes().len().div_ceil(16) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_path("vector");
    Some(result)
}

/// Byte-wise brightness transform with CMYK explicitly admitted. CMYK's K
/// sample is data, not alpha, so this wrapper uses an all-channel mask rather
/// than the RGBA-preserving mask used by [`native_byte_transform`].
fn native_brightness_transform<F>(
    img: &DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> Option<DynamicImage>
where
    F: Fn(u8x16) -> u8x16,
{
    let channels = native_brightness_layout(img, mode)?;
    if !has_nonempty_byte_data(img, channels) {
        return None;
    }
    let result = match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 1, &transform);
            DynamicImage::ImageLuma8(result)
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 2, &transform);
            DynamicImage::ImageLumaA8(result)
        }
        DynamicImage::ImageRgb8(image)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 3, &transform);
            DynamicImage::ImageRgb8(result)
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 4, &transform);
            DynamicImage::ImageRgba8(result)
        }
        DynamicImage::ImageRgba8(image) if mode == Some("CMYK") => {
            let mut result = image.clone();
            // CMYK has no alpha channel; all four packed bytes are active.
            native_all_channel_transform_bytes(result.as_mut(), &transform);
            DynamicImage::ImageRgba8(result)
        }
        _ => return None,
    };
    crate::compute::record_pipeline_operation_vector_blocks(img.as_bytes().len().div_ceil(16) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    Some(result)
}

#[inline]
fn clamp_trunc_u8(value: f64) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

#[inline]
fn cmyk_contrast_gray(source: &[u8], start: usize) -> u8 {
    let c = u32::from(source[start]);
    let m = u32::from(source[start + 1]);
    let y = u32::from(source[start + 2]);
    let k = u32::from(source[start + 3]);
    let nk = 255u32.saturating_sub(k);
    let r = (nk as i32 - crate::color::muldiv255(c, nk) as i32).clamp(0, 255) as u8;
    let g = (nk as i32 - crate::color::muldiv255(m, nk) as i32).clamp(0, 255) as u8;
    let b = (nk as i32 - crate::color::muldiv255(y, nk) as i32).clamp(0, 255) as u8;
    crate::color::rgb_to_luma_u8(r, g, b)
}

#[inline]
fn cmyk_color_gray(source: &[u8], start: usize) -> u8 {
    let c = u32::from(source[start]);
    let m = u32::from(source[start + 1]);
    let y = u32::from(source[start + 2]);
    let k = u32::from(source[start + 3]);
    let nk = 255u32.saturating_sub(k);
    // ImageEnhance.Color's CMYK path uses integer floor division while the
    // Contrast path uses the shared MULDIV255 conversion. Keep both Pillow
    // control-plane contracts explicit before vectorizing the blend below.
    let r = ((255 - c) * nk / 255) as u8;
    let g = ((255 - m) * nk / 255) as u8;
    let b = ((255 - y) * nk / 255) as u8;
    crate::color::rgb_to_luma_u8(r, g, b)
}

#[inline]
fn native_enhance_gray(
    source: &[u8],
    pixel: usize,
    channels: usize,
    mode: Option<&str>,
    cmyk_color_path: bool,
) -> f64 {
    let start = pixel * channels;
    let value = match (channels, mode) {
        (1 | 2, _) => source[start],
        (3, _) => crate::color::rgb_to_luma_u8(source[start], source[start + 1], source[start + 2]),
        (4, Some("CMYK")) if cmyk_color_path => cmyk_color_gray(source, start),
        (4, Some("CMYK")) => cmyk_contrast_gray(source, start),
        (4, _) => crate::color::rgb_to_luma_u8(source[start], source[start + 1], source[start + 2]),
        _ => 0,
    };
    f64::from(value)
}

fn native_enhance_mean(
    img: &DynamicImage,
    mode: Option<&str>,
    channels: usize,
    cmyk_color_path: bool,
) -> Option<u8> {
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if pixels == 0 {
        return None;
    }
    let expected = pixels.checked_mul(channels)?;
    let source = img.as_bytes();
    if source.len() != expected {
        return None;
    }
    let sum = (0..pixels)
        .map(|pixel| native_enhance_gray(source, pixel, channels, mode, cmyk_color_path) as u64)
        .sum::<u64>();
    Some(((sum as f64 / pixels as f64) + 0.5) as u8)
}

fn vectorize_contrast_bytes(
    source: &[u8],
    output: &mut [u8],
    channels: usize,
    active_channels: usize,
    mode: Option<&str>,
    mean: u8,
    factor: f64,
) -> (u64, u64) {
    let mut vector_blocks = 0u64;
    for (block, chunk) in output.chunks_exact_mut(8).enumerate() {
        let offset = block * 8;
        let mut input = [0.0; 8];
        let mut base = [0.0; 8];
        for lane in 0..8 {
            let index = offset + lane;
            let channel = index % channels;
            input[lane] = f64::from(source[index]);
            base[lane] = if mode == Some("CMYK") {
                if channel == 3 {
                    255.0 - f64::from(mean)
                } else {
                    0.0
                }
            } else {
                f64::from(mean)
            };
        }
        let values = f64x8::new(base) * f64x8::splat(1.0 - factor)
            + f64x8::new(input) * f64x8::splat(factor);
        for (lane, value) in values.to_array().into_iter().enumerate() {
            if lane % channels < active_channels {
                chunk[lane] = clamp_trunc_u8(value);
            }
        }
        vector_blocks += 1;
    }
    let full_len = output.len() / 8 * 8;
    let remainder = &mut output[full_len..];
    let offset = full_len;
    for (lane, destination) in remainder.iter_mut().enumerate() {
        let index = offset + lane;
        let channel = index % channels;
        if channel >= active_channels {
            continue;
        }
        let base = if mode == Some("CMYK") && channel == 3 {
            255.0 - f64::from(mean)
        } else if mode == Some("CMYK") {
            0.0
        } else {
            f64::from(mean)
        };
        *destination = clamp_trunc_u8(base * (1.0 - factor) + f64::from(source[index]) * factor);
    }
    (vector_blocks, remainder.len() as u64)
}

fn vectorize_color_bytes(
    source: &[u8],
    output: &mut [u8],
    channels: usize,
    active_channels: usize,
    mode: Option<&str>,
    factor: f64,
) -> (u64, u64) {
    let mut vector_blocks = 0u64;
    for (block, chunk) in output.chunks_exact_mut(8).enumerate() {
        let offset = block * 8;
        let mut input = [0.0; 8];
        let mut base = [0.0; 8];
        for lane in 0..8 {
            let index = offset + lane;
            let channel = index % channels;
            input[lane] = f64::from(source[index]);
            let gray = native_enhance_gray(
                source,
                index / channels,
                channels,
                mode,
                mode == Some("CMYK"),
            );
            base[lane] = if mode == Some("CMYK") && channel == 3 {
                255.0 - gray
            } else if mode == Some("CMYK") {
                0.0
            } else {
                gray
            };
        }
        let input = f64x8::new(input);
        let base = f64x8::new(base);
        let values = base + f64x8::splat(factor) * (input - base);
        for (lane, value) in values.to_array().into_iter().enumerate() {
            if lane % channels < active_channels {
                chunk[lane] = clamp_trunc_u8(value);
            }
        }
        vector_blocks += 1;
    }
    let full_len = output.len() / 8 * 8;
    let remainder = &mut output[full_len..];
    let offset = full_len;
    for (lane, destination) in remainder.iter_mut().enumerate() {
        let index = offset + lane;
        let channel = index % channels;
        if channel >= active_channels {
            continue;
        }
        let gray = native_enhance_gray(
            source,
            index / channels,
            channels,
            mode,
            mode == Some("CMYK"),
        );
        let base = if mode == Some("CMYK") && channel == 3 {
            255.0 - gray
        } else if mode == Some("CMYK") {
            0.0
        } else {
            gray
        };
        *destination = clamp_trunc_u8(base + factor * (f64::from(source[index]) - base));
    }
    (vector_blocks, remainder.len() as u64)
}

fn native_enhance_output(
    img: &DynamicImage,
    mode: Option<&str>,
    channels: usize,
    active_channels: usize,
    factor: f64,
    contrast: bool,
    mean: Option<u8>,
) -> Option<(DynamicImage, u64, u64)> {
    let source = img.as_bytes();
    let mut result = img.clone();
    let output = result.as_bytes_mut()?;
    if output.len() != source.len() {
        return None;
    }
    let (vector_blocks, scalar_tail) = if contrast {
        vectorize_contrast_bytes(
            source,
            output,
            channels,
            active_channels,
            mode,
            mean?,
            factor,
        )
    } else {
        vectorize_color_bytes(
            source,
            output,
            channels,
            active_channels,
            mode,
            factor,
        )
    };
    Some((result, vector_blocks, scalar_tail))
}

/// Invert selected byte channels with one portable 16-byte vector operation.
///
/// `wide` selects the target's safe SIMD implementation (NEON, SSE2, or its
/// scalar fallback) without introducing unsafe code into this crate. The mask
/// keeps alpha bytes unchanged when Pillow's operation does not invert alpha,
/// while still allowing LA/RGBA to use the same interleaved path.
fn invert_native_bytes(bytes: &mut [u8], channels: usize, invert_alpha: bool) {
    let active = NATIVE_BYTE_INVERT_MASKS[channels][invert_alpha as usize];
    let active_vector = u8x16::new(*active);
    let inactive = u8x16::splat(u8::MAX) - active_vector;
    let mut chunks = bytes.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let input = <[u8; 16]>::try_from(&*chunk).expect("chunks_exact_mut yields 16-byte chunks");
        let input = u8x16::new(input);
        let inverted = u8x16::splat(u8::MAX) - input;
        let output = (inverted & active_vector) | (input & inactive);
        chunk.copy_from_slice(&output.to_array());
    }
    let remainder = chunks.into_remainder();
    for (index, value) in remainder.iter_mut().enumerate() {
        let mask = active[index % 16];
        let inverted = u8::MAX - *value;
        *value = (inverted & mask) | (*value & !mask);
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
    if let Some(channels) = native_invert_layout(img, mode) {
        if has_empty_native_bytes(img, channels) {
            crate::compute::record_pipeline_operation_path("native-copy");
            return Some(img.clone());
        }
        record_native_row_work(img.width() as usize, img.height() as usize, channels);
    }
    let mut result = img.clone();
    let (width, height) = result.dimensions();
    match &mut result {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("1" | "L" | "P")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 1, |row| {
                invert_native_bytes(row, 1, false);
            });
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA" | "PA")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 2, |row| {
                invert_native_bytes(row, 2, invert_alpha);
            });
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 3, |row| {
                invert_native_bytes(row, 3, false);
            });
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA" | "CMYK")) => {
            apply_native_rows(image.as_mut(), width as usize, height as usize, 4, |row| {
                invert_native_bytes(row, 4, invert_alpha || mode == Some("CMYK"));
            });
        }
        _ => return None,
    }
    Some(result)
}

/// Apply a native byte transform to an already-owned intermediate image.
///
/// The ordinary adapter takes `&DynamicImage` because the first pipeline
/// operation must not mutate its source.  Later operations in a SIMD segment
/// own the previous result, so cloning that result would add a full-frame
/// allocation with no semantic benefit.  This helper is only called after
/// contextual SIMD preflight has succeeded and therefore cannot become a CPU
/// fallback.
fn native_byte_transform_in_place<F>(
    img: &mut DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> bool
where
    F: Fn(u8x16) -> u8x16 + Send + Sync,
{
    let Some(channels) = native_byte_layout(img, mode) else {
        return false;
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    if width == 0 || height == 0 {
        return false;
    }
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let row_bytes = width.saturating_mul(channels);
    let vector_blocks = row_bytes.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    apply_native_rows(bytes, width, height, channels, |row| {
        native_byte_transform_bytes(row, channels, &transform);
    });
    crate::compute::record_pipeline_operation_path("vector");
    true
}

/// In-place brightness transform for native byte images, including CMYK.
/// Unlike RGBA, CMYK keeps all four lanes active because K is a color sample.
fn native_brightness_transform_in_place<F>(
    img: &mut DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> bool
where
    F: Fn(u8x16) -> u8x16 + Send + Sync,
{
    let Some(channels) = native_brightness_layout(img, mode) else {
        return false;
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    if width == 0 || height == 0 {
        return false;
    }
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let row_bytes = width.saturating_mul(channels);
    let vector_blocks = row_bytes.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    if mode == Some("CMYK") {
        apply_native_rows(bytes, width, height, channels, |row| {
            native_all_channel_transform_bytes(row, &transform);
        });
    } else {
        apply_native_rows(bytes, width, height, channels, |row| {
            native_byte_transform_bytes(row, channels, &transform);
        });
    }
    crate::compute::record_pipeline_operation_path("vector");
    true
}

/// In-place counterpart of [`native_invert`].
fn native_invert_in_place(
    img: &mut DynamicImage,
    mode: Option<&str>,
    invert_alpha: bool,
) -> bool {
    let Some(channels) = native_invert_layout(img, mode) else {
        return false;
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    if has_empty_native_bytes(img, channels) {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    record_native_row_work(width, height, channels);
    apply_native_rows(bytes, width, height, channels, |row| {
        invert_native_bytes(row, channels, invert_alpha || mode == Some("CMYK"));
    });
    crate::compute::record_pipeline_operation_path("vector");
    true
}

/// Apply a composed byte-domain point lookup without widening the native
/// `L`/`RGB` storage. Other modes are rejected by contextual preflight until
/// their alpha, palette, or typed-sample contracts have native kernels; this
/// helper never enters a packed scalar fallback.
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

fn native_lut_tables_for_channels(
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

/// Apply one LUT per native byte band. The table lookup is vectorized with
/// `u8x16`; only the interleaved-band gather/scatter is scalar because the
/// portable `wide` API has no byte-gather instruction.
fn native_lut_apply(
    bytes: &mut [u8],
    channels: usize,
    lut: &[u8],
) -> Option<(u64, u64)> {
    let tables = native_lut_tables_for_channels(lut, channels)?;
    let vector_len = bytes.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for start in (0..vector_len).step_by(16) {
        let mut input = [[0u8; 16]; 4];
        let mut lanes = [0usize; 4];
        let mut locations = [(0usize, 0usize); 16];
        for lane in 0..16 {
            let channel = (start + lane) % channels;
            let slot = lanes[channel];
            input[channel][slot] = bytes[start + lane];
            locations[lane] = (channel, slot);
            lanes[channel] += 1;
        }
        let output: [[u8; 16]; 4] = std::array::from_fn(|channel| {
            native_lut_chunk(u8x16::new(input[channel]), &tables[channel]).to_array()
        });
        for lane in 0..16 {
            let (channel, slot) = locations[lane];
            bytes[start + lane] = output[channel][slot];
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let mut scalar_tail = 0u64;
    for (index, value) in bytes[vector_len..].iter_mut().enumerate() {
        let channel = (vector_len + index) % channels;
        *value = lut[channel * 256 + usize::from(*value)];
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

fn native_point_lut_in_place(
    img: &mut DynamicImage,
    mode: Option<&str>,
    lut: &[u8],
) -> bool {
    let Some(channels) = native_point_channels(img, mode) else {
        return false;
    };
    if lut.len() != 256 * channels || img.as_bytes().len() < 16 {
        return false;
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let Some((vector_blocks, scalar_tail)) = native_lut_apply(bytes, channels, lut) else {
        return false;
    };
    record_native_row_work(width, height, channels);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    true
}

/// Whether an admitted operation can reuse an owned intermediate without
/// changing its concrete layout.  This is deliberately narrower than SIMD
/// capability: native-copy, blur, convolution, and other shape-changing
/// kernels still need their own output/scratch storage.
pub(crate) fn simd_in_place_supported(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::Invert | PipelineOp::InvertChops => native_invert_layout(img, mode).is_some(),
        PipelineOp::Solarize { .. }
        | PipelineOp::Posterize { .. } => native_byte_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels)),
        PipelineOp::Brightness { .. } => native_brightness_layout(img, mode)
            .is_some_and(|channels| has_nonempty_byte_data(img, channels)),
        PipelineOp::Eval { lut } => native_point_channels(img, mode)
            .is_some_and(|channels| lut.len() == 256 * channels && img.as_bytes().len() >= 16),
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => native_paste_plan_for_image(
            img,
            source,
            *x,
            *y,
            *w,
            *h,
            mask.as_ref(),
            *mask_alpha,
            mode,
        )
        .is_some(),
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            scale.is_finite()
                && *scale != 0.0
                && offset.is_finite()
                && native_chops_layout(img, mode)
                    .is_some_and(|channels| has_affine_vector_rows(img, channels))
        }
        _ => false,
    }
}

/// Reuse an owned SIMD-segment buffer for operations whose native byte
/// contract is unchanged.  Shape-changing operations deliberately return
/// `false` and continue through their allocating native kernel.
pub(crate) fn simd_execute_in_place(
    img: &mut DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    match op {
        PipelineOp::Invert => Ok(native_invert_in_place(img, mode, false)),
        PipelineOp::InvertChops => Ok(native_invert_in_place(img, mode, true)),
        PipelineOp::Solarize { threshold } => Ok(native_byte_transform_in_place(
            img,
            mode,
            |input| {
                input
                    .simd_ge(u8x16::splat(*threshold))
                    .select(u8x16::splat(u8::MAX) - input, input)
            },
        )),
        PipelineOp::Posterize { bits } => {
            let shift = 8u32
                .checked_sub(*bits as u32)
                .ok_or_else(|| PilError::ValueError("posterize bits must be at most 8".into()))?;
            Ok(native_byte_transform_in_place(
                img,
                mode,
                |input| (input >> shift) << shift,
            ))
        }
        PipelineOp::Brightness { factor } => {
            let factor_fp = (*factor * 1000.0) as u32;
            let lut: Vec<u8> = (0u32..=255)
                .map(|value| ((value as u64 * factor_fp as u64) / 1000).min(255) as u8)
                .collect();
            let Some(tables) = native_lut_tables(&lut) else {
                return Ok(false);
            };
            Ok(native_brightness_transform_in_place(
                img,
                mode,
                |input| native_lut_chunk(input, &tables),
            ))
        }
        PipelineOp::Paste {
            source,
            x,
            y,
            w,
            h,
            mask,
            mask_alpha,
        } => native_paste_in_place(
            img,
            source,
            *x,
            *y,
            *w,
            *h,
            mask.as_ref(),
            *mask_alpha,
            mode,
        ),
        PipelineOp::Eval { lut } => Ok(native_point_lut_in_place(img, mode, lut)),
        PipelineOp::Multiply { other } => {
            let other = materialize_chops_operand(other, mode)?;
            let channels = native_chops_pair_channels(&*img, &other, mode);
            let Some(channels) = channels else {
                return Ok(false);
            };
            let width = img.width() as usize;
            let height = img.height() as usize;
            let right = other.as_bytes();
            let Some(left) = img.as_bytes_mut() else {
                return Ok(false);
            };
            Ok(apply_native_blend_rows_in_place(
                left, right, width, height, channels, false,
            ))
        }
        PipelineOp::Screen { other } => {
            let other = materialize_chops_operand(other, mode)?;
            let channels = native_chops_pair_channels(&*img, &other, mode);
            let Some(channels) = channels else {
                return Ok(false);
            };
            let width = img.width() as usize;
            let height = img.height() as usize;
            let right = other.as_bytes();
            let Some(left) = img.as_bytes_mut() else {
                return Ok(false);
            };
            Ok(apply_native_blend_rows_in_place(
                left, right, width, height, channels, true,
            ))
        }
        PipelineOp::Darker { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left.min(right),
                |left, right| left.min(right),
            ))
        }
        PipelineOp::Lighter { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left.max(right),
                |left, right| left.max(right),
            ))
        }
        PipelineOp::Difference { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left.max(right) - left.min(right),
                |left, right| left.abs_diff(right),
            ))
        }
        PipelineOp::AddModulo { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left + right,
                |left, right| left.wrapping_add(right),
            ))
        }
        PipelineOp::SubtractModulo { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left - right,
                |left, right| left.wrapping_sub(right),
            ))
        }
        PipelineOp::LogicalAnd { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left & right,
                |left, right| left & right,
            ))
        }
        PipelineOp::LogicalOr { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left | right,
                |left, right| left | right,
            ))
        }
        PipelineOp::LogicalXor { other } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left ^ right,
                |left, right| left ^ right,
            ))
        }
        PipelineOp::Add {
            other,
            scale,
            offset,
        } if *scale == 1.0
            && *offset == 0.0
            && native_chops_layout(img, mode)
                .is_some_and(|channels| has_vectorized_byte_rows(img, channels)) =>
        {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left.saturating_add(right),
                |left, right| left.saturating_add(right),
            ))
        }
        PipelineOp::Subtract {
            other,
            scale,
            offset,
        } if *scale == 1.0
            && *offset == 0.0
            && native_chops_layout(img, mode)
                .is_some_and(|channels| has_vectorized_byte_rows(img, channels)) =>
        {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_bytewise_in_place(
                img,
                &other,
                mode,
                |left, right| left.saturating_sub(right),
                |left, right| left.saturating_sub(right),
            ))
        }
        PipelineOp::Add {
            other,
            scale,
            offset,
        } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_affine_in_place(
                img, &other, mode, *scale, *offset, false,
            ))
        }
        PipelineOp::Subtract {
            other,
            scale,
            offset,
        } => {
            let other = materialize_chops_operand(other, mode)?;
            Ok(native_chops_affine_in_place(
                img, &other, mode, *scale, *offset, true,
            ))
        }
        _ => Ok(false),
    }
}

pub(crate) fn native_point_lut(
    img: &DynamicImage,
    mode: Option<&str>,
    lut: &[u8],
) -> Option<DynamicImage> {
    let channels = native_point_channels(img, mode)?;
    if lut.len() != 256 * channels || img.as_bytes().len() < 16 {
        return None;
    }
    let mut result = img.clone();
    let width = result.width() as usize;
    let height = result.height() as usize;
    let bytes = result.as_bytes_mut()?;
    let (vector_blocks, scalar_tail) = native_lut_apply(bytes, channels, lut)?;
    record_native_row_work(width, height, channels);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Some(result)
}

#[derive(Clone, Copy)]
struct NativeConvertLayout {
    source_channels: usize,
    target_channels: usize,
    source_is_luma: bool,
    source_is_rgbx: bool,
    target_is_luma: bool,
    target_is_cmyk: bool,
    target_is_hsv: bool,
    target_is_ycbcr: bool,
    target_is_integer: bool,
    target_is_float: bool,
}

fn native_convert_target(mode: &ColorMode) -> Option<(usize, bool, bool, bool, bool)> {
    match mode {
        ColorMode::L => Some((1, true, false, false, false)),
        ColorMode::LA => Some((2, true, false, false, false)),
        ColorMode::RGB => Some((3, false, false, false, false)),
        ColorMode::RGBA => Some((4, false, false, false, false)),
        // Convert.c writes C/M/Y/K directly for byte RGB-family and luma
        // sources. The raster layer carries those four native samples in
        // Rgba8, but the fourth byte is K, never alpha.
        ColorMode::CMYK => Some((4, false, true, false, false)),
        // Pillow stores HSV and YCbCr in an RGB-sized byte buffer. These
        // destinations have distinct arithmetic contracts, so they are
        // admitted only through their dedicated native kernels below.
        ColorMode::HSV => Some((3, false, false, true, false)),
        ColorMode::YCbCr => Some((3, false, false, false, true)),
        // Pillow keeps I and F as four-byte scalar samples. The surrounding
        // image layer stores those bytes in Rgba8, but the logical target is
        // a luma sample, not an RGBA pixel; their arithmetic is handled by
        // the dedicated vector path below.
        ColorMode::I | ColorMode::F => Some((4, false, false, false, false)),
        _ => None,
    }
}

fn native_convert_luma16_supported(
    img: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
) -> bool {
    matches!(
        target,
        ColorMode::L | ColorMode::LA | ColorMode::RGB | ColorMode::RGBA | ColorMode::CMYK
    )
        && matches!(
            mode,
            Some("I;16" | "I;16L" | "I;16B" | "I;16N")
        )
        && matches!(img, DynamicImage::ImageLuma16(_))
        && (img.width() as usize)
            .checked_mul(img.height() as usize)
            .is_some_and(|pixels| pixels != 0)
}

/// Convert native unsigned-16-bit luma to the byte L/LA destinations.
///
/// Pillow's I;16 converter clamps each unsigned sample to the byte range; it
/// does not scale 65535 down to 255. The sample load and clamp use eight
/// `u16` lanes, then the result is packed into the native byte destination.
/// This stays separate from the ordinary byte converter so raw endianness is
/// resolved once by `Image::frombytes` and is never reinterpreted as pixels.
fn native_convert_luma16_bytes(
    img: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
) -> Option<(Vec<u8>, u64, u64)> {
    if !native_convert_luma16_supported(img, target, mode) {
        return None;
    }
    let DynamicImage::ImageLuma16(source) = img else {
        return None;
    };
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    if source.as_raw().len() != pixels {
        return None;
    }
    let source = source.as_raw();
    let output_channels = match target {
        ColorMode::L => 1,
        ColorMode::LA => 2,
        ColorMode::RGB => 3,
        ColorMode::RGBA | ColorMode::CMYK => 4,
        _ => return None,
    };
    let mut output = Vec::with_capacity(pixels * output_channels);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let samples = std::array::from_fn(|lane| {
            if lane < active {
                source[start + lane]
            } else {
                0
            }
        });
        let samples = u16x8::new(samples)
            .simd_gt(u16x8::splat(u16::from(u8::MAX)))
            .select(u16x8::splat(u16::from(u8::MAX)), u16x8::new(samples))
            .to_array()
            .map(|value| value as u8);
        for &value in samples.iter().take(active) {
            match target {
                ColorMode::L => output.push(value),
                ColorMode::LA => output.extend_from_slice(&[value, u8::MAX]),
                ColorMode::RGB => output.extend_from_slice(&[value, value, value]),
                ColorMode::RGBA => output.extend_from_slice(&[value, value, value, u8::MAX]),
                ColorMode::CMYK => {
                    output.extend_from_slice(&[0, 0, 0, u8::MAX.saturating_sub(value)])
                }
                _ => return None,
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

fn native_convert_layout(
    img: &DynamicImage,
    target: &ColorMode,
    mode: Option<&str>,
) -> Option<NativeConvertLayout> {
    let (
        target_channels,
        target_is_luma,
        target_is_cmyk,
        target_is_hsv,
        target_is_ycbcr,
    ) = native_convert_target(target)?;
    let source_channels = if target_is_hsv || target_is_ycbcr {
        match img {
            DynamicImage::ImageRgba8(_) if mode == Some("RGBX") => 4,
            _ => native_byte_layout(img, mode)?,
        }
    } else {
        native_byte_layout(img, mode)?
    };
    Some(NativeConvertLayout {
        source_channels,
        target_channels,
        source_is_luma: source_channels <= 2,
        source_is_rgbx: mode == Some("RGBX"),
        target_is_luma,
        target_is_cmyk,
        target_is_hsv,
        target_is_ycbcr,
        target_is_integer: matches!(target, ColorMode::I),
        target_is_float: matches!(target, ColorMode::F),
    })
}

fn native_convert_pixels_per_vector(layout: NativeConvertLayout) -> usize {
    if layout.target_is_cmyk {
        // Four output samples are emitted per input pixel. The inverse and
        // luma-to-K subtraction both run in byte vector lanes.
        4
    } else if layout.target_is_hsv || layout.target_is_ycbcr {
        // Both color-space kernels process eight RGB-family pixels per wide
        // block. Their output is three bytes per pixel, so the block is
        // gathered/scattered by lane while the arithmetic stays vectorized.
        8
    } else if layout.target_is_integer || layout.target_is_float {
        // I/F emit one four-byte scalar per pixel. Gather eight source
        // pixels and vectorize the luma arithmetic before packing the native
        // little-endian samples.
        8
    } else if !layout.source_is_luma && layout.target_is_luma {
        // The luma formula uses eight u32 lanes. Input bytes are gathered
        // into those lanes before the vector arithmetic begins.
        8
    } else {
        // The layout-only path loads one padded u8x16 block and stores at
        // most one u8x16 block. This also handles three-byte RGB pixels
        // without converting them through a packed RGBA representation.
        16 / layout.source_channels.max(layout.target_channels)
    }
}

fn native_convert_supported_for_image(
    img: &DynamicImage,
    target: &ColorMode,
    matrix: Option<&[f64]>,
    mode: Option<&str>,
) -> bool {
    // Pillow ignores the dither enum for these standard byte destinations;
    // only matrix conversion changes the arithmetic contract.
    if matrix.is_some() {
        return false;
    }
    if native_convert_luma16_supported(img, target, mode) {
        return true;
    }
    let Some(layout) = native_convert_layout(img, target, mode) else {
        return false;
    };
    let Some(pixel_count) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    let Some(expected_bytes) = pixel_count.checked_mul(layout.source_channels) else {
        return false;
    };
    // Every native converter pads its final vector block. This includes
    // one-pixel images: the inactive lanes are zero-filled and never copied
    // to the output, so a short public input still exercises the real SIMD
    // data path instead of being rejected solely for its width.
    let pixel_count_supported = pixel_count != 0;
    pixel_count_supported && img.as_bytes().len() == expected_bytes
}

fn native_convert_shape_layout(
    shape: SimdImageShape,
    target: &ColorMode,
    mode: Option<&str>,
) -> Option<NativeConvertLayout> {
    let (
        target_channels,
        target_is_luma,
        target_is_cmyk,
        target_is_hsv,
        target_is_ycbcr,
    ) = native_convert_target(target)?;
    let source_channels = if target_is_hsv || target_is_ycbcr {
        match (shape.layout, mode) {
            (SimdLayout::Rgba8, Some("RGBX")) => 4,
            _ => shape_native_byte_channels(shape, mode)?,
        }
    } else {
        shape_native_byte_channels(shape, mode)?
    };
    Some(NativeConvertLayout {
        source_channels,
        target_channels,
        source_is_luma: source_channels <= 2,
        source_is_rgbx: mode == Some("RGBX"),
        target_is_luma,
        target_is_cmyk,
        target_is_hsv,
        target_is_ycbcr,
        target_is_integer: matches!(target, ColorMode::I),
        target_is_float: matches!(target, ColorMode::F),
    })
}

fn native_convert_supported_for_shape(
    shape: SimdImageShape,
    target: &ColorMode,
    matrix: Option<&[f64]>,
    mode: Option<&str>,
) -> bool {
    if matrix.is_some() {
        return false;
    }
    if matches!(
        target,
        ColorMode::L | ColorMode::LA | ColorMode::RGB | ColorMode::RGBA | ColorMode::CMYK
    )
        && matches!(
            mode,
            Some("I;16" | "I;16L" | "I;16B" | "I;16N")
        )
        && matches!(shape.layout, SimdLayout::Luma16)
    {
        return (shape.width as usize)
            .checked_mul(shape.height as usize)
            .is_some_and(|pixels| pixels != 0);
    }
    let Some(_layout) = native_convert_shape_layout(shape, target, mode) else {
        return false;
    };
    let pixel_count = (shape.width as usize).saturating_mul(shape.height as usize);
    // The image adapter zero-pads incomplete final blocks, so shape-only
    // preflight must admit the same nonempty inputs.
    pixel_count != 0
}

/// Convert native byte luma/RGB-family samples to Pillow's four-byte CMYK
/// representation. Convert.c's RGB inverse is C=255-R, M=255-G, Y=255-B,
/// K=0; its luma branch is C=M=Y=0, K=255-L. Keep the input and output in
/// their native interleaved layouts and vectorize the per-channel subtraction.
fn native_convert_cmyk_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(4)?;
    if source.len() != expected_source_bytes {
        return None;
    }

    let pixels_per_vector = 4usize;
    let mut output = Vec::with_capacity(expected_output_bytes);
    let mut vector_blocks = 0u64;
    for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
        let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
        let mut first = [0u8; 16];
        let mut second = [0u8; 16];
        let mut third = [0u8; 16];
        for lane in 0..pixels_per_vector {
            if lane < active_pixels {
                let source_start = (start_pixel + lane) * layout.source_channels;
                first[lane] = source[source_start];
                if !layout.source_is_luma {
                    second[lane] = source[source_start + 1];
                    third[lane] = source[source_start + 2];
                }
            }
        }
        let first = (u8x16::splat(255) - u8x16::new(first)).to_array();
        let second = (u8x16::splat(255) - u8x16::new(second)).to_array();
        let third = (u8x16::splat(255) - u8x16::new(third)).to_array();
        for lane in 0..active_pixels {
            let source_start = (start_pixel + lane) * layout.source_channels;
            let k = if layout.source_is_luma {
                255u8.saturating_sub(source[source_start])
            } else {
                0
            };
            let pixel = if layout.source_is_luma {
                [0, 0, 0, k]
            } else {
                [first[lane], second[lane], third[lane], k]
            };
            output.extend_from_slice(&pixel);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

fn native_ycbcr_table(coeff: f64) -> [i32; 256] {
    let mut table = [0i32; 256];
    for (value, output) in table.iter_mut().enumerate() {
        *output = (value as f64 * coeff * 64.0 + 0.5) as i32;
    }
    table
}

fn native_ycbcr_tables() -> (
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
    &'static [i32; 256],
) {
    static Y_R: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_G: OnceLock<[i32; 256]> = OnceLock::new();
    static Y_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_R: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CB_B: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_G: OnceLock<[i32; 256]> = OnceLock::new();
    static CR_B: OnceLock<[i32; 256]> = OnceLock::new();

    (
        Y_R.get_or_init(|| native_ycbcr_table(0.299)),
        Y_G.get_or_init(|| native_ycbcr_table(0.587)),
        Y_B.get_or_init(|| native_ycbcr_table(0.114)),
        CB_R.get_or_init(|| native_ycbcr_table(-0.16874)),
        CB_G.get_or_init(|| native_ycbcr_table(-0.33126)),
        CB_B.get_or_init(|| native_ycbcr_table(0.5)),
        CR_G.get_or_init(|| native_ycbcr_table(-0.41869)),
        CR_B.get_or_init(|| native_ycbcr_table(-0.08131)),
    )
}

/// Convert native luma/RGB-family bytes to Pillow's YCbCr storage. The table
/// gathers are scalar because portable `wide` has no byte-gather primitive;
/// the fixed-point additions, shifts, and per-lane result arithmetic remain
/// vector operations. Luma sources use Pillow's direct neutral-chroma path.
fn native_convert_ycbcr_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    if !layout.target_is_ycbcr || layout.target_channels != 3 {
        return None;
    }
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(3)?;
    if source.len() != expected_source_bytes {
        return None;
    }

    let pixels_per_vector = 8usize;
    let mut output = Vec::with_capacity(expected_output_bytes);
    let mut vector_blocks = 0u64;
    let (y_r, y_g, y_b, cb_r, cb_g, cb_b, cr_g, cr_b) = native_ycbcr_tables();

    for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
        let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
        let red_values = std::array::from_fn(|lane| {
            if lane < active_pixels {
                source[(start_pixel + lane) * layout.source_channels]
            } else {
                0
            }
        });
        let (y, cb, cr) = if layout.source_is_luma {
            let gray = i32x8::new(red_values.map(i32::from));
            (gray, i32x8::splat(128), i32x8::splat(128))
        } else {
            let green = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 1]
                } else {
                    0
                }
            });
            let blue = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 2]
                } else {
                    0
                }
            });
            let red = i32x8::new(red_values.map(|value| y_r[usize::from(value)]));
            let green_y = i32x8::new(green.map(|value| y_g[usize::from(value)]));
            let blue_y = i32x8::new(blue.map(|value| y_b[usize::from(value)]));
            let y = (red + green_y + blue_y) >> 6u32;

            let red_cb = i32x8::new(std::array::from_fn(|lane| {
                cb_r[usize::from(red_values[lane])]
            }));
            let green_cb = i32x8::new(green.map(|value| cb_g[usize::from(value)]));
            let blue_cb = i32x8::new(blue.map(|value| cb_b[usize::from(value)]));
            let cb = ((red_cb + green_cb + blue_cb) >> 6u32) + i32x8::splat(128);

            let red_cr = i32x8::new(std::array::from_fn(|lane| {
                cb_b[usize::from(red_values[lane])]
            }));
            let green_cr = i32x8::new(green.map(|value| cr_g[usize::from(value)]));
            let blue_cr = i32x8::new(blue.map(|value| cr_b[usize::from(value)]));
            let cr = ((red_cr + green_cr + blue_cr) >> 6u32) + i32x8::splat(128);
            (y, cb, cr)
        };
        let y = y.to_array().map(|value| value.clamp(0, 255) as u8);
        let cb = cb.to_array().map(|value| value.clamp(0, 255) as u8);
        let cr = cr.to_array().map(|value| value.clamp(0, 255) as u8);
        for lane in 0..active_pixels {
            output.extend_from_slice(&[y[lane], cb[lane], cr[lane]]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

/// Convert native luma/RGB-family bytes to Pillow's HSV storage. The max/min
/// and ratio arithmetic uses wide floating-point lanes; per-lane max-channel
/// selection is scalar control, matching Pillow's tie-breaking order exactly.
fn native_convert_hsv_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    if !layout.target_is_hsv || layout.target_channels != 3 {
        return None;
    }
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(3)?;
    if source.len() != expected_source_bytes {
        return None;
    }

    let pixels_per_vector = 8usize;
    let mut output = Vec::with_capacity(expected_output_bytes);
    let mut vector_blocks = 0u64;

    for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
        let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
        let red = std::array::from_fn(|lane| {
            if lane < active_pixels {
                source[(start_pixel + lane) * layout.source_channels]
            } else {
                0
            }
        });
        let max_bytes = if layout.source_is_luma {
            red
        } else {
            let green = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 1]
                } else {
                    0
                }
            });
            let blue = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 2]
                } else {
                    0
                }
            });
            let red_vector = f32x8::new(red.map(f32::from));
            let green_vector = f32x8::new(green.map(f32::from));
            let blue_vector = f32x8::new(blue.map(f32::from));
            let max_vector = red_vector.max(green_vector.max(blue_vector));
            let min_vector = red_vector.min(green_vector.min(blue_vector));
            let delta_vector = max_vector - min_vector;
            let safe_delta = delta_vector
                .simd_eq(f32x8::splat(0.0))
                .select(f32x8::splat(1.0), delta_vector);
            let rc = (max_vector - red_vector) / safe_delta;
            let gc = (max_vector - green_vector) / safe_delta;
            let bc = (max_vector - blue_vector) / safe_delta;

            // PIL applies the sector constants in double precision after the
            // f32 ratios have been computed. Build all three candidates in
            // f64 lanes, then select the candidate per lane below.
            let red_h = f64x8::new(bc.to_array().map(f64::from))
                - f64x8::new(gc.to_array().map(f64::from));
            let green_h = f64x8::splat(2.0)
                + f64x8::new(rc.to_array().map(f64::from))
                - f64x8::new(bc.to_array().map(f64::from));
            let blue_h = f64x8::splat(4.0)
                + f64x8::new(gc.to_array().map(f64::from))
                - f64x8::new(rc.to_array().map(f64::from));
            let red_h = red_h.to_array();
            let green_h = green_h.to_array();
            let blue_h = blue_h.to_array();
            let saturation = (delta_vector
                / max_vector
                    .simd_eq(f32x8::splat(0.0))
                    .select(f32x8::splat(1.0), max_vector))
            .to_array();
            let max_values = max_vector.to_array().map(|value| value as u8);
            let delta_values = delta_vector.to_array();
            let red_values = red;
            let green_values = green;
            let selected_h = std::array::from_fn(|lane| {
                if red_values[lane] == max_values[lane] {
                    red_h[lane]
                } else if green_values[lane] == max_values[lane] {
                    green_h[lane]
                } else {
                    blue_h[lane]
                }
            });
            // Pillow stores the sector result in a C `float` before applying
            // the hue wrap. Preserve that narrowing step per lane before the
            // vectorized f64 divide/add; otherwise values such as 0.8 become
            // 0.799999997 in the wider lanes and truncate one HSV byte low.
            let selected_h = selected_h.map(|value| (value as f32) as f64);
            // fmod(x, 1.0) is equivalent here to subtracting floor(x): the
            // selected hue is bounded by Pillow's RGB sector construction.
            let hue = f64x8::new(selected_h) / f64x8::splat(6.0) + f64x8::splat(1.0);
            let hue = (hue - hue.floor()).to_array();
            for lane in 0..active_pixels {
                if delta_values[lane] == 0.0 {
                    output.extend_from_slice(&[0, 0, max_values[lane]]);
                    continue;
                }
                let h_stored = hue[lane] as f32;
                let hue_byte = (f64::from(h_stored) * 255.0) as u8;
                let saturation_byte = (f64::from(saturation[lane]) * 255.0) as u8;
                output.extend_from_slice(&[hue_byte, saturation_byte, max_values[lane]]);
            }
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        };

        // L/LA -> HSV is Pillow's RGB normalization followed by the gray
        // branch: H=0, S=0, V=L. The lane construction above is unnecessary
        // for this mode, but the block still uses one vectorized byte group.
        for value in max_bytes.into_iter().take(active_pixels) {
            output.extend_from_slice(&[0, 0, value]);
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((output, vector_blocks, 0))
}

#[inline]
fn native_convert_layout_block(
    input: &[u8],
    layout: NativeConvertLayout,
    pixels: usize,
) -> Option<[u8; 16]> {
    let input_len = pixels.checked_mul(layout.source_channels)?;
    let output_len = pixels.checked_mul(layout.target_channels)?;
    if input.len() < input_len || input_len > 16 || output_len > 16 {
        return None;
    }

    let mut padded = [0u8; 16];
    padded[..input_len].copy_from_slice(&input[..input_len]);
    let opaque_index = 15usize;
    if (layout.target_channels == 2 && layout.source_is_luma && layout.source_channels == 1)
        || (layout.target_channels == 4
            && ((layout.source_is_luma && layout.source_channels == 1)
                || (!layout.source_is_luma && layout.source_channels == 3)
                || layout.source_is_rgbx))
    {
        // Every block needing an inserted alpha has fewer than sixteen input
        // bytes, so lane 15 is outside the source payload.
        padded[opaque_index] = u8::MAX;
    }

    let mut indices = [0u8; 16];
    for pixel in 0..pixels {
        for channel in 0..layout.target_channels {
            let source_index = if layout.source_is_luma {
                if channel == 0 || (layout.target_channels == 3 && channel < 3) {
                    pixel * layout.source_channels
                } else if channel == 3 || (layout.target_channels == 2 && channel == 1) {
                    if layout.source_channels == 2 {
                        pixel * layout.source_channels + 1
                    } else {
                        opaque_index
                    }
                } else {
                    pixel * layout.source_channels
                }
            } else if channel < 3 {
                pixel * layout.source_channels + channel
            } else if layout.source_channels == 4 && !layout.source_is_rgbx {
                pixel * layout.source_channels + 3
            } else {
                opaque_index
            };
            indices[pixel * layout.target_channels + channel] =
                u8::try_from(source_index).ok()?;
        }
    }
    Some(
        u8x16::new(padded)
            .swizzle_relaxed(u8x16::new(indices))
            .to_array(),
    )
}

fn native_convert_bytes(
    img: &DynamicImage,
    layout: NativeConvertLayout,
) -> Option<(Vec<u8>, u64, u64)> {
    if layout.target_is_cmyk {
        return native_convert_cmyk_bytes(img, layout);
    }
    if layout.target_is_hsv {
        return native_convert_hsv_bytes(img, layout);
    }
    if layout.target_is_ycbcr {
        return native_convert_ycbcr_bytes(img, layout);
    }
    let source = img.as_bytes();
    let pixel_count = (img.width() as usize).checked_mul(img.height() as usize)?;
    let expected_source_bytes = pixel_count.checked_mul(layout.source_channels)?;
    let expected_output_bytes = pixel_count.checked_mul(layout.target_channels)?;
    if source.len() != expected_source_bytes {
        return None;
    }
    let mut output = Vec::with_capacity(expected_output_bytes);
    let pixels_per_vector = native_convert_pixels_per_vector(layout);
    let mut vector_blocks = 0u64;

    if layout.target_is_integer {
        for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
            let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
            let red = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels]
                } else {
                    0
                }
            });
            let values = if layout.source_is_luma {
                u32x8::new(red.map(u32::from))
                    .to_array()
                    .map(|value| value as i32)
            } else {
                let green = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 1]
                    } else {
                        0
                    }
                });
                let blue = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 2]
                    } else {
                        0
                    }
                });
                (u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
                    + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
                    + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
                    + u32x8::splat(32768))
                    .to_array()
                    .map(|value| (value >> 16) as i32)
            };
            for value in values.into_iter().take(active_pixels) {
                output.extend_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if layout.target_is_float {
        for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
            let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
            let red = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels]
                } else {
                    0
                }
            });
            let values = if layout.source_is_luma {
                f32x8::new(red.map(f32::from)).to_array()
            } else {
                let green = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 1]
                    } else {
                        0
                    }
                });
                let blue = std::array::from_fn(|lane| {
                    if lane < active_pixels {
                        source[(start_pixel + lane) * layout.source_channels + 2]
                    } else {
                        0
                    }
                });
                let sum = u32x8::new(red.map(u32::from)) * u32x8::splat(299)
                    + u32x8::new(green.map(u32::from)) * u32x8::splat(587)
                    + u32x8::new(blue.map(u32::from)) * u32x8::splat(114);
                (f32x8::new(sum.to_array().map(|value| value as f32))
                    / f32x8::splat(1000.0))
                .to_array()
            };
            for value in values.into_iter().take(active_pixels) {
                output.extend_from_slice(&value.to_le_bytes());
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }

    if !layout.target_is_integer
        && !layout.target_is_float
        && !layout.source_is_luma
        && layout.target_is_luma
    {
        // RGB/RGBA to L/LA: the byte gather is scalar because `wide` has no
        // portable byte-gather instruction, but the complete luma arithmetic
        // runs in eight u32 lanes. Alpha is only gathered for the LA result.
        for start_pixel in (0..pixel_count).step_by(8) {
            let active_pixels = (pixel_count - start_pixel).min(8);
            let red = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels]
                } else {
                    0
                }
            });
            let green = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 1]
                } else {
                    0
                }
            });
            let blue = std::array::from_fn(|lane| {
                if lane < active_pixels {
                    source[(start_pixel + lane) * layout.source_channels + 2]
                } else {
                    0
                }
            });
            let luma = (u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
                + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
                + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
                + u32x8::splat(32768))
                >> 16u32;
            let gray = luma.to_array().map(|value| value.min(255) as u8);
            if layout.target_channels == 1 {
                let mut packed = [0u8; 16];
                packed[..8].copy_from_slice(&gray);
                output.extend_from_slice(&u8x16::new(packed).to_array()[..active_pixels]);
            } else {
                let alpha: [u8; 8] = std::array::from_fn(|lane| {
                    if lane < active_pixels
                        && layout.source_channels == 4
                        && !layout.source_is_rgbx
                    {
                        source[(start_pixel + lane) * layout.source_channels + 3]
                    } else {
                        u8::MAX
                    }
                });
                let mut gray_lanes = [0u8; 16];
                gray_lanes[..8].copy_from_slice(&gray);
                let mut alpha_lanes = [0u8; 16];
                alpha_lanes[..8].copy_from_slice(&alpha);
                let duplicate = u8x16::new([
                    0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7,
                ]);
                let even = u8x16::new([
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
                ]);
                let odd = u8x16::splat(u8::MAX) ^ even;
                let interleaved =
                    (u8x16::new(gray_lanes).swizzle_relaxed(duplicate) & even)
                        | (u8x16::new(alpha_lanes).swizzle_relaxed(duplicate) & odd);
                output.extend_from_slice(&interleaved.to_array()[..active_pixels * 2]);
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if !layout.target_is_integer && !layout.target_is_float {
        for start_pixel in (0..pixel_count).step_by(pixels_per_vector) {
            let active_pixels = (pixel_count - start_pixel).min(pixels_per_vector);
            let source_start = start_pixel * layout.source_channels;
            let input_len = active_pixels * layout.source_channels;
            let mut padded = [0u8; 16];
            padded[..input_len].copy_from_slice(&source[source_start..source_start + input_len]);
            let block = native_convert_layout_block(&padded, layout, pixels_per_vector)?;
            let output_len = active_pixels * layout.target_channels;
            output.extend_from_slice(&block[..output_len]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Some((output, vector_blocks, 0))
}

/// Apply one of the exact byte-domain ImageChops blend formulas without
/// widening ordinary images to packed RGBA pixels.
///
/// Indexed, typed, and mode-converted images are rejected during SIMD
/// preflight; this helper never widens them into a packed scalar fallback.
/// These four native variants are the common public byte layouts, and
/// preserving them avoids two full-frame conversions for every dual-image
/// operation in a pipeline.
#[inline]
fn native_blend_byte(left: u8, right: u8, screen: bool) -> u8 {
    if screen {
        255 - ((255 - left) as u16 * (255 - right) as u16 / 255) as u8
    } else {
        (left as u16 * right as u16 / 255) as u8
    }
}

#[inline]
fn native_blend_vector8(left: [u8; 8], right: [u8; 8], screen: bool) -> [u8; 8] {
    let left = u16x8::new(left.map(u16::from));
    let right = u16x8::new(right.map(u16::from));
    let values = if screen {
        u16x8::splat(255) - simd_div255_u16x8(
            (u16x8::splat(255) - left) * (u16x8::splat(255) - right),
        )
    } else {
        simd_div255_u16x8(left * right)
    };
    values.to_array().map(|value| value as u8)
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
    let vector_blocks = (row_stride / 8).saturating_mul(height);
    let scalar_tail = (row_stride % 8).saturating_mul(height);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
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
            let left = u16x16::from(u8x16::new(left_chunk));
            let right = u16x16::from(u8x16::new(right_chunk));
            let values = if screen {
                let inverse_left = u16x16::splat(255) - left;
                let inverse_right = u16x16::splat(255) - right;
                u16x16::splat(255) - simd_div255(inverse_left * inverse_right)
            } else {
                simd_div255(left * right)
            };
            output_row[start..start + 16].copy_from_slice(&simd_pack_u16x16(values).to_array());
        }
        let vector_len8 = output_row.len() / 8 * 8;
        for start in (vector_len..vector_len8).step_by(8) {
            let Ok(left_chunk) = <[u8; 8]>::try_from(&left_row[start..start + 8]) else {
                return;
            };
            let Ok(right_chunk) = <[u8; 8]>::try_from(&right_row[start..start + 8]) else {
                return;
            };
            output_row[start..start + 8]
                .copy_from_slice(&native_blend_vector8(left_chunk, right_chunk, screen));
        }
        for ((output, &left), &right) in output_row[vector_len8..]
            .iter_mut()
            .zip(&left_row[vector_len8..])
            .zip(&right_row[vector_len8..])
        {
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

/// Apply the same blend kernel while reusing the owned destination buffer.
/// `left` is mutated only after each sixteen-byte source block has been
/// copied into a value-owned vector, so the in-place path is equivalent to the
/// allocating path even when the output aliases the left operand.
fn apply_native_blend_rows_in_place(
    left: &mut [u8],
    right: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    screen: bool,
) -> bool {
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return false;
    }
    let vector_blocks = (row_stride / 8).saturating_mul(height);
    let scalar_tail = (row_stride % 8).saturating_mul(height);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }

    for row_index in 0..height {
        let row_start = row_index * row_stride;
        let left_row = &mut left[row_start..row_start + row_stride];
        let right_row = &right[row_start..row_start + row_stride];
        let vector_len = left_row.len() / 16 * 16;
        for start in (0..vector_len).step_by(16) {
            let left = u16x16::from(u8x16::new(
                <[u8; 16]>::try_from(&left_row[start..start + 16])
                    .expect("native blend block has 16 bytes"),
            ));
            let right = u16x16::from(u8x16::new(
                <[u8; 16]>::try_from(&right_row[start..start + 16])
                    .expect("native blend block has 16 bytes"),
            ));
            let values = if screen {
                let inverse_left = u16x16::splat(255) - left;
                let inverse_right = u16x16::splat(255) - right;
                u16x16::splat(255) - simd_div255(inverse_left * inverse_right)
            } else {
                simd_div255(left * right)
            };
            left_row[start..start + 16]
                .copy_from_slice(&simd_pack_u16x16(values).to_array());
        }
        let vector_len8 = left_row.len() / 8 * 8;
        for start in (vector_len..vector_len8).step_by(8) {
            let left_chunk = <[u8; 8]>::try_from(&left_row[start..start + 8])
                .expect("native blend block has 8 bytes");
            let right_chunk = <[u8; 8]>::try_from(&right_row[start..start + 8])
                .expect("native blend block has 8 bytes");
            left_row[start..start + 8]
                .copy_from_slice(&native_blend_vector8(left_chunk, right_chunk, screen));
        }
        for index in vector_len8..left_row.len() {
            left_row[index] = native_blend_byte(left_row[index], right_row[index], screen);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    true
}

fn native_chops_blend(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    screen: bool,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return None;
    };
    if img.as_bytes().len() != expected_len || other.as_bytes().len() != expected_len {
        return None;
    }
    if expected_len == 0 {
        // Pillow returns a typed zero-pixel result without entering the blend
        // loop. The source clone preserves the logical mode at the pipeline
        // boundary and records a genuine native no-work path.
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    let mut output = vec![0u8; expected_len];
    if !apply_native_blend_rows(
        img.as_bytes(),
        other.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        screen,
    ) {
        return None;
    }
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result));
    if result.is_some() {
        crate::compute::record_pipeline_operation_path("vector");
    }
    result
}

/// Apply an exact byte-wise Chops operation without widening the native image.
///
/// These formulas are lane-local and preserve Pillow's byte semantics exactly:
/// min/max, absolute difference, modulo add/subtract, and logical operations
/// do not need the widening and rounding used by multiply/screen. The helper
/// keeps the scalar tail outside the vector loop and returns `None` when the
/// logical mode does not match a supported native byte layout.
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
    let channels = native_chops_pair_channels(img, other, mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || left.len() % channels != 0 {
        return None;
    }
    let Some(expected_len) = img
        .width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(channels as u32))
    else {
        return None;
    };
    if expected_len as usize != left.len() {
        return None;
    }
    if left.is_empty() {
        // Empty Chops is a valid native zero-work operation. Do not pass the
        // empty buffer through raw image reconstruction, whose normal typed
        // constructor intentionally requires at least one pixel.
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if left.len() < 16 {
        return None;
    }

    let mut output = vec![0u8; left.len()];
    let vector_len = output.len() / 16 * 16;
    for start in (0..vector_len).step_by(16) {
        let left_chunk = u8x16::new(<[u8; 16]>::try_from(&left[start..start + 16]).ok()?);
        let right_chunk = u8x16::new(<[u8; 16]>::try_from(&right[start..start + 16]).ok()?);
        output[start..start + 16].copy_from_slice(&vector_op(left_chunk, right_chunk).to_array());
    }
    for index in vector_len..output.len() {
        output[index] = scalar_op(left[index], right[index]);
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_len / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((output.len() - vector_len) as u64);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

/// Apply a lane-local Chops operation to an owned destination buffer.  The
/// secondary operand remains immutable, so this removes only the unnecessary
/// destination allocation and never changes the public two-image contract.
fn native_chops_bytewise_in_place<F, G>(
    img: &mut DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    vector_op: F,
    scalar_op: G,
) -> bool
where
    F: Fn(u8x16, u8x16) -> u8x16,
    G: Fn(u8, u8) -> u8,
{
    let channels = match native_chops_pair_channels(&*img, other, mode) {
        Some(channels) => channels,
        None => return false,
    };
    let right = other.as_bytes();
    let width = img.width() as usize;
    let height = img.height() as usize;
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    let Some(left) = img.as_bytes_mut() else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return false;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    if left.len() < 16 {
        return false;
    }
    let vector_len = left.len() / 16 * 16;
    for start in (0..vector_len).step_by(16) {
        let left_chunk = u8x16::new(
            <[u8; 16]>::try_from(&left[start..start + 16])
                .expect("native Chops block has 16 bytes"),
        );
        let right_chunk = u8x16::new(
            <[u8; 16]>::try_from(&right[start..start + 16])
                .expect("native Chops block has 16 bytes"),
        );
        left[start..start + 16].copy_from_slice(&vector_op(left_chunk, right_chunk).to_array());
    }
    for index in vector_len..left.len() {
        left[index] = scalar_op(left[index], right[index]);
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_len / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((left.len() - vector_len) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    true
}

/// Exact integer division by 127 for the product range used by Overlay and
/// HardLight.  For `0 <= value <= 255 * 255`, the reciprocal multiply below
/// produces `floor(value / 127)` for every lane; it is not the common `/ 255`
/// approximation used by the other blend modes.
#[inline]
fn simd_div127(value: u32x8) -> u32x8 {
    ((value + u32x8::splat(1)) * u32x8::splat(16_513)) >> 21u32
}

#[inline]
fn native_chops_lut_vector(left: [u8; 8], right: [u8; 8], hard_light: bool) -> [u8; 8] {
    let left = u32x8::new(left.map(u32::from));
    let right = u32x8::new(right.map(u32::from));
    let low = simd_div127(left * right);
    let inverse_left = u32x8::splat(255) - left;
    let inverse_right = u32x8::splat(255) - right;
    let high = u32x8::splat(255) - simd_div127(inverse_left * inverse_right);
    let low_condition = if hard_light {
        right.simd_lt(u32x8::splat(128))
    } else {
        left.simd_lt(u32x8::splat(128))
    };
    low_condition
        .select(low, high)
        .to_array()
        .map(|value| value as u8)
}

#[inline]
fn native_chops_lut_byte(left: u8, right: u8, hard_light: bool) -> u8 {
    if if hard_light { right } else { left } < 128 {
        (u32::from(left) * u32::from(right) / 127) as u8
    } else {
        (255
            - (u32::from(255 - left) * u32::from(255 - right) / 127)) as u8
    }
}

/// Apply Pillow's exact 256×256 Overlay/HardLight LUT formula to native
/// bytes.  The operation is independent for each stored sample, so the
/// vector stream may cross image-row boundaries.  This is important for
/// small-width images: they still receive a real SIMD data path when the
/// frame contains at least one complete eight-byte vector.
fn native_chops_lut_formula(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    hard_light: bool,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || left.len() % channels != 0 {
        return None;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if left.len() < 8 {
        return None;
    }
    let mut output = vec![0u8; left.len()];
    let vector_len = output.len() / 8 * 8;
    for start in (0..vector_len).step_by(8) {
        let left_block = <[u8; 8]>::try_from(&left[start..start + 8]).ok()?;
        let right_block = <[u8; 8]>::try_from(&right[start..start + 8]).ok()?;
        output[start..start + 8]
            .copy_from_slice(&native_chops_lut_vector(left_block, right_block, hard_light));
    }
    for index in vector_len..output.len() {
        output[index] = native_chops_lut_byte(left[index], right[index], hard_light);
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_len / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((output.len() - vector_len) as u64);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

fn native_chops_overlay(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    native_chops_lut_formula(img, other, mode, false)
}

fn native_chops_hard_light(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    native_chops_lut_formula(img, other, mode, true)
}

/// Evaluate eight exact Pillow SoftLight samples with integer SIMD lanes.
///
/// Pillow's CHOP2 implementation is:
/// `((255-a)*a*b)/65536 + (a*(255-((255-a)*(255-b)/255)))/255`.
/// The intermediate divisions are truncating integer divisions, so preserve
/// them explicitly instead of using an approximate floating-point formula.
#[inline]
fn native_chops_soft_light_vector(left: [u8; 8], right: [u8; 8]) -> [u8; 8] {
    let a = u32x8::new(left.map(u32::from));
    let b = u32x8::new(right.map(u32::from));
    let inverse_a = u32x8::splat(255) - a;
    let term1 = (inverse_a * a * b) >> 16u32;

    let inverse_product = inverse_a * (u32x8::splat(255) - b);
    let divided_inverse = u32x8::new(
        simd_div255_u16x8(u16x8::new(inverse_product.to_array().map(|value| value as u16)))
            .to_array()
            .map(u32::from),
    );
    let term2_product = a * (u32x8::splat(255) - divided_inverse);
    let term2 = u32x8::new(
        simd_div255_u16x8(u16x8::new(term2_product.to_array().map(|value| value as u16)))
            .to_array()
            .map(u32::from),
    );
    (term1 + term2).to_array().map(|value| value.min(255) as u8)
}

#[inline]
fn native_chops_soft_light_byte(left: u8, right: u8) -> u8 {
    let a = u32::from(left);
    let b = u32::from(right);
    let term1 = ((255 - a) * a * b) / 65_536;
    let term2 = (a * (255 - ((255 - a) * (255 - b) / 255))) / 255;
    (term1 + term2).min(255) as u8
}

/// Apply SoftLight directly to native interleaved bytes. Chops treats alpha
/// as an ordinary stored sample, so LA/RGBA and CMYK all use the same active
/// channel stream. Row boundaries are not semantic boundaries for this
/// independent per-byte formula and may be crossed by vector blocks.
fn native_chops_soft_light(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || left.len() % channels != 0 {
        return None;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if left.len() < 8 {
        return None;
    }
    let mut output = vec![0u8; left.len()];
    let vector_len = output.len() / 8 * 8;
    for start in (0..vector_len).step_by(8) {
        let left_block = <[u8; 8]>::try_from(&left[start..start + 8]).ok()?;
        let right_block = <[u8; 8]>::try_from(&right[start..start + 8]).ok()?;
        output[start..start + 8]
            .copy_from_slice(&native_chops_soft_light_vector(left_block, right_block));
    }
    for index in vector_len..output.len() {
        output[index] = native_chops_soft_light_byte(left[index], right[index]);
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_len / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((output.len() - vector_len) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

#[inline]
fn native_module_blend_byte(left: u8, right: u8, alpha: f64) -> u8 {
    let value = f64::from(left) * (1.0 - alpha) + f64::from(right) * alpha;
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

/// Blend two matching native byte images with eight-wide f64 arithmetic.
///
/// Pillow's `Image.blend` interpolates every stored sample independently,
/// including alpha and CMYK K. Since the formula has no row-dependent state,
/// vectors may cross row boundaries and only the final partial block is scalar.
fn native_module_blend(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    other_mode: Option<&str>,
    alpha: f64,
) -> Option<DynamicImage> {
    let channels = native_module_blend_pair_channels(img, other, mode, other_mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return None;
    };
    if left.len() != right.len()
        || left.len() != expected_len
        || left.len() % channels != 0
    {
        return None;
    }
    if left.is_empty() {
        // Pillow returns a correctly typed zero-pixel image without entering
        // its arithmetic loop. Keep this a native-copy receipt; non-empty
        // inputs remain subject to the real eight-byte vector path below.
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if left.len() < 8 {
        return None;
    }
    let mut output = vec![0u8; left.len()];
    let vector_len = output.len() / 8 * 8;
    let inverse = f64x8::splat(1.0 - alpha);
    let alpha_value = alpha;
    let alpha = f64x8::splat(alpha_value);
    for start in (0..vector_len).step_by(8) {
        let left_block = <[u8; 8]>::try_from(&left[start..start + 8]).ok()?;
        let right_block = <[u8; 8]>::try_from(&right[start..start + 8]).ok()?;
        let left_block = f64x8::from(left_block.map(f64::from));
        let right_block = f64x8::from(right_block.map(f64::from));
        let values = left_block * inverse + right_block * alpha;
        for (lane, value) in values.to_array().into_iter().enumerate() {
            output[start + lane] = if value <= 0.0 {
                0
            } else if value >= 255.0 {
                255
            } else {
                value as u8
            };
        }
    }
    for index in vector_len..output.len() {
        output[index] = native_module_blend_byte(left[index], right[index], alpha_value);
    }
    crate::compute::record_pipeline_operation_vector_blocks((vector_len / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((output.len() - vector_len) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

#[inline]
fn native_chops_affine_byte(value: f64) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

#[inline]
fn native_chops_affine_vector(
    left: [u8; 8],
    right: [u8; 8],
    scale: f64,
    offset: f64,
    subtract: bool,
) -> [u8; 8] {
    let left = f64x8::new(left.map(f64::from));
    let right = f64x8::new(right.map(f64::from));
    let value = if subtract {
        (left - right) / f64x8::splat(scale) + f64x8::splat(offset)
    } else {
        (left + right) / f64x8::splat(scale) + f64x8::splat(offset)
    };
    value.to_array().map(native_chops_affine_byte)
}

fn native_chops_pair_channels(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = native_chops_layout(img, mode)?;
    (native_chops_layout(other, mode) == Some(channels)
        && img.dimensions() == other.dimensions())
    .then_some(channels)
}

/// Apply Pillow's scaled/offset Chops formula to native byte samples.
///
/// The arithmetic is vectorized in eight exact `f64` lanes because Pillow
/// evaluates `(left +/- right) / scale + offset` in double precision before
/// clamping and truncating to a byte. Loads are native interleaved bytes; no
/// packed RGBA conversion is introduced for RGB, LA, RGBA, CMYK, HSV, YCbCr,
/// RGBa, or RGBX storage.
fn native_chops_affine(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    scale: f64,
    offset: f64,
    subtract: bool,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    if !scale.is_finite() || scale == 0.0 || !offset.is_finite() {
        return None;
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let row_stride = width.checked_mul(channels)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return None;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if row_stride < 8 {
        return None;
    }

    let mut output = vec![0u8; left.len()];
    let vector_blocks = (row_stride / 8).saturating_mul(height);
    let scalar_tail = (row_stride % 8).saturating_mul(height);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);

    for row in 0..height {
        let row_start = row * row_stride;
        let left_row = &left[row_start..row_start + row_stride];
        let right_row = &right[row_start..row_start + row_stride];
        let output_row = &mut output[row_start..row_start + row_stride];
        let vector_len = output_row.len() / 8 * 8;
        for start in (0..vector_len).step_by(8) {
            let left_block = <[u8; 8]>::try_from(&left_row[start..start + 8]).ok()?;
            let right_block = <[u8; 8]>::try_from(&right_row[start..start + 8]).ok()?;
            output_row[start..start + 8].copy_from_slice(&native_chops_affine_vector(
                left_block,
                right_block,
                scale,
                offset,
                subtract,
            ));
        }
        for index in vector_len..output_row.len() {
            let value = if subtract {
                (f64::from(left_row[index]) - f64::from(right_row[index])) / scale + offset
            } else {
                (f64::from(left_row[index]) + f64::from(right_row[index])) / scale + offset
            };
            output_row[index] = native_chops_affine_byte(value);
        }
    }
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result));
    if result.is_some() {
        crate::compute::record_pipeline_operation_path("vector");
    }
    result
}

/// In-place version of [`native_chops_affine`] for an owned SIMD intermediate.
/// The right operand is materialized once and remains immutable while the
/// left buffer is updated after each vector block has been loaded.
fn native_chops_affine_in_place(
    img: &mut DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    scale: f64,
    offset: f64,
    subtract: bool,
) -> bool {
    let Some(channels) = native_chops_pair_channels(&*img, other, mode) else {
        return false;
    };
    if !scale.is_finite() || scale == 0.0 || !offset.is_finite() {
        return false;
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    let right = other.as_bytes();
    let Some(left) = img.as_bytes_mut() else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return false;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    if row_stride < 8 {
        return false;
    }

    let vector_blocks = (row_stride / 8).saturating_mul(height);
    let scalar_tail = (row_stride % 8).saturating_mul(height);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    for row in 0..height {
        let row_start = row * row_stride;
        let left_row = &mut left[row_start..row_start + row_stride];
        let right_row = &right[row_start..row_start + row_stride];
        let vector_len = left_row.len() / 8 * 8;
        for start in (0..vector_len).step_by(8) {
            let left_block = <[u8; 8]>::try_from(&left_row[start..start + 8])
                .expect("native affine Chops block has 8 bytes");
            let right_block = <[u8; 8]>::try_from(&right_row[start..start + 8])
                .expect("native affine Chops block has 8 bytes");
            left_row[start..start + 8].copy_from_slice(&native_chops_affine_vector(
                left_block,
                right_block,
                scale,
                offset,
                subtract,
            ));
        }
        for index in vector_len..left_row.len() {
            let value = if subtract {
                (f64::from(left_row[index]) - f64::from(right_row[index])) / scale + offset
            } else {
                (f64::from(left_row[index]) + f64::from(right_row[index])) / scale + offset
            };
            left_row[index] = native_chops_affine_byte(value);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    true
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
fn simd_div255_u16x8(value: u16x8) -> u16x8 {
    let incremented = value + u16x8::splat(1);
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
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
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
    let vector_blocks = (row_stride / 16).saturating_mul(height as usize);
    let scalar_tail = (row_stride % 16).saturating_mul(height as usize);
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
    Ok(Some((
        preserve_mode(img, result),
        vector_blocks as u64,
        scalar_tail as u64,
    )))
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

/// Reverse sixteen RGB pixels with three native byte vectors.
///
/// RGB pixels are three bytes wide, so a single `u8x16` cannot contain an
/// integral number of pixels. Three source vectors and fixed lane selects
/// still cover the complete 48-byte group without scalar pixel arithmetic.
#[inline]
fn reverse_rgb_block(input: &[u8]) -> Option<[u8; 48]> {
    let first = u8x16::new(<[u8; 16]>::try_from(input.get(..16)?).ok()?);
    let second = u8x16::new(<[u8; 16]>::try_from(input.get(16..32)?).ok()?);
    let third = u8x16::new(<[u8; 16]>::try_from(input.get(32..48)?).ok()?);

    let block0_from_third = third.swizzle_relaxed(u8x16::new([
        13, 14, 15, 10, 11, 12, 7, 8, 9, 4, 5, 6, 1, 2, 3, 0,
    ]));
    let block0_from_second = second.swizzle_relaxed(u8x16::splat(14));
    let block0 = u8x16::new([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, u8::MAX,
    ])
    .select(block0_from_second, block0_from_third);

    let block1_from_second = second.swizzle_relaxed(u8x16::new([
        15, 0, 11, 12, 13, 8, 9, 10, 5, 6, 7, 2, 3, 4, 0, 0,
    ]));
    let block1_from_third = third.swizzle_relaxed(u8x16::splat(0));
    let block1_from_first = first.swizzle_relaxed(u8x16::splat(15));
    let block1 = u8x16::new([
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, u8::MAX, 0,
    ])
    .select(
        block1_from_first,
        u8x16::new([
            0, u8::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ])
        .select(block1_from_third, block1_from_second),
    );

    let block2_from_first = first.swizzle_relaxed(u8x16::new([
        0, 12, 13, 14, 9, 10, 11, 6, 7, 8, 3, 4, 5, 0, 1, 2,
    ]));
    let block2_from_second = second.swizzle_relaxed(u8x16::splat(1));
    let block2 = u8x16::new([
        u8::MAX, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ])
    .select(block2_from_second, block2_from_first);

    let mut output = [0u8; 48];
    output[..16].copy_from_slice(&block0.to_array());
    output[16..32].copy_from_slice(&block1.to_array());
    output[32..].copy_from_slice(&block2.to_array());
    Some(output)
}

/// Copy a byte span through the native vector type and account for its tail.
#[inline]
fn copy_native_bytes(source: &[u8], output: &mut [u8]) -> Option<(u64, u64)> {
    if source.len() != output.len() {
        return None;
    }
    if source.is_empty() {
        return Some((0, 0));
    }
    let vector_len = source.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for offset in (0..vector_len).step_by(16) {
        let block = u8x16::new(<[u8; 16]>::try_from(&source[offset..offset + 16]).ok()?);
        output[offset..offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = source.len() - vector_len;
    if scalar_tail != 0 {
        let mut padded = [0u8; 16];
        padded[..scalar_tail].copy_from_slice(&source[vector_len..]);
        let block = u8x16::new(padded);
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Load sixteen bytes from a circular row without splitting the operation
/// into scalar copies when the offset crosses the row boundary. The two
/// source vectors are selected with lane masks; the byte indices are control
/// data, not per-pixel arithmetic.
#[inline]
fn circular_byte_block(source: &[u8], start: usize) -> Option<u8x16> {
    if source.len() < 16 || start >= source.len() {
        return None;
    }
    if start + 16 <= source.len() {
        return Some(u8x16::new(
            <[u8; 16]>::try_from(source.get(start..start + 16)?).ok()?,
        ));
    }

    let first_len = source.len() - start;
    let tail = u8x16::new(<[u8; 16]>::try_from(source.get(source.len() - 16..)?).ok()?);
    let head = u8x16::new(<[u8; 16]>::try_from(source.get(..16)?).ok()?);
    let mut tail_indices = [0u8; 16];
    let mut head_indices = [0u8; 16];
    let mut tail_mask = [0u8; 16];
    for lane in 0..16 {
        if lane < first_len {
            tail_indices[lane] = u8::try_from(16 - first_len + lane).ok()?;
            tail_mask[lane] = u8::MAX;
        } else {
            head_indices[lane] = u8::try_from(lane - first_len).ok()?;
        }
    }
    let tail_values = tail.swizzle_relaxed(u8x16::new(tail_indices));
    let head_values = head.swizzle_relaxed(u8x16::new(head_indices));
    let mask = u8x16::new(tail_mask);
    Some((tail_values & mask) | (head_values & (u8x16::splat(u8::MAX) ^ mask)))
}

/// Copy one wrapped offset row through complete native vector blocks. The
/// final incomplete store is the only scalar tail; the circular source load
/// remains vectorized even when the shift divides the row into two short
/// spans.
#[inline]
fn offset_native_row(
    source: &[u8],
    output: &mut [u8],
    shift_bytes: usize,
) -> Option<(u64, u64)> {
    if source.len() != output.len() || source.len() < 16 || shift_bytes >= source.len() {
        return None;
    }
    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let source_start = (shift_bytes + output_offset) % source.len();
        let block = circular_byte_block(source, source_start)?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output.len() - vector_len;
    if scalar_tail != 0 {
        let source_start = (shift_bytes + vector_len) % source.len();
        let block = circular_byte_block(source, source_start)?;
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Load one narrow row and rotate its byte lanes. The load may cross into the
/// following row; the swizzle mask only references the current row, so those
/// extra bytes are never observed. A final source row without sixteen bytes
/// remaining is the only place where a stack-padded load is necessary.
#[inline]
fn narrow_rotated_row(
    source: &[u8],
    row_start: usize,
    row_len: usize,
    shift_bytes: usize,
) -> Option<u8x16> {
    if row_len == 0 || row_len >= 16 || shift_bytes >= row_len {
        return None;
    }
    let row_end = row_start.checked_add(row_len)?;
    let row = source.get(row_start..row_end)?;
    let loaded = if row_start.checked_add(16)?.le(&source.len()) {
        u8x16::new(<[u8; 16]>::try_from(source.get(row_start..row_start + 16)?).ok()?)
    } else {
        let mut padded = [0u8; 16];
        padded[..row_len].copy_from_slice(row);
        u8x16::new(padded)
    };
    let mut indices = [0u8; 16];
    for (lane, index) in indices.iter_mut().enumerate() {
        *index = u8::try_from((shift_bytes + lane) % row_len).ok()?;
    }
    Some(loaded.swizzle_relaxed(u8x16::new(indices)))
}

/// Process a narrow-row offset as a vector stream over the frame buffer.
/// Output blocks may contain the end of one row and the beginning of the next;
/// each segment is selected from a vectorized, independently rotated source
/// row before one contiguous vector store. This keeps the copy native without
/// allocating a row-sized staging buffer.
#[inline]
fn offset_narrow_bytes(
    source: &[u8],
    output: &mut [u8],
    row_len: usize,
    height: usize,
    shift_bytes: usize,
    yshift: usize,
) -> Option<(u64, u64)> {
    if source.len() != output.len()
        || row_len == 0
        || row_len >= 16
        || source.len() < 16
        || height == 0
        || yshift >= height
        || shift_bytes >= row_len
    {
        return None;
    }
    let vector_len = output.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let mut block = u8x16::splat(0);
        let mut lane = 0usize;
        while lane < 16 {
            let absolute = output_offset.checked_add(lane)?;
            if absolute >= output.len() {
                break;
            }
            let output_row = absolute / row_len;
            let row_start = output_row.checked_mul(row_len)?;
            let row_offset = absolute - row_start;
            let row_end_lane = (row_len - row_offset).min(16 - lane);
            let source_row = (output_row + yshift) % height;
            let source_start = source_row.checked_mul(row_len)?;
            let rotated = narrow_rotated_row(source, source_start, row_len, shift_bytes)?;
            let mut indices = [0u8; 16];
            let mut mask = [0u8; 16];
            for destination_lane in lane..lane + row_end_lane {
                indices[destination_lane] = u8::try_from(
                    row_offset + destination_lane - lane,
                )
                .ok()?;
                mask[destination_lane] = u8::MAX;
            }
            let selected = rotated.swizzle_relaxed(u8x16::new(indices));
            block = u8x16::new(mask).select(selected, block);
            lane += row_end_lane;
        }
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output.len() - vector_len;
    if scalar_tail != 0 {
        let mut block = u8x16::splat(0);
        let mut lane = 0usize;
        while lane < scalar_tail {
            let absolute = vector_len.checked_add(lane)?;
            let output_row = absolute / row_len;
            let row_start = output_row.checked_mul(row_len)?;
            let row_offset = absolute - row_start;
            let row_end_lane = (row_len - row_offset).min(scalar_tail - lane);
            let source_row = (output_row + yshift) % height;
            let source_start = source_row.checked_mul(row_len)?;
            let rotated = narrow_rotated_row(source, source_start, row_len, shift_bytes)?;
            let mut indices = [0u8; 16];
            let mut mask = [0u8; 16];
            for destination_lane in lane..lane + row_end_lane {
                indices[destination_lane] = u8::try_from(
                    row_offset + destination_lane - lane,
                )
                .ok()?;
                mask[destination_lane] = u8::MAX;
            }
            let selected = rotated.swizzle_relaxed(u8x16::new(indices));
            block = u8x16::new(mask).select(selected, block);
            lane += row_end_lane;
        }
        output[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Copy one Pillow `I;16*` offset row through native byte vectors.
///
/// The scalar reference follows Pillow's historical `ImageChops` byte path:
/// it rotates the first `width` bytes of a `width * 2` row and leaves the
/// remaining bytes zero. Preserve that observable contract instead of
/// treating the samples as ordinary `u16` pixels. The byte-order conversion
/// is control-plane work; the copied portion uses the same circular `u8x16`
/// kernel as ordinary byte images.
#[inline]
fn offset_luma16_native_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    shift_bytes: usize,
) -> Option<(u64, u64)> {
    let row_bytes = width.checked_mul(2)?;
    if width < 16
        || source.len() != row_bytes
        || output.len() != row_bytes
        || shift_bytes >= width
    {
        return None;
    }
    let visible_source = source.get(..width)?;
    let vector_len = width / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let source_start = (shift_bytes + output_offset) % width;
        let block = circular_byte_block(visible_source, source_start)?;
        output[output_offset..output_offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = width - vector_len;
    if scalar_tail != 0 {
        let source_start = (shift_bytes + vector_len) % width;
        let block = circular_byte_block(visible_source, source_start)?;
        output[vector_len..width].copy_from_slice(&block.to_array()[..scalar_tail]);
    }
    Some((vector_blocks, scalar_tail as u64))
}

fn native_offset_luma16(
    img: &DynamicImage,
    xoffset: i32,
    yoffset: i32,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    let DynamicImage::ImageLuma16(image) = img else {
        return None;
    };
    if !matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        return None;
    }
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return image
            .as_raw()
            .is_empty()
            .then_some((img.clone(), 0, 0));
    }
    if width < 16 || height == 0 {
        return None;
    }
    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let row_bytes = width_usize.checked_mul(2)?;
    let total_bytes = row_bytes.checked_mul(height_usize)?;
    if image.as_raw().len().checked_mul(2)? != total_bytes {
        return None;
    }

    // Match `raster::dynamic::offset_luma16`: I;16B is the only explicit
    // big-endian variant in this byte-oriented ImageChops path.
    let big_endian = mode == Some("I;16B");
    let source = if big_endian {
        image
            .as_raw()
            .iter()
            .flat_map(|sample| sample.to_be_bytes())
            .collect::<Vec<_>>()
    } else {
        bytemuck::cast_slice(image.as_raw()).to_vec()
    };
    let mut output = vec![0u8; total_bytes];
    let xshift = (-i64::from(xoffset)).rem_euclid(i64::from(width)) as usize;
    let yshift = (-i64::from(yoffset)).rem_euclid(i64::from(height)) as usize;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for output_y in 0..height_usize {
        let source_y = (output_y + yshift) % height_usize;
        let source_start = source_y.checked_mul(row_bytes)?;
        let output_start = output_y.checked_mul(row_bytes)?;
        let (blocks, tail) = offset_luma16_native_row(
            source.get(source_start..source_start + row_bytes)?,
            output.get_mut(output_start..output_start + row_bytes)?,
            width_usize,
            xshift,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }

    let samples = output
        .chunks_exact(2)
        .map(|bytes| {
            if big_endian {
                u16::from_be_bytes([bytes[0], bytes[1]])
            } else {
                u16::from_ne_bytes([bytes[0], bytes[1]])
            }
        })
        .collect();
    let result = crate::raster::ImageBuffer::from_raw(width, height, samples)?;
    Some((
        DynamicImage::ImageLuma16(result),
        vector_blocks,
        scalar_tail,
    ))
}

/// Write a horizontally reversed row into a separate native output buffer.
///
/// Using a separate row is intentional: it avoids the source/destination
/// overlap constraints of an in-place swap and lets every complete group use
/// a fixed vector load/store. Only incomplete pixel groups use scalar copies.
fn mirror_native_row(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    channels: usize,
) -> Option<(u64, u64)> {
    let row_len = width.checked_mul(channels)?;
    if source.len() != row_len || output.len() != row_len {
        return None;
    }
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    if channels == 3 {
        let vector_pixels = width / 16 * 16;
        for output_pixel in (0..vector_pixels).step_by(16) {
            let source_pixel = width - output_pixel - 16;
            let source_start = source_pixel.checked_mul(3)?;
            let output_start = output_pixel.checked_mul(3)?;
            let block = reverse_rgb_block(source.get(source_start..source_start + 48)?)?;
            output[output_start..output_start + 48].copy_from_slice(&block);
            vector_blocks = vector_blocks.saturating_add(3);
        }
        let remainder_pixels = width - vector_pixels;
        if remainder_pixels != 0 {
            let remainder_len = remainder_pixels.checked_mul(3)?;
            let mut padded = [0u8; 48];
            let pad = 48usize.checked_sub(remainder_len)?;
            padded[pad..].copy_from_slice(source.get(..remainder_len)?);
            let block = reverse_rgb_block(&padded)?;
            let output_start = vector_pixels.checked_mul(3)?;
            output[output_start..output_start + remainder_len]
                .copy_from_slice(&block[..remainder_len]);
            vector_blocks = vector_blocks.saturating_add(3);
            scalar_tail = scalar_tail.saturating_add(remainder_pixels as u64);
        }
        return Some((vector_blocks, scalar_tail));
    }
    let pixels_per_vector = 16usize.checked_div(channels)?;
    let vector_pixels = width / pixels_per_vector * pixels_per_vector;
    for output_pixel in (0..vector_pixels).step_by(pixels_per_vector) {
        let source_pixel = width - output_pixel - pixels_per_vector;
        let source_start = source_pixel.checked_mul(channels)?;
        let output_start = output_pixel.checked_mul(channels)?;
        let block = reverse_pixel_block(source.get(source_start..source_start + 16)?, channels)?;
        output[output_start..output_start + 16].copy_from_slice(&block);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder_pixels = width - vector_pixels;
    if remainder_pixels != 0 {
        let remainder_len = remainder_pixels.checked_mul(channels)?;
        let mut padded = [0u8; 16];
        let pad = 16usize.checked_sub(remainder_len)?;
        padded[pad..].copy_from_slice(source.get(..remainder_len)?);
        let block = reverse_pixel_block(&padded, channels)?;
        let output_start = vector_pixels.checked_mul(channels)?;
        output[output_start..output_start + remainder_len]
            .copy_from_slice(&block[..remainder_len]);
        vector_blocks = vector_blocks.saturating_add(1);
        scalar_tail = scalar_tail.saturating_add(remainder_pixels as u64);
    }
    Some((vector_blocks, scalar_tail))
}

/// Flip rows vertically in the source's native byte layout.
fn native_flip_vertical(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    let channels = native_copy_layout(img, mode)?;
    let (width, height) = img.dimensions();
    let row_len = (width as usize).checked_mul(channels)?;
    let total_len = row_len.checked_mul(height as usize)?;
    let source = img.as_bytes().get(..total_len)?;
    let mut output = vec![0u8; total_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for output_row in 0..height as usize {
        let source_row = height as usize - 1 - output_row;
        let source_start = source_row.checked_mul(row_len)?;
        let output_start = output_row.checked_mul(row_len)?;
        let (blocks, tail) = copy_native_bytes(
            source.get(source_start..source_start + row_len)?,
            output.get_mut(output_start..output_start + row_len)?,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = crate::image_utils::raw_bytes_to_image(width, height, output, channels).ok()?;
    Some((preserve_mode(img, result), vector_blocks, scalar_tail))
}

/// Offset a native byte image by copying at most two contiguous source spans
/// per output row. The wrapped coordinate arithmetic is scalar control-plane
/// work; each span is transferred through the same `u8x16` copy kernel used by
/// flip/crop. This preserves Pillow's pixel-coordinate wrapping without
/// materializing or widening the image through packed RGBA storage.
fn native_offset(
    img: &DynamicImage,
    xoffset: i32,
    yoffset: i32,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    let channels = native_copy_layout(img, mode)?;
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return has_empty_native_bytes(img, channels).then_some((img.clone(), 0, 0));
    }
    let width_usize = usize::try_from(width).ok()?;
    let height_usize = usize::try_from(height).ok()?;
    let row_len = width_usize.checked_mul(channels)?;
    let total_len = row_len.checked_mul(height_usize)?;
    let source = img.as_bytes().get(..total_len)?;
    let mut output = vec![0u8; total_len];
    let xshift = (-i64::from(xoffset)).rem_euclid(i64::from(width)) as usize;
    let yshift = (-i64::from(yoffset)).rem_euclid(i64::from(height)) as usize;
    let shift_bytes = xshift.checked_mul(channels)?;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    if row_len < 16 {
        let (blocks, tail) = offset_narrow_bytes(
            source,
            &mut output,
            row_len,
            height_usize,
            shift_bytes,
            yshift,
        )?;
        vector_blocks = blocks;
        scalar_tail = tail;
    } else {
        for output_y in 0..height_usize {
            let source_y = (output_y + yshift) % height_usize;
            let source_row = source_y.checked_mul(row_len)?;
            let output_row = output_y.checked_mul(row_len)?;
            let (blocks, tail) = offset_native_row(
                source.get(source_row..source_row + row_len)?,
                output.get_mut(output_row..output_row + row_len)?,
                shift_bytes,
            )?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
    }

    let result = crate::image_utils::raw_bytes_to_image(width, height, output, channels).ok()?;
    Some((preserve_mode(img, result), vector_blocks, scalar_tail))
}

/// Fast native-layout mirror for ordinary 8-bit byte images.
fn native_mirror(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(DynamicImage, u64, u64)> {
    let (width, height, channels) = match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("1" | "L" | "P")) => {
            (image.width(), image.height(), 1)
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA" | "PA")) => {
            (image.width(), image.height(), 2)
        }
        DynamicImage::ImageRgb8(image)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            (image.width(), image.height(), 3)
        }
        DynamicImage::ImageRgba8(image)
            if matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            ) =>
        {
            (image.width(), image.height(), 4)
        }
        _ => return None,
    };
    let row_len = (width as usize).checked_mul(channels)?;
    let total_len = row_len.checked_mul(height as usize)?;
    let source = img.as_bytes().get(..total_len)?;
    let mut output = vec![0u8; total_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for row in 0..height as usize {
        let start = row.checked_mul(row_len)?;
        let (blocks, tail) = mirror_native_row(
            source.get(start..start + row_len)?,
            output.get_mut(start..start + row_len)?,
            width as usize,
            channels,
        )?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(width, height, output)?),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(width, height, output)?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(width, height, output)?),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(width, height, output)?),
        _ => return None,
    };
    Some((result, vector_blocks, scalar_tail))
}

/// Reorder a native byte image for one Pillow transpose method.
///
/// Geometry does not need an RGBA representation: each pixel is an opaque
/// byte group whose channels must move together. Keeping the operation in its
/// original layout avoids the pack/kernel/unpack round trip that dominated
/// the SIMD transpose benchmark for RGB and RGBA images.
fn transpose_gathered_row(
    source: &[u8],
    output: &mut [u8],
    source_row_bytes: usize,
    source_height: usize,
    channels: usize,
    source_x: usize,
    reverse_source_y: bool,
) -> Option<(u64, u64)> {
    let output_row_bytes = output.len();
    let vector_len = output_row_bytes / 16 * 16;
    let mut vector_blocks = 0u64;
    for output_offset in (0..vector_len).step_by(16) {
        let mut block = [0u8; 16];
        for (lane, value) in block.iter_mut().enumerate() {
            let output_byte = output_offset + lane;
            let output_pixel = output_byte / channels;
            let channel = output_byte % channels;
            let source_y = if reverse_source_y {
                source_height - 1 - output_pixel
            } else {
                output_pixel
            };
            let source_offset = source_y
                .checked_mul(source_row_bytes)?
                .checked_add(source_x.checked_mul(channels)?)?
                .checked_add(channel)?;
            *value = *source.get(source_offset)?;
        }
        output[output_offset..output_offset + 16]
            .copy_from_slice(&u8x16::new(block).to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = output_row_bytes - vector_len;
    for output_offset in vector_len..output_row_bytes {
        let output_pixel = output_offset / channels;
        let channel = output_offset % channels;
        let source_y = if reverse_source_y {
            source_height - 1 - output_pixel
        } else {
            output_pixel
        };
        let source_offset = source_y
            .checked_mul(source_row_bytes)?
            .checked_add(source_x.checked_mul(channels)?)?
            .checked_add(channel)?;
        output[output_offset] = *source.get(source_offset)?;
    }
    Some((vector_blocks, scalar_tail as u64))
}

fn native_transpose_bytes(
    bytes: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    method: TransposeMethod,
) -> Option<(Vec<u8>, u32, u32, u64, u64)> {
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
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    match method {
        TransposeMethod::FlipLeftRight => {
            let row_bytes = width.checked_mul(channels)?;
            for y in 0..height {
                let start = y.checked_mul(row_bytes)?;
                let (blocks, tail) = mirror_native_row(
                    source.get(start..start + row_bytes)?,
                    output.get_mut(start..start + row_bytes)?,
                    width,
                    channels,
                )?;
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
        }
        TransposeMethod::FlipTopBottom => {
            let row_bytes = width.checked_mul(channels)?;
            for y in 0..height {
                let source_start = (height - 1 - y).checked_mul(row_bytes)?;
                let output_start = y.checked_mul(row_bytes)?;
                let (blocks, tail) = copy_native_bytes(
                    source.get(source_start..source_start + row_bytes)?,
                    output.get_mut(output_start..output_start + row_bytes)?,
                )?;
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
        }
        TransposeMethod::Rotate180 => {
            let row_bytes = width.checked_mul(channels)?;
            for y in 0..height {
                let source_row = height - 1 - y;
                let start = source_row.checked_mul(row_bytes)?;
                let output_start = y.checked_mul(row_bytes)?;
                let (blocks, tail) = mirror_native_row(
                    source.get(start..start + row_bytes)?,
                    output.get_mut(output_start..output_start + row_bytes)?,
                    width,
                    channels,
                )?;
                vector_blocks = vector_blocks.saturating_add(blocks);
                scalar_tail = scalar_tail.saturating_add(tail);
            }
        }
        method => {
            // Validate the row strides once. A transpose keeps one source
            // column fixed for each output row; scalar indexing selects that
            // column, while complete output byte blocks use the native vector
            // load/store path.
            let source_row_bytes = width.checked_mul(channels)?;
            let output_row_bytes = out_width.checked_mul(channels)?;
            let write_output_rows_serial = |output: &mut [u8]| -> Option<(u64, u64)> {
                let mut vector_blocks = 0u64;
                let mut scalar_tail = 0u64;
                for output_row in 0..out_height {
                    let (source_x, reverse_source_y) = match method {
                        TransposeMethod::Rotate90 => (width - 1 - output_row, false),
                        TransposeMethod::Rotate270 => (output_row, true),
                        TransposeMethod::Transpose => (output_row, false),
                        TransposeMethod::Transverse => (width - 1 - output_row, true),
                        _ => unreachable!("same-dimension transpose handled above"),
                    };
                    let output_start = output_row.checked_mul(output_row_bytes)?;
                    let (blocks, tail) = transpose_gathered_row(
                        source,
                        output.get_mut(output_start..output_start + output_row_bytes)?,
                        source_row_bytes,
                        height,
                        channels,
                        source_x,
                        reverse_source_y,
                    )?;
                    vector_blocks = vector_blocks.saturating_add(blocks);
                    scalar_tail = scalar_tail.saturating_add(tail);
                }
                Some((vector_blocks, scalar_tail))
            };
            #[cfg(feature = "parallel")]
            {
                if pixels >= 256 * 1024 {
                    crate::par_rows_mut!(
                        &mut output,
                        output_row_bytes,
                        out_height,
                        |_row_start, _row_end, output_row, output_row_slice| {
                            let (source_x, reverse_source_y) = match method {
                                TransposeMethod::Rotate90 => {
                                    (width - 1 - output_row as usize, false)
                                }
                                TransposeMethod::Rotate270 => (output_row as usize, true),
                                TransposeMethod::Transpose => (output_row as usize, false),
                                TransposeMethod::Transverse => {
                                    (width - 1 - output_row as usize, true)
                                }
                                _ => unreachable!("same-dimension transpose handled above"),
                            };
                            let _ = transpose_gathered_row(
                                source,
                                output_row_slice,
                                source_row_bytes,
                                height,
                                channels,
                                source_x,
                                reverse_source_y,
                            );
                        }
                    );
                    vector_blocks = (out_height as u64)
                        .saturating_mul((output_row_bytes / 16) as u64);
                    scalar_tail = (out_height as u64)
                        .saturating_mul((output_row_bytes % 16) as u64);
                } else {
                    (vector_blocks, scalar_tail) = write_output_rows_serial(&mut output)?;
                }
            }
            #[cfg(not(feature = "parallel"))]
            {
                (vector_blocks, scalar_tail) = write_output_rows_serial(&mut output)?;
            }
        }
    }

    Some((
        output,
        u32::try_from(out_width).ok()?,
        u32::try_from(out_height).ok()?,
        vector_blocks,
        scalar_tail,
    ))
}

/// Apply transpose while retaining the native `I;16*` DynamicImage variant.
///
/// The shared byte kernel moves complete two-byte samples as opaque groups;
/// reconstructing `ImageLuma16` only restores the native typed buffer after
/// the reorder. Endianness is deliberately not changed here: DynamicImage's
/// `ImageLuma16` storage is host-native, and the public mode boundary handles
/// the requested `I;16B`/`I;16L` byte order.
fn native_transpose_luma16(
    img: &DynamicImage,
    mode: Option<&str>,
    method: TransposeMethod,
) -> Option<(DynamicImage, u64, u64)> {
    if !native_luma16_transpose_layout(img, mode) {
        return None;
    }
    let (bytes, width, height, vector_blocks, scalar_tail) =
        native_transpose_bytes(img.as_bytes(), img.width(), img.height(), 2, method)?;
    let mut chunks = bytes.chunks_exact(2);
    let samples = chunks
        .by_ref()
        .map(|sample| u16::from_ne_bytes([sample[0], sample[1]]))
        .collect::<Vec<_>>();
    if !chunks.remainder().is_empty() {
        return None;
    }
    let result = ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, samples)?;
    Some((DynamicImage::ImageLuma16(result), vector_blocks, scalar_tail))
}

/// Apply transpose while retaining the native 8-bit DynamicImage variant.
fn native_transpose(
    img: &DynamicImage,
    mode: Option<&str>,
    method: TransposeMethod,
) -> Option<(DynamicImage, u64, u64)> {
    if matches!(img, DynamicImage::ImageLuma16(_)) {
        return native_transpose_luma16(img, mode, method);
    }
    let channels = match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => 1,
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => 2,
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => 3,
        DynamicImage::ImageRgba8(_)
            if matches!(
                mode,
                None | Some("RGBA" | "RGBa" | "CMYK" | "RGBX" | "I" | "F")
            ) =>
        {
            4
        }
        _ => return None,
    };
    let (bytes, width, height, vector_blocks, scalar_tail) =
        native_transpose_bytes(img.as_bytes(), img.width(), img.height(), channels, method)?;
    let result = crate::image_utils::raw_bytes_to_image(width, height, bytes, channels).ok()?;
    Some((preserve_mode(img, result), vector_blocks, scalar_tail))
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

fn simd_native_chops(
    img: &DynamicImage,
    other: &Arc<Image>,
    mode: Option<&str>,
    native: fn(&DynamicImage, &DynamicImage, Option<&str>) -> Option<DynamicImage>,
) -> Result<DynamicImage, PilError> {
    let other_img = materialize_chops_operand(other, mode)?;
    if let Some(result) = native(img, &other_img, mode) {
        return Ok(result);
    }
    Err(PilError::NotImplementedError(
        "SIMD Chops requires matching native byte layouts".into(),
    ))
}

// ═══════════════════════════════════════════════════════════════════════
// Section A: Simple single-image ops (no extra params beyond mode)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_invert(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    native_invert(img, mode, false).ok_or_else(|| simd_unsupported("Invert"))
}

pub fn simd_invert_chops(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    native_invert(img, mode, true).ok_or_else(|| simd_unsupported("InvertChops"))
}

// ═══════════════════════════════════════════════════════════════════════
// Section B: Single-image with extra params (solarize, posterize, ...)
// ═══════════════════════════════════════════════════════════════════════

/// Convert an admitted native byte image to `L` without widening it through
/// packed RGBA storage.
///
/// The source-byte gather is scalar control because `wide` does not expose a
/// portable byte-gather instruction.  RGB/RGBA luma arithmetic runs in eight
/// `u32` lanes with Pillow's fixed-point coefficients; LA uses a vector
/// shuffle to drop alpha; L uses a native vector copy.  The final partial
/// group is zero-padded and processed by the same vector kernel, so strict
/// SIMD never becomes a scalar-only implementation for a short valid image.
fn native_grayscale_bytes(
    img: &DynamicImage,
    channels: usize,
) -> Option<(Vec<u8>, u64, u64)> {
    if !(1..=4).contains(&channels) {
        return None;
    }
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source = img.as_bytes();
    if pixels == 0 || source.len() != pixels.checked_mul(channels)? {
        return None;
    }

    let mut output = vec![0u8; pixels];
    let mut vector_blocks = 0u64;

    match channels {
        1 => {
            for start in (0..pixels).step_by(16) {
                let active = (pixels - start).min(16);
                let mut padded = [0u8; 16];
                padded[..active].copy_from_slice(&source[start..start + active]);
                let block = u8x16::new(padded);
                output[start..start + active]
                    .copy_from_slice(&block.to_array()[..active]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        2 => {
            let select_luma = u8x16::new([
                0, 2, 4, 6, 8, 10, 12, 14, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            for start in (0..pixels).step_by(8) {
                let active = (pixels - start).min(8);
                let source_start = start * 2;
                let source_len = active * 2;
                let mut padded = [0u8; 16];
                padded[..source_len]
                    .copy_from_slice(&source[source_start..source_start + source_len]);
                let luma = u8x16::new(padded)
                    .swizzle_relaxed(select_luma)
                    .to_array();
                output[start..start + active].copy_from_slice(&luma[..active]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        3 | 4 => {
            for start in (0..pixels).step_by(8) {
                let active = (pixels - start).min(8);
                let mut red = [0u8; 8];
                let mut green = [0u8; 8];
                let mut blue = [0u8; 8];
                for lane in 0..active {
                    let source_start = (start + lane) * channels;
                    red[lane] = source[source_start];
                    green[lane] = source[source_start + 1];
                    blue[lane] = source[source_start + 2];
                }
                let luma = (u32x8::new(red.map(u32::from)) * u32x8::splat(19595)
                    + u32x8::new(green.map(u32::from)) * u32x8::splat(38470)
                    + u32x8::new(blue.map(u32::from)) * u32x8::splat(7471)
                    + u32x8::splat(32768))
                    >> 16u32;
                let luma = luma.to_array().map(|value| value.min(255) as u8);
                output[start..start + active].copy_from_slice(&luma[..active]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        _ => return None,
    }

    Some((output, vector_blocks, 0))
}

/// Apply ImageOps.colorize's three per-value LUTs to native `L` samples.
///
/// Each sixteen-pixel group performs three vectorized LUT gathers and four
/// vectorized four-pixel RGB interleaves.  The interleave setup is scalar
/// control only; no pixel arithmetic or per-sample LUT evaluation falls back
/// to the packed CPU adapter.  A partial group is padded and stores only its
/// valid output bytes.
fn native_colorize_bytes(
    img: &DynamicImage,
    lut: &[[u8; 256]; 3],
) -> Option<(Vec<u8>, u64, u64)> {
    if !matches!(img, DynamicImage::ImageLuma8(_)) {
        return None;
    }
    let pixels = (img.width() as usize).checked_mul(img.height() as usize)?;
    let source = img.as_bytes();
    let output_len = pixels.checked_mul(3)?;
    if pixels == 0 || source.len() != pixels {
        return None;
    }
    let tables = [
        native_lut_tables(&lut[0])?,
        native_lut_tables(&lut[1])?,
        native_lut_tables(&lut[2])?,
    ];
    let interleave = u8x16::new([
        0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11, 12, 13, 14, 15,
    ]);
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active = (pixels - start).min(16);
        let mut padded = [0u8; 16];
        padded[..active].copy_from_slice(&source[start..start + active]);
        let input = u8x16::new(padded);
        let red = native_lut_chunk(input, &tables[0]).to_array();
        let green = native_lut_chunk(input, &tables[1]).to_array();
        let blue = native_lut_chunk(input, &tables[2]).to_array();

        for group in 0..4 {
            let lane = group * 4;
            let packed = u8x16::new([
                red[lane],
                red[lane + 1],
                red[lane + 2],
                red[lane + 3],
                green[lane],
                green[lane + 1],
                green[lane + 2],
                green[lane + 3],
                blue[lane],
                blue[lane + 1],
                blue[lane + 2],
                blue[lane + 3],
                0,
                0,
                0,
                0,
            ])
            .swizzle_relaxed(interleave)
            .to_array();
            let group_pixels = active.saturating_sub(lane).min(4);
            if group_pixels != 0 {
                let output_start = (start + lane) * 3;
                let output_len = group_pixels * 3;
                output[output_start..output_start + output_len]
                    .copy_from_slice(&packed[..output_len]);
            }
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }
    Some((output, vector_blocks, 0))
}

pub fn simd_grayscale(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let Some(channels) = native_grayscale_layout(img, mode) else {
        return Err(simd_unsupported("Grayscale"));
    };
    let Some((output, vector_blocks, scalar_tail)) = native_grayscale_bytes(img, channels) else {
        return Err(simd_unsupported("Grayscale"));
    };
    crate::compute::record_pipeline_operation_path(if channels == 1 {
        "native-copy"
    } else {
        "vector"
    });
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    GrayImage::from_raw(img.width(), img.height(), output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD grayscale buffer mismatch".into()))
}

pub fn simd_colorize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Colorize {
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    } = op
    else {
        return Err(PilError::ValueError("expected Colorize op".into()));
    };
    if !matches!(mode, None | Some("L")) {
        return Err(simd_unsupported("Colorize"));
    }
    let lut = crate::compute::pool_cpu::ops::imageops::colorize_lut(
        black,
        white,
        *mid,
        *blackpoint,
        *midpoint,
        *whitepoint,
    );
    let Some((output, vector_blocks, scalar_tail)) = native_colorize_bytes(img, &lut) else {
        return Err(simd_unsupported("Colorize"));
    };
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    RgbImage::from_raw(img.width(), img.height(), output)
        .map(DynamicImage::ImageRgb8)
        .ok_or_else(|| PilError::InternalError("SIMD colorize buffer mismatch".into()))
}

pub fn simd_solarize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Solarize { threshold } = op else {
        return Err(PilError::ValueError("expected Solarize op".into()));
    };
    if let Some(result) = native_byte_transform(img, mode, |input| {
        input
            .simd_ge(u8x16::splat(*threshold))
            .select(u8x16::splat(u8::MAX) - input, input)
    }) {
        return Ok(result);
    }
    Err(simd_unsupported("Solarize"))
}

pub fn simd_posterize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Posterize { bits } = op else {
        return Err(PilError::ValueError("expected Posterize op".into()));
    };

    // ImageOps.posterize validates the public mode before it queues this
    // operation: only L and RGB reach the pipeline, and both use the native
    // byte layouts above. The old packed-u32 fallback therefore had no
    // supported public input; keep an explicit diagnostic for an invalid
    // internal dispatch instead of maintaining a second implementation.
    let shift = 8u32
        .checked_sub(*bits as u32)
        .ok_or_else(|| PilError::ValueError("posterize bits must be at most 8".into()))?;
    native_byte_transform(img, mode, |input| (input >> shift) << shift).ok_or_else(|| {
        PilError::NotImplementedError(
            "SIMD posterize requires a validated native L or RGB byte image".into(),
        )
    })
}

pub fn simd_brightness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Brightness { factor } = op else {
        return Err(PilError::ValueError("expected Brightness op".into()));
    };
    // The packed scalar adapter intentionally quantizes the public factor
    // to the same fixed-point domain as the SIMD operation. Build that exact
    // 256-entry map once and apply it in native L/LA/RGB/RGBA/CMYK storage;
    // CMYK is admitted explicitly because all four bytes are active samples.
    let factor_fp = (*factor * 1000.0) as u32;
    let lut: Vec<u8> = (0u32..=255)
        .map(|value| ((value as u64 * factor_fp as u64) / 1000).min(255) as u8)
        .collect();
    if let Some(tables) = native_lut_tables(&lut) {
        if let Some(result) =
            native_brightness_transform(img, mode, |input| native_lut_chunk(input, &tables))
        {
            return Ok(result);
        }
    }
    Err(simd_unsupported("Brightness"))
}

/// Compute Pillow's scalar image-wide contrast midpoint, then apply the
/// native interleaved blend with `f64x8`.  Alpha bytes remain untouched for
/// LA/RGBA; CMYK's K byte uses the mode-specific degenerate value.
pub fn simd_contrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Contrast { factor } = op else {
        return Err(PilError::ValueError("expected Contrast op".into()));
    };
    let Some((channels, active_channels)) = native_enhance_layout(img, mode) else {
        return Err(simd_unsupported("Contrast"));
    };
    if !factor.is_finite() || !has_vectorized_float_bytes(img, channels) {
        return Err(simd_unsupported("Contrast"));
    }
    let mean = native_enhance_mean(img, mode, channels, false)
        .ok_or_else(|| simd_unsupported("Contrast"))?;
    let Some((result, vector_blocks, scalar_tail)) = native_enhance_output(
        img,
        mode,
        channels,
        active_channels,
        *factor,
        true,
        Some(mean),
    ) else {
        return Err(simd_unsupported("Contrast"));
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

/// Apply ImageEnhance.Color's per-pixel grayscale blend.  Grayscale and CMYK
/// bases are derived in scalar control code; the channel blend itself runs in
/// eight-lane vectors and never widens the whole image to packed RGBA.
pub fn simd_color_saturation(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::ColorSaturation { factor } = op else {
        return Err(PilError::ValueError("expected ColorSaturation op".into()));
    };
    let Some((channels, active_channels)) = native_enhance_layout(img, mode) else {
        return Err(simd_unsupported("ColorSaturation"));
    };
    if !factor.is_finite() || !has_vectorized_float_bytes(img, channels) {
        return Err(simd_unsupported("ColorSaturation"));
    }
    let Some((result, vector_blocks, scalar_tail)) = native_enhance_output(
        img,
        mode,
        channels,
        active_channels,
        *factor,
        false,
        None,
    ) else {
        return Err(simd_unsupported("ColorSaturation"));
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

/// Blend Sharpness's smoothed and original samples with eight-wide f64 lanes.
///
/// The samples are gathered by channel because Pillow keeps LA/RGBA alpha
/// untouched. Coordinate arithmetic and the interleaved stores are scalar
/// control; the per-sample blend and clamp are vector arithmetic. A scalar
/// tail is used only for the final pixels that do not fill a vector block.
fn native_sharpness_blend(
    source: &[u8],
    blurred: &[u8],
    output: &mut [u8],
    channels: usize,
    active_channels: usize,
    factor: f64,
) -> Option<(u64, u64)> {
    if source.len() != blurred.len()
        || source.len() != output.len()
        || !(1..=4).contains(&channels)
        || active_channels == 0
        || active_channels > channels
        || source.len() % channels != 0
    {
        return None;
    }
    let pixels = source.len() / channels;
    let factor_value = factor;
    let inverse = f64x8::splat(1.0 - factor_value);
    let factor = f64x8::splat(factor_value);
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for channel in 0..active_channels {
        let mut pixel = 0usize;
        while pixel + 8 <= pixels {
            let original = std::array::from_fn(|lane| {
                source[(pixel + lane) * channels + channel] as f64
            });
            let smooth = std::array::from_fn(|lane| {
                blurred[(pixel + lane) * channels + channel] as f64
            });
            let values = f64x8::from(smooth) * inverse + f64x8::from(original) * factor;
            for (lane, value) in values.to_array().into_iter().enumerate() {
                output[(pixel + lane) * channels + channel] = if value <= 0.0 {
                    0
                } else if value >= 255.0 {
                    255
                } else {
                    value as u8
                };
            }
            vector_blocks = vector_blocks.saturating_add(1);
            pixel += 8;
        }
        while pixel < pixels {
            let value = blurred[pixel * channels + channel] as f64 * (1.0 - factor_value)
                + source[pixel * channels + channel] as f64 * factor_value;
            output[pixel * channels + channel] = value.clamp(0.0, 255.0) as u8;
            scalar_tail = scalar_tail.saturating_add(1);
            pixel += 1;
        }
    }
    Some((vector_blocks, scalar_tail))
}

/// Apply ImageEnhance.Sharpness using the native interleaved layout.
///
/// Pillow first applies the 3x3 SMOOTH kernel, then blends that result with
/// the original image. The existing exact 3x3 SIMD kernel supplies the
/// neighborhood pass; this adapter keeps alpha out of that pass and uses
/// f64x8 for the final blend. No CPU pixel adapter or packed RGBA conversion
/// is used.
pub fn simd_sharpness(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Sharpness { factor } = op else {
        return Err(PilError::ValueError("expected Sharpness op".into()));
    };
    let Some((channels, active_channels)) = native_sharpness_layout(img, mode) else {
        return Err(simd_unsupported("Sharpness"));
    };
    if !factor.is_finite() || !has_vectorized_sharpness_bytes(img, channels) {
        return Err(simd_unsupported("Sharpness"));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let source = img.as_bytes();
    let mut blurred = source.to_vec();
    let kernel = [
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        5.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
        1.0f32 / 13.0,
    ];
    let (blur_blocks, _blur_tail) = native_filter_3x3_rows_active(
        source,
        &mut blurred,
        width,
        height,
        channels,
        active_channels,
        &kernel,
        0.5,
    );
    if blur_blocks == 0 {
        return Err(simd_unsupported("Sharpness"));
    }
    let mut output = source.to_vec();
    let (blend_blocks, blend_tail) = native_sharpness_blend(
        source,
        &blurred,
        &mut output,
        channels,
        active_channels,
        *factor,
    )
    .ok_or_else(|| simd_unsupported("Sharpness"))?;
    crate::compute::record_pipeline_operation_vector_blocks(blend_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(blend_tail);
    crate::compute::record_pipeline_operation_path("vector");
    let result = crate::image_utils::raw_bytes_to_image(
        img.width(),
        img.height(),
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Build the histogram and percentile LUT with the shared scalar control
/// plane, then apply every native L/RGB byte through the vector LUT kernel.
/// Masks affect only LUT construction; they do not force the output pass back
/// through the CPU pixel implementation.
pub fn simd_autocontrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Autocontrast { cutoff, mask } = op else {
        return Err(PilError::ValueError("expected Autocontrast op".into()));
    };
    let Some(channels) = native_autocontrast_layout(img, mode) else {
        return Err(simd_unsupported("Autocontrast"));
    };
    if !cutoff.is_finite()
        || !autocontrast_mask_supported(img.width(), img.height(), mask.as_ref())
    {
        return Err(simd_unsupported("Autocontrast"));
    }
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return Err(simd_unsupported("Autocontrast"));
    };
    if expected_len < 16 || img.as_bytes().len() != expected_len {
        return Err(simd_unsupported("Autocontrast"));
    }

    let lut = crate::compute::pool_cpu::ops::imageops::autocontrast_lut(
        img,
        *cutoff,
        mask.as_ref(),
    )?;
    let mut result = img.clone();
    let Some(bytes) = result.as_bytes_mut() else {
        return Err(simd_unsupported("Autocontrast"));
    };
    let Some((vector_blocks, scalar_tail)) = native_lut_apply(bytes, channels, &lut) else {
        return Err(simd_unsupported("Autocontrast"));
    };
    record_native_row_work(img.width() as usize, img.height() as usize, channels);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

/// Apply Pillow's equalize LUT directly to native L/RGB storage.
/// Histogram construction is shared scalar control; every stored output byte
/// is then handled by the vector LUT kernel without an RGB widening copy.
pub fn simd_equalize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::Equalize) {
        return Err(PilError::ValueError("expected Equalize op".into()));
    }
    let Some(channels) = native_autocontrast_layout(img, mode) else {
        return Err(simd_unsupported("Equalize"));
    };
    if !has_vectorized_flat_bytes(img, channels) {
        return Err(simd_unsupported("Equalize"));
    }
    let Some(lut) = crate::compute::pool_cpu::ops::imageops::equalize_lut(img, channels) else {
        return Err(simd_unsupported("Equalize"));
    };
    let mut result = img.clone();
    let Some(bytes) = result.as_bytes_mut() else {
        return Err(simd_unsupported("Equalize"));
    };
    let Some((vector_blocks, scalar_tail)) = native_lut_apply(bytes, channels, &lut) else {
        return Err(simd_unsupported("Equalize"));
    };
    record_native_row_work(img.width() as usize, img.height() as usize, channels);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

/// Extract one channel from a native 8-bit image without expanding it to
/// packed RGBA storage.  The public `getchannel` operation is a byte-domain
/// copy for L/LA/RGB/RGBA (and CMYK, which uses the four-byte RGBA storage in
/// this crate), so widening it would add a conversion boundary for no gain.
/// Typed samples are deliberately left on the exact CPU implementation and
/// recorded as an internal fallback rather than being mislabeled as SIMD.
pub fn simd_extract_band(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::ExtractBand { index } = op else {
        return Err(PilError::ValueError("expected ExtractBand op".into()));
    };

    let channels = native_extract_layout(img, mode).ok_or_else(|| simd_unsupported("ExtractBand"))?;
    let source = img.as_bytes();

    let (width, height) = img.dimensions();
    let pixel_count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::InternalError("ExtractBand pixel count overflow".into()))?;
    let source_len = pixel_count
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("ExtractBand source length overflow".into()))?;
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "ExtractBand source buffer shape mismatch".into(),
        ));
    }

    // `getchannel` validates the public index before queuing the operation.
    // Keep the CPU operation's defensive clamping for direct internal
    // PipelineOp callers while using the native storage stride here.
    let channel = usize::from(*index).min(channels - 1);
    let mut output = vec![0u8; pixel_count];
    // One shuffle consumes at most 16 source bytes.  The native layouts have
    // one to four bytes per pixel, so this processes 16, 8, 5, or 4 pixels
    // per vector respectively.
    let pixels_per_vector = 16 / channels;
    let mut pixel = 0usize;
    while pixel + pixels_per_vector <= pixel_count {
        let source_start = pixel * channels;
        let source_bytes = pixels_per_vector * channels;
        let mut source_block = [0u8; 16];
        source_block[..source_bytes]
            .copy_from_slice(&source[source_start..source_start + source_bytes]);
        let indices = std::array::from_fn(|lane| {
            ((lane % pixels_per_vector) * channels + channel) as u8
        });
        let extracted = u8x16::new(source_block)
            .swizzle_relaxed(u8x16::new(indices))
            .to_array();
        output[pixel..pixel + pixels_per_vector]
            .copy_from_slice(&extracted[..pixels_per_vector]);
        pixel += pixels_per_vector;
    }
    for pixel in pixel..pixel_count {
        output[pixel] = source[pixel * channels + channel];
    }
    let vector_blocks = pixel_count / pixels_per_vector;
    let scalar_tail = pixel_count % pixels_per_vector;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }
    crate::compute::record_pipeline_operation_path("vector");

    GrayImage::from_raw(width, height, output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD ExtractBand buffer shape mismatch".into()))
}

/// Draw a width-one line directly in the destination's native byte layout.
/// Geometry, clipping, and mode admission are scalar control work; contiguous
/// raster runs are filled/blended by the masked `u8x16` kernel.  Wide and typed
/// lines are rejected by preflight so they cannot silently enter the CPU draw
/// adapter.
pub fn simd_draw_line(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawLine {
        x0,
        y0,
        x1,
        y1,
        fill,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawLine op".into()));
    };
    simd_draw_line_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *fill,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawLine"))
}

/// Draw one or more points through the destination's native byte layout.
pub fn simd_draw_point(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawPoint {
        points,
        fill,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawPoint op".into()));
    };
    let Some(channels) = native_draw_layout(img, mode) else {
        return Err(simd_unsupported("DrawPoint"));
    };
    simd_draw_points_native(img, points, *fill, *alpha_blend_rgb, channels)?
        .ok_or_else(|| simd_unsupported("DrawPoint"))
}

/// Draw a rectangle with scalar edge geometry and native-byte SIMD spans.
///
/// The outline's placement and clipping follow the CPU Pillow-compatible
/// primitive. Horizontal and one-pixel vertical spans use masked vector
/// stores, so the adapter never delegates a supported rectangle to the CPU
/// draw canvas or widens its samples through RGBA.
pub fn simd_draw_rectangle(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawRectangle {
        x0,
        y0,
        x1,
        y1,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawRectangle op".into()));
    };
    if native_draw_layout(img, mode).is_some()
        && valid_draw_rectangle(img.width(), img.height(), *x0, *y0, *x1, *y1)
        && !has_visible_draw_rectangle(
            img.width(),
            img.height(),
            *x0,
            *y0,
            *x1,
            *y1,
            *fill,
            *outline,
            *width,
        )
    {
        // A valid native-mode rectangle can be a public no-op (for example,
        // ImageDraw on L with both inks omitted) or can lie completely outside
        // the canvas. No pixel data needs a vector kernel in that case, but it
        // is still a SIMD control-plane result rather than a CPU fallback.
        crate::compute::record_pipeline_operation_path("scalar-control");
        return Ok(img.clone());
    }
    simd_draw_rectangle_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawRectangle"))
}

/// Draw a polygon through Pillow's scalar scanline geometry and native SIMD
/// span writes.  The current admission contract covers fills and the public
/// no-op cases where the outline is absent, equal to the fill, or zero-width.
/// A distinct outline is deliberately rejected during preflight because
/// Pillow masks wide polygon strokes against the fill; advertising that path
/// before a native masked-stroke kernel exists would reintroduce a hidden CPU
/// data path.
pub fn simd_draw_polygon(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawPolygon {
        points,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawPolygon op".into()));
    };
    simd_draw_polygon_native(
        img,
        points,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawPolygon"))
}

/// Draw a rounded rectangle using scalar corner geometry and native SIMD
/// spans for fills, arcs, and straight edge sections.
pub fn simd_draw_rounded_rect(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawRoundedRect {
        x0,
        y0,
        x1,
        y1,
        radius,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError(
            "expected DrawRoundedRect op".into(),
        ));
    };
    simd_draw_rounded_rect_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *radius,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawRoundedRect"))
}

/// Draw an ellipse through the scalar geometry/SIMD span split.
pub fn simd_draw_ellipse(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawEllipse {
        x0,
        y0,
        x1,
        y1,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawEllipse op".into()));
    };
    simd_draw_ellipse_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawEllipse"))
}

/// Draw a circle through the ellipse scan converter and native SIMD spans.
pub fn simd_draw_circle(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawCircle {
        cx,
        cy,
        radius,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawCircle op".into()));
    };
    simd_draw_ellipse_native(
        img,
        cx.saturating_sub(*radius),
        cy.saturating_sub(*radius),
        cx.saturating_add(*radius),
        cy.saturating_add(*radius),
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawCircle"))
}

/// Draw an arc through scalar clipping and native SIMD span writes.
pub fn simd_draw_arc(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawArc {
        x0,
        y0,
        x1,
        y1,
        start,
        end,
        fill,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawArc op".into()));
    };
    simd_draw_arc_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *start,
        *end,
        *fill,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawArc"))
}

/// Draw a chord through scalar clipping and native SIMD span writes.
pub fn simd_draw_chord(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawChord {
        x0,
        y0,
        x1,
        y1,
        start,
        end,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawChord op".into()));
    };
    simd_draw_chord_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *start,
        *end,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawChord"))
}

/// Draw a pieslice through scalar clipping and native SIMD span writes.
pub fn simd_draw_pieslice(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::DrawPieslice {
        x0,
        y0,
        x1,
        y1,
        start,
        end,
        fill,
        outline,
        width,
        alpha_blend_rgb,
    } = op
    else {
        return Err(PilError::ValueError("expected DrawPieslice op".into()));
    };
    simd_draw_pieslice_native(
        img,
        *x0,
        *y0,
        *x1,
        *y1,
        *start,
        *end,
        *fill,
        *outline,
        *width,
        *alpha_blend_rgb,
        mode,
    )?
    .ok_or_else(|| simd_unsupported("DrawPieslice"))
}

pub fn simd_offset(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Offset { x, y } = op else {
        return Err(PilError::ValueError("expected Offset op".into()));
    };
    if let Some((result, vector_blocks, scalar_tail)) = native_offset_luma16(img, *x, *y, mode) {
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return Ok(result);
    }
    if let Some((result, vector_blocks, scalar_tail)) = native_offset(img, *x, *y, mode) {
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return Ok(result);
    }
    Err(simd_unsupported("Offset"))
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
    if let Some((result, vector_blocks, scalar_tail)) = native_flip_vertical(img, mode) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    Err(simd_unsupported("Flip"))
}

pub fn simd_mirror(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let Some((result, vector_blocks, scalar_tail)) = native_mirror(img, mode) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    Err(simd_unsupported("Mirror"))
}

/// Generate a linear gradient through native vector stores.
///
/// The public `Image.linear_gradient` constructor is eager, so it does not
/// enter the ordinary `PipelineOp` dispatcher.  Its data contract is still a
/// straightforward row fill: every pixel in row `y` carries the scalar value
/// `y`, with the mode-specific sample width retained in the native bytes.  A
/// `u8x16` store is therefore the complete data plane for all supported modes;
/// only the row value and its four-byte encoding are scalar control work.
pub(crate) fn simd_linear_gradient_generate(mode: &str) -> Result<DynamicImage, PilError> {
    let channels = match mode {
        "1" | "L" | "P" => 1,
        "I" | "F" => 4,
        _ => return Err(simd_unsupported("LinearGradient")),
    };
    let row_bytes = 256usize
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD LinearGradient row length overflow".into()))?;
    let output_len = row_bytes
        .checked_mul(256)
        .ok_or_else(|| PilError::ValueError("SIMD LinearGradient output length overflow".into()))?;
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for (y, row) in output.chunks_exact_mut(row_bytes).enumerate() {
        let block = match mode {
            "I" => {
                let bytes = (y as i32).to_le_bytes();
                u8x16::new([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                ])
            }
            "F" => {
                let bytes = (y as f32).to_le_bytes();
                u8x16::new([
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                    bytes[0], bytes[1], bytes[2], bytes[3], bytes[0], bytes[1], bytes[2], bytes[3],
                ])
            }
            "1" if y != 0 => u8x16::splat(0xff),
            _ => u8x16::splat(y as u8),
        };
        let block = block.to_array();
        let vector_len = row.len() / 16 * 16;
        for chunk in row[..vector_len].chunks_exact_mut(16) {
            chunk.copy_from_slice(&block);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let tail = row.len() - vector_len;
        if tail != 0 {
            row[vector_len..].copy_from_slice(&block[..tail]);
            vector_blocks = vector_blocks.saturating_add(1);
            scalar_tail = scalar_tail.saturating_add(tail as u64);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(256, 256, output, channels)
}

/// Execute the retained deferred `LinearGradient` descriptor through the same
/// native generator used by the eager public constructor.
pub fn simd_linear_gradient(
    _img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::LinearGradient { mode } = op else {
        return Err(PilError::ValueError("expected LinearGradient op".into()));
    };
    simd_linear_gradient_generate(color_mode_name(mode))
}

/// Generate Pillow's deterministic effect-noise stream with scalar RNG and
/// Box-Muller control, then vectorize the per-sample affine/clamp arithmetic.
/// The RNG sequence and rejection order remain scalar and process-global;
/// every complete eight-pixel group uses the SIMD data plane for
/// `128 + sigma * deviate` before the final byte conversion.
pub fn simd_effect_noise(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::EffectNoise { sigma } = op else {
        return Err(PilError::ValueError("expected EffectNoise op".into()));
    };
    let (width, height) = img.dimensions();
    if height == 0 || !sigma.is_finite() {
        return Err(simd_unsupported("EffectNoise"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD EffectNoise pixel count overflow".into()))?;
    if pixels == 0 {
        return Err(simd_unsupported("EffectNoise"));
    }
    let sigma = f64::from(*sigma as f32);
    const RAND_MAX_F64: f64 = 2_147_483_647.0;
    let mut output = vec![0u8; pixels];

    crate::compute::pool_cpu::ops::effects::with_process_rng(|rng| {
        let mut pixel = 0usize;
        while pixel < pixels {
            let active = (pixels - pixel).min(8);
            let mut deviates = [0.0; 8];
            for deviate in deviates.iter_mut().take(active) {
                let (v1, radius) = loop {
                    let v1 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                    let v2 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
                    let radius = v1 * v1 + v2 * v2;
                    if radius < 1.0 {
                        break (v1, radius);
                    }
                };
                let factor = (-2.0 * radius.ln() / radius).sqrt();
                *deviate = factor * v1;
            }
            let values = f64x8::new(deviates) * f64x8::splat(sigma) + f64x8::splat(128.0);
            for (destination, value) in output[pixel..pixel + active]
                .iter_mut()
                .zip(values.to_array().into_iter().take(active))
            {
                *destination = if value <= 0.0 {
                    0
                } else if value >= 255.0 {
                    255
                } else {
                    value as u8
                };
            }
            // The final partial batch still uses the vector arithmetic path;
            // inactive lanes are zero-filled and are never written. This is
            // important for effect_noise because every accepted pixel must
            // consume its RNG pair even when the image has fewer than eight
            // pixels, otherwise later cases observe a shifted process stream.
            pixel += active;
        }
    })?;

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(pixels.div_ceil(8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    GrayImage::from_raw(width, height, output)
        .map(DynamicImage::ImageLuma8)
        .ok_or_else(|| PilError::InternalError("SIMD EffectNoise buffer shape mismatch".into()))
}

#[inline]
fn color3dlut_prepare_fixed(value: f64) -> i16 {
    // Keep this conversion identical to `op_color3dlut`: Pillow first
    // narrows table values to float32 and then rounds signed 12.4 samples.
    const PRECISION_BITS: i32 = 4;
    let item = value as f32;
    let scaled = item * ((255 << PRECISION_BITS) as f32);
    if scaled >= i16::MAX as f32 - 0.5 {
        i16::MAX
    } else if scaled <= i16::MIN as f32 + 0.5 {
        i16::MIN
    } else if item < 0.0 {
        (scaled - 0.5) as i16
    } else {
        (scaled + 0.5) as i16
    }
}

#[inline]
fn color3dlut_interpolate_scalar(a: i16, b: i16, shift: i32) -> i16 {
    const SHIFT_BITS: i32 = 15;
    let value = (i64::from(a) * i64::from((1 << SHIFT_BITS) - shift)
        + i64::from(b) * i64::from(shift))
        >> SHIFT_BITS;
    value as i16
}

#[inline]
fn color3dlut_interpolate_vector(a: i32x8, b: i32x8, shift: i32x8) -> i32x8 {
    const SHIFT_BITS: u32 = 15;
    let one = i32x8::splat(1i32 << SHIFT_BITS);
    (a * (one - shift) + b * shift) >> SHIFT_BITS
}

#[inline]
fn color3dlut_table_index(x: usize, y: usize, z: usize, sx: usize, sxy: usize) -> usize {
    x + y * sx + z * sxy
}

fn color3dlut_write_scalar_pixel(
    source: &[u8],
    pixel: usize,
    source_channels: usize,
    prepared: &[i16],
    size: (usize, usize, usize),
    scales: [u32; 3],
    channels: usize,
    target_channels: usize,
    output: &mut [u8],
) {
    const PRECISION_BITS: i32 = 4;
    const SCALE_BITS: u32 = 18;
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
    const SHIFT_BITS: u32 = 15;
    let (sx, sy, _) = size;
    let sxy = sx * sy;
    let source_offset = pixel * source_channels;
    let indices = [
        u32::from(source[source_offset]) * scales[0],
        u32::from(source[source_offset + 1]) * scales[1],
        u32::from(source[source_offset + 2]) * scales[2],
    ];
    let shifts = indices.map(|index| ((SCALE_MASK & index) >> (SCALE_BITS - SHIFT_BITS)) as i32);
    let base = color3dlut_table_index(
        (indices[0] >> SCALE_BITS) as usize,
        (indices[1] >> SCALE_BITS) as usize,
        (indices[2] >> SCALE_BITS) as usize,
        sx,
        sxy,
    ) * channels;
    let output_offset = pixel * target_channels;
    for c in 0..channels {
        let left_left = color3dlut_interpolate_scalar(
            prepared[base + c],
            prepared[base + channels + c],
            shifts[0],
        );
        let left_right = color3dlut_interpolate_scalar(
            prepared[base + sx * channels + c],
            prepared[base + sx * channels + channels + c],
            shifts[0],
        );
        let left = color3dlut_interpolate_scalar(left_left, left_right, shifts[1]);
        let right_left = color3dlut_interpolate_scalar(
            prepared[base + sxy * channels + c],
            prepared[base + sxy * channels + channels + c],
            shifts[0],
        );
        let right_right = color3dlut_interpolate_scalar(
            prepared[base + sxy * channels + sx * channels + c],
            prepared[base + sxy * channels + sx * channels + channels + c],
            shifts[0],
        );
        let right = color3dlut_interpolate_scalar(right_left, right_right, shifts[1]);
        let result = color3dlut_interpolate_scalar(left, right, shifts[2]);
        output[output_offset + c] = ((i32::from(result) + (1 << (PRECISION_BITS - 1)))
            >> PRECISION_BITS)
            .clamp(0, 255) as u8;
    }
    if channels == 3 && target_channels == 4 {
        output[output_offset + 3] = if source_channels == 4 {
            source[source_offset + 3]
        } else {
            255
        };
    }
}

fn color3dlut_write_vector_batch(
    source: &[u8],
    first_pixel: usize,
    source_channels: usize,
    prepared: &[i16],
    size: (usize, usize, usize),
    scales: [u32; 3],
    channels: usize,
    target_channels: usize,
    output: &mut [u8],
) {
    const PRECISION_BITS: i32 = 4;
    const SCALE_BITS: u32 = 18;
    const SCALE_MASK: u32 = (1 << SCALE_BITS) - 1;
    const SHIFT_BITS: u32 = 15;
    let (sx, sy, _) = size;
    let sxy = sx * sy;
    let mut bases = [0usize; 8];
    let mut shift_x = [0i32; 8];
    let mut shift_y = [0i32; 8];
    let mut shift_z = [0i32; 8];
    for lane in 0..8 {
        let source_offset = (first_pixel + lane) * source_channels;
        let indices = [
            u32::from(source[source_offset]) * scales[0],
            u32::from(source[source_offset + 1]) * scales[1],
            u32::from(source[source_offset + 2]) * scales[2],
        ];
        bases[lane] = color3dlut_table_index(
            (indices[0] >> SCALE_BITS) as usize,
            (indices[1] >> SCALE_BITS) as usize,
            (indices[2] >> SCALE_BITS) as usize,
            sx,
            sxy,
        ) * channels;
        shift_x[lane] = ((SCALE_MASK & indices[0]) >> (SCALE_BITS - SHIFT_BITS)) as i32;
        shift_y[lane] = ((SCALE_MASK & indices[1]) >> (SCALE_BITS - SHIFT_BITS)) as i32;
        shift_z[lane] = ((SCALE_MASK & indices[2]) >> (SCALE_BITS - SHIFT_BITS)) as i32;
    }
    let shift_x = i32x8::new(shift_x);
    let shift_y = i32x8::new(shift_y);
    let shift_z = i32x8::new(shift_z);

    for c in 0..channels {
        let left_left = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| prepared[bases[lane] + c] as i32)),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + channels + c] as i32
            })),
            shift_x,
        );
        let left_right = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sx * channels + c] as i32
            })),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sx * channels + channels + c] as i32
            })),
            shift_x,
        );
        let left = color3dlut_interpolate_vector(left_left, left_right, shift_y);
        let right_left = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + c] as i32
            })),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + channels + c] as i32
            })),
            shift_x,
        );
        let right_right = color3dlut_interpolate_vector(
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + sx * channels + c] as i32
            })),
            i32x8::new(std::array::from_fn(|lane| {
                prepared[bases[lane] + sxy * channels + sx * channels + channels + c] as i32
            })),
            shift_x,
        );
        let right = color3dlut_interpolate_vector(right_left, right_right, shift_y);
        let result = color3dlut_interpolate_vector(left, right, shift_z).to_array();
        for (lane, value) in result.into_iter().enumerate() {
            let output_offset = (first_pixel + lane) * target_channels;
            output[output_offset + c] = ((value + (1 << (PRECISION_BITS - 1)))
                >> PRECISION_BITS)
                .clamp(0, 255) as u8;
        }
    }
    if channels == 3 && target_channels == 4 {
        for lane in 0..8 {
            let source_offset = (first_pixel + lane) * source_channels;
            let output_offset = (first_pixel + lane) * target_channels;
            output[output_offset + 3] = if source_channels == 4 {
                source[source_offset + 3]
            } else {
                255
            };
        }
    }
}

/// Apply a validated Color3DLUT with scalar table-index gathers and a
/// fixed-point eight-lane interpolation kernel.  Table addressing and the
/// scalar tail stay in the control plane because the LUT is an indirect
/// memory access; all trilinear multiply/shift stages for complete batches
/// execute through `i32x8`.
pub fn simd_color3dlut(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Color3DLut {
        size,
        table,
        channels,
        source_mode,
        target_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected Color3DLut op".into()));
    };
    if !color3dlut_supported_for_image(
        img,
        *size,
        table.len(),
        *channels,
        *source_mode,
        *target_mode,
        mode,
    ) {
        return Err(simd_unsupported("Color3DLut"));
    }
    let source_channels = color3dlut_source_channels_for_image(img, *source_mode)
        .ok_or_else(|| simd_unsupported("Color3DLut"))?;
    let target_channels = color3dlut_target_channels(*target_mode)
        .ok_or_else(|| simd_unsupported("Color3DLut"))?;
    let (width, height) = img.dimensions();
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD Color3DLut pixel count overflow".into()))?;
    let output_len = pixels
        .checked_mul(target_channels)
        .ok_or_else(|| PilError::ValueError("SIMD Color3DLut output size overflow".into()))?;
    let prepared: Vec<i16> = table
        .iter()
        .copied()
        .map(color3dlut_prepare_fixed)
        .collect();
    let size_usize = (size.0 as usize, size.1 as usize, size.2 as usize);
    const SCALE_BITS: u32 = 18;
    let scales = [
        ((size_usize.0 - 1) as f64 / 255.0 * f64::from(1u32 << SCALE_BITS)) as u32,
        ((size_usize.1 - 1) as f64 / 255.0 * f64::from(1u32 << SCALE_BITS)) as u32,
        ((size_usize.2 - 1) as f64 / 255.0 * f64::from(1u32 << SCALE_BITS)) as u32,
    ];
    let source = img.as_bytes();
    let mut output = vec![0u8; output_len];
    let vector_end = pixels - pixels % 8;
    for pixel in (0..vector_end).step_by(8) {
        color3dlut_write_vector_batch(
            source,
            pixel,
            source_channels,
            &prepared,
            size_usize,
            scales,
            *channels as usize,
            target_channels,
            &mut output,
        );
    }
    for pixel in vector_end..pixels {
        color3dlut_write_scalar_pixel(
            source,
            pixel,
            source_channels,
            &prepared,
            size_usize,
            scales,
            *channels as usize,
            target_channels,
            &mut output,
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks((vector_end / 8) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_end) as u64);
    match target_mode {
        PixelMode::RGB => RgbImage::from_raw(width, height, output)
            .map(DynamicImage::ImageRgb8)
            .ok_or_else(|| PilError::InternalError("SIMD Color3DLut output shape mismatch".into())),
        PixelMode::RGBA | PixelMode::CMYK => RgbaImage::from_raw(width, height, output)
            .map(DynamicImage::ImageRgba8)
            .ok_or_else(|| PilError::InternalError("SIMD Color3DLut output shape mismatch".into())),
        _ => Err(simd_unsupported("Color3DLut")),
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Section D: Filter/window ops (convolution and blur)
// ═══════════════════════════════════════════════════════════════════════

/// The packed convolution adapter is scalar and widens every non-native image
/// to RGBA before evaluating its window. Native byte layouts use the row
/// kernels below, including small images, so SIMD does not pay a conversion
/// boundary or hide a CPU fallback behind an area threshold. The width and
/// height checks ensure that the adapter has at least one complete vector block
/// in the interior; the remaining pixels are handled as scalar tails.
fn use_native_byte_convolution_path(
    img: &DynamicImage,
    mode: Option<&str>,
    border: usize,
) -> bool {
    native_filter_byte_layout(img, mode).is_some()
        && img.width() as usize >= border.saturating_mul(2).saturating_add(1)
        && img.height() as usize > border.saturating_mul(2)
}

fn use_native_i32_convolution_path(
    img: &DynamicImage,
    mode: Option<&str>,
    border: usize,
) -> bool {
    if mode != Some("I") || !matches!(img, DynamicImage::ImageRgba8(_)) {
        return false;
    }
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    img.width() as usize >= border.saturating_mul(2).saturating_add(1)
        && img.height() as usize > border.saturating_mul(2)
        && img.as_bytes().len() == expected_len
}

fn native_filter_identity_supported_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
    border: usize,
) -> bool {
    let valid_layout = native_filter_byte_layout(img, mode).is_some()
        || (mode == Some("I") && native_typed_filter_layout(img, mode).is_some());
    let no_interior = img.width() as usize <= border.saturating_mul(2)
        || img.height() as usize <= border.saturating_mul(2);
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(if mode == Some("I") {
            4
        } else {
            native_filter_byte_layout(img, mode).unwrap_or(0)
        }))
    else {
        return false;
    };
    valid_layout
        && no_interior
        && expected_len != 0
        && img.as_bytes().len() == expected_len
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
    native_filter_3x3_rows_active(
        raw,
        out,
        width,
        height,
        channels,
        channels,
        kernel,
        rounding_bias,
    );
}

/// Apply a native-byte 3x3 convolution to only the active channels.
///
/// LA/RGBA Sharpness uses this form because Pillow smooths the RGB samples
/// while copying alpha unchanged. CMYK passes all four stored samples as
/// active channels. The vector arithmetic and scalar interior tail are shared
/// with the ordinary Filter3x3 path.
fn native_filter_3x3_rows_active(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    active_channels: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> (u64, u64) {
    if !(1..=4).contains(&channels)
        || active_channels == 0
        || active_channels > channels
        || width < 3
        || height < 3
    {
        return (0, 0);
    }
    let interior_width = width - 2;
    let interior_height = height - 2;
    let vector_blocks = interior_width.div_ceil(8)
        .saturating_mul(active_channels)
        .saturating_mul(interior_height);
    let scalar_tail = 0;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }
    let row_stride = width * channels;
    let apply_row = |y: usize, row: &mut [u8]| {
        for channel in 0..active_channels {
            let mut x = 1usize;
            while x < width - 1 {
                let active = (width - 1 - x).min(8);
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
                for (lane, value) in values.into_iter().enumerate().take(active) {
                    row[(x + lane) * channels + channel] = value;
                }
                x += active;
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
    (vector_blocks as u64, scalar_tail as u64)
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
    let interior_width = width - 4;
    let interior_height = height - 4;
    let vector_blocks = interior_width.div_ceil(8)
        .saturating_mul(channels)
        .saturating_mul(interior_height);
    let scalar_tail = 0;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    }
    let row_stride = width * channels;
    let apply_row = |y: usize, row: &mut [u8]| {
        for channel in 0..channels {
            let mut x = 2usize;
            while x < width - 2 {
                let active = (width - 2 - x).min(8);
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
                for (lane, value) in values.into_iter().enumerate().take(active) {
                    row[(x + lane) * channels + channel] = value;
                }
                x += active;
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

/// Evaluate eight I-mode samples of a 3x3 kernel in parallel.
///
/// I-mode stores one signed little-endian i32 per pixel in the four-byte
/// native buffer. Address calculation and byte decoding stay scalar because
/// the portable `wide` API has no gather instruction; accumulation, FMA
/// ordering, clamping, and conversion are performed by the vector lanes.
#[inline]
fn native_filter_3x3_i32_vector(
    raw: &[u8],
    width: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> [i32; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut left = [0.0f32; 8];
        let mut middle = [0.0f32; 8];
        let mut right = [0.0f32; 8];
        let source_y = (y as isize + dy) as usize;
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 2);
            let read = |source_x: usize| -> f32 {
                let base = (source_y * width + source_x) * 4;
                i32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]])
                    as f32
            };
            left[lane] = read(x - 1);
            middle[lane] = read(x);
            right[lane] = read(x + 1);
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
        if values[lane] >= 0.0 {
            values[lane] as i32
        } else {
            0
        }
    })
}

fn native_filter_3x3_i32_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 9],
    rounding_bias: f32,
) -> (u64, u64) {
    if width < 3 || height < 3 {
        return (0, 0);
    }
    let interior_width = width - 2;
    let interior_height = height - 2;
    let vector_blocks = interior_width.div_ceil(8).saturating_mul(interior_height);
    let row_stride = width * 4;
    let apply_row = |y: usize, destination: &mut [u8]| {
        let mut x = 1usize;
        while x < width - 1 {
            let active = (width - 1 - x).min(8);
            let values = native_filter_3x3_i32_vector(
                raw,
                width,
                y,
                x,
                kernel,
                rounding_bias,
            );
            for (lane, value) in values.into_iter().enumerate().take(active) {
                let base = (x + lane) * 4;
                destination[base..base + 4].copy_from_slice(&value.to_le_bytes());
            }
            x += active;
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
    (vector_blocks as u64, 0)
}

/// Evaluate eight I-mode samples of a 5x5 kernel in parallel, preserving the
/// same reversed-row and middle-first FMA order as Pillow's CPU path.
#[inline]
fn native_filter_5x5_i32_vector(
    raw: &[u8],
    width: usize,
    y: usize,
    x_start: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> [i32; 8] {
    let row = |dy: isize, kernel_start: usize| -> f32x8 {
        let mut samples = [[0.0f32; 8]; 5];
        let source_y = (y as isize + dy) as usize;
        for lane in 0..8 {
            let x = (x_start + lane).min(width - 3);
            for tap in 0..5 {
                let source_x = x + tap - 2;
                let base = (source_y * width + source_x) * 4;
                samples[tap][lane] =
                    i32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]])
                        as f32;
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
        if values[lane] >= 0.0 {
            values[lane] as i32
        } else {
            0
        }
    })
}

fn native_filter_5x5_i32_rows(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) -> (u64, u64) {
    if width < 5 || height < 5 {
        return (0, 0);
    }
    let interior_width = width - 4;
    let interior_height = height - 4;
    let vector_blocks = interior_width.div_ceil(8).saturating_mul(interior_height);
    let row_stride = width * 4;
    let apply_row = |y: usize, destination: &mut [u8]| {
        let mut x = 2usize;
        while x < width - 2 {
            let active = (width - 2 - x).min(8);
            let values = native_filter_5x5_i32_vector(
                raw,
                width,
                y,
                x,
                kernel,
                rounding_bias,
            );
            for (lane, value) in values.into_iter().enumerate().take(active) {
                let base = (x + lane) * 4;
                destination[base..base + 4].copy_from_slice(&value.to_le_bytes());
            }
            x += active;
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
    (vector_blocks as u64, 0)
}

fn simd_filter_3x3_i32(
    img: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: i32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !use_native_i32_convolution_path(img, mode, 1) {
        return Err(simd_unsupported("Filter3x3"));
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = native_filter_3x3_i32_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset as f32 + 0.5,
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

fn simd_filter_5x5_i32(
    img: &DynamicImage,
    kernel: &[f32; 25],
    scale: f32,
    offset: i32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !use_native_i32_convolution_path(img, mode, 2) {
        return Err(simd_unsupported("Filter5x5"));
    }
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / scale);
    let mut output = img.as_bytes().to_vec();
    let (vector_blocks, scalar_tail) = native_filter_5x5_i32_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        &normalized_kernel,
        offset as f32 + 0.5,
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

fn simd_filter_identity(
    img: &DynamicImage,
    mode: Option<&str>,
    operation: &str,
) -> Result<DynamicImage, PilError> {
    let channels = if mode == Some("I") {
        4
    } else {
        native_filter_byte_layout(img, mode).ok_or_else(|| simd_unsupported(operation))?
    };
    let mut output = vec![0u8; img.as_bytes().len()];
    let (vector_blocks, scalar_tail) = copy_native_bytes(img.as_bytes(), &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD filter identity buffer mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

pub fn simd_filter_3x3(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Filter3x3 {
        kernel,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Filter3x3 op".into()));
    };
    if native_filter_identity_supported_for_image(img, mode, 1) {
        return simd_filter_identity(img, mode, "Filter3x3");
    }
    if mode == Some("I") {
        return simd_filter_3x3_i32(img, kernel, *scale, *offset, mode);
    }
    if !use_native_byte_convolution_path(img, mode, 1) {
        return Err(simd_unsupported("Filter3x3"));
    }
    let channels = native_filter_byte_layout(img, mode)
        .ok_or_else(|| simd_unsupported("Filter3x3"))?;
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
    let mut output = img.as_bytes().to_vec();
    crate::compute::record_pipeline_operation_path("vector");
    native_filter_3x3_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        &normalized_kernel,
        *offset as f32 + 0.5,
    );
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

pub fn simd_filter_5x5(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Filter5x5 {
        kernel,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Filter5x5 op".into()));
    };
    if native_filter_identity_supported_for_image(img, mode, 2) {
        return simd_filter_identity(img, mode, "Filter5x5");
    }
    if mode == Some("I") {
        return simd_filter_5x5_i32(img, kernel, *scale, *offset, mode);
    }
    if !use_native_byte_convolution_path(img, mode, 2) {
        return Err(simd_unsupported("Filter5x5"));
    }
    let channels = native_filter_byte_layout(img, mode)
        .ok_or_else(|| simd_unsupported("Filter5x5"))?;
    let normalized_kernel = std::array::from_fn(|index| kernel[index] / *scale);
    let mut output = img.as_bytes().to_vec();
    crate::compute::record_pipeline_operation_path("vector");
    native_filter_5x5_rows(
        img.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        &normalized_kernel,
        *offset as f32 + 0.5,
    );
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)?;
    Ok(preserve_mode(img, result))
}

// ── Native rank-family extrema ─────────────────────────────────────────────

const SIMD_RANK_FILTER_LANES: usize = 16;
const SIMD_RANK_FILTER_MIN_VECTOR_PIXELS: usize = 1;

fn native_float_rank_supported_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
    size: u32,
    rank: u32,
) -> bool {
    let area = u64::from(size).saturating_mul(u64::from(size));
    native_typed_filter_layout(img, mode).is_some()
        && mode == Some("F")
        && size != 0
        && size % 2 == 1
        && size <= SIMD_ORDER_STATISTIC_SORT_MAX_SIZE
        && u64::from(rank) < area
        && img.width() != 0
        && img.height() != 0
        && img
            .width()
            .checked_mul(img.height())
            .and_then(|pixels| pixels.checked_mul(4))
            .is_some_and(|bytes| img.as_bytes().len() == bytes as usize)
}

/// Choose a useful active width for one `u8x16` pixel block.
///
/// The image is interleaved, so the vector operates on one channel at a time.
/// A partial block is still a real vector operation: inactive lanes are held
/// at the operation's identity value and are never stored. This also keeps
/// narrow images on the vector data path instead of making a valid public
/// input appear to be CPU-only merely because it has fewer than eight pixels.
#[inline]
fn rank_filter_vector_pixels(remaining: usize) -> usize {
    if remaining >= SIMD_RANK_FILTER_LANES {
        SIMD_RANK_FILTER_LANES
    } else {
        remaining.min(SIMD_RANK_FILTER_LANES)
    }
}

#[inline]
fn rank_filter_identity(select_max: bool) -> u8 {
    if select_max { 0 } else { u8::MAX }
}

/// Load one clamped horizontal window into the low lanes of a byte vector.
/// Coordinate arithmetic and interleaved-channel addressing are scalar
/// control; the window comparison itself is performed by the vector kernel.
#[inline]
fn rank_filter_horizontal_window_vector(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    half: usize,
    window_offset: usize,
    select_max: bool,
) -> u8x16 {
    let mut values = [rank_filter_identity(select_max); SIMD_RANK_FILTER_LANES];
    let last = width - 1;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        *value = raw[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

/// Load one clamped vertical window from the horizontal intermediate.
#[inline]
fn rank_filter_vertical_window_vector(
    horizontal: &[u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    y: usize,
    half: usize,
    window_offset: usize,
    select_max: bool,
) -> u8x16 {
    let mut values = [rank_filter_identity(select_max); SIMD_RANK_FILTER_LANES];
    let last = height - 1;
    let source_y = y
        .saturating_add(window_offset)
        .saturating_sub(half)
        .min(last);
    let row_start = source_y * row_stride;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start + lane;
        debug_assert!(source_x < width);
        *value = horizontal[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

#[inline]
fn store_rank_filter_channel_vector(
    output: &mut [u8],
    row_start: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    values: u8x16,
) {
    let values = values.to_array();
    for (lane, value) in values.into_iter().enumerate().take(active) {
        output[row_start + (x_start + lane) * channels + channel] = value;
    }
}

#[inline]
fn rank_filter_horizontal_scalar(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x: usize,
    half: usize,
    select_max: bool,
) -> u8 {
    let last = width - 1;
    let mut selected = rank_filter_identity(select_max);
    let window_len = half.saturating_mul(2).saturating_add(1);
    for window_offset in 0..window_len {
        let source_x = x
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        let value = raw[row_start + source_x * channels + channel];
        selected = if select_max {
            selected.max(value)
        } else {
            selected.min(value)
        };
    }
    selected
}

#[inline]
fn rank_filter_vertical_scalar(
    horizontal: &[u8],
    row_stride: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x: usize,
    y: usize,
    half: usize,
    select_max: bool,
) -> u8 {
    let last = height - 1;
    let mut selected = rank_filter_identity(select_max);
    let window_len = half.saturating_mul(2).saturating_add(1);
    for window_offset in 0..window_len {
        let source_y = y
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        let value = horizontal[source_y * row_stride + x * channels + channel];
        selected = if select_max {
            selected.max(value)
        } else {
            selected.min(value)
        };
    }
    selected
}

/// Horizontal native-byte extrema pass. It intentionally keeps all source
/// samples in their Pillow layout and vectorizes comparisons across output
/// pixels, while clamped edge coordinates remain scalar control.
fn rank_filter_horizontal_vectorized(
    raw: &[u8],
    horizontal: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
) -> (u64, u64) {
    let row_stride = width * channels;
    let effective_half = half.min(width - 1);
    let window_len = effective_half
        .saturating_mul(2)
        .saturating_add(1);
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..height {
        let row_start = y * row_stride;
        for channel in 0..channels {
            let mut x = 0usize;
            while x < width {
                let active = rank_filter_vector_pixels(width - x);
                if active == 0 {
                    let tail = width - x;
                    for tail_x in x..width {
                        horizontal[row_start + tail_x * channels + channel] =
                            rank_filter_horizontal_scalar(
                                raw,
                                row_start,
                                width,
                                channels,
                                channel,
                                tail_x,
                                effective_half,
                                select_max,
                            );
                    }
                    scalar_tail = scalar_tail.saturating_add(tail as u64);
                    break;
                }
                let mut selected = u8x16::splat(rank_filter_identity(select_max));
                for window_offset in 0..window_len {
                    let values = rank_filter_horizontal_window_vector(
                        raw,
                        row_start,
                        width,
                        channels,
                        channel,
                        x,
                        active,
                        effective_half,
                        window_offset,
                        select_max,
                    );
                    selected = if select_max {
                        selected.max(values)
                    } else {
                        selected.min(values)
                    };
                }
                store_rank_filter_channel_vector(
                    horizontal,
                    row_start,
                    channels,
                    channel,
                    x,
                    active,
                    selected,
                );
                vector_blocks = vector_blocks.saturating_add(1);
                x += active;
            }
        }
    }
    (vector_blocks, scalar_tail)
}

/// Vertical native-byte extrema pass over the horizontal intermediate.
fn rank_filter_vertical_vectorized(
    horizontal: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
) -> (u64, u64) {
    let row_stride = width * channels;
    let effective_half = half.min(height - 1);
    let window_len = effective_half
        .saturating_mul(2)
        .saturating_add(1);
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..height {
        let row_start = y * row_stride;
        for channel in 0..channels {
            let mut x = 0usize;
            while x < width {
                let active = rank_filter_vector_pixels(width - x);
                if active == 0 {
                    let tail = width - x;
                    for tail_x in x..width {
                        output[row_start + tail_x * channels + channel] =
                            rank_filter_vertical_scalar(
                                horizontal,
                                row_stride,
                                height,
                                channels,
                                channel,
                                tail_x,
                                y,
                                effective_half,
                                select_max,
                            );
                    }
                    scalar_tail = scalar_tail.saturating_add(tail as u64);
                    break;
                }
                let mut selected = u8x16::splat(rank_filter_identity(select_max));
                for window_offset in 0..window_len {
                    let values = rank_filter_vertical_window_vector(
                        horizontal,
                        row_stride,
                        width,
                        height,
                        channels,
                        channel,
                        x,
                        active,
                        y,
                        effective_half,
                        window_offset,
                        select_max,
                    );
                    selected = if select_max {
                        selected.max(values)
                    } else {
                        selected.min(values)
                    };
                }
                store_rank_filter_channel_vector(
                    output,
                    row_start,
                    channels,
                    channel,
                    x,
                    active,
                    selected,
                );
                vector_blocks = vector_blocks.saturating_add(1);
                x += active;
            }
        }
    }
    (vector_blocks, scalar_tail)
}

/// Execute a native-byte MaxFilter or MinFilter.
///
/// The extrema are separable, so the scalar border/index work is performed in
/// two passes and every complete group of eight or sixteen output pixels uses
/// `u8x16` min/max instructions. This is a data-plane SIMD implementation,
/// not a call into the packed CPU rank-filter implementation.
fn simd_extreme_filter(
    img: &DynamicImage,
    size: u32,
    mode: Option<&str>,
    select_max: bool,
) -> Result<DynamicImage, PilError> {
    let channels = native_filter_byte_layout(img, mode)
        .ok_or_else(|| simd_unsupported(if select_max { "MaxFilter" } else { "MinFilter" }))?;
    if size == 0 || size % 2 == 0 || img.width() == 0 || img.height() == 0 {
        return Err(simd_unsupported(if select_max { "MaxFilter" } else { "MinFilter" }));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let expected_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rank-filter dimensions overflow".into()))?;
    let raw = img.as_bytes();
    if raw.len() != expected_len {
        return Err(PilError::InternalError(
            "SIMD rank-filter source buffer shape mismatch".into(),
        ));
    }
    let half = (size / 2) as usize;
    let mut horizontal = vec![0u8; expected_len];
    let mut output = vec![0u8; expected_len];
    let (horizontal_blocks, horizontal_tail) = rank_filter_horizontal_vectorized(
        raw,
        &mut horizontal,
        width,
        height,
        channels,
        half,
        select_max,
    );
    let (vertical_blocks, vertical_tail) = rank_filter_vertical_vectorized(
        &horizontal,
        &mut output,
        width,
        height,
        channels,
        half,
        select_max,
    );
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        horizontal_blocks.saturating_add(vertical_blocks),
    );
    crate::compute::record_pipeline_operation_scalar_tail(
        horizontal_tail.saturating_add(vertical_tail),
    );
    let result = crate::image_utils::raw_bytes_to_image(
        img.width(),
        img.height(),
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Native SIMD adapter for Pillow's maximum filter.
pub fn simd_max_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::MaxFilter { size } = op else {
        return Err(PilError::ValueError("expected MaxFilter op".into()));
    };
    if mode == Some("F") {
        return simd_float_order_statistic_filter(
            img,
            *size,
            size.saturating_mul(*size).saturating_sub(1),
            mode,
        );
    }
    simd_extreme_filter(img, *size, mode, true)
}

/// Native SIMD adapter for Pillow's minimum filter.
pub fn simd_min_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::MinFilter { size } = op else {
        return Err(PilError::ValueError("expected MinFilter op".into()));
    };
    if mode == Some("F") {
        return simd_float_order_statistic_filter(img, *size, 0, mode);
    }
    simd_extreme_filter(img, *size, mode, false)
}

const SIMD_ORDER_STATISTIC_MAX_AREA: usize = 225;
const SIMD_ORDER_STATISTIC_SORT_MAX_SIZE: u32 = 9;
const SIMD_ORDER_STATISTIC_HISTOGRAM_MAX_SIZE: u32 = 15;

/// Sort one order-statistic window independently in every vector lane.
///
/// Odd-even transposition is used here because it is a fixed compare/exchange
/// network: every comparison is a `u8x16` min/max, and no lane can branch on
/// the value it is sorting. It is intentionally limited to the smaller
/// windows; larger windows use the vector binary-select implementation below
/// rather than paying this quadratic setup.
#[inline]
fn sort_order_statistic_vectors(values: &mut [u8x16]) {
    for pass in 0..values.len() {
        let mut index = pass & 1;
        while index + 1 < values.len() {
            let left = values[index];
            let right = values[index + 1];
            values[index] = left.min(right);
            values[index + 1] = left.max(right);
            index += 2;
        }
    }
}

#[inline]
fn rank_filter_order_statistic_scalar(
    raw: &[u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x: usize,
    y: usize,
    half: usize,
    rank: usize,
) -> u8 {
    let size = half.saturating_mul(2).saturating_add(1);
    let area = size.saturating_mul(size);
    debug_assert!(area <= SIMD_ORDER_STATISTIC_MAX_AREA);
    let mut values = [0u8; SIMD_ORDER_STATISTIC_MAX_AREA];
    let mut index = 0usize;
    for row_offset in 0..size {
        let source_y = y
            .saturating_add(row_offset)
            .saturating_sub(half)
            .min(height - 1);
        for column_offset in 0..size {
            let source_x = x
                .saturating_add(column_offset)
                .saturating_sub(half)
                .min(width - 1);
            values[index] = raw[source_y * row_stride + source_x * channels + channel];
            index += 1;
        }
    }
    values[..area].sort_unstable();
    values[rank]
}

/// Gather one sample from each output pixel in a vector block. The source
/// coordinates are clamped by scalar control; the resulting order statistic
/// is selected by vector compare/exchange operations.
#[inline]
fn rank_filter_order_statistic_window_vector(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    half: usize,
    column_offset: usize,
) -> u8x16 {
    // The upper lanes are never stored for an incomplete block. Zero is a
    // valid padding value because the sorting network is lane-independent.
    let mut values = [0u8; SIMD_RANK_FILTER_LANES];
    let last = width - 1;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(last);
        *value = raw[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

/// Select one order statistic from a set of vector lanes without sorting it.
///
/// Each iteration counts, in parallel, how many window samples are less than
/// or equal to the current per-lane midpoint. Eight binary-search rounds are
/// sufficient for the byte domain. The contextual preflight caps this path at
/// a 15×15 window (225 samples), so the per-lane `u8x16` counters cannot
/// overflow. The gather remains scalar control; the comparisons, counts, and
/// narrowing decisions are vector data-plane operations.
#[inline]
fn select_order_statistic_vectors(values: &[u8x16], rank: u8) -> u8x16 {
    let mut lower = u8x16::splat(0);
    let mut upper = u8x16::splat(u8::MAX);
    let rank = u8x16::splat(rank);
    let one = u8x16::splat(1);

    for _ in 0..u8::BITS {
        let midpoint = lower + ((upper - lower) >> 1u32);
        let mut count = u8x16::splat(0);
        for &value in values {
            let at_or_below = value.simd_le(midpoint);
            count += at_or_below.select(one, u8x16::splat(0));
        }
        let at_or_above_rank = count.simd_gt(rank);
        upper = at_or_above_rank.select(midpoint, upper);
        lower = at_or_above_rank.select(lower, midpoint + one);
    }
    lower
}

/// Native SIMD MedianFilter/RankFilter for 8-bit interleaved layouts.
///
/// The window gather and border handling are scalar control. The actual
/// order-statistic work is lane-wise vector compare/exchange or vector binary
/// selection, and the output remains in the source's native L/LA/RGB/RGBA
/// byte layout.
fn simd_order_statistic_filter(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = native_filter_byte_layout(img, mode).ok_or_else(|| {
        simd_unsupported("MedianFilter/RankFilter")
    })?;
    let area = u64::from(size).saturating_mul(u64::from(size));
    if size == 0
        || size % 2 == 0
        || size > SIMD_ORDER_STATISTIC_HISTOGRAM_MAX_SIZE
        || area > SIMD_ORDER_STATISTIC_MAX_AREA as u64
        || u64::from(rank) >= area
        || img.width() < SIMD_RANK_FILTER_MIN_VECTOR_PIXELS as u32
        || img.height() == 0
    {
        return Err(simd_unsupported("MedianFilter/RankFilter"));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let row_stride = width
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD rank-filter row overflow".into()))?;
    let expected_len = row_stride
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("SIMD rank-filter dimensions overflow".into()))?;
    let raw = img.as_bytes();
    if raw.len() != expected_len {
        return Err(PilError::InternalError(
            "SIMD rank-filter source buffer shape mismatch".into(),
        ));
    }
    let half = (size / 2) as usize;
    let area = area as usize;
    let rank = rank as usize;
    let mut output = vec![0u8; expected_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for y in 0..height {
        let output_row = y * row_stride;
        for channel in 0..channels {
            let mut x = 0usize;
            while x < width {
                let active = rank_filter_vector_pixels(width - x);
                if active == 0 {
                    let tail = width - x;
                    for tail_x in x..width {
                        output[output_row + tail_x * channels + channel] =
                            rank_filter_order_statistic_scalar(
                                raw,
                                row_stride,
                                width,
                                height,
                                channels,
                                channel,
                                tail_x,
                                y,
                                half,
                                rank,
                            );
                    }
                    scalar_tail = scalar_tail.saturating_add(tail as u64);
                    break;
                }

                let mut values = [u8x16::splat(0); SIMD_ORDER_STATISTIC_MAX_AREA];
                let mut value_index = 0usize;
                for row_offset in 0..size as usize {
                    let source_y = y
                        .saturating_add(row_offset)
                        .saturating_sub(half)
                        .min(height - 1);
                    let source_row = source_y * row_stride;
                    for column_offset in 0..size as usize {
                        values[value_index] = rank_filter_order_statistic_window_vector(
                            raw,
                            source_row,
                            width,
                            channels,
                            channel,
                            x,
                            active,
                            half,
                            column_offset,
                        );
                        value_index += 1;
                    }
                }
                let selected = if size <= SIMD_ORDER_STATISTIC_SORT_MAX_SIZE {
                    sort_order_statistic_vectors(&mut values[..area]);
                    values[rank]
                } else {
                    select_order_statistic_vectors(&values[..area], rank as u8)
                };
                store_rank_filter_channel_vector(
                    &mut output,
                    output_row,
                    channels,
                    channel,
                    x,
                    active,
                    selected,
                );
                vector_blocks = vector_blocks.saturating_add(1);
                x += active;
            }
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        img.width(),
        img.height(),
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

const SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA: usize = 81;
const SIMD_FLOAT_ORDER_STATISTIC_LANES: usize = 8;

#[inline]
fn sort_float_order_statistic_vectors(values: &mut [f32x8]) {
    for pass in 0..values.len() {
        let mut index = pass & 1;
        while index + 1 < values.len() {
            let left = values[index];
            let right = values[index + 1];
            values[index] = left.min(right);
            values[index + 1] = left.max(right);
            index += 2;
        }
    }
}

#[inline]
fn rank_filter_float_scalar(
    raw: &[u8],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    half: usize,
    rank: usize,
) -> f32 {
    let size = half.saturating_mul(2).saturating_add(1);
    let area = size.saturating_mul(size);
    debug_assert!(area <= SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA);
    let mut values = [0.0f32; SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA];
    let mut index = 0usize;
    for row_offset in 0..size {
        let source_y = y
            .saturating_add(row_offset)
            .saturating_sub(half)
            .min(height - 1);
        for column_offset in 0..size {
            let source_x = x
                .saturating_add(column_offset)
                .saturating_sub(half)
                .min(width - 1);
            let base = (source_y * width + source_x) * 4;
            values[index] =
                f32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]]);
            index += 1;
        }
    }
    values[..area].sort_unstable_by(|left, right| {
        left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal)
    });
    values[rank]
}

#[inline]
fn rank_filter_float_window_vector(
    raw: &[u8],
    width: usize,
    height: usize,
    x_start: usize,
    active: usize,
    y: usize,
    half: usize,
    row_offset: usize,
    column_offset: usize,
) -> f32x8 {
    let mut values = [0.0f32; SIMD_FLOAT_ORDER_STATISTIC_LANES];
    let source_y = y
        .saturating_add(row_offset)
        .saturating_sub(half)
        .min(height - 1);
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(column_offset)
            .saturating_sub(half)
            .min(width - 1);
        let base = (source_y * width + source_x) * 4;
        *value =
            f32::from_le_bytes([raw[base], raw[base + 1], raw[base + 2], raw[base + 3]]);
    }
    f32x8::from(values)
}

#[inline]
fn store_rank_filter_float_vector(
    output: &mut [u8],
    width: usize,
    y: usize,
    x_start: usize,
    active: usize,
    values: f32x8,
) {
    let values = values.to_array();
    for (lane, value) in values.into_iter().enumerate().take(active) {
        let base = (y * width + x_start + lane) * 4;
        output[base..base + 4].copy_from_slice(&value.to_le_bytes());
    }
}

/// Native SIMD rank-family implementation for F-mode images.
///
/// F-mode pixels are one little-endian f32 in the four-byte native buffer.
/// Coordinates are gathered with scalar control, while extrema and the
/// compare/exchange sorting network operate independently in eight lanes.
/// The bounded size keeps the fixed network deterministic and matches the
/// exact scalar ordering used by Pillow for the supported public contract.
fn simd_float_order_statistic_filter(
    img: &DynamicImage,
    size: u32,
    rank: u32,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !native_float_rank_supported_for_image(img, mode, size, rank) {
        return Err(simd_unsupported("MedianFilter/RankFilter"));
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let size = size as usize;
    let half = size / 2;
    let area = size * size;
    let rank = rank as usize;
    let raw = img.as_bytes();
    let mut output = raw.to_vec();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    for y in 0..height {
        let mut x = 0usize;
        while x < width {
            let active = (width - x).min(SIMD_FLOAT_ORDER_STATISTIC_LANES);
            if active == 0 {
                output[(y * width + x) * 4..(y * width + x + 1) * 4]
                    .copy_from_slice(
                        &rank_filter_float_scalar(raw, width, height, x, y, half, rank)
                            .to_le_bytes(),
                    );
                scalar_tail = scalar_tail.saturating_add(1);
                x += 1;
                continue;
            }
            let mut values = [f32x8::splat(0.0); SIMD_FLOAT_ORDER_STATISTIC_MAX_AREA];
            let mut value_index = 0usize;
            for row_offset in 0..size {
                for column_offset in 0..size {
                    values[value_index] = rank_filter_float_window_vector(
                        raw,
                        width,
                        height,
                        x,
                        active,
                        y,
                        half,
                        row_offset,
                        column_offset,
                    );
                    value_index += 1;
                }
            }
            let selected = if rank == 0 {
                values[..area]
                    .iter()
                    .copied()
                    .fold(f32x8::splat(f32::INFINITY), |current, value| {
                        current.min(value)
                    })
            } else if rank + 1 == area {
                values[..area]
                    .iter()
                    .copied()
                    .fold(f32x8::splat(f32::NEG_INFINITY), |current, value| {
                        current.max(value)
                    })
            } else {
                sort_float_order_statistic_vectors(&mut values[..area]);
                values[rank]
            };
            store_rank_filter_float_vector(&mut output, width, y, x, active, selected);
            vector_blocks = vector_blocks.saturating_add(1);
            x += active;
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, 4)?;
    Ok(preserve_mode(img, result))
}

/// Native SIMD adapter for Pillow's median filter.
pub fn simd_median_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::MedianFilter { size } = op else {
        return Err(PilError::ValueError("expected MedianFilter op".into()));
    };
    if mode == Some("F") {
        return simd_float_order_statistic_filter(img, *size, size.saturating_mul(*size) / 2, mode);
    }
    simd_order_statistic_filter(img, *size, size.saturating_mul(*size) / 2, mode)
}

/// Native SIMD adapter for Pillow's rank filter.
pub fn simd_rank_filter(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::RankFilter { size, rank } = op else {
        return Err(PilError::ValueError("expected RankFilter op".into()));
    };
    if mode == Some("F") {
        return simd_float_order_statistic_filter(img, *size, *rank, mode);
    }
    simd_order_statistic_filter(img, *size, *rank, mode)
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

#[inline]
fn simd_blur_work(line_length: usize, radius: usize) -> (u64, u64) {
    let edge_a = (radius + 1).min(line_length);
    let edge_b = line_length.saturating_sub(radius + 1);
    let mut vector_blocks = 0u64;
    let scalar_tail = 0u64;
    let mut count_region = |start: usize, end: usize| {
        let length = end.saturating_sub(start);
        // `simd_blur_block` keeps the exact scalar recurrence but performs
        // the independent fixed-point pack for every group of up to eight
        // outputs in `u32x8`. Partial groups therefore still use the vector
        // arithmetic path; they are not scalar tails.
        vector_blocks = vector_blocks.saturating_add(length.div_ceil(8) as u64);
    };
    if edge_a <= edge_b {
        count_region(0, edge_a);
        count_region(edge_a, edge_b);
        count_region(edge_b, line_length);
    } else {
        count_region(0, edge_b);
        count_region(edge_b, edge_a);
        count_region(edge_a, line_length);
    }
    (vector_blocks, scalar_tail)
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
    let (vector_blocks_per_line, scalar_tail_per_line) = simd_blur_work(width, radius);
    let vector_blocks = vector_blocks_per_line.saturating_mul(height as u64);
    let scalar_tail = scalar_tail_per_line.saturating_mul(height as u64);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }

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
    simd_pil_box_blur_xy(img, radius, radius, passes, channels)
}

fn simd_pil_box_blur_xy(
    img: &DynamicImage,
    radius_x: f32,
    radius_y: f32,
    passes: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let dimensions = CheckedDims::new(img.width(), img.height(), channels as u8)?;
    if img.as_bytes().len() != dimensions.total_bytes()
        || !radius_x.is_finite()
        || !radius_y.is_finite()
        || radius_x < 0.0
        || radius_y < 0.0
        || (radius_x == 0.0 && radius_y == 0.0)
        || passes == 0
    {
        return Err(simd_unsupported("BoxBlur"));
    }
    crate::compute::record_pipeline_operation_path("vector");
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let blur_parameters = |radius: f32| {
        let integer_radius = radius as i32 as usize;
        let window_pixels = (2 * integer_radius + 1) as u32;
        let whole_weight = (SIMD_BOX_BLUR_SCALE as f32 / (radius * 2.0 + 1.0)) as u32;
        let fractional_weight =
            SIMD_BOX_BLUR_SCALE.wrapping_sub(window_pixels.wrapping_mul(whole_weight)) / 2;
        (integer_radius, whole_weight, fractional_weight)
    };
    let (horizontal_radius, horizontal_weight, horizontal_fractional_weight) =
        blur_parameters(radius_x);
    let (vertical_radius, vertical_weight, vertical_fractional_weight) =
        blur_parameters(radius_y);

    let mut work = img.as_bytes().to_vec();
    let mut scratch = dimensions.alloc_buffer();
    for _ in 0..passes {
        simd_blur_rows(
            &work,
            &mut scratch,
            width,
            height,
            channels,
            horizontal_radius,
            horizontal_weight,
            horizontal_fractional_weight,
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
            vertical_radius,
            vertical_weight,
            vertical_fractional_weight,
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

const SIMD_REDUCE_LANES: usize = 8;

/// Return the fixed-point block parameters used by Pillow's Reduce.c for one
/// output pixel. The address selection is scalar control work; the sums and
/// fixed-point average for a group of output pixels are vectorized below.
#[inline]
fn native_reduce_block_parameters(
    width: usize,
    height: usize,
    x_factor: usize,
    y_factor: usize,
    x: usize,
    y: usize,
) -> Option<(usize, usize, usize, usize, u32, u32)> {
    let main_width = width / x_factor;
    let main_height = height / y_factor;
    let right_width = width % x_factor;
    let bottom_height = height % y_factor;
    let (block_width, source_x) = if x < main_width {
        (x_factor, x.checked_mul(x_factor)?)
    } else {
        (right_width, main_width.checked_mul(x_factor)?)
    };
    let (block_height, source_y) = if y < main_height {
        (y_factor, y.checked_mul(y_factor)?)
    } else {
        (bottom_height, main_height.checked_mul(y_factor)?)
    };
    if block_width == 0 || block_height == 0 {
        return None;
    }
    let count = block_width.checked_mul(block_height)?;
    // The u32 vector arithmetic below is exact while sum+amend fits in u32.
    // Larger public factors remain valid CPU work but are explicitly outside
    // this SIMD contract rather than being truncated by a narrower lane.
    if count > (u32::MAX / 256) as usize {
        return None;
    }
    let multiplier = ((1u128 << 32) / (u128::from(count as u64) * 256)) as u32;
    Some((
        source_x,
        source_y,
        block_width,
        block_height,
        multiplier,
        (count / 2) as u32,
    ))
}

#[inline]
fn native_reduce_supported_buffer(
    img: &DynamicImage,
    channels: usize,
    mode: Option<&str>,
) -> bool {
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    native_reduce_layout(img, mode).is_some_and(|(layout, _)| layout == channels)
        && img.as_bytes().len() == expected_bytes
        && expected_bytes >= 16
}

fn native_reduce_supported_for_image(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    mode: Option<&str>,
) -> bool {
    if x_factor == 0 || y_factor == 0 {
        return false;
    }
    let Some((channels, _premultiplied_alpha)) = native_reduce_layout(img, mode) else {
        // A 1×1 reduction is an exact native copy even for indexed/raw-byte
        // logical modes. Any actual averaging needs a standard byte mode so
        // alpha and channel semantics remain explicit.
        return x_factor == 1
            && y_factor == 1
            && native_copy_layout(img, mode)
                .is_some_and(|channels| native_reduce_supported_buffer(img, channels, mode));
    };
    if !native_reduce_supported_buffer(img, channels, mode) {
        return false;
    }
    if x_factor == 1 && y_factor == 1 {
        return true;
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let output_width = width.div_ceil(x_factor as usize);
    let output_height = height.div_ceil(y_factor as usize);
    output_width
        .checked_mul(output_height)
        .is_some_and(|pixels| pixels != 0)
        && output_height != 0
        && native_reduce_block_parameters(
            width,
            height,
            x_factor as usize,
            y_factor as usize,
            0,
            0,
        )
        .is_some()
}

fn native_reduce_supported_for_shape(
    shape: SimdImageShape,
    x_factor: u32,
    y_factor: u32,
    mode: Option<&str>,
) -> bool {
    if x_factor == 0 || y_factor == 0 {
        return false;
    }
    let Some((channels, _premultiplied_alpha)) = shape_native_reduce_layout(shape, mode) else {
        return x_factor == 1
            && y_factor == 1
            && shape_native_copy_channels(shape, mode)
                .is_some_and(|channels| {
                    shape
                        .width
                        .checked_mul(shape.height)
                        .and_then(|pixels| pixels.checked_mul(channels as u32))
                        .is_some_and(|bytes| bytes >= 16)
                });
    };
    let Some(expected_bytes) = shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
    else {
        return false;
    };
    if expected_bytes < 16 {
        return false;
    }
    if x_factor == 1 && y_factor == 1 {
        return true;
    }
    let output_width = shape.width.div_ceil(x_factor);
    let output_height = shape.height.div_ceil(y_factor);
    output_width
        .checked_mul(output_height)
        .is_some_and(|pixels| pixels != 0)
        && output_height != 0
        && native_reduce_block_parameters(
            shape.width as usize,
            shape.height as usize,
            x_factor as usize,
            y_factor as usize,
            0,
            0,
        )
        .is_some()
}

#[inline]
fn native_reduce_pixel_sums(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    premultiplied_alpha: bool,
    x_factor: usize,
    y_factor: usize,
    x: usize,
    y: usize,
) -> Option<([u32; 4], u32, u32, u32)> {
    let (source_x, source_y, block_width, block_height, multiplier, amend) =
        native_reduce_block_parameters(
            width,
            height,
            x_factor,
            y_factor,
            x,
            y,
        )?;
    let mut sums = [0u64; 4];
    for dy in 0..block_height {
        for dx in 0..block_width {
            let source_index = ((source_y + dy) * width + source_x + dx) * channels;
            for channel in 0..channels {
                let mut sample = u32::from(source[source_index + channel]);
                if premultiplied_alpha && channel + 1 < channels {
                    let alpha = u32::from(source[source_index + channels - 1]);
                    sample = (sample * alpha + 127) / 255;
                }
                sums[channel] = sums[channel].checked_add(u64::from(sample))?;
            }
        }
    }
    Some((
        [
            u32::try_from(sums[0]).ok()?,
            u32::try_from(sums[1]).ok()?,
            u32::try_from(sums[2]).ok()?,
            u32::try_from(sums[3]).ok()?,
        ],
        (block_width * block_height) as u32,
        multiplier,
        amend,
    ))
}

#[inline]
fn native_reduce_average(sum: u32, multiplier: u32, amend: u32) -> u8 {
    (((u64::from(sum) + u64::from(amend)) * u64::from(multiplier)) >> 24)
        .min(u64::from(u8::MAX)) as u8
}

#[inline]
fn native_reduce_scalar_pixel(
    source: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    premultiplied_alpha: bool,
    x_factor: usize,
    y_factor: usize,
    x: usize,
    y: usize,
) -> Option<[u8; 4]> {
    let (sums, _count, multiplier, amend) =
        native_reduce_pixel_sums(
            source,
            width,
            height,
            channels,
            premultiplied_alpha,
            x_factor,
            y_factor,
            x,
            y,
        )?;
    let mut output = [0u8; 4];
    let alpha = if premultiplied_alpha {
        native_reduce_average(sums[channels - 1], multiplier, amend)
    } else {
        u8::MAX
    };
    for channel in 0..channels {
        let value = native_reduce_average(sums[channel], multiplier, amend);
        output[channel] = if premultiplied_alpha && channel + 1 < channels {
            if alpha == 0 {
                value
            } else {
                (u16::from(value) * 255 / u16::from(alpha)) as u8
            }
        } else {
            value
        };
    }
    Some(output)
}

#[inline]
fn native_reduce_vector_block(
    source: &[u8],
    width: usize,
    height: usize,
    output_width: usize,
    output_pixels: usize,
    channels: usize,
    premultiplied_alpha: bool,
    x_factor: usize,
    y_factor: usize,
    start_index: usize,
) -> Option<[u8; SIMD_REDUCE_LANES * 4]> {
    let mut sums = [[0u32; SIMD_REDUCE_LANES]; 4];
    let mut multipliers = [0u32; SIMD_REDUCE_LANES];
    let mut amends = [0u32; SIMD_REDUCE_LANES];
    for lane in 0..SIMD_REDUCE_LANES {
        if start_index + lane >= output_pixels {
            continue;
        }
        let (pixel_sums, _count, multiplier, amend) = native_reduce_pixel_sums(
            source,
            width,
            height,
            channels,
            premultiplied_alpha,
            x_factor,
            y_factor,
            (start_index + lane) % output_width,
            (start_index + lane) / output_width,
        )?;
        for channel in 0..channels {
            sums[channel][lane] = pixel_sums[channel];
        }
        multipliers[lane] = multiplier;
        amends[lane] = amend;
    }
    let multipliers = u32x8::new(multipliers);
    let amends = u32x8::new(amends);
    let mut averaged = [[0u8; SIMD_REDUCE_LANES]; 4];
    for channel in 0..channels {
        let values = (u32x8::new(sums[channel]) + amends) * multipliers >> 24u32;
        averaged[channel] = values.to_array().map(|value| value.min(255) as u8);
    }
    let mut output = [0u8; SIMD_REDUCE_LANES * 4];
    let has_alpha = premultiplied_alpha;
    for lane in 0..SIMD_REDUCE_LANES {
        let alpha = if has_alpha {
            averaged[channels - 1][lane]
        } else {
            u8::MAX
        };
        for channel in 0..channels {
            let value = averaged[channel][lane];
            output[lane * channels + channel] =
                if has_alpha && channel + 1 < channels && alpha != 0 {
                    (u16::from(value) * 255 / u16::from(alpha)) as u8
                } else {
                    value
                };
        }
    }
    Some(output)
}

fn native_reduce_bytes(
    img: &DynamicImage,
    channels: usize,
    premultiplied_alpha: bool,
    x_factor: u32,
    y_factor: u32,
) -> Option<(Vec<u8>, u32, u32, u64, u64)> {
    let width = img.width() as usize;
    let height = img.height() as usize;
    let x_factor = usize::try_from(x_factor).ok()?;
    let y_factor = usize::try_from(y_factor).ok()?;
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let output_len = output_width.checked_mul(output_height)?.checked_mul(channels)?;
    let source_len = width.checked_mul(height)?.checked_mul(channels)?;
    let source = img.as_bytes();
    if source.len() != source_len || output_height == 0 {
        return None;
    }
    let mut output = Vec::with_capacity(output_len);
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let output_pixels = output_width.checked_mul(output_height)?;
    let vector_pixels = output_pixels / SIMD_REDUCE_LANES * SIMD_REDUCE_LANES;
    let vector_limit = if vector_pixels == 0 {
        output_pixels
    } else {
        vector_pixels
    };
    for start_index in (0..vector_limit).step_by(SIMD_REDUCE_LANES) {
        let block = native_reduce_vector_block(
            source,
            width,
            height,
            output_width,
            output_pixels,
            channels,
            premultiplied_alpha,
            x_factor,
            y_factor,
            start_index,
        )?;
        let valid_pixels = (output_pixels - start_index).min(SIMD_REDUCE_LANES);
        output.extend_from_slice(&block[..valid_pixels * channels]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    for index in vector_limit..output_pixels {
        let pixel = native_reduce_scalar_pixel(
            source,
            width,
            height,
            channels,
            premultiplied_alpha,
            x_factor,
            y_factor,
            index % output_width,
            index / output_width,
        )?;
        output.extend_from_slice(&pixel[..channels]);
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((
        output,
        output_width as u32,
        output_height as u32,
        vector_blocks,
        scalar_tail,
    ))
}

/// Reduce the scalar samples used by `Image.thumbnail` without interpreting
/// their four-byte storage as RGBA. The source gathers and block geometry are
/// scalar control work; each eight-lane block performs the average in the
/// scalar sample domain and writes only its valid prefix.
fn simd_thumbnail_reduce_f(
    img: &DynamicImage,
    factor_x: u32,
    factor_y: u32,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_x = usize::try_from(factor_x).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_y = usize::try_from(factor_y).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.as_bytes().len() != source_len {
        return Err(simd_unsupported("Thumbnail"));
    }
    let output_width = source_width.div_ceil(factor_x.max(1));
    let output_height = source_height.div_ceil(factor_y.max(1));
    let output_pixels = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source: Vec<f32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let mut output = Vec::with_capacity(output_pixels.checked_mul(4).ok_or_else(|| {
        simd_unsupported("Thumbnail")
    })?);
    let mut vector_blocks = 0u64;
    for start in (0..output_pixels).step_by(SIMD_RESIZE_LANES) {
        let count = (output_pixels - start).min(SIMD_RESIZE_LANES);
        let mut sums = [0.0f32; SIMD_RESIZE_LANES];
        let mut counts = [1.0f32; SIMD_RESIZE_LANES];
        for lane in 0..count {
            let index = start + lane;
            let output_y = index / output_width;
            let output_x = index % output_width;
            let source_x = output_x * factor_x;
            let source_y = output_y * factor_y;
            let block_width = factor_x.min(source_width - source_x);
            let block_height = factor_y.min(source_height - source_y);
            counts[lane] = (block_width * block_height) as f32;
            for dy in 0..block_height {
                for dx in 0..block_width {
                    sums[lane] += source[(source_y + dy) * source_width + source_x + dx];
                }
            }
        }
        let values = (f32x8::new(sums) / f32x8::new(counts)).to_array();
        for value in values.into_iter().take(count) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok((result, vector_blocks, 0))
}

fn simd_thumbnail_reduce_i(
    img: &DynamicImage,
    factor_x: u32,
    factor_y: u32,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_x = usize::try_from(factor_x).map_err(|_| simd_unsupported("Thumbnail"))?;
    let factor_y = usize::try_from(factor_y).map_err(|_| simd_unsupported("Thumbnail"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.as_bytes().len() != source_len {
        return Err(simd_unsupported("Thumbnail"));
    }
    let output_width = source_width.div_ceil(factor_x.max(1));
    let output_height = source_height.div_ceil(factor_y.max(1));
    let output_pixels = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let source: Vec<i32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let mut output = Vec::with_capacity(output_pixels.checked_mul(4).ok_or_else(|| {
        simd_unsupported("Thumbnail")
    })?);
    let mut vector_blocks = 0u64;
    for start in (0..output_pixels).step_by(SIMD_RESIZE_LANES) {
        let count = (output_pixels - start).min(SIMD_RESIZE_LANES);
        let mut sums = [0i64; SIMD_RESIZE_LANES];
        let mut counts = [1.0f64; SIMD_RESIZE_LANES];
        for lane in 0..count {
            let index = start + lane;
            let output_y = index / output_width;
            let output_x = index % output_width;
            let source_x = output_x * factor_x;
            let source_y = output_y * factor_y;
            let block_width = factor_x.min(source_width - source_x);
            let block_height = factor_y.min(source_height - source_y);
            counts[lane] = (block_width * block_height) as f64;
            for dy in 0..block_height {
                for dx in 0..block_width {
                    sums[lane] += i64::from(
                        source[(source_y + dy) * source_width + source_x + dx],
                    );
                }
            }
        }
        let averages = (f64x8::new(sums.map(|sum| sum as f64)) / f64x8::new(counts)).to_array();
        let rounded = averages.map(|value| round_up(value) as i32);
        let values = i32x8::new(rounded).to_array();
        for value in values.into_iter().take(count) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok((result, vector_blocks, 0))
}

const SIMD_RESIZE_LANES: usize = 8;
const SIMD_RESIZE_NEAREST_BYTES: usize = 16;

fn resize_native_channels_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    native_resize_byte_layout_for_image(img, mode)
        .map(|(channels, _)| channels)
}

/// Return the byte-domain resize layout and alpha contract.
///
/// The concrete raster variant is the storage contract; the logical Pillow
/// mode decides whether the final byte is alpha or an ordinary sample.  This
/// deliberately excludes `I` and `F`: their four bytes form one scalar sample
/// and must go through the typed kernels below rather than byte convolution.
fn native_resize_byte_layout_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("I" | "F")) {
        return None;
    }
    let channels = native_copy_layout(img, mode)?;
    let premultiplied_alpha = match channels {
        2 => matches!(mode, None | Some("LA")),
        4 => matches!(mode, None | Some("RGBA")),
        _ => false,
    };
    Some((channels, premultiplied_alpha))
}

fn native_resize_byte_layout_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("I" | "F")) {
        return None;
    }
    let channels = shape_native_copy_channels(shape, mode)?;
    let premultiplied_alpha = match channels {
        2 => matches!(mode, None | Some("LA")),
        4 => matches!(mode, None | Some("RGBA")),
        _ => false,
    };
    Some((channels, premultiplied_alpha))
}

fn resize_nearest_indices(source_size: u32, output_size: u32) -> Option<Vec<usize>> {
    if source_size == 0 || output_size == 0 {
        return None;
    }
    let scale = f64::from(source_size) / f64::from(output_size);
    let mut position = scale * 0.5;
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    for _ in 0..output_size {
        let index = position as u32;
        indices.push(if index >= source_size {
            usize::try_from(source_size - 1).ok()?
        } else {
            usize::try_from(index).ok()?
        });
        position += scale;
    }
    Some(indices)
}

fn resize_nearest_vectorizable(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> bool {
    if channels == 0
        || !(1..=4).contains(&channels)
        || source_width == 0
        || source_height == 0
        || output_width == 0
        || output_height == 0
    {
        return false;
    }
    // Nearest-neighbour coordinates are inherently a scalar gather on
    // architectures without a portable byte-gather instruction. That does
    // not make the operation a CPU fallback: the gathered samples are packed
    // into native vector blocks and the output data plane is written by the
    // SIMD kernel. Admission therefore depends on valid non-empty dimensions,
    // not on the source span of a particular gather block.
    resize_nearest_indices(source_width, output_width).is_some()
        && resize_nearest_indices(source_height, output_height).is_some()
}

/// Round a positive Pillow dimension with Python's ties-to-even rule.
///
/// `ImageOps.pad` performs this calculation in its scalar control plane
/// before the resize and placement kernels run. Keeping the rule here avoids
/// importing a CPU pixel implementation while retaining the public
/// dimension contract.
fn native_pad_round_dimension(value: f64) -> Option<u32> {
    if !value.is_finite() || value < 0.0 {
        return None;
    }
    let floor = value.floor();
    let fraction = value - floor;
    let rounded = if fraction < 0.5 {
        floor
    } else if fraction > 0.5 || (floor as u64) % 2 == 1 {
        floor + 1.0
    } else {
        floor
    };
    if rounded > f64::from(u32::MAX) {
        None
    } else {
        Some(rounded as u32)
    }
}

fn native_pad_contained_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    let source_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(target_width) / f64::from(target_height);
    if (source_ratio - target_ratio).abs() < 1e-10 {
        Some((target_width, target_height))
    } else if source_ratio > target_ratio {
        Some((
            target_width,
            native_pad_round_dimension(
                f64::from(source_height) / f64::from(source_width)
                    * f64::from(target_width),
            )?,
        ))
    } else {
        Some((
            native_pad_round_dimension(
                f64::from(source_width) / f64::from(source_height)
                    * f64::from(target_height),
            )?,
            target_height,
        ))
    }
}

/// Compute the aspect-preserving dimensions used by `ImageOps.cover`.
///
/// This is only the scalar control plane. The resulting dimensions are fed to
/// the same native-layout resize kernels used by `Resize` and `Pad`; the SIMD
/// adapter never delegates the pixel work to the CPU implementation.
fn native_cover_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    let source_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(target_width) / f64::from(target_height);
    if (source_ratio - target_ratio).abs() < 1e-10 {
        Some((target_width, target_height))
    } else if source_ratio < target_ratio {
        Some((
            target_width,
            native_pad_round_dimension(
                f64::from(source_height) / f64::from(source_width)
                    * f64::from(target_width),
            )?,
        ))
    } else {
        Some((
            native_pad_round_dimension(
                f64::from(source_width) / f64::from(source_height)
                    * f64::from(target_height),
            )?,
            target_height,
        ))
    }
}

/// Compute the source box used by `ImageOps.fit` without touching pixels.
///
/// Pillow keeps this calculation in the ImageOps layer and passes the four
/// coordinates to its boxed resampler. Keeping the same scalar control plane
/// here lets the SIMD data plane consume the exact crop, including fractional
/// bleed and centering values.
fn native_fit_box(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
    bleed: f64,
    centering: (f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    if source_height == 0
        || target_width == 0
        || target_height == 0
        || !bleed.is_finite()
        || !centering.0.is_finite()
        || !centering.1.is_finite()
    {
        return None;
    }
    let source_width = f64::from(source_width);
    let source_height = f64::from(source_height);
    let bleed_width = bleed * source_width;
    let bleed_height = bleed * source_height;
    let live_width = (source_width - 2.0 * bleed_width).max(1.0);
    let live_height = (source_height - 2.0 * bleed_height).max(1.0);
    let live_ratio = live_width / live_height;
    let target_ratio = f64::from(target_width) / f64::from(target_height);
    let (crop_width, crop_height) = if (live_ratio - target_ratio).abs() < 1e-10 {
        (live_width, live_height)
    } else if live_ratio >= target_ratio {
        (target_ratio * live_height, live_height)
    } else {
        (live_width, live_width / target_ratio)
    };
    let crop_left = bleed_width + (live_width - crop_width) * centering.0;
    let crop_top = bleed_height + (live_height - crop_height) * centering.1;
    Some((
        crop_left,
        crop_top,
        crop_left + crop_width,
        crop_top + crop_height,
    ))
}

/// Return the raw byte layout and alpha contract accepted by the boxed SIMD
/// resampler. P/PA are included as raw indexed samples: P forces nearest
/// sampling in Pillow, while PA filters its index and alpha bytes without RGBA
/// premultiplication.
fn native_fit_layout_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => {
            Some((1, false))
        }
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, true)),
        DynamicImage::ImageLumaA8(_) if mode == Some("PA") => Some((2, false)),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some((3, false)),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "RGBX")) => {
            Some((4, true))
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, Some("RGBa" | "CMYK")) =>
        {
            Some((4, false))
        }
        _ => None,
    }
}

fn native_fit_layout_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => {
            Some((1, false))
        }
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, true)),
        SimdLayout::LumaA8 if mode == Some("PA") => Some((2, false)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some((3, false)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "RGBX")) => {
            Some((4, true))
        }
        SimdLayout::Rgba8 if matches!(mode, Some("RGBa" | "CMYK")) => Some((4, false)),
        _ => None,
    }
}

fn native_fit_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

fn native_fit_float_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    bleed: f64,
    centering: (f64, f64),
    mode: Option<&str>,
) -> bool {
    if mode != Some("F") || !matches!(img, DynamicImage::ImageRgba8(_)) {
        return false;
    }
    let output_width = target_width.max(1);
    let output_height = target_height.max(1);
    if native_fit_box(
        img.width(),
        img.height(),
        output_width,
        output_height,
        bleed,
        centering,
    )
    .is_none()
    {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
}

fn native_fit_float_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    bleed: f64,
    centering: (f64, f64),
    mode: Option<&str>,
) -> bool {
    mode == Some("F")
        && shape.layout == SimdLayout::Rgba8
        && native_fit_box(
            shape.width,
            shape.height,
            target_width.max(1),
            target_height.max(1),
            bleed,
            centering,
        )
        .is_some()
}

fn native_fit_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    mode: Option<&str>,
) -> bool {
    if native_fit_float_supported_for_image(
        img,
        target_width,
        target_height,
        bleed,
        centering,
        mode,
    ) {
        return true;
    }
    let Some((channels, _premultiplied_alpha)) = native_fit_layout_for_image(img, mode) else {
        return false;
    };
    let output_width = target_width.max(1);
    let output_height = target_height.max(1);
    if native_fit_box(
        img.width(),
        img.height(),
        output_width,
        output_height,
        bleed,
        centering,
    )
    .is_none()
    {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    if img.width() == 0 {
        // Pillow's boxed resampler returns an all-zero image for a valid
        // zero-width L source. There is no source sample to resize, but the
        // output fill/store is still a real native byte data plane.
        return img.as_bytes().len() == expected_bytes;
    }
    img.as_bytes().len() == expected_bytes
        && native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            output_width,
            output_height,
            native_fit_filter(mode, filter),
            channels,
        )
}

fn native_fit_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    mode: Option<&str>,
) -> bool {
    if native_fit_float_supported_for_shape(
        shape,
        target_width,
        target_height,
        bleed,
        centering,
        mode,
    ) {
        return true;
    }
    let Some((channels, _premultiplied_alpha)) = native_fit_layout_for_shape(shape, mode) else {
        return false;
    };
    let output_width = target_width.max(1);
    let output_height = target_height.max(1);
    native_fit_box(
        shape.width,
        shape.height,
        output_width,
        output_height,
        bleed,
        centering,
    )
    .is_some()
        && (shape.width == 0
            || native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                output_width,
                output_height,
                native_fit_filter(mode, filter),
                channels,
            ))
}

/// Compute the aspect-preserving dimensions used by `Image.thumbnail`.
///
/// `Image.thumbnail` clamps each requested bound to the current image before
/// queuing the operation. The remaining calculation is Pillow's
/// `round_aspect` rule: choose the floor or ceiling that minimizes the aspect
/// ratio error, preferring the floor on a tie. This is scalar control-plane
/// work; the returned dimensions are consumed by the native nearest resize
/// kernel below.
fn native_thumbnail_dimensions(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    let bound_width = target_width.min(source_width);
    let bound_height = target_height.min(source_height);
    if bound_width == 0 || bound_height == 0 {
        return None;
    }
    let aspect = f64::from(source_width) / f64::from(source_height);
    let (width, height) = if f64::from(bound_width) / f64::from(bound_height) >= aspect {
        let adjusted = native_thumbnail_round_aspect(
            f64::from(bound_height) * aspect,
            |candidate| (aspect - candidate / f64::from(bound_height)).abs(),
        )?;
        (adjusted, bound_height)
    } else {
        let adjusted = native_thumbnail_round_aspect(
            f64::from(bound_width) / aspect,
            |candidate| {
                if candidate == 0.0 {
                    0.0
                } else {
                    (aspect - f64::from(bound_width) / candidate).abs()
                }
            },
        )?;
        (bound_width, adjusted)
    };
    Some((width.min(source_width).max(1), height.min(source_height).max(1)))
}

fn native_thumbnail_round_aspect(
    number: f64,
    key: impl Fn(f64) -> f64,
) -> Option<u32> {
    if !number.is_finite() || number < 0.0 {
        return None;
    }
    let floor = number.trunc();
    if number == floor {
        return (floor <= f64::from(u32::MAX)).then_some(floor as u32);
    }
    let ceil = floor + 1.0;
    let best = if key(floor) <= key(ceil) { floor } else { ceil };
    (best >= 0.0 && best <= f64::from(u32::MAX)).then_some(best as u32)
}

#[inline]
fn native_thumbnail_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if matches!(mode, Some("1" | "P")) {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

#[inline]
fn native_thumbnail_has_alpha(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) && !matches!(mode, Some("F" | "I" | "CMYK"))
}

#[inline]
fn native_thumbnail_reduction_factors(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    has_alpha: bool,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || output_width == 0 || output_height == 0 {
        return None;
    }
    if matches!(filter, ResampleFilter::Nearest) || has_alpha {
        return Some((1, 1));
    }
    let factor_x = ((f64::from(source_width) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y = ((f64::from(source_height) / f64::from(output_height) / 2.0) as u32).max(1);
    Some((factor_x, factor_y))
}

fn native_thumbnail_layout_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("F" | "I")) {
        return matches!(img, DynamicImage::ImageRgba8(_)).then_some((4, true));
    }
    native_resize_byte_layout_for_image(img, mode).map(|(channels, _)| (channels, false))
}

fn native_thumbnail_layout_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("F" | "I")) {
        return (shape.layout == SimdLayout::Rgba8).then_some((4, true));
    }
    native_resize_byte_layout_for_shape(shape, mode).map(|(channels, _)| (channels, false))
}

fn native_thumbnail_typed_reduce_supported_for_image(
    img: &DynamicImage,
    factor_x: u32,
    factor_y: u32,
) -> bool {
    if !matches!(img, DynamicImage::ImageRgba8(_)) || img.width() == 0 || img.height() == 0 {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return false;
    };
    let output_pixels = (img.width() / factor_x.max(1)).saturating_add(u32::from(
        img.width() % factor_x.max(1) != 0,
    )) * (img.height() / factor_y.max(1)).saturating_add(u32::from(
        img.height() % factor_y.max(1) != 0,
    ));
    img.as_bytes().len() == expected_bytes && output_pixels != 0
}

fn native_thumbnail_typed_reduce_supported_for_shape(
    shape: SimdImageShape,
    factor_x: u32,
    factor_y: u32,
) -> bool {
    if shape.layout != SimdLayout::Rgba8 || shape.width == 0 || shape.height == 0 {
        return false;
    }
    let output_width = shape.width.div_ceil(factor_x.max(1));
    let output_height = shape.height.div_ceil(factor_y.max(1));
    output_width != 0 && output_height != 0
}

fn native_thumbnail_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some((channels, typed_scalar)) = native_thumbnail_layout_for_image(img, mode) else {
        return false;
    };
    let Some((output_width, output_height)) =
        native_thumbnail_dimensions(img.width(), img.height(), target_width, target_height)
    else {
        return false;
    };
    let filter = native_thumbnail_filter(mode, filter);
    let Some((factor_x, factor_y)) = native_thumbnail_reduction_factors(
        img.width(),
        img.height(),
        output_width,
        output_height,
        filter,
        native_thumbnail_has_alpha(img, mode),
    ) else {
        return false;
    };
    if factor_x != 1 || factor_y != 1 {
        let reduction_supported = if typed_scalar {
            native_thumbnail_typed_reduce_supported_for_image(img, factor_x, factor_y)
        } else {
            native_reduce_supported_for_image(img, factor_x, factor_y, mode)
        };
        if !reduction_supported {
            return false;
        }
    }
    channels != 0 && native_resize_supported_for_image(img, output_width, output_height, filter, mode)
}

fn native_thumbnail_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some((channels, typed_scalar)) = native_thumbnail_layout_for_shape(shape, mode) else {
        return false;
    };
    let Some((output_width, output_height)) =
        native_thumbnail_dimensions(shape.width, shape.height, target_width, target_height)
    else {
        return false;
    };
    let filter = native_thumbnail_filter(mode, filter);
    let Some((factor_x, factor_y)) = native_thumbnail_reduction_factors(
        shape.width,
        shape.height,
        output_width,
        output_height,
        filter,
        matches!(shape.layout, SimdLayout::LumaA8 | SimdLayout::Rgba8)
            && !matches!(mode, Some("F" | "I" | "CMYK")),
    ) else {
        return false;
    };
    if factor_x != 1 || factor_y != 1 {
        let reduction_supported = if typed_scalar {
            native_thumbnail_typed_reduce_supported_for_shape(shape, factor_x, factor_y)
        } else {
            native_reduce_supported_for_shape(shape, factor_x, factor_y, mode)
        };
        if !reduction_supported {
            return false;
        }
    }
    channels != 0 && native_resize_supported_for_shape(shape, output_width, output_height, filter, mode)
}

fn native_pad_offsets(
    contained_width: u32,
    contained_height: u32,
    target_width: u32,
    target_height: u32,
    centering: (f64, f64),
) -> Option<(usize, usize)> {
    if !centering.0.is_finite() || !centering.1.is_finite() {
        return None;
    }
    let width_gap = target_width.checked_sub(contained_width)?;
    let height_gap = target_height.checked_sub(contained_height)?;
    if width_gap != 0 {
        Some((
            usize::try_from(native_pad_round_dimension(
                f64::from(width_gap) * centering.0.clamp(0.0, 1.0),
            )?)
            .ok()?,
            0,
        ))
    } else {
        Some((
            0,
            usize::try_from(native_pad_round_dimension(
                f64::from(height_gap) * centering.1.clamp(0.0, 1.0),
            )?)
            .ok()?,
        ))
    }
}

/// Pillow keeps `P` and `PA` in their native indexed sample layouts during
/// `ImageOps.pad`; `F` and `I` keep one scalar sample in four raw bytes, and
/// HSV/CMYK use their ordinary packed byte layouts.  The adapter must admit
/// those logical modes without pretending that the stored bytes are RGBA.
fn native_pad_channels_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_)
            if matches!(mode, None | Some("1" | "L" | "P")) =>
        {
            Some(1)
        }
        DynamicImage::ImageLumaA8(_)
            if matches!(mode, None | Some("LA" | "PA")) =>
        {
            Some(2)
        }
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "CMYK" | "I" | "F")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_pad_channels_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        SimdLayout::Rgba8
            if matches!(mode, None | Some("RGBA" | "CMYK" | "I" | "F")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

/// Pillow's indexed `P` pad path uses nearest-neighbour sampling regardless
/// of the requested filter. `PA` is different: its index and alpha bytes are
/// filtered independently, so it retains the requested kernel.
fn native_pad_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

fn native_pad_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    if img.as_bytes().len() != expected_bytes {
        return false;
    }
    let filter = native_pad_filter(mode, filter);
    // A zero-width/zero-height source still has a valid Pillow pad contract:
    // the output is a filled canvas. There is no aspect-ratio geometry to
    // compute, so admission is based on the vector fill data plane directly.
    if img.width() == 0 || img.height() == 0 {
        return target_width != 0
            && target_height != 0
            && native_resize_supported_for_dimensions(
                img.width(),
                img.height(),
                target_width,
                target_height,
                filter,
                channels,
            );
    }
    let Some((contained_width, contained_height)) = native_pad_contained_dimensions(
        img.width(),
        img.height(),
        target_width,
        target_height,
    ) else {
        return false;
    };
    if native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        (0.5, 0.5),
    )
    .is_none()
    {
        return false;
    }
    (img.width(), img.height()) == (contained_width, contained_height)
        || native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            contained_width,
            contained_height,
            filter,
            channels,
        )
}

fn native_pad_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    let filter = native_pad_filter(mode, filter);
    if shape.width == 0 || shape.height == 0 {
        return target_width != 0
            && target_height != 0
            && native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                target_width,
                target_height,
                filter,
                channels,
            );
    }
    let Some((contained_width, contained_height)) = native_pad_contained_dimensions(
        shape.width,
        shape.height,
        target_width,
        target_height,
    ) else {
        return false;
    };
    native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        (0.5, 0.5),
    )
    .is_some()
        && ((shape.width, shape.height) == (contained_width, contained_height)
            || native_resize_supported_for_dimensions(
                shape.width,
                shape.height,
                contained_width,
                contained_height,
                filter,
                channels,
            ))
}

fn native_contain_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    let Some((output_width, output_height)) = native_pad_contained_dimensions(
        img.width(),
        img.height(),
        target_width,
        target_height,
    ) else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
        && native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            output_width,
            output_height,
            filter,
            channels,
        )
}

fn native_contain_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    let Some((output_width, output_height)) = native_pad_contained_dimensions(
        shape.width,
        shape.height,
        target_width,
        target_height,
    ) else {
        return false;
    };
    native_resize_supported_for_dimensions(
        shape.width,
        shape.height,
        output_width,
        output_height,
        filter,
        channels,
    )
}

fn native_cover_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    let Some((output_width, output_height)) = native_cover_dimensions(
        img.width(),
        img.height(),
        target_width,
        target_height,
    ) else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
        && native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            output_width,
            output_height,
            filter,
            channels,
        )
}

fn native_cover_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    let Some((output_width, output_height)) = native_cover_dimensions(
        shape.width,
        shape.height,
        target_width,
        target_height,
    ) else {
        return false;
    };
    native_resize_supported_for_dimensions(
        shape.width,
        shape.height,
        output_width,
        output_height,
        filter,
        channels,
    )
}

fn native_affine_nearest_transform_channels(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    // Affine nearest sampling copies complete native samples.  Indexed and
    // color-space modes are therefore safe here even though they are not
    // admitted to arithmetic kernels: P copies indices, CMYK copies K as the
    // fourth sample, and I/F copy their four raw little-endian bytes.
    native_copy_layout(img, mode)
}

fn native_affine_nearest_transform_channels_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    shape_native_copy_channels(shape, mode)
}

fn native_affine_bilinear_layout(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, Option<usize>)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, None)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, Some(1))),
        DynamicImage::ImageRgb8(_)
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some((3, None))
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, Some(3))),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, None)),
        _ => None,
    }
}

fn shape_native_affine_bilinear_layout(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, Option<usize>)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, None)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, Some(1))),
        SimdLayout::Rgb8
            if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) =>
        {
            Some((3, None))
        }
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, Some(3))),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, None)),
        _ => None,
    }
}

fn native_affine_nearest_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let channels = match filter {
        ResampleFilter::Nearest => native_affine_nearest_transform_channels(img, mode),
        ResampleFilter::Bilinear => {
            native_affine_bilinear_layout(img, mode).map(|(channels, _)| channels)
        }
        _ => None,
    };
    let Some(channels) = channels else {
        return false;
    };
    if !matches!(method, TransformMethod::Affine)
        || width == 0
        || height == 0
        || data.len() != 6
        || data.iter().any(|value| !value.is_finite())
    {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
}

fn native_affine_nearest_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let channels = match filter {
        ResampleFilter::Nearest => native_affine_nearest_transform_channels_for_shape(shape, mode),
        ResampleFilter::Bilinear => {
            shape_native_affine_bilinear_layout(shape, mode).map(|(channels, _)| channels)
        }
        _ => None,
    };
    channels.is_some()
        && matches!(method, TransformMethod::Affine)
        && width != 0
        && height != 0
        && data.len() == 6
        && data.iter().all(|value| value.is_finite())
}

fn native_affine_luma16_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    mode: Option<&str>,
) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(method, TransformMethod::Affine)
        && width != 0
        && height != 0
        && img.width() != 0
        && img.height() != 0
        && data.len() == 6
        && data.iter().all(|value| value.is_finite())
}

fn native_affine_luma16_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    mode: Option<&str>,
) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(method, TransformMethod::Affine)
        && width != 0
        && height != 0
        && shape.width != 0
        && shape.height != 0
        && data.len() == 6
        && data.iter().all(|value| value.is_finite())
}

fn native_projective_nearest_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(method, TransformMethod::Perspective | TransformMethod::Quad)
        && matches!(filter, ResampleFilter::Nearest | ResampleFilter::Bilinear)
        && width != 0
        && height != 0
        && data.len() == 8
        && native_affine_nearest_transform_channels(img, mode).is_some_and(|channels| {
            (img.width() as usize)
                .checked_mul(img.height() as usize)
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some_and(|bytes| img.as_bytes().len() == bytes)
        })
}

fn native_projective_nearest_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(method, TransformMethod::Perspective | TransformMethod::Quad)
        && matches!(filter, ResampleFilter::Nearest | ResampleFilter::Bilinear)
        && width != 0
        && height != 0
        && data.len() == 8
        && native_affine_nearest_transform_channels_for_shape(shape, mode).is_some()
}

fn native_mesh_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(filter, ResampleFilter::Nearest)
        && width != 0
        && height != 0
        && img.width() != 0
        && img.height() != 0
        && !data.is_empty()
        && data.len() % 12 == 0
        && data.iter().all(|value| value.is_finite())
        && native_affine_nearest_transform_channels(img, mode).is_some_and(|channels| {
            (img.width() as usize)
                .checked_mul(img.height() as usize)
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some_and(|bytes| img.as_bytes().len() == bytes)
        })
}

fn native_mesh_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(filter, ResampleFilter::Nearest)
        && width != 0
        && height != 0
        && shape.width != 0
        && shape.height != 0
        && !data.is_empty()
        && data.len() % 12 == 0
        && data.iter().all(|value| value.is_finite())
        && native_affine_nearest_transform_channels_for_shape(shape, mode).is_some()
}

fn native_pad_bytes(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return Ok(None);
    };
    let filter = native_pad_filter(mode, filter);
    if !native_pad_supported_for_image(img, target_width, target_height, filter, mode) {
        return Ok(None);
    }

    // `ImageOps.pad` has no contained image when a source dimension is zero;
    // Pillow returns the requested filled canvas. Fill it as a native byte
    // plane so this remains a real SIMD data path for P/PA/F/I and color-space
    // modes as well as ordinary byte images.
    if img.width() == 0 || img.height() == 0 {
        let target_width_usize = usize::try_from(target_width)
            .map_err(|_| simd_unsupported("Pad"))?;
        let target_height_usize = usize::try_from(target_height)
            .map_err(|_| simd_unsupported("Pad"))?;
        let target_stride = target_width_usize
            .checked_mul(channels)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let output_len = target_stride
            .checked_mul(target_height_usize)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let default_fill = if channels == 2 || channels == 4 {
            (0, 0, 0, 0)
        } else {
            (0, 0, 0, u8::MAX)
        };
        let fill = color.unwrap_or(default_fill);
        let mut output = vec![0u8; output_len];
        let mut vector_blocks = 0u64;
        let mut scalar_tail = 0u64;
        for row in output.chunks_exact_mut(target_stride) {
            let (blocks, tail) = native_fill_row(row, fill, channels)
                .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        let result = crate::image_utils::raw_bytes_to_image(
            target_width,
            target_height,
            output,
            channels,
        )?;
        return Ok(Some((
            preserve_mode(img, result),
            vector_blocks,
            scalar_tail,
        )));
    }
    let Some((contained_width, contained_height)) = native_pad_contained_dimensions(
        img.width(),
        img.height(),
        target_width,
        target_height,
    ) else {
        return Ok(None);
    };
    let Some((offset_x, offset_y)) = native_pad_offsets(
        contained_width,
        contained_height,
        target_width,
        target_height,
        centering,
    ) else {
        return Ok(None);
    };
    // If the contain step leaves the source dimensions unchanged, Pillow
    // copies the source directly. Resampling an equal-sized image changes
    // edge pixels for convolution filters, which caused the large RGBA pad
    // cases to diverge even though no resize was required.
    let mut resize_vector_blocks = 0u64;
    let mut resize_scalar_tail = 0u64;
    let resized = if (img.width(), img.height()) == (contained_width, contained_height) {
        let mut copied = vec![0u8; img.as_bytes().len()];
        let (blocks, tail) = copy_native_bytes(img.as_bytes(), &mut copied)
            .ok_or_else(|| PilError::InternalError("SIMD pad source copy shape mismatch".into()))?;
        resize_vector_blocks = resize_vector_blocks.saturating_add(blocks);
        resize_scalar_tail = resize_scalar_tail.saturating_add(tail);
        let result = crate::image_utils::raw_bytes_to_image(
            img.width(),
            img.height(),
            copied,
            channels,
        )?;
        preserve_mode(img, result)
    } else if mode == Some("F") {
        simd_resize_f(img, contained_width, contained_height, &filter)?
    } else if mode == Some("I") {
        simd_resize_i32(img, contained_width, contained_height, &filter)?
    } else {
        match filter {
            ResampleFilter::Nearest => {
                simd_resize_nearest(img, contained_width, contained_height, channels)?
            }
            _ => simd_resize_convolution(
                img,
                contained_width,
                contained_height,
                filter,
                channels,
                match channels {
                    2 => matches!(mode, None | Some("LA")),
                    4 => matches!(mode, None | Some("RGBA")),
                    _ => false,
                },
            )?,
        }
    };
    let source_width = usize::try_from(contained_width)
        .map_err(|_| simd_unsupported("Pad"))?;
    let source_height = usize::try_from(contained_height)
        .map_err(|_| simd_unsupported("Pad"))?;
    let target_width = usize::try_from(target_width)
        .map_err(|_| simd_unsupported("Pad"))?;
    let target_height = usize::try_from(target_height)
        .map_err(|_| simd_unsupported("Pad"))?;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let target_stride = target_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    let output_len = target_stride
        .checked_mul(target_height)
        .ok_or_else(|| simd_unsupported("Pad"))?;
    if resized.as_bytes().len() != source_stride.saturating_mul(source_height)
        || offset_x.saturating_add(source_width) > target_width
        || offset_y.saturating_add(source_height) > target_height
    {
        return Ok(None);
    }

    let default_fill = if channels == 2 || channels == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    };
    let fill = color.unwrap_or(default_fill);
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = resize_vector_blocks;
    let mut scalar_tail = resize_scalar_tail;
    for row in output.chunks_exact_mut(target_stride) {
        let (blocks, tail) = native_fill_row(row, fill, channels)
            .ok_or_else(|| PilError::InternalError("SIMD pad fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let source = resized.as_bytes();
    for source_y in 0..source_height {
        let source_start = source_y
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let output_start = (offset_y + source_y)
            .checked_mul(target_stride)
            .and_then(|row| row.checked_add(offset_x.checked_mul(channels)?))
            .ok_or_else(|| simd_unsupported("Pad"))?;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_start + source_stride],
            &mut output[output_start..output_start + source_stride],
        )
        .ok_or_else(|| PilError::InternalError("SIMD pad copy shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        target_width as u32,
        target_height as u32,
        output,
        channels,
    )?;
    Ok(Some((preserve_mode(img, result), vector_blocks, scalar_tail)))
}

/// Return the dimensions Pillow's scalar `ImageOps.scale` validation computes.
///
/// Scale is scalar control work followed by the same native resize data plane
/// as `Image.resize`.  Keeping the ties-to-even rounding here makes SIMD
/// preflight agree with the public operation without moving validation into a
/// CPU adapter.
fn native_scale_dimensions(source_width: u32, source_height: u32, factor: f64) -> Option<(u32, u32)> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let round = |dimension: u32| {
        native_pad_round_dimension(f64::from(dimension) * factor)
            .filter(|rounded| *rounded > 0)
    };
    Some((round(source_width)?, round(source_height)?))
}

fn native_resize_supported_for_dimensions(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
) -> bool {
    if !(1..=4).contains(&channels) || output_width == 0 || output_height == 0 {
        return false;
    }
    // Pillow's scalar-domain resize paths produce a zero-filled output when
    // one source dimension is empty.  The result still has a valid positive
    // destination shape, so this is a vector zero-fill data plane rather than
    // a reason to route the operation through CPU.
    if source_width == 0 || source_height == 0 {
        return true;
    }
    if (source_width, source_height) == (output_width, output_height) {
        return source_width
            .checked_mul(source_height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes >= SIMD_RESIZE_NEAREST_BYTES as u32);
    }
    match filter {
        ResampleFilter::Nearest => resize_nearest_vectorizable(
            source_width,
            source_height,
            output_width,
            output_height,
            channels,
        ),
        _ => {
            usize::try_from(source_height)
                .ok()
                .and_then(|height| {
                    usize::try_from(output_width)
                        .ok()
                        .and_then(|width| height.checked_mul(width))
                })
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some()
                && usize::try_from(output_width)
                    .ok()
                    .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
                    .and_then(|pixels| pixels.checked_mul(channels))
                    .is_some()
        }
    }
}

fn native_resize_supported_for_image(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        if !matches!(img, DynamicImage::ImageLuma16(_)) {
            return false;
        }
        let Some(expected_bytes) = (img.width() as usize)
            .checked_mul(img.height() as usize)
            .and_then(|pixels| pixels.checked_mul(2))
        else {
            return false;
        };
        return img.as_bytes().len() == expected_bytes
            && native_resize_supported_for_dimensions(
                img.width(),
                img.height(),
                output_width,
                output_height,
                filter,
                2,
            );
    }
    let is_typed_scalar = matches!(mode, Some("I" | "F"));
    if is_typed_scalar && !matches!(img, DynamicImage::ImageRgba8(_)) {
        return false;
    }
    let channels = if is_typed_scalar {
        4
    } else if let Some(channels) = resize_native_channels_for_image(img, mode) {
        channels
    } else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
        && native_resize_supported_for_dimensions(
            img.width(),
            img.height(),
            output_width,
            output_height,
            filter,
            channels,
        )
}

fn native_resize_supported_for_shape(
    shape: SimdImageShape,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let (channels, valid) = if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N")) {
        (2, shape.layout == SimdLayout::Luma16)
    } else if matches!(mode, Some("I" | "F")) {
        (4, shape.layout == SimdLayout::Rgba8)
    } else {
        match native_resize_byte_layout_for_shape(shape, mode) {
            Some((channels, _)) => (channels, true),
            None => (0, false),
        }
    };
    valid
        && native_resize_supported_for_dimensions(
            shape.width,
            shape.height,
            output_width,
            output_height,
            filter,
            channels,
        )
}

#[inline]
fn resize_fixed_point_to_u8(sum: f64) -> u8 {
    let value = (sum as i64 + (1_i64 << 21)) >> 22;
    value.clamp(0, 255) as u8
}

#[inline]
fn resize_coeff_slice(coeffs: &FilterCoeffs, index: usize) -> Option<&[i64]> {
    let start = *coeffs.offsets.get(index)?;
    let count = *coeffs.count.get(index)?;
    coeffs.weights.get(start..start.checked_add(count)?)
}

#[inline]
fn resize_premultiply_u8(value: u8, alpha: u8) -> u8 {
    ((u16::from(value) * u16::from(alpha) + 127) / 255) as u8
}

#[inline]
fn resize_horizontal_scalar(
    source_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    output_x: usize,
    channel: usize,
    premultiplied_alpha: bool,
) -> Option<u8> {
    let x0 = usize::try_from(*coeffs.xmin.get(output_x)?).ok()?;
    let weights = resize_coeff_slice(coeffs, output_x)?;
    let alpha_channel = channels - 1;
    let mut sum = 0.0;
    for (tap, &weight) in weights.iter().enumerate() {
        let source_x = x0.checked_add(tap)?;
        let source_index = source_x
            .checked_mul(channels)?
            .checked_add(channel)?;
        let value = *source_row.get(source_index)?;
        let value = if premultiplied_alpha && channel != alpha_channel {
            resize_premultiply_u8(value, *source_row.get(source_x * channels + alpha_channel)?)
        } else {
            value
        };
        sum += f64::from(value) * weight as f64;
    }
    Some(resize_fixed_point_to_u8(sum))
}

fn resize_horizontal_vector_row(
    source_row: &[u8],
    channels: usize,
    coeffs: &FilterCoeffs,
    output_width: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
) -> Option<(u64, u64)> {
    // A short row is still one padded vector block. The inactive lanes are
    // left at zero and are never written to the output row.
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let alpha_channel = channels - 1;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let mut result = [[0u8; SIMD_RESIZE_LANES]; 4];
        for channel in 0..channels {
            let mut sums = f64x8::splat(0.0);
            let max_count = (0..SIMD_RESIZE_LANES)
                .filter_map(|lane| coeffs.count.get(output_x + lane))
                .copied()
                .max()
                .unwrap_or(0);
            for tap in 0..max_count {
                let mut samples = [0u8; SIMD_RESIZE_LANES];
                let mut alphas = [0u8; SIMD_RESIZE_LANES];
                let mut weights = [0.0; SIMD_RESIZE_LANES];
                for lane in 0..SIMD_RESIZE_LANES {
                    let index = output_x + lane;
                    if let (Some(&count), Some(&xmin), Some(&offset)) = (
                        coeffs.count.get(index),
                        coeffs.xmin.get(index),
                        coeffs.offsets.get(index),
                    ) && tap < count
                    {
                        let source_x = usize::try_from(xmin).ok()?.checked_add(tap)?;
                        let source_index = source_x.checked_mul(channels)?.checked_add(channel)?;
                        samples[lane] = *source_row.get(source_index)?;
                        if premultiplied_alpha && channel != alpha_channel {
                            alphas[lane] = *source_row.get(source_x * channels + alpha_channel)?;
                        }
                        let weight_index = offset.checked_add(tap)?;
                        weights[lane] = *coeffs.weights.get(weight_index)? as f64;
                    }
                }
                let samples = if premultiplied_alpha && channel != alpha_channel {
                    let values = u16x8::new(samples.map(u16::from))
                        * u16x8::new(alphas.map(u16::from))
                        + u16x8::splat(127);
                    simd_div255_u16x8(values).to_array().map(f64::from)
                } else {
                    samples.map(f64::from)
                };
                sums += f64x8::new(samples) * f64x8::new(weights);
            }
            result[channel] = sums.to_array().map(resize_fixed_point_to_u8);
        }
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane >= output_width {
                continue;
            }
            let output_start = (output_x + lane).checked_mul(channels)?;
            for channel in 0..channels {
                *output_row.get_mut(output_start + channel)? = result[channel][lane];
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        vector_width
    };
    for output_x in scalar_start..output_width {
        let output_start = output_x.checked_mul(channels)?;
        for channel in 0..channels {
            *output_row.get_mut(output_start + channel)? = resize_horizontal_scalar(
                source_row,
                channels,
                coeffs,
                output_x,
                channel,
                premultiplied_alpha,
            )?;
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

fn resize_vertical_vector_row(
    intermediate: &[u8],
    output_width: usize,
    source_height: usize,
    channels: usize,
    coeffs: &FilterCoeffs,
    output_y: usize,
    output_row: &mut [u8],
    premultiplied_alpha: bool,
) -> Option<(u64, u64)> {
    let weights = resize_coeff_slice(coeffs, output_y)?;
    let y0 = usize::try_from(*coeffs.xmin.get(output_y)?).ok()?;
    // A short output row is handled as one padded vector block. Inactive
    // lanes are zero-filled and skipped when the valid prefix is stored.
    let vector_width = if output_width < SIMD_RESIZE_LANES {
        SIMD_RESIZE_LANES
    } else {
        output_width / SIMD_RESIZE_LANES * SIMD_RESIZE_LANES
    };
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let alpha_channel = channels - 1;
    for output_x in (0..vector_width).step_by(SIMD_RESIZE_LANES) {
        let mut result = [[0u8; SIMD_RESIZE_LANES]; 4];
        for channel in 0..channels {
            let mut sums = f64x8::splat(0.0);
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0.checked_add(tap)?;
                if source_y >= source_height {
                    return None;
                }
                let mut samples = [0.0; SIMD_RESIZE_LANES];
                for lane in 0..SIMD_RESIZE_LANES {
                    if output_x + lane >= output_width {
                        continue;
                    }
                    let source_index = source_y
                        .checked_mul(output_width)?
                        .checked_add(output_x + lane)?
                        .checked_mul(channels)?
                        .checked_add(channel)?;
                    samples[lane] = f64::from(*intermediate.get(source_index)?);
                }
                sums += f64x8::new(samples) * f64x8::splat(weight as f64);
            }
            result[channel] = sums.to_array().map(resize_fixed_point_to_u8);
        }
        if premultiplied_alpha {
            let alpha = result[alpha_channel];
            for channel in 0..alpha_channel {
                let restored = (f64x8::new(result[channel].map(f64::from))
                    * f64x8::splat(255.0)
                    / f64x8::new(alpha.map(f64::from)).max(f64x8::splat(1.0)))
                    .to_array();
                for lane in 0..SIMD_RESIZE_LANES {
                    if alpha[lane] != 0 {
                        result[channel][lane] = restored[lane] as u8;
                    }
                }
            }
        }
        for lane in 0..SIMD_RESIZE_LANES {
            if output_x + lane >= output_width {
                continue;
            }
            let output_start = (output_x + lane).checked_mul(channels)?;
            for channel in 0..channels {
                *output_row.get_mut(output_start + channel)? = result[channel][lane];
            }
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_start = if output_width < SIMD_RESIZE_LANES {
        output_width
    } else {
        vector_width
    };
    for output_x in scalar_start..output_width {
        let output_start = output_x.checked_mul(channels)?;
        let mut result = [0u8; 4];
        for channel in 0..channels {
            let mut sum = 0.0;
            for (tap, &weight) in weights.iter().enumerate() {
                let source_y = y0.checked_add(tap)?;
                let source_index = source_y
                    .checked_mul(output_width)?
                    .checked_add(output_x)?
                    .checked_mul(channels)?
                    .checked_add(channel)?;
                sum += f64::from(*intermediate.get(source_index)?) * weight as f64;
            }
            result[channel] = resize_fixed_point_to_u8(sum);
        }
        if premultiplied_alpha {
            let alpha = result[alpha_channel];
            for channel in 0..alpha_channel {
                if alpha != 0 {
                    result[channel] =
                        (f64::from(result[channel]) * 255.0 / f64::from(alpha)) as u8;
                }
            }
        }
        for channel in 0..channels {
            *output_row.get_mut(output_start + channel)? = result[channel];
        }
        scalar_tail = scalar_tail.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail))
}

/// Gather one nearest-neighbour output block when the selected source pixels
/// do not fit in a single sixteen-byte shuffle window. Byte gathers are not
/// portable across the SIMD targets supported by this crate, so the index
/// arithmetic and loads stay scalar; packing and the contiguous output store
/// remain part of the SIMD data plane.
fn nearest_gather_block(
    source: &[u8],
    source_row: usize,
    x_indices: &[usize],
    output_x: usize,
    pixel_count: usize,
    channels: usize,
) -> Option<[u8; SIMD_RESIZE_NEAREST_BYTES]> {
    let mut block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
    for pixel in 0..pixel_count {
        let source_pixel = x_indices
            .get(output_x.checked_add(pixel)?)?
            .checked_mul(channels)?;
        let source_start = source_row.checked_add(source_pixel)?;
        let source_end = source_start.checked_add(channels)?;
        let output_start = pixel.checked_mul(channels)?;
        block
            .get_mut(output_start..output_start.checked_add(channels)?)?
            .copy_from_slice(source.get(source_start..source_end)?);
    }
    Some(block)
}

fn simd_resize_nearest(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Resize"))?;
    let output_width_usize = usize::try_from(output_width)
        .map_err(|_| simd_unsupported("Resize"))?;
    let output_height_usize = usize::try_from(output_height)
        .map_err(|_| simd_unsupported("Resize"))?;
    let x_indices = resize_nearest_indices(img.width(), output_width)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let y_indices = resize_nearest_indices(img.height(), output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let output_len = output_width_usize
        .checked_mul(output_height_usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD resize source buffer shape mismatch".into(),
        ));
    }
    let pixels_per_vector = SIMD_RESIZE_NEAREST_BYTES / channels;
    let vector_width = output_width_usize / pixels_per_vector * pixels_per_vector;
    let block_bytes = pixels_per_vector * channels;
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output_y
            .checked_mul(output_width_usize)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Resize"))?;
        if output_width_usize < pixels_per_vector {
            // A short row still has a vector data plane: gather the selected
            // samples into a padded u8x16 block and store only the valid
            // prefix. This keeps narrow ImageOps.pad/resize rows out of the
            // scalar-only admission bucket without reading past the source.
            let source_start = x_indices[0]
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let source_end = x_indices[output_width_usize - 1]
                .checked_mul(channels)
                .and_then(|offset| offset.checked_add(channels))
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let span = source_end
                .checked_sub(source_start)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let values = if span <= SIMD_RESIZE_NEAREST_BYTES {
                let mut source_block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                let source_slice_start = source_row
                    .checked_add(source_start)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let source_slice_end = source_slice_start
                    .checked_add(span)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                source_block[..span].copy_from_slice(
                    source
                        .get(source_slice_start..source_slice_end)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                );
                let mut indices = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                for pixel in 0..output_width_usize {
                    let source_pixel = x_indices[pixel]
                        .checked_mul(channels)
                        .and_then(|offset| offset.checked_sub(source_start))
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    for channel in 0..channels {
                        indices[pixel * channels + channel] =
                            u8::try_from(source_pixel + channel)
                                .map_err(|_| simd_unsupported("Resize"))?;
                    }
                }
                u8x16::new(source_block)
                    .swizzle_relaxed(u8x16::new(indices))
                    .to_array()
            } else {
                u8x16::new(
                    nearest_gather_block(
                        source,
                        source_row,
                        &x_indices,
                        0,
                        output_width_usize,
                        channels,
                    )
                    .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .to_array()
            };
            let output_bytes = output_width_usize
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            output
                .get_mut(output_row..output_row + output_bytes)
                .ok_or_else(|| simd_unsupported("Resize"))?
                .copy_from_slice(&values[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        for output_x in (0..vector_width).step_by(pixels_per_vector) {
            let source_start = x_indices[output_x]
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let source_end = x_indices[output_x + pixels_per_vector - 1]
                .checked_mul(channels)
                .and_then(|offset| offset.checked_add(channels))
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let span = source_end
                .checked_sub(source_start)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let values = if span <= SIMD_RESIZE_NEAREST_BYTES {
                let mut source_block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                let source_slice_start = source_row
                    .checked_add(source_start)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let source_slice_end = source_slice_start
                    .checked_add(span)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                source_block[..span].copy_from_slice(
                    source
                        .get(source_slice_start..source_slice_end)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                );
                let mut indices = [0u8; SIMD_RESIZE_NEAREST_BYTES];
                for pixel in 0..pixels_per_vector {
                    let source_pixel = x_indices[output_x + pixel]
                        .checked_mul(channels)
                        .and_then(|offset| offset.checked_sub(source_start))
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    for channel in 0..channels {
                        indices[pixel * channels + channel] =
                            u8::try_from(source_pixel + channel)
                                .map_err(|_| simd_unsupported("Resize"))?;
                    }
                }
                u8x16::new(source_block)
                    .swizzle_relaxed(u8x16::new(indices))
                    .to_array()
            } else {
                u8x16::new(
                    nearest_gather_block(
                        source,
                        source_row,
                        &x_indices,
                        output_x,
                        pixels_per_vector,
                        channels,
                    )
                    .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .to_array()
            };
            let output_start = output_row
                .checked_add(output_x.checked_mul(channels).ok_or_else(|| {
                    simd_unsupported("Resize")
                })?)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            output
                .get_mut(output_start..output_start + block_bytes)
                .ok_or_else(|| simd_unsupported("Resize"))?
                .copy_from_slice(&values[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        for output_x in vector_width..output_width_usize {
            let source_start = source_row
                .checked_add(
                    x_indices[output_x]
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                )
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_start = output_row
                .checked_add(output_x.checked_mul(channels).ok_or_else(|| {
                    simd_unsupported("Resize")
                })?)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            output
                .get_mut(output_start..output_start + channels)
                .ok_or_else(|| simd_unsupported("Resize"))?
                .copy_from_slice(
                    source
                        .get(source_start..source_start + channels)
                        .ok_or_else(|| simd_unsupported("Resize"))?,
                );
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Build the affine nearest-neighbour map used by boxed P/PA resampling.
/// Coordinates are evaluated in f64 lanes after Pillow's float box boundary
/// conversion; individual source loads remain scalar gathers because portable
/// byte gather is not available on every SIMD target.
fn boxed_nearest_indices(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> Option<Vec<usize>> {
    if source_size == 0
        || output_size == 0
        || !box_start.is_finite()
        || !box_end.is_finite()
    {
        return None;
    }
    let box_start = box_start as f32 as f64;
    let box_end = box_end as f32 as f64;
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    let last = f64::from(source_size - 1);
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    for output_x in (0..output_size as usize).step_by(SIMD_F64_LANES) {
        let count = (output_size as usize - output_x).min(SIMD_F64_LANES);
        let coordinates = (f64x8::splat(box_start)
            + f64x8::splat(scale)
                * f64x8::new(std::array::from_fn(|lane| {
                    if lane < count {
                        (output_x + lane) as f64 + 0.5
                    } else {
                        0.0
                    }
                })))
        .to_array();
        for coordinate in coordinates.into_iter().take(count) {
            indices.push(coordinate.floor().clamp(0.0, last) as usize);
        }
    }
    Some(indices)
}

/// Execute boxed resampling for an F-mode image. The source and intermediate
/// values are f32 samples, not four independent bytes. Coordinate/index
/// construction and sample accumulation use SIMD lanes; scalar work is
/// limited to portable gathers and IEEE-754 byte serialization.
fn simd_resize_f_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Fit"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Fit"))?;
    let output_width = usize::try_from(output_width)
        .map_err(|_| simd_unsupported("Fit"))?;
    let output_height = usize::try_from(output_height)
        .map_err(|_| simd_unsupported("Fit"))?;
    let pixel_count = source_width
        .checked_mul(source_height)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let output_count = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_len = pixel_count
        .checked_mul(4)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD Fit F source buffer shape mismatch".into(),
        ));
    }
    let mut vector_blocks = 0u64;
    let source: Vec<f32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();

    let mut output_floats = vec![0.0f32; output_count];
    if source_width == 0 || source_height == 0 {
        let zero = f32x8::splat(0.0).to_array();
        for block in output_floats.chunks_exact_mut(SIMD_F64_LANES) {
            block.copy_from_slice(&zero);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output_floats.len() % SIMD_F64_LANES;
        if remainder != 0 {
            let start = output_floats.len() - remainder;
            output_floats[start..].copy_from_slice(&zero[..remainder]);
        }
    } else if matches!(filter, ResampleFilter::Nearest) {
        let x_indices = boxed_nearest_indices(
            source_width as u32,
            output_width as u32,
            box_left,
            box_right,
        )
        .ok_or_else(|| simd_unsupported("Fit"))?;
        let y_indices = boxed_nearest_indices(
            source_height as u32,
            output_height as u32,
            box_top,
            box_bottom,
        )
        .ok_or_else(|| simd_unsupported("Fit"))?;
        for (output_y, &source_y) in y_indices.iter().enumerate() {
            let output_row = output_y * output_width;
            for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
                let count = (output_width - output_x).min(SIMD_F64_LANES);
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        source[source_y * source_width + x_indices[output_x + lane]]
                    } else {
                        0.0
                    }
                });
                output_floats[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&f32x8::new(values).to_array()[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    } else {
        let horizontal = precompute_coeffs_f64_boxed(
            output_width as u32,
            source_width as u32,
            box_left,
            box_right,
            filter,
        );
        let vertical = precompute_coeffs_f64_boxed(
            output_height as u32,
            source_height as u32,
            box_top,
            box_bottom,
            filter,
        );
        let mut intermediate = vec![0.0f32; source_height * output_width];
        for source_y in 0..source_height {
            let source_row = source_y * source_width;
            let intermediate_row = source_y * output_width;
            for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
                let count = (output_width - output_x).min(SIMD_F64_LANES);
                let max_count = (0..count)
                    .map(|lane| horizontal.weights[output_x + lane].len())
                    .max()
                    .unwrap_or(0);
                let mut sums = f64x8::splat(0.0);
                for tap in 0..max_count {
                    let mut values = [0.0; SIMD_F64_LANES];
                    let mut weights = [0.0; SIMD_F64_LANES];
                    for lane in 0..count {
                        let output_index = output_x + lane;
                        if tap < horizontal.weights[output_index].len() {
                            let source_x = horizontal.xmin[output_index] as usize + tap;
                            values[lane] = f64::from(source[source_row + source_x]);
                            weights[lane] = horizontal.weights[output_index][tap];
                        }
                    }
                    sums += f64x8::new(values) * f64x8::new(weights);
                }
                let values = sums.to_array().map(|value| {
                    let value = value as f32;
                    if value == 0.0 { 0.0 } else { value }
                });
                intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                    .copy_from_slice(&values[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        for output_y in 0..output_height {
            let y0 = vertical.xmin[output_y] as usize;
            let weights = &vertical.weights[output_y];
            let output_row = output_y * output_width;
            for output_x in (0..output_width).step_by(SIMD_F64_LANES) {
                let count = (output_width - output_x).min(SIMD_F64_LANES);
                let mut sums = f64x8::splat(0.0);
                for (tap, &weight) in weights.iter().enumerate() {
                    let source_row = (y0 + tap) * output_width;
                    let values = std::array::from_fn(|lane| {
                        if lane < count {
                            f64::from(intermediate[source_row + output_x + lane])
                        } else {
                            0.0
                        }
                    });
                    sums += f64x8::new(values) * f64x8::splat(weight);
                }
                let values = sums.to_array().map(|value| {
                    let value = value as f32;
                    if value == 0.0 { 0.0 } else { value }
                });
                output_floats[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&values[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let output: Vec<u8> = output_floats
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    Ok(crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?)
}

/// Execute boxed nearest-neighbour sampling for indexed P/PA images. The
/// coordinate equations and output packing use SIMD lanes; only the portable
/// byte gathers and unavoidable short-row/tail stores are scalar.
fn simd_resize_nearest_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Fit"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Fit"))?;
    let output_width = usize::try_from(output_width)
        .map_err(|_| simd_unsupported("Fit"))?;
    let output_height = usize::try_from(output_height)
        .map_err(|_| simd_unsupported("Fit"))?;
    let x_indices = boxed_nearest_indices(
        source_width as u32,
        output_width as u32,
        box_left,
        box_right,
    )
    .ok_or_else(|| simd_unsupported("Fit"))?;
    let y_indices = boxed_nearest_indices(
        source_height as u32,
        output_height as u32,
        box_top,
        box_bottom,
    )
    .ok_or_else(|| simd_unsupported("Fit"))?;
    let output_len = output_width
        .checked_mul(output_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD Fit source buffer shape mismatch".into(),
        ));
    }
    let source = img.as_bytes();
    let pixels_per_vector = SIMD_RESIZE_NEAREST_BYTES / channels;
    let vector_width = output_width / pixels_per_vector * pixels_per_vector;
    let block_bytes = pixels_per_vector * channels;
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for (output_y, &source_y) in y_indices.iter().enumerate() {
        let source_row = source_y
            .checked_mul(source_width)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let output_row = output_y
            .checked_mul(output_width)
            .and_then(|offset| offset.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Fit"))?;
        if output_width < pixels_per_vector {
            let values = u8x16::new(
                nearest_gather_block(source, source_row, &x_indices, 0, output_width, channels)
                    .ok_or_else(|| simd_unsupported("Fit"))?,
            )
            .to_array();
            let output_bytes = output_width
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Fit"))?;
            output
                .get_mut(output_row..output_row + output_bytes)
                .ok_or_else(|| simd_unsupported("Fit"))?
                .copy_from_slice(&values[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            continue;
        }
        for output_x in (0..vector_width).step_by(pixels_per_vector) {
            let values = u8x16::new(
                nearest_gather_block(
                    source,
                    source_row,
                    &x_indices,
                    output_x,
                    pixels_per_vector,
                    channels,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?,
            )
            .to_array();
            let output_start = output_row
                .checked_add(
                    output_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            output
                .get_mut(output_start..output_start + block_bytes)
                .ok_or_else(|| simd_unsupported("Fit"))?
                .copy_from_slice(&values[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        for output_x in vector_width..output_width {
            let source_start = source_row
                .checked_add(
                    x_indices[output_x]
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            let output_start = output_row
                .checked_add(
                    output_x
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                )
                .ok_or_else(|| simd_unsupported("Fit"))?;
            output
                .get_mut(output_start..output_start + channels)
                .ok_or_else(|| simd_unsupported("Fit"))?
                .copy_from_slice(
                    source
                        .get(source_start..source_start + channels)
                        .ok_or_else(|| simd_unsupported("Fit"))?,
                );
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

fn simd_resize_convolution(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width)
        .map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height)
        .map_err(|_| simd_unsupported("Resize"))?;
    let intermediate_len = source_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let output_len = output_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD resize source buffer shape mismatch".into(),
        ));
    }
    let horizontal = precompute_coeffs(output_width as u32, source_width as u32, filter);
    let vertical = precompute_coeffs(output_height as u32, source_height as u32, filter);
    let mut intermediate = vec![0u8; intermediate_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let intermediate_stride = output_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    for source_y in 0..source_height {
        let source_start = source_y
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_start = source_y
            .checked_mul(intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let source_row = img
            .as_bytes()
            .get(source_start..source_start + source_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let intermediate_row = intermediate
            .get_mut(intermediate_start..intermediate_start + intermediate_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let (blocks, tail) = resize_horizontal_vector_row(
            source_row,
            channels,
            &horizontal,
            output_width,
            intermediate_row,
            premultiplied_alpha,
        )
        .ok_or_else(|| simd_unsupported("Resize"))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let mut output = vec![0u8; output_len];
    let output_stride = intermediate_stride;
    for output_y in 0..output_height {
        let output_start = output_y
            .checked_mul(output_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let output_row = output
            .get_mut(output_start..output_start + output_stride)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let (blocks, tail) = resize_vertical_vector_row(
            &intermediate,
            output_width,
            source_height,
            channels,
            &vertical,
            output_y,
            output_row,
            premultiplied_alpha,
        )
        .ok_or_else(|| simd_unsupported("Resize"))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Resize a fractional source box with the native SIMD two-pass resampler.
///
/// The box coordinates are scalar control data only.  Coefficients are built
/// by the same Pillow-compatible fixed-point builder used by
/// `pil_resize_boxed`; the row kernels then vectorize the pixel accumulations
/// without creating a cropped source image or retrying through CPU.
fn simd_resize_convolution_boxed(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    box_left: f64,
    box_top: f64,
    box_right: f64,
    box_bottom: f64,
    filter: ResampleFilter,
    channels: usize,
    premultiplied_alpha: bool,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Fit"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Fit"))?;
    let output_width = usize::try_from(output_width)
        .map_err(|_| simd_unsupported("Fit"))?;
    let output_height = usize::try_from(output_height)
        .map_err(|_| simd_unsupported("Fit"))?;
    let intermediate_len = source_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let output_len = output_height
        .checked_mul(output_width)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD Fit source buffer shape mismatch".into(),
        ));
    }

    let horizontal = precompute_coeffs_boxed_for_filter(
        output_width as u32,
        source_width as u32,
        box_left,
        box_right,
        filter,
    );
    let vertical = precompute_coeffs_boxed_for_filter(
        output_height as u32,
        source_height as u32,
        box_top,
        box_bottom,
        filter,
    );
    let mut intermediate = vec![0u8; intermediate_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    let intermediate_stride = output_width
        .checked_mul(channels)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    for source_y in 0..source_height {
        let source_start = source_y
            .checked_mul(source_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let intermediate_start = source_y
            .checked_mul(intermediate_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let source_row = img
            .as_bytes()
            .get(source_start..source_start + source_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let intermediate_row = intermediate
            .get_mut(intermediate_start..intermediate_start + intermediate_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let (blocks, tail) = resize_horizontal_vector_row(
            source_row,
            channels,
            &horizontal,
            output_width,
            intermediate_row,
            premultiplied_alpha,
        )
        .ok_or_else(|| simd_unsupported("Fit"))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }

    let mut output = vec![0u8; output_len];
    let output_stride = intermediate_stride;
    for output_y in 0..output_height {
        let output_start = output_y
            .checked_mul(output_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let output_row = output
            .get_mut(output_start..output_start + output_stride)
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let (blocks, tail) = resize_vertical_vector_row(
            &intermediate,
            output_width,
            source_height,
            channels,
            &vertical,
            output_y,
            output_row,
            premultiplied_alpha,
        )
        .ok_or_else(|| simd_unsupported("Fit"))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Run an aspect-ratio operation after its scalar dimension calculation.
///
/// `Contain` and `Cover` differ only in that control-plane calculation. Once
/// the output dimensions are known, both use the same native-layout resize
/// data plane as `Resize`; they never call the CPU imageops implementation.
fn native_aspect_resize_bytes(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
    dimensions: fn(u32, u32, u32, u32) -> Option<(u32, u32)>,
    operation: &str,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return Ok(None);
    };
    let Some((output_width, output_height)) =
        dimensions(img.width(), img.height(), target_width, target_height)
    else {
        return Ok(None);
    };
    if !native_resize_supported_for_image(img, output_width, output_height, filter, mode) {
        return Ok(None);
    }
    if (img.width(), img.height()) == (output_width, output_height) {
        return native_copy_image_bytes(img, mode);
    }
    let result = match filter {
        ResampleFilter::Nearest => simd_resize_nearest(img, output_width, output_height, channels),
        _ => simd_resize_convolution(
            img,
            output_width,
            output_height,
            filter,
            channels,
            matches!(channels, 2 | 4),
        ),
    }?;
    if result.width() != output_width || result.height() != output_height {
        return Err(PilError::InternalError(format!(
            "SIMD {operation} resize shape mismatch"
        )));
    }
    Ok(Some(result))
}

fn simd_affine_luma16_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !native_affine_luma16_transform_supported_for_image(
        img,
        width,
        height,
        &TransformMethod::Affine,
        data,
        mode,
    ) {
        return Ok(None);
    }
    let DynamicImage::ImageLuma16(source) = img else {
        return Ok(None);
    };
    let source_width = source.width() as usize;
    let source_height = source.height() as usize;
    let destination_width = width as usize;
    let destination_height = height as usize;
    let [a, b, c, d, e, f] = data else {
        return Ok(None);
    };
    let fill = fill.map_or(0, |color| u16::from_le_bytes([color.0, color.1]));
    let mut output = Vec::with_capacity(destination_width * destination_height);
    let mut vector_blocks = 0u64;

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let x = f64x8::new(std::array::from_fn(|lane| {
                if lane < count {
                    (destination_x + lane) as f64
                } else {
                    0.0
                }
            }));
            let y = f64x8::splat(destination_y as f64);
            let source_x = (f64x8::splat(*a) * x
                + f64x8::splat(*b) * y
                + f64x8::splat(*c))
                .to_array();
            let source_y = (f64x8::splat(*d) * x
                + f64x8::splat(*e) * y
                + f64x8::splat(*f))
                .to_array();
            let mut values = [fill; SIMD_F64_LANES];
            for lane in 0..count {
                let input_x = if source_x[lane].is_finite() {
                    (source_x[lane] + 0.5).floor() as i64
                } else {
                    -1
                };
                let input_y = if source_y[lane].is_finite() {
                    (source_y[lane] + 0.5).floor() as i64
                } else {
                    -1
                };
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    values[lane] = source.get_pixel(input_x as u32, input_y as u32)[0];
                }
            }
            output.extend_from_slice(&u16x8::new(values).to_array()[..count]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    ImageBuffer::<Luma<u16>, Vec<u16>>::from_raw(width, height, output)
        .map(DynamicImage::ImageLuma16)
        .map(Some)
        .ok_or_else(|| PilError::InternalError("SIMD transform I;16 buffer shape mismatch".into()))
}

fn simd_mesh_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_width = usize::try_from(width)
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height)
        .map_err(|_| simd_unsupported("Transform"))?;
    if source_width == 0
        || source_height == 0
        || destination_width == 0
        || destination_height == 0
        || data.is_empty()
        || data.len() % 12 != 0
    {
        return Ok(None);
    }
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD mesh transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let mut output = vec![0u8; destination_len];
    for destination in output.chunks_exact_mut(channels) {
        destination.copy_from_slice(&fill[..channels]);
    }
    let mut vector_blocks = 0u64;

    for mesh in data.chunks_exact(12) {
        let x0_d = mesh[0] as i32;
        let y0_d = mesh[1] as i32;
        let x1_d = mesh[2] as i32;
        let y1_d = mesh[3] as i32;
        let x0_s = mesh[4];
        let y0_s = mesh[5];
        let x1_s = mesh[6];
        let y1_s = mesh[7];
        let x2_s = mesh[8];
        let y2_s = mesh[9];
        let x3_s = mesh[10];
        let y3_s = mesh[11];

        // Match the core mesh implementation's clipping and inclusive lower
        // bound rules before entering the vectorized span loop.
        let bx0 = x0_d.max(0).min(width as i32);
        let by0 = y0_d.max(0).min(height as i32);
        let bx1 = x1_d.max(1).min(width as i32);
        let by1 = y1_d.max(1).min(height as i32);
        let bw = (bx1 - bx0).max(1) as f64;
        let bh = (by1 - by0).max(1) as f64;

        for destination_y in by0..by1 {
            let v = (destination_y - y0_d) as f64 / bh;
            let one_minus_v = 1.0 - v;
            let base_x = one_minus_v * x0_s + v * x1_s;
            let delta_x = one_minus_v * (x3_s - x0_s) + v * (x2_s - x1_s);
            let base_y = one_minus_v * y0_s + v * y1_s;
            let delta_y = one_minus_v * (y3_s - y0_s) + v * (y2_s - y1_s);
            let mut destination_x = bx0;
            while destination_x < bx1 {
                let count = ((bx1 - destination_x) as usize).min(SIMD_F64_LANES);
                let u = f64x8::new(std::array::from_fn(|lane| {
                    if lane < count {
                        (destination_x + lane as i32 - x0_d) as f64 / bw
                    } else {
                        0.0
                    }
                }));
                let source_x = (f64x8::splat(base_x) + f64x8::splat(delta_x) * u).to_array();
                let source_y = (f64x8::splat(base_y) + f64x8::splat(delta_y) * u).to_array();
                for lane in 0..count {
                    let input_x = if source_x[lane].is_finite() {
                        (source_x[lane] + 0.5).floor() as i64
                    } else {
                        -1
                    };
                    let input_y = if source_y[lane].is_finite() {
                        (source_y[lane] + 0.5).floor() as i64
                    } else {
                        -1
                    };
                    let output_start = ((destination_y as usize * destination_width)
                        + destination_x as usize
                        + lane)
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    if input_x >= 0
                        && input_x < source_width as i64
                        && input_y >= 0
                        && input_y < source_height as i64
                    {
                        let source_start = (input_y as usize * source_width + input_x as usize)
                            .checked_mul(channels)
                            .ok_or_else(|| simd_unsupported("Transform"))?;
                        output[output_start..output_start + channels]
                            .copy_from_slice(&source[source_start..source_start + channels]);
                    }
                }
                vector_blocks = vector_blocks.saturating_add(1);
                destination_x += count as i32;
            }
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

fn simd_affine_nearest_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_width = usize::try_from(width)
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height)
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or(if channels == 2 || channels == 4 {
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, u8::MAX)
    });
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f] = data else {
        return Ok(None);
    };
    let pixels_per_vector = SIMD_RESIZE_NEAREST_BYTES / channels;
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    for destination_y in 0..destination_height {
        let row_start = destination_y
            .checked_mul(destination_width)
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Transform"))?;
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = pixels_per_vector.min(destination_width - destination_x);
            let mut block = [0u8; SIMD_RESIZE_NEAREST_BYTES];
            for pixel in 0..count {
                let x = destination_x + pixel;
                let y = destination_y;
                let source_x = *a * (x as f64 + 0.5)
                    + *b * (y as f64 + 0.5)
                    + *c;
                let source_y = *d * (x as f64 + 0.5)
                    + *e * (y as f64 + 0.5)
                    + *f;
                // Match Geometry.c's nearest affine contract: negative
                // coordinates are outside, while non-negative coordinates
                // truncate toward zero before the bounds check.
                let input_x = if source_x < 0.0 {
                    -1
                } else {
                    source_x as i64
                };
                let input_y = if source_y < 0.0 {
                    -1
                } else {
                    source_y as i64
                };
                let block_start = pixel
                    .checked_mul(channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    let source_start = (input_y as usize)
                        .checked_mul(source_width)
                        .and_then(|pixels| pixels.checked_add(input_x as usize))
                        .and_then(|pixel| pixel.checked_mul(channels))
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    let source_end = source_start
                        .checked_add(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    block
                        .get_mut(block_start..block_start + channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?
                        .copy_from_slice(
                            source
                                .get(source_start..source_end)
                                .ok_or_else(|| simd_unsupported("Transform"))?,
                        );
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] = fill[channel];
                    }
                }
            }
            let output_start = row_start
                .checked_add(destination_x.checked_mul(channels).ok_or_else(|| {
                    simd_unsupported("Transform")
                })?)
                .ok_or_else(|| simd_unsupported("Transform"))?;
            let output_bytes = count
                .checked_mul(channels)
                .ok_or_else(|| simd_unsupported("Transform"))?;
            output
                .get_mut(output_start..output_start + output_bytes)
                .ok_or_else(|| simd_unsupported("Transform"))?
                .copy_from_slice(&u8x16::new(block).to_array()[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Vectorized nearest sampling for perspective and quadrilateral transforms.
/// The homography/quad coordinate equations run in eight `f64` lanes; source
/// address validation and the irregular gathers remain scalar control work.
/// No CPU transform is retried after this adapter has been admitted.
fn simd_projective_nearest_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
    quad: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let destination_width = usize::try_from(width)
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height)
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Transform"))?;
    if destination_width == 0
        || destination_height == 0
        || source_width == 0
        || source_height == 0
        || data.len() != 8
    {
        return Ok(None);
    }
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD projective transform source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 255));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f, g, h] = data else {
        return Ok(None);
    };
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let destination_width_f = destination_width as f64;
    let destination_height_f = destination_height as f64;

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let x_values = std::array::from_fn(|lane| {
                if lane < count {
                    (destination_x + lane) as f64
                } else {
                    0.0
                }
            });
            let y_values = [destination_y as f64; SIMD_F64_LANES];
            let x = f64x8::new(x_values);
            let y = f64x8::new(y_values);
            let (source_x, source_y) = if quad {
                let x0 = *a;
                let y0 = *b;
                let inv_width = 1.0 / destination_width_f;
                let inv_height = 1.0 / destination_height_f;
                let sx = f64x8::splat(x0)
                    + f64x8::splat(*g - x0) * x * f64x8::splat(inv_width)
                    + f64x8::splat(*c - x0) * y * f64x8::splat(inv_height)
                    + f64x8::splat(*e - *c - *g + x0)
                        * x
                        * y
                        * f64x8::splat(inv_width * inv_height);
                let sy = f64x8::splat(y0)
                    + f64x8::splat(*h - y0) * x * f64x8::splat(inv_width)
                    + f64x8::splat(*d - y0) * y * f64x8::splat(inv_height)
                    + f64x8::splat(*f - *d - *h + y0)
                        * x
                        * y
                        * f64x8::splat(inv_width * inv_height);
                (sx.to_array(), sy.to_array())
            } else {
                let denominator = f64x8::splat(*g) * x
                    + f64x8::splat(*h) * y
                    + f64x8::splat(1.0);
                let sx = (f64x8::splat(*a) * x
                    + f64x8::splat(*b) * y
                    + f64x8::splat(*c))
                    / denominator;
                let sy = (f64x8::splat(*d) * x
                    + f64x8::splat(*e) * y
                    + f64x8::splat(*f))
                    / denominator;
                (sx.to_array(), sy.to_array())
            };

            for lane in 0..count {
                let output_start = (destination_y * destination_width + destination_x + lane)
                    .checked_mul(channels)
                    .ok_or_else(|| simd_unsupported("Transform"))?;
                let sx = source_x[lane];
                let sy = source_y[lane];
                let input_x = if sx.is_finite() {
                    (sx + 0.5).floor() as i64
                } else {
                    -1
                };
                let input_y = if sy.is_finite() {
                    (sy + 0.5).floor() as i64
                } else {
                    -1
                };
                if input_x >= 0
                    && input_x < source_width as i64
                    && input_y >= 0
                    && input_y < source_height as i64
                {
                    let source_start = (input_y as usize * source_width + input_x as usize)
                        .checked_mul(channels)
                        .ok_or_else(|| simd_unsupported("Transform"))?;
                    output[output_start..output_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    output[output_start..output_start + channels]
                        .copy_from_slice(&fill[..channels]);
                }
            }
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Vectorized bilinear sampling for perspective and quadrilateral transforms.
///
/// The inverse-map equations and four-neighbour interpolation run in
/// `f64x8` blocks. Source addresses are irregular gathers, so their bounds
/// checks and byte loads remain scalar control work; they are packed into the
/// vector lanes before interpolation. Invalid coordinates use Pillow's fill
/// sample and never trigger a CPU retry.
fn simd_projective_bilinear_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
    quad: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_affine_nearest_transform_channels(img, mode) else {
        return Ok(None);
    };
    let destination_width = usize::try_from(width)
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height)
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Transform"))?;
    if destination_width == 0
        || destination_height == 0
        || source_width == 0
        || source_height == 0
        || data.len() != 8
    {
        return Ok(None);
    }
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD projective bilinear source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 255));
    let fill = [fill.0, fill.1, fill.2, fill.3];
    let [a, b, c, d, e, f, g, h] = data else {
        return Ok(None);
    };
    let inverse_width = 1.0 / destination_width as f64;
    let inverse_height = 1.0 / destination_height as f64;
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let one = f64x8::splat(1.0);

    for destination_y in 0..destination_height {
        let mut destination_x = 0usize;
        while destination_x < destination_width {
            let count = (destination_width - destination_x).min(SIMD_F64_LANES);
            let x = f64x8::new(std::array::from_fn(|lane| {
                if lane < count {
                    (destination_x + lane) as f64
                } else {
                    0.0
                }
            }));
            let y = f64x8::splat(destination_y as f64);
            let (source_x, source_y) = if quad {
                let source_x = f64x8::splat(*a)
                    + f64x8::splat(*g - *a) * x * f64x8::splat(inverse_width)
                    + f64x8::splat(*c - *a) * y * f64x8::splat(inverse_height)
                    + f64x8::splat(*e - *c - *g + *a)
                        * x
                        * y
                        * f64x8::splat(inverse_width * inverse_height);
                let source_y = f64x8::splat(*b)
                    + f64x8::splat(*h - *b) * x * f64x8::splat(inverse_width)
                    + f64x8::splat(*d - *b) * y * f64x8::splat(inverse_height)
                    + f64x8::splat(*f - *d - *h + *b)
                        * x
                        * y
                        * f64x8::splat(inverse_width * inverse_height);
                (source_x.to_array(), source_y.to_array())
            } else {
                let denominator = f64x8::splat(*g) * x
                    + f64x8::splat(*h) * y
                    + f64x8::splat(1.0);
                let source_x = (f64x8::splat(*a) * x
                    + f64x8::splat(*b) * y
                    + f64x8::splat(*c))
                    / denominator;
                let source_y = (f64x8::splat(*d) * x
                    + f64x8::splat(*e) * y
                    + f64x8::splat(*f))
                    / denominator;
                (source_x.to_array(), source_y.to_array())
            };

            let mut fx_values = [0.0; SIMD_F64_LANES];
            let mut fy_values = [0.0; SIMD_F64_LANES];
            let mut neighbors = [[[0.0; SIMD_F64_LANES]; 4]; 4];
            for channel in 0..channels {
                for neighbor in 0..4 {
                    for lane in 0..count {
                        neighbors[channel][neighbor][lane] = fill[channel] as f64;
                    }
                }
            }
            for lane in 0..count {
                let sx = source_x[lane];
                let sy = source_y[lane];
                if !sx.is_finite()
                    || !sy.is_finite()
                    || sx < 0.0
                    || sx >= source_width as f64
                    || sy < 0.0
                    || sy >= source_height as f64
                {
                    continue;
                }
                let x0 = sx.floor() as usize;
                let y0 = sy.floor() as usize;
                let x1 = (x0 + 1).min(source_width - 1);
                let y1 = (y0 + 1).min(source_height - 1);
                fx_values[lane] = sx - x0 as f64;
                fy_values[lane] = sy - y0 as f64;
                let coordinates = [(x0, y0), (x1, y0), (x0, y1), (x1, y1)];
                for (neighbor, &(source_x, source_y)) in coordinates.iter().enumerate() {
                    let source_start = (source_y * source_width + source_x) * channels;
                    for channel in 0..channels {
                        neighbors[channel][neighbor][lane] =
                            source[source_start + channel] as f64;
                    }
                }
            }

            let fx = f64x8::new(fx_values);
            let fy = f64x8::new(fy_values);
            let mut block = [0u8; SIMD_F64_LANES * 4];
            for channel in 0..channels {
                let p00 = f64x8::new(neighbors[channel][0]);
                let p10 = f64x8::new(neighbors[channel][1]);
                let p01 = f64x8::new(neighbors[channel][2]);
                let p11 = f64x8::new(neighbors[channel][3]);
                let interpolated = (one - fx) * ((one - fy) * p00 + fy * p01)
                    + fx * ((one - fy) * p10 + fy * p11);
                let values = interpolated
                    .to_array()
                    .map(|value| value.clamp(0.0, 255.0).round() as u8);
                for lane in 0..count {
                    block[lane * channels + channel] = values[lane];
                }
            }
            let output_start = (destination_y * destination_width + destination_x) * channels;
            let output_bytes = count * channels;
            output[output_start..output_start + output_bytes]
                .copy_from_slice(&block[..output_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += count;
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Vectorized affine bilinear sampling for native byte layouts.
///
/// Coordinate construction, bounds checks, and source-neighbour indices are
/// scalar control work. The four-neighbour interpolation itself runs in
/// `f64x8` blocks, with the same premultiplied-alpha byte boundaries used by
/// Pillow for `LA` and `RGBA`. A short final row segment is handled by the
/// scalar tail; it never retries the CPU transform implementation.
fn simd_affine_bilinear_transform_bytes(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some((channels, alpha_channel)) = native_affine_bilinear_layout(img, mode) else {
        return Ok(None);
    };
    let destination_width = usize::try_from(width)
        .map_err(|_| simd_unsupported("Transform"))?;
    let destination_height = usize::try_from(height)
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_width = usize::try_from(img.width())
        .map_err(|_| simd_unsupported("Transform"))?;
    let source_height = usize::try_from(img.height())
        .map_err(|_| simd_unsupported("Transform"))?;
    if destination_height == 0 || source_width == 0 || source_height == 0 {
        return Ok(None);
    }
    let [a, b, c, d, e, f] = data else {
        return Ok(None);
    };
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Transform"))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD affine bilinear source buffer shape mismatch".into(),
        ));
    }

    // `Image.transform` carries LA/PA alpha in fill.1, while the shared
    // rotate gather helper reads a two-channel alpha fill from fill.3.
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fill = if channels == 2 {
        (fill.0, 0, 0, fill.1)
    } else {
        fill
    };
    let affine = [*a, *b, *c, *d, *e, *f];
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    // A narrow output row still uses one padded vector block. Inactive lanes
    // are computed into the temporary block and omitted from the store; they
    // are never exposed as a scalar-only admission failure.
    let vector_width = if destination_width < SIMD_F64_LANES {
        destination_width
    } else {
        destination_width / SIMD_F64_LANES * SIMD_F64_LANES
    };

    for y in 0..destination_height {
        let output_row = y * destination_width * channels;
        let mut x = 0usize;
        while x < vector_width {
            let count = (destination_width - x).min(SIMD_F64_LANES);
            let samples = std::array::from_fn(|lane| {
                if lane < count {
                    simd_rotate_sample_coordinates(
                        affine,
                        source_width,
                        source_height,
                        x + lane,
                        y,
                    )
                } else {
                    SimdRotateSample::default()
                }
            });
            let fx = f64x8::new(samples.map(|sample| sample.fx));
            let fy = f64x8::new(samples.map(|sample| sample.fy));
            let mut block = [0u8; SIMD_F64_LANES * 4];
            let mut alpha = [0u8; SIMD_F64_LANES];

            if let Some(alpha_channel) = alpha_channel {
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    source_width,
                    channels,
                    alpha_channel,
                    fill,
                    &samples,
                    None,
                );
                alpha = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                )
                .to_array()
                .map(|value| value as u8);
                for (lane, &value) in alpha.iter().enumerate() {
                    block[lane * channels + alpha_channel] = value;
                }
            }

            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    source_width,
                    channels,
                    channel,
                    fill,
                    &samples,
                    alpha_channel,
                );
                let interpolated = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                );
                let values = if alpha_channel.is_some() {
                    rotate_unpremultiply_vector(interpolated, alpha)
                } else {
                    interpolated.to_array().map(|value| value as u8)
                };
                for (lane, &value) in values.iter().enumerate() {
                    block[lane * channels + channel] = value;
                }
            }

            let output_start = output_row + x * channels;
            let block_bytes = count * channels;
            output[output_start..output_start + block_bytes]
                .copy_from_slice(&block[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += count;
        }

        while x < destination_width {
            let sample = simd_rotate_sample_coordinates(affine, source_width, source_height, x, y);
            let output_start = output_row + x * channels;
            let mut alpha = 0u8;
            if let Some(alpha_channel) = alpha_channel {
                let alpha_value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * source_width + source_x) * channels + alpha_channel]
                            as f64
                    };
                    bilinear_rotate_scalar(
                        read(sample.x0, sample.y0),
                        read(sample.x1, sample.y0),
                        read(sample.x0, sample.y1),
                        read(sample.x1, sample.y1),
                        sample.fx,
                        sample.fy,
                    )
                } else {
                    rotate_fill_sample(fill, channels, alpha_channel) as f64
                };
                alpha = alpha_value as u8;
                output[output_start + alpha_channel] = alpha;
            }

            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * source_width + source_x) * channels + channel] as f64
                    };
                    let mut p00 = read(sample.x0, sample.y0);
                    let mut p10 = read(sample.x1, sample.y0);
                    let mut p01 = read(sample.x0, sample.y1);
                    let mut p11 = read(sample.x1, sample.y1);
                    if let Some(alpha_channel) = alpha_channel {
                        let read_alpha = |source_x: usize, source_y: usize| {
                            source[(source_y * source_width + source_x) * channels + alpha_channel]
                                as f64
                        };
                        let premultiply = |value: f64, alpha: f64| {
                            (value * alpha / 255.0 + 0.5) as u8 as f64
                        };
                        p00 = premultiply(p00, read_alpha(sample.x0, sample.y0));
                        p10 = premultiply(p10, read_alpha(sample.x1, sample.y0));
                        p01 = premultiply(p01, read_alpha(sample.x0, sample.y1));
                        p11 = premultiply(p11, read_alpha(sample.x1, sample.y1));
                    }
                    bilinear_rotate_scalar(p00, p10, p01, p11, sample.fx, sample.fy)
                } else {
                    rotate_fill_sample(fill, channels, channel) as f64
                };
                let value = value as u8;
                output[output_start + channel] = if alpha_channel.is_some() && alpha != 0 {
                    (f64::from(value) * 255.0 / f64::from(alpha)) as u8
                } else {
                    value
                };
            }
            x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Produce an all-zero resize result for an empty source dimension.
///
/// Pillow accepts a zero-width/zero-height source for the typed resize paths
/// and returns a positive-sized zero image. The destination is written through
/// padded `u8x16` blocks, so this is still a SIMD data plane rather than a
/// scalar CPU fallback.
fn simd_resize_luma16(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma16(source) = img else {
        return Err(simd_unsupported("Resize"));
    };
    let source_width = usize::try_from(source.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(source.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD I;16 resize source buffer shape mismatch".into(),
        ));
    }
    let output_len = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let big_endian = luma16_resample_big_endian(mode);
    let source_values = source.as_raw();
    let mut output = vec![0u16; output_len];
    let mut vector_blocks = 0u64;

    if source_width == 0 || source_height == 0 {
        let zero = u16x8::splat(0).to_array();
        for block in output.chunks_exact_mut(zero.len()) {
            block.copy_from_slice(&zero);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output.len() % zero.len();
        if remainder != 0 {
            let start = output.len() - remainder;
            output[start..].copy_from_slice(&zero[..remainder]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if (source_width, source_height) == (output_width, output_height) {
        for (chunk, values) in output
            .chunks_exact_mut(SIMD_RESIZE_LANES)
            .zip(source_values.chunks_exact(SIMD_RESIZE_LANES))
        {
            chunk.copy_from_slice(&u16x8::new(<[u16; 8]>::try_from(values).map_err(|_| {
                simd_unsupported("Resize")
            })?)
            .to_array());
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output.len() % SIMD_RESIZE_LANES;
        if remainder != 0 {
            let start = output.len() - remainder;
            let mut padded = [0u16; SIMD_RESIZE_LANES];
            padded[..remainder].copy_from_slice(&source_values[start..]);
            output[start..].copy_from_slice(&u16x8::new(padded).to_array()[..remainder]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if matches!(filter, ResampleFilter::Nearest) {
        let x_indices = resize_nearest_indices(source.width(), output_width as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let y_indices = resize_nearest_indices(source.height(), output_height as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for (output_y, &source_y) in y_indices.iter().enumerate() {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        source_values[source_row + x_indices[output_x + lane]]
                    } else {
                        0
                    }
                });
                let packed = u16x8::new(values).to_array();
                output[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&packed[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    } else {
        let (kernel, support) = filter_from_resample(*filter);
        let horizontal = precompute_coeffs_f64(
            output_width as u32,
            source_width as u32,
            kernel,
            support,
        );
        let vertical = precompute_coeffs_f64(
            output_height as u32,
            source_height as u32,
            kernel,
            support,
        );
        let intermediate_len = source_height
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let mut intermediate = vec![0u16; intermediate_len];

        for source_y in 0..source_height {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let intermediate_row = source_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let max_count = (0..count)
                    .map(|lane| horizontal.weights[output_x + lane].len())
                    .max()
                    .unwrap_or(0);
                let mut sums = f64x8::splat(0.0);
                for tap in 0..max_count {
                    let mut values = [0.0; SIMD_RESIZE_LANES];
                    let mut weights = [0.0; SIMD_RESIZE_LANES];
                    for lane in 0..count {
                        let output_index = output_x + lane;
                        let lane_weights = horizontal
                            .weights
                            .get(output_index)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        let Some(&weight) = lane_weights.get(tap) else {
                            continue;
                        };
                        let source_x = usize::try_from(
                            *horizontal
                                .xmin
                                .get(output_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?,
                        )
                        .map_err(|_| simd_unsupported("Resize"))?
                        .checked_add(tap)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                        let source_index = source_row
                            .checked_add(source_x)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        values[lane] = f64::from(luma16_resample_read(
                            *source_values
                                .get(source_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?,
                            big_endian,
                        ));
                        weights[lane] = weight;
                    }
                    sums += f64x8::new(values) * f64x8::new(weights);
                }
                let rounded = sums
                    .to_array()
                    .map(|value| luma16_resample_write(value, big_endian));
                for lane in 0..count {
                    let index = intermediate_row
                        .checked_add(output_x + lane)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    *intermediate
                        .get_mut(index)
                        .ok_or_else(|| simd_unsupported("Resize"))? = rounded[lane];
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }

        for output_y in 0..output_height {
            let y0 = usize::try_from(
                *vertical
                    .xmin
                    .get(output_y)
                    .ok_or_else(|| simd_unsupported("Resize"))?,
            )
            .map_err(|_| simd_unsupported("Resize"))?;
            let weights = vertical
                .weights
                .get(output_y)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
            .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let mut sums = f64x8::splat(0.0);
                for (tap, &weight) in weights.iter().enumerate() {
                    let source_y = y0
                        .checked_add(tap)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let source_row = source_y
                        .checked_mul(output_width)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let values = std::array::from_fn(|lane| {
                        if lane < count {
                            f64::from(luma16_resample_read(
                                intermediate[source_row + output_x + lane],
                                big_endian,
                            ))
                        } else {
                            0.0
                        }
                    });
                    sums += f64x8::new(values) * f64x8::splat(weight);
                }
                let rounded = sums
                    .to_array()
                    .map(|value| luma16_resample_write(value, big_endian));
                output[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&u16x8::new(rounded).to_array()[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result = ImageBuffer::from_raw(output_width as u32, output_height as u32, output)
        .ok_or_else(|| PilError::InternalError("SIMD I;16 resize buffer shape mismatch".into()))?;
    Ok(preserve_mode(img, DynamicImage::ImageLuma16(result)))
}

/// Resize an `F` image with the ordinary integer geometry coefficients.
///
/// The boxed F kernel is used by `ImageOps.fit`; direct `Image.resize` needs
/// the unboxed coefficient builder used by Pillow's native F path. Keeping
/// the two builders distinct avoids f32 box-boundary rounding from creating
/// signed-zero/near-zero differences in otherwise zero pixels.
fn simd_resize_f(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD F resize source buffer shape mismatch".into(),
        ));
    }
    let output_count = output_width
        .checked_mul(output_height)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source: Vec<f32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| f32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let mut output_floats = vec![0.0f32; output_count];
    let mut vector_blocks = 0u64;

    if source_width == 0 || source_height == 0 {
        let zero = f32x8::splat(0.0).to_array();
        for block in output_floats.chunks_exact_mut(SIMD_RESIZE_LANES) {
            block.copy_from_slice(&zero);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output_floats.len() % SIMD_RESIZE_LANES;
        if remainder != 0 {
            let start = output_floats.len() - remainder;
            output_floats[start..].copy_from_slice(&zero[..remainder]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    } else if matches!(filter, ResampleFilter::Nearest) {
        let x_indices = resize_nearest_indices(img.width(), output_width as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let y_indices = resize_nearest_indices(img.height(), output_height as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for (output_y, &source_y) in y_indices.iter().enumerate() {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        source[source_row + x_indices[output_x + lane]]
                    } else {
                        0.0
                    }
                });
                output_floats[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&f32x8::new(values).to_array()[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    } else {
        let (kernel, support) = resample_kernel(filter);
        let needs_horizontal = output_width != source_width;
        let needs_vertical = output_height != source_height;
        let intermediate_len = source_height
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let mut intermediate = vec![0.0f32; intermediate_len];

        if needs_horizontal {
            let horizontal = precompute_coeffs_f64(
                output_width as u32,
                source_width as u32,
                kernel,
                support,
            );
            for source_y in 0..source_height {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let intermediate_row = source_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let max_count = (0..count)
                    .map(|lane| horizontal.weights[output_x + lane].len())
                    .max()
                    .unwrap_or(0);
                // Keep Pillow's left-to-right f64 reduction order for each
                // lane. The eight products are still calculated together;
                // reducing the vector with reassociated adds would create
                // tiny side lobes in cancellation-heavy Lanczos samples.
                let mut sums = [0.0; SIMD_RESIZE_LANES];
                for tap in 0..max_count {
                    let mut values = [0.0; SIMD_RESIZE_LANES];
                    let mut weights = [0.0; SIMD_RESIZE_LANES];
                    for lane in 0..count {
                        let output_index = output_x + lane;
                        let lane_weights = horizontal
                            .weights
                            .get(output_index)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        let Some(&weight) = lane_weights.get(tap) else {
                            continue;
                        };
                        let source_x = usize::try_from(
                            *horizontal
                                .xmin
                                .get(output_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?,
                        )
                        .map_err(|_| simd_unsupported("Resize"))?
                        .checked_add(tap)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                        let source_index = source_row
                            .checked_add(source_x)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        values[lane] = f64::from(
                            *source
                                .get(source_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?,
                        );
                        weights[lane] = weight;
                    }
                    let products = (f64x8::new(values) * f64x8::new(weights)).to_array();
                    for lane in 0..count {
                        sums[lane] += products[lane];
                    }
                }
                let values = sums.map(|value| {
                    let value = value as f32;
                    if value == 0.0 { 0.0 } else { value }
                });
                intermediate[intermediate_row + output_x..intermediate_row + output_x + count]
                    .copy_from_slice(&values[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        } else {
            // Resample.c skips an axis whose destination size already equals
            // the source size. Copy that axis in vector-sized blocks instead
            // of applying an unnecessary convolution that changes the bytes.
            for source_y in 0..source_height {
                let source_row = source_y
                    .checked_mul(source_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                let intermediate_row = source_y
                    .checked_mul(output_width)
                    .ok_or_else(|| simd_unsupported("Resize"))?;
                for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                    let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                    let values = std::array::from_fn(|lane| {
                        if lane < count {
                            source[source_row + output_x + lane]
                        } else {
                            0.0
                        }
                    });
                    intermediate[intermediate_row + output_x
                        ..intermediate_row + output_x + count]
                        .copy_from_slice(&f32x8::new(values).to_array()[..count]);
                    vector_blocks = vector_blocks.saturating_add(1);
                }
            }
        }

        if needs_vertical {
            let vertical = precompute_coeffs_f64(
                output_height as u32,
                source_height as u32,
                kernel,
                support,
            );
            for output_y in 0..output_height {
            let y0 = usize::try_from(
                *vertical
                    .xmin
                    .get(output_y)
                    .ok_or_else(|| simd_unsupported("Resize"))?,
            )
            .map_err(|_| simd_unsupported("Resize"))?;
            let weights = vertical
                .weights
                .get(output_y)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                // As in the horizontal pass, preserve the scalar tap order
                // after vector multiplication so exact Pillow bytes remain
                // stable for symmetric kernels.
                let mut sums = [0.0; SIMD_RESIZE_LANES];
                for (tap, &weight) in weights.iter().enumerate() {
                    let source_y = y0
                        .checked_add(tap)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let source_row = source_y
                        .checked_mul(output_width)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let values = std::array::from_fn(|lane| {
                        if lane < count {
                            f64::from(intermediate[source_row + output_x + lane])
                        } else {
                            0.0
                        }
                    });
                    let products = (f64x8::new(values) * f64x8::splat(weight)).to_array();
                    for lane in 0..count {
                        sums[lane] += products[lane];
                    }
                }
                let values = sums.map(|value| {
                    let value = value as f32;
                    if value == 0.0 { 0.0 } else { value }
                });
                output_floats[output_row + output_x..output_row + output_x + count]
                    .copy_from_slice(&values[..count]);
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
        } else {
            output_floats.copy_from_slice(&intermediate);
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let output: Vec<u8> = output_floats
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok(preserve_mode(img, result))
}

fn simd_resize_zero_source(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> Result<DynamicImage, PilError> {
    if !img.as_bytes().is_empty() {
        return Err(PilError::InternalError(
            "SIMD resize empty-source buffer shape mismatch".into(),
        ));
    }
    let output_len = usize::try_from(output_width)
        .ok()
        .and_then(|width| {
            usize::try_from(output_height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let zero = u8x16::splat(0).to_array();
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    for block in output.chunks_exact_mut(zero.len()) {
        block.copy_from_slice(&zero);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let remainder = output.len() % zero.len();
    if remainder != 0 {
        let start = output.len() - remainder;
        let mut padded = [0u8; 16];
        padded.copy_from_slice(&zero);
        output[start..].copy_from_slice(&padded[..remainder]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result = crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(preserve_mode(img, result))
}

/// Resize an `I` image without widening its four-byte samples into an ordinary
/// byte image. Source gathers and IEEE/sample serialization are scalar control
/// work; horizontal and vertical weighted sums use `f64x8`, and nearest output
/// values are packed through `i32x8`. This mirrors the CPU `resize_i` ordering,
/// including its rounded intermediate row.
fn simd_resize_i32(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    let source_width = usize::try_from(img.width()).map_err(|_| simd_unsupported("Resize"))?;
    let source_height = usize::try_from(img.height()).map_err(|_| simd_unsupported("Resize"))?;
    let output_width = usize::try_from(output_width).map_err(|_| simd_unsupported("Resize"))?;
    let output_height = usize::try_from(output_height).map_err(|_| simd_unsupported("Resize"))?;
    let source_len = source_width
        .checked_mul(source_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.as_bytes().len() != source_len {
        return Err(PilError::InternalError(
            "SIMD I resize source buffer shape mismatch".into(),
        ));
    }
    if source_width == 0 || source_height == 0 {
        return simd_resize_zero_source(img, output_width as u32, output_height as u32, 4);
    }
    let output_len = output_width
        .checked_mul(output_height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| simd_unsupported("Resize"))?;
    let source_values: Vec<i32> = img
        .as_bytes()
        .chunks_exact(4)
        .map(|sample| i32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]))
        .collect();
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;

    if matches!(filter, ResampleFilter::Nearest) {
        let x_indices = resize_nearest_indices(img.width(), output_width as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let y_indices = resize_nearest_indices(img.height(), output_height as u32)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        for (output_y, &source_y) in y_indices.iter().enumerate() {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let values = std::array::from_fn(|lane| {
                    if lane < count {
                        source_values[source_row + x_indices[output_x + lane]]
                    } else {
                        0
                    }
                });
                let packed = i32x8::new(values).to_array();
                for lane in 0..count {
                    let output_start = output_row
                        .checked_add(output_x + lane)
                        .and_then(|pixel| pixel.checked_mul(4))
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let output_end = output_start
                        .checked_add(4)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    output
                        .get_mut(output_start..output_end)
                        .ok_or_else(|| simd_unsupported("Resize"))?
                        .copy_from_slice(&packed[lane].to_le_bytes());
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    } else {
        // Use the same scalar kernel table as the CPU I/F path. Building the
        // coefficients is control-plane work; the sample accumulation below
        // remains entirely in the SIMD adapter.
        let (kernel, support) = resample_kernel(filter);
        let horizontal = precompute_coeffs_f64(
            output_width as u32,
            source_width as u32,
            kernel,
            support,
        );
        let vertical = precompute_coeffs_f64(
            output_height as u32,
            source_height as u32,
            kernel,
            support,
        );
        let intermediate_len = source_height
            .checked_mul(output_width)
            .ok_or_else(|| simd_unsupported("Resize"))?;
        let mut intermediate = vec![0.0f64; intermediate_len];

        for source_y in 0..source_height {
            let source_row = source_y
                .checked_mul(source_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let intermediate_row = source_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let max_count = (0..count)
                    .map(|lane| horizontal.weights[output_x + lane].len())
                    .max()
                    .unwrap_or(0);
                let mut sums = f64x8::splat(0.0);
                for tap in 0..max_count {
                    let mut values = [0.0; SIMD_RESIZE_LANES];
                    let mut weights = [0.0; SIMD_RESIZE_LANES];
                    for lane in 0..count {
                        let output_index = output_x + lane;
                        let lane_weights = horizontal
                            .weights
                            .get(output_index)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        let Some(&weight) = lane_weights.get(tap) else {
                            continue;
                        };
                        let source_x = usize::try_from(
                            *horizontal
                                .xmin
                                .get(output_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?,
                        )
                        .map_err(|_| simd_unsupported("Resize"))?
                        .checked_add(tap)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                        let source_index = source_row
                            .checked_add(source_x)
                            .ok_or_else(|| simd_unsupported("Resize"))?;
                        values[lane] = f64::from(
                            *source_values
                                .get(source_index)
                                .ok_or_else(|| simd_unsupported("Resize"))?,
                        );
                        weights[lane] = weight;
                    }
                    sums += f64x8::new(values) * f64x8::new(weights);
                }
                let values = sums.to_array();
                for lane in 0..count {
                    let index = intermediate_row
                        .checked_add(output_x + lane)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    *intermediate
                        .get_mut(index)
                        .ok_or_else(|| simd_unsupported("Resize"))? = round_up(values[lane]);
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }

        for output_y in 0..output_height {
            let y0 = usize::try_from(
                *vertical
                    .xmin
                    .get(output_y)
                    .ok_or_else(|| simd_unsupported("Resize"))?,
            )
            .map_err(|_| simd_unsupported("Resize"))?;
            let weights = vertical
                .weights
                .get(output_y)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            let output_row = output_y
                .checked_mul(output_width)
                .ok_or_else(|| simd_unsupported("Resize"))?;
            for output_x in (0..output_width).step_by(SIMD_RESIZE_LANES) {
                let count = (output_width - output_x).min(SIMD_RESIZE_LANES);
                let mut sums = f64x8::splat(0.0);
                for (tap, &weight) in weights.iter().enumerate() {
                    let source_y = y0
                        .checked_add(tap)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let source_row = source_y
                        .checked_mul(output_width)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let values = std::array::from_fn(|lane| {
                        if lane < count {
                            intermediate[source_row + output_x + lane]
                        } else {
                            0.0
                        }
                    });
                    sums += f64x8::new(values) * f64x8::splat(weight);
                }
                let rounded = sums.to_array().map(|value| round_up(value) as i32);
                let packed = i32x8::new(rounded).to_array();
                for lane in 0..count {
                    let output_start = output_row
                        .checked_add(output_x + lane)
                        .and_then(|pixel| pixel.checked_mul(4))
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    let output_end = output_start
                        .checked_add(4)
                        .ok_or_else(|| simd_unsupported("Resize"))?;
                    output
                        .get_mut(output_start..output_end)
                        .ok_or_else(|| simd_unsupported("Resize"))?
                        .copy_from_slice(&packed[lane].to_le_bytes());
                }
                vector_blocks = vector_blocks.saturating_add(1);
            }
        }
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    let result = crate::image_utils::raw_bytes_to_image(
        output_width as u32,
        output_height as u32,
        output,
        4,
    )?;
    Ok(preserve_mode(img, result))
}

pub fn simd_resize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Resize { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Resize op".into()));
    };
    if matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(img, DynamicImage::ImageLuma16(_))
    {
        if !native_resize_supported_for_image(img, *w, *h, *filter, mode) {
            return Err(simd_unsupported("Resize"));
        }
        return simd_resize_luma16(img, *w, *h, filter, mode);
    }
    if (img.width(), img.height()) == (*w, *h) {
        let result = native_copy_image_bytes(img, mode)?
            .ok_or_else(|| simd_unsupported("Resize"))?;
        return Ok(preserve_mode(img, result));
    }
    if !native_resize_supported_for_image(img, *w, *h, *filter, mode) {
        return Err(simd_unsupported("Resize"));
    }

    // `I` and `F` store one signed-integer or float sample in four bytes.
    // Their byte representation is not four independent color channels, so
    // keep them in their sample domain instead of sending them through the
    // native byte resampler.
    if mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return simd_resize_f(img, *w, *h, filter);
    }
    if mode == Some("I") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return simd_resize_i32(img, *w, *h, filter);
    }

    let (channels, premultiplied_alpha) = native_resize_byte_layout_for_image(img, mode)
        .ok_or_else(|| simd_unsupported("Resize"))?;
    if img.width() == 0 || img.height() == 0 {
        return simd_resize_zero_source(img, *w, *h, channels);
    }
    match filter {
        ResampleFilter::Nearest => simd_resize_nearest(img, *w, *h, channels),
        _ => simd_resize_convolution(
            img,
            *w,
            *h,
            *filter,
            channels,
            premultiplied_alpha,
        ),
    }
}

/// Execute `ImageOps.scale` through the native/vectorized resize kernels.
///
/// Scale contributes only scalar dimension calculation and capability
/// preflight.  The pixel data plane is delegated to `simd_resize`, which uses
/// native-copy, nearest gather, or vectorized convolution paths and never
/// retries through the CPU adapter.
pub fn simd_scale(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Scale { factor, filter } = op else {
        return Err(PilError::ValueError("expected Scale op".into()));
    };
    let (w, h) = native_scale_dimensions(img.width(), img.height(), *factor)
        .ok_or_else(|| simd_unsupported("Scale"))?;
    if !native_resize_supported_for_image(img, w, h, *filter, mode) {
        return Err(simd_unsupported("Scale"));
    }
    let resize = PipelineOp::Resize {
        w,
        h,
        filter: *filter,
    };
    match simd_resize(img, &resize, mode) {
        Err(PilError::NotImplementedError(_)) => Err(simd_unsupported("Scale")),
        result => result,
    }
}

/// Execute the nearest-neighbour subset of `Image.thumbnail`.
///
/// Aspect-ratio selection and bound clamping are scalar control work. Every
/// selected pixel is then handled by the native byte resize kernel; this
/// adapter never calls `execute_thumbnail` or retries through the CPU path.
pub fn simd_thumbnail(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Thumbnail { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Thumbnail op".into()));
    };
    if !native_thumbnail_supported_for_image(img, *w, *h, *filter, mode) {
        return Err(simd_unsupported("Thumbnail"));
    }
    let effective_filter = native_thumbnail_filter(mode, *filter);
    let (output_width, output_height) =
        native_thumbnail_dimensions(img.width(), img.height(), *w, *h)
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if (img.width(), img.height()) == (output_width, output_height) {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("Thumbnail"));
    }
    let has_alpha = native_thumbnail_has_alpha(img, mode);
    let (factor_x, factor_y) = native_thumbnail_reduction_factors(
        img.width(),
        img.height(),
        output_width,
        output_height,
        effective_filter,
        has_alpha,
    )
    .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    let typed_scalar = matches!(mode, Some("F" | "I"));
    let mut work_img = img.clone();
    if factor_x != 1 || factor_y != 1 {
        let (reduced, vector_blocks, scalar_tail) = if typed_scalar && mode == Some("F") {
            simd_thumbnail_reduce_f(img, factor_x, factor_y)?
        } else if typed_scalar && mode == Some("I") {
            simd_thumbnail_reduce_i(img, factor_x, factor_y)?
        } else {
            let channels = native_resize_byte_layout_for_image(img, mode)
                .map(|(channels, _)| channels)
                .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            let (output, width, height, vector_blocks, scalar_tail) = native_reduce_bytes(
                img,
                channels,
                native_thumbnail_has_alpha(img, mode),
                factor_x,
                factor_y,
            )
            .ok_or_else(|| simd_unsupported("Thumbnail"))?;
            (
                crate::image_utils::raw_bytes_to_image(width, height, output, channels)?,
                vector_blocks,
                scalar_tail,
            )
        };
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        work_img = reduced;
    }
    if typed_scalar && mode == Some("F") {
        return simd_resize_f(&work_img, output_width, output_height, &effective_filter);
    }
    if typed_scalar && mode == Some("I") {
        return simd_resize_i32(&work_img, output_width, output_height, &effective_filter);
    }
    let (channels, premultiplied_alpha) = native_resize_byte_layout_for_image(&work_img, mode)
        .ok_or_else(|| simd_unsupported("Thumbnail"))?;
    if factor_x != 1 || factor_y != 1 {
        let box_right = f64::from(img.width()) / f64::from(factor_x);
        let box_bottom = f64::from(img.height()) / f64::from(factor_y);
        return simd_resize_convolution_boxed(
            &work_img,
            output_width,
            output_height,
            0.0,
            0.0,
            box_right,
            box_bottom,
            effective_filter,
            channels,
            premultiplied_alpha,
        );
    }
    match effective_filter {
        ResampleFilter::Nearest => {
            simd_resize_nearest(&work_img, output_width, output_height, channels)
        }
        _ => simd_resize_convolution(
            &work_img,
            output_width,
            output_height,
            effective_filter,
            channels,
            premultiplied_alpha,
        ),
    }
}

/// Execute `ImageOps.contain` with scalar aspect-ratio control and a native
/// SIMD resize data plane. Explicit SIMD reports capability failure before any
/// pixel work when the mode, dimensions, or filter are outside this contract.
pub fn simd_contain(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Contain { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Contain op".into()));
    };
    native_aspect_resize_bytes(
        img,
        *w,
        *h,
        *filter,
        mode,
        native_pad_contained_dimensions,
        "Contain",
    )?
    .ok_or_else(|| simd_unsupported("Contain"))
}

/// Execute `ImageOps.cover` with scalar aspect-ratio control and a native SIMD
/// resize data plane. The operation intentionally retains the core Pillow
/// contract: it resizes to the covering dimensions and does not crop here.
pub fn simd_cover(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Cover { w, h, filter } = op else {
        return Err(PilError::ValueError("expected Cover op".into()));
    };
    native_aspect_resize_bytes(
        img,
        *w,
        *h,
        *filter,
        mode,
        native_cover_dimensions,
        "Cover",
    )?
    .ok_or_else(|| simd_unsupported("Cover"))
}

/// Execute `ImageOps.fit` with scalar crop-box construction and a native
/// boxed SIMD resize data plane. Indexed P/PA nearest sampling uses the
/// affine boxed path; all other admitted layouts use Pillow's fixed-point
/// two-pass boxed coefficients, including standard-mode nearest filtering.
pub fn simd_fit(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Fit {
        w,
        h,
        filter,
        bleed,
        centering,
    } = op
    else {
        return Err(PilError::ValueError("expected Fit op".into()));
    };
    if !native_fit_supported_for_image(img, *w, *h, *filter, *bleed, *centering, mode) {
        return Err(simd_unsupported("Fit"));
    }
    let output_width = (*w).max(1);
    let output_height = (*h).max(1);
    let (box_left, box_top, box_right, box_bottom) = native_fit_box(
        img.width(),
        img.height(),
        output_width,
        output_height,
        *bleed,
        *centering,
    )
    .ok_or_else(|| simd_unsupported("Fit"))?;
    if mode == Some("F") && matches!(img, DynamicImage::ImageRgba8(_)) {
        return simd_resize_f_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            *filter,
        );
    }
    let (channels, premultiplied_alpha) = native_fit_layout_for_image(img, mode)
        .ok_or_else(|| simd_unsupported("Fit"))?;
    if img.width() == 0 {
        let output_len = usize::try_from(output_width)
            .ok()
            .and_then(|width| usize::try_from(output_height).ok()?.checked_mul(width))
            .and_then(|pixels| pixels.checked_mul(channels))
            .ok_or_else(|| simd_unsupported("Fit"))?;
        let mut output = vec![0u8; output_len];
        let zero_block = u8x16::splat(0).to_array();
        let mut vector_blocks = 0u64;
        for block in output.chunks_exact_mut(zero_block.len()) {
            block.copy_from_slice(&zero_block);
            vector_blocks = vector_blocks.saturating_add(1);
        }
        let remainder = output.len() % zero_block.len();
        if remainder != 0 {
            let start = output.len() - remainder;
            output[start..].copy_from_slice(&zero_block[..remainder]);
        }
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(u64::from(remainder != 0));
        let result = crate::image_utils::raw_bytes_to_image(
            output_width,
            output_height,
            output,
            channels,
        )?;
        return Ok(preserve_mode(img, result));
    }
    let resize_filter = native_fit_filter(mode, *filter);
    if matches!(resize_filter, ResampleFilter::Nearest)
        && matches!(mode, Some("P") | Some("PA"))
    {
        return simd_resize_nearest_boxed(
            img,
            output_width,
            output_height,
            box_left,
            box_top,
            box_right,
            box_bottom,
            channels,
        );
    }
    simd_resize_convolution_boxed(
        img,
        output_width,
        output_height,
        box_left,
        box_top,
        box_right,
        box_bottom,
        resize_filter,
        channels,
        premultiplied_alpha,
    )
}

/// Execute the native byte affine subset of `Image.transform`.
/// Scalar coordinate construction and bounds checks form the control plane;
/// every output group is packed and stored through a native SIMD byte block.
pub fn simd_transform(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        fill,
        palette_fill: _,
    } = op
    else {
        return Err(PilError::ValueError("expected Transform op".into()));
    };
    let luma16_supported = native_affine_luma16_transform_supported_for_image(
        img, *w, *h, method, data, mode,
    );
    let supported = match method {
        TransformMethod::Affine => {
            luma16_supported
                || native_affine_nearest_transform_supported_for_image(
                    img, *w, *h, method, data, *filter, mode,
                )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            native_projective_nearest_transform_supported_for_image(
                img, *w, *h, method, data, *filter, mode,
            )
        }
        TransformMethod::Mesh => native_mesh_transform_supported_for_image(
            img, *w, *h, data, *filter, mode,
        ),
    };
    if !supported {
        return Err(simd_unsupported("Transform"));
    }
    if luma16_supported {
        return simd_affine_luma16_transform_bytes(img, *w, *h, data, *fill, mode)?
            .ok_or_else(|| simd_unsupported("Transform"));
    }
    match method {
        TransformMethod::Affine => match filter {
            ResampleFilter::Nearest => simd_affine_nearest_transform_bytes(
                img, *w, *h, data, *fill, mode,
            )?
            .ok_or_else(|| simd_unsupported("Transform")),
            ResampleFilter::Bilinear => simd_affine_bilinear_transform_bytes(
                img, *w, *h, data, *fill, mode,
            )?
            .ok_or_else(|| simd_unsupported("Transform")),
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Perspective => match filter {
            ResampleFilter::Nearest => simd_projective_nearest_transform_bytes(
                img, *w, *h, data, *fill, mode, false,
            )?
            .ok_or_else(|| simd_unsupported("Transform")),
            ResampleFilter::Bilinear => simd_projective_bilinear_transform_bytes(
                img, *w, *h, data, *fill, mode, false,
            )?
            .ok_or_else(|| simd_unsupported("Transform")),
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Quad => match filter {
            ResampleFilter::Nearest => simd_projective_nearest_transform_bytes(
                img, *w, *h, data, *fill, mode, true,
            )?
            .ok_or_else(|| simd_unsupported("Transform")),
            ResampleFilter::Bilinear => simd_projective_bilinear_transform_bytes(
                img, *w, *h, data, *fill, mode, true,
            )?
            .ok_or_else(|| simd_unsupported("Transform")),
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Mesh => simd_mesh_transform_bytes(
            img, *w, *h, data, *fill, mode,
        )?
        .ok_or_else(|| simd_unsupported("Transform")),
    }
}

/// Execute `ImageOps.pad` with a scalar contain/centering control plane and
/// native-layout SIMD resize, fill, and row-copy data planes. Explicit SIMD
/// never retries the CPU pad implementation when this contract is not met.
pub fn simd_pad(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Pad {
        w,
        h,
        filter,
        color,
        centering,
    } = op
    else {
        return Err(PilError::ValueError("expected Pad op".into()));
    };
    let Some((result, vector_blocks, scalar_tail)) =
        native_pad_bytes(img, *w, *h, *filter, *color, *centering, mode)?
    else {
        return Err(simd_unsupported("Pad"));
    };
    if vector_blocks != 0 || scalar_tail != 0 {
        crate::compute::record_pipeline_operation_path("vector");
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    Ok(result)
}

pub fn simd_reduce(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Reduce {
        x_factor,
        y_factor,
    } = op
    else {
        return Err(PilError::ValueError("expected Reduce op".into()));
    };
    if *x_factor == 0 || *y_factor == 0 {
        return Err(simd_unsupported("Reduce"));
    }
    if *x_factor == 1 && *y_factor == 1 {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("Reduce"));
    }
    let (channels, premultiplied_alpha) =
        native_reduce_layout(img, mode).ok_or_else(|| simd_unsupported("Reduce"))?;
    let Some((output, width, height, vector_blocks, scalar_tail)) =
        native_reduce_bytes(img, channels, premultiplied_alpha, *x_factor, *y_factor)
    else {
        return Err(simd_unsupported("Reduce"));
    };
    if vector_blocks == 0 {
        return Err(simd_unsupported("Reduce"));
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
}

pub fn simd_box_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = simd_native_blur_channels(img, mode)
        .ok_or_else(|| simd_unsupported("BoxBlur"))?;
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    if pixel_count == 0 {
        return Err(simd_unsupported("BoxBlur"));
    }
    match op {
        PipelineOp::BoxBlur { radius } if *radius == 0 => native_copy_image_bytes(img, mode)?
            .ok_or_else(|| simd_unsupported("BoxBlur")),
        PipelineOp::BoxBlur { radius } => simd_pil_box_blur(img, *radius as f32, 1, channels),
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } if *radius_x == 0.0 && *radius_y == 0.0 => native_copy_image_bytes(img, mode)?
            .ok_or_else(|| simd_unsupported("BoxBlur")),
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => simd_pil_box_blur_xy(img, *radius_x, *radius_y, *passes, channels),
        _ => Err(PilError::ValueError("expected BoxBlur op".into())),
    }
}

pub fn simd_gaussian_blur(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if let PipelineOp::GaussianBlur { sigma } = op {
        let channels = simd_native_blur_channels(img, mode)
            .ok_or_else(|| simd_unsupported("GaussianBlur"))?;
        let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
        if !sigma.is_finite() || pixel_count == 0 {
            return Err(simd_unsupported("GaussianBlur"));
        }
        let sigma = sigma.abs();
        if sigma == 0.0 {
            return native_copy_image_bytes(img, mode)?
                .ok_or_else(|| simd_unsupported("GaussianBlur"));
        }
        let blur_radius = gaussian_blur_radius(sigma)
            .ok_or_else(|| simd_unsupported("GaussianBlur"))?;
        if blur_radius <= 0.0 {
            return Err(simd_unsupported("GaussianBlur"));
        }
        return simd_pil_box_blur(img, blur_radius, 3, channels);
    }
    Err(PilError::ValueError("expected GaussianBlur op".into()))
}

// ═══════════════════════════════════════════════════════════════════════
// Section E: Dual-image per-pixel ops (Add, Subtract, Multiply, ...)
// ═══════════════════════════════════════════════════════════════════════

macro_rules! native_dual_op_adapter {
    ($name:ident, $variant:ident, $native:path) => {
        pub fn $name(
            img: &DynamicImage,
            op: &PipelineOp,
            mode: Option<&str>,
        ) -> Result<DynamicImage, PilError> {
            let PipelineOp::$variant { other } = op else {
                return Err(PilError::ValueError(
                    concat!("expected ", stringify!($variant), " op").into(),
                ));
            };
            simd_native_chops(img, other, mode, $native)
        }
    };
}

pub fn simd_multiply(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Multiply { other } = op else {
        return Err(PilError::ValueError("expected Multiply op".into()));
    };
        simd_native_chops(
        img,
        other,
        mode,
        |image, operand, current_mode| native_chops_blend(image, operand, current_mode, false),
    )
}

pub fn simd_screen(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Screen { other } = op else {
        return Err(PilError::ValueError("expected Screen op".into()));
    };
        simd_native_chops(
        img,
        other,
        mode,
        |image, operand, current_mode| native_chops_blend(image, operand, current_mode, true),
    )
}

pub fn simd_overlay(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Overlay { other } = op else {
        return Err(PilError::ValueError("expected Overlay op".into()));
    };
    simd_native_chops(img, other, mode, native_chops_overlay)
}

pub fn simd_hard_light(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::HardLight { other } = op else {
        return Err(PilError::ValueError("expected HardLight op".into()));
    };
    simd_native_chops(img, other, mode, native_chops_hard_light)
}

pub fn simd_soft_light(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::SoftLight { other } = op else {
        return Err(PilError::ValueError("expected SoftLight op".into()));
    };
    simd_native_chops(img, other, mode, native_chops_soft_light)
}

pub fn simd_blend_module(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::BlendModule { other, alpha } = op else {
        return Err(PilError::ValueError("expected BlendModule op".into()));
    };
    if !alpha.is_finite() {
        return Err(simd_unsupported("BlendModule"));
    }
    let other_mode = other.mode()?;
    let other_img = materialize_chops_operand(other, mode)?;
    native_module_blend(img, &other_img, mode, Some(&other_mode), *alpha)
        .ok_or_else(|| simd_unsupported("BlendModule"))
}

native_dual_op_adapter!(
    simd_darker,
    Darker,
    native_chops_darker
);
native_dual_op_adapter!(
    simd_lighter,
    Lighter,
    native_chops_lighter
);
native_dual_op_adapter!(
    simd_difference,
    Difference,
    native_chops_difference
);
native_dual_op_adapter!(
    simd_add_modulo,
    AddModulo,
    native_chops_add_modulo
);
native_dual_op_adapter!(
    simd_subtract_modulo,
    SubtractModulo,
    native_chops_subtract_modulo
);
native_dual_op_adapter!(
    simd_logical_and,
    LogicalAnd,
    native_chops_logical_and
);
native_dual_op_adapter!(
    simd_logical_or,
    LogicalOr,
    native_chops_logical_or
);
native_dual_op_adapter!(
    simd_logical_xor,
    LogicalXor,
    native_chops_logical_xor
);

pub fn simd_add(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Add {
        other,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Add op".into()));
    };
    let other_img = materialize_chops_operand(other, mode)?;
    // Keep ordinary byte images in their native interleaved layout for both
    // the default and scaled/offset contracts. The arithmetic helper follows
    // Pillow's `(left + right) / scale + offset` order before clamping.
    if *scale == 1.0
        && *offset == 0.0
        && native_chops_layout(img, mode)
            .is_some_and(|channels| has_vectorized_byte_rows(img, channels))
    {
        if let Some(result) = native_chops_add_clamped(img, &other_img, mode) {
            return Ok(result);
        }
    }
    if *scale == 1.0 && *offset == 0.0 {
        if let Some(result) = native_chops_affine(img, &other_img, mode, 1.0, 0.0, false) {
            return Ok(result);
        }
    } else if let Some(result) = native_chops_affine(img, &other_img, mode, *scale, *offset, false) {
        return Ok(result);
    }
    Err(simd_unsupported("Add"))
}

pub fn simd_subtract(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Subtract {
        other,
        scale,
        offset,
    } = op
    else {
        return Err(PilError::ValueError("expected Subtract op".into()));
    };
    let other_img = materialize_chops_operand(other, mode)?;
    // For both default and scaled parameters use native bytes only when the
    // representation and public mode are an exact match. The arithmetic
    // helper retains Pillow's subtraction/division/addition ordering.
    if *scale == 1.0
        && *offset == 0.0
        && native_chops_layout(img, mode)
            .is_some_and(|channels| has_vectorized_byte_rows(img, channels))
    {
        if let Some(result) = native_chops_subtract_clamped(img, &other_img, mode) {
            return Ok(result);
        }
    }
    if *scale == 1.0 && *offset == 0.0 {
        if let Some(result) = native_chops_affine(img, &other_img, mode, 1.0, 0.0, true) {
            return Ok(result);
        }
    } else if let Some(result) = native_chops_affine(img, &other_img, mode, *scale, *offset, true) {
        return Ok(result);
    }
    Err(simd_unsupported("Subtract"))
}

// ═══════════════════════════════════════════════════════════════════════
// Section F: Ops that change dimensions (return new pixel buffer)
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_transpose(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Transpose { method } = op else {
        return Err(PilError::ValueError("expected Transpose op".into()));
    };
    if let Some((result, vector_blocks, scalar_tail)) = native_transpose(img, mode, method.clone()) {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(result);
    }
    Err(simd_unsupported("Transpose"))
}

// ═══════════════════════════════════════════════════════════════════════
// Section G: New-buffer ops with PipelineOp dispatch
// ═══════════════════════════════════════════════════════════════════════

/// Copy a validated crop directly in the source's native byte layout.
///
/// This is intentionally a `native-copy` kernel rather than arithmetic SIMD:
/// the useful optimization is preserving the interleaved L/LA/RGB/RGBA
/// buffer and copying complete row spans without an RGBA expansion.
fn native_crop_bytes(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    mode: Option<&str>,
    allow_empty: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_copy_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    if left > right || top > bottom || right > image_width || bottom > image_height {
        return Ok(None);
    }
    let width = right - left;
    let height = bottom - top;
    if width == 0 || height == 0 {
        if !allow_empty {
            return Ok(None);
        }
        crate::compute::record_pipeline_operation_path("native-copy");
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            width,
            height,
            Vec::new(),
            channels,
        )
        .map(Some);
    }
    let source_stride = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD crop source stride overflow".into()))?;
    let output_stride = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD crop output stride overflow".into()))?;
    let output_len = output_stride
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD crop output length overflow".into()))?;
    let mut output = vec![0u8; output_len];
    let source = img.as_bytes();
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..height as usize {
        let source_start = (top as usize + y)
            .checked_mul(source_stride)
            .and_then(|offset| offset.checked_add(left as usize * channels))
            .ok_or_else(|| PilError::ValueError("SIMD crop source offset overflow".into()))?;
        let output_start = y * output_stride;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_start + output_stride],
            &mut output[output_start..output_start + output_stride],
        )
        .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels).map(Some)
}

pub fn simd_crop_border(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::CropBorder { border } = op else {
        return Err(PilError::ValueError("expected CropBorder op".into()));
    };
    // CropBorder is a contiguous byte movement, not an arithmetic SIMD
    // workload. On ordinary native layouts, avoid widening every source
    // pixel to the packed RGBA representation only to copy it back out.
    if native_copy_layout(img, mode).is_some() {
        let (w, h) = img.dimensions();
        // Avoid the packed adapter's representation conversion while
        // preserving ImageOps.crop's public error text.  The checked
        // half-size form is equivalent to Pillow's `2 * border > size`
        // rule without allowing a u32 multiplication to wrap.
        if *border > w / 2 {
            return Err(PilError::ValueError(
                "Coordinate 'right' is less than 'left'".into(),
            ));
        }
        if *border > h / 2 {
            return Err(PilError::ValueError(
                "Coordinate 'lower' is less than 'upper'".into(),
            ));
        }
        if let Some(result) = native_crop_bytes(
            img,
            *border,
            *border,
            w - *border,
            h - *border,
            mode,
            true,
        )? {
            return Ok(result);
        }
    }
    Err(simd_unsupported("CropBorder"))
}

pub fn simd_crop(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Crop {
        left,
        top,
        right,
        bottom,
    } = op
    else {
        return Err(PilError::ValueError("expected Crop op".into()));
    };
    // Crop is a byte movement, not an arithmetic SIMD workload. The native
    // implementation copies rows directly for ordinary byte layouts; typed
    // and empty-output layouts are rejected by contextual preflight instead
    // of entering a packed scalar implementation.
    if let Some(result) = native_crop_bytes(img, *left, *top, *right, *bottom, mode, false)? {
        return Ok(result);
    }
    Err(simd_unsupported("Crop"))
}

/// Assemble one complete native output block from independent L bands.
///
/// `Image.merge` is an interleave operation: its scalar control plane chooses
/// the source band for each output channel, while these fixed shuffle masks
/// perform the hot byte rearrangement in `u8x16`. RGB uses four pixels per
/// block because twelve output bytes fit in one vector; the unused lanes are
/// never stored. There is no packed-RGBA conversion here, so CMYK's fourth
/// sample remains K throughout the copy.
fn native_merge_vector_block(
    bands: &[Vec<u8>],
    pixel: usize,
    channels: usize,
) -> Option<(usize, [u8; 16])> {
    let pixels_per_vector = match channels {
        1 => 16,
        2 => 8,
        3 | 4 => 4,
        _ => return None,
    };
    let mut packed = [0u8; 16];
    for channel in 0..channels {
        let source = bands.get(channel)?.get(pixel..pixel + pixels_per_vector)?;
        let start = channel * pixels_per_vector;
        packed[start..start + pixels_per_vector].copy_from_slice(source);
    }
    let indices = match channels {
        1 => [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
        ],
        2 => [
            0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15,
        ],
        3 => [
            0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11, 12, 13, 14, 15,
        ],
        4 => [
            0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15,
        ],
        _ => return None,
    };
    Some((pixels_per_vector * channels, u8x16::new(packed)
        .swizzle_relaxed(u8x16::new(indices))
        .to_array()))
}

fn native_merge_luma_band(band: &Image) -> Result<Option<Vec<u8>>, PilError> {
    let materialized = band.materialize_for_ops()?;
    Ok(match materialized {
        DynamicImage::ImageLuma8(image) => Some(image.into_raw()),
        _ => None,
    })
}

/// Merge validated L bands into the target's native byte layout.
///
/// The source bands are necessarily separate buffers, so one output buffer is
/// required for the interleaved result. Apart from that required result
/// allocation, the operation does not widen samples or materialize through a
/// color conversion. Complete channel groups use vector shuffles; only the
/// final incomplete group is scalar.
fn native_merge_bytes(
    img: &DynamicImage,
    target_mode: &ColorMode,
    bands: &[Image],
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    let Some((channels, pixels)) = native_merge_contract_for_image(img, target_mode, bands, mode)
    else {
        return Ok(None);
    };
    let width = img.width();
    let height = img.height();
    let mut band_bytes = Vec::with_capacity(channels);
    band_bytes.push(img.as_bytes().to_vec());
    for band in bands.iter().skip(1) {
        let Some(bytes) = native_merge_luma_band(band)? else {
            return Ok(None);
        };
        if bytes.len() != pixels {
            return Ok(None);
        }
        band_bytes.push(bytes);
    }

    let output_len = pixels
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD merge output length overflow".into()))?;
    let mut output = vec![0u8; output_len];
    let pixels_per_vector = match channels {
        1 => 16,
        2 => 8,
        3 | 4 => 4,
        _ => return Ok(None),
    };
    let mut pixel = 0usize;
    let mut vector_blocks = 0u64;
    while pixel + pixels_per_vector <= pixels {
        let (block_bytes, block) = native_merge_vector_block(&band_bytes, pixel, channels)
            .ok_or_else(|| PilError::InternalError("SIMD merge vector shape mismatch".into()))?;
        let output_start = pixel * channels;
        output[output_start..output_start + block_bytes]
            .copy_from_slice(&block[..block_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
        pixel += pixels_per_vector;
    }
    for output_pixel in pixel..pixels {
        let output_start = output_pixel * channels;
        for channel in 0..channels {
            output[output_start + channel] = band_bytes[channel][output_pixel];
        }
    }
    let scalar_tail = ((pixels - pixel) * channels) as u64;
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels)?;
    Ok(Some((result, vector_blocks, scalar_tail)))
}

pub fn simd_merge(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Merge {
        mode: target_mode,
        bands,
    } = op
    else {
        return Err(PilError::ValueError("expected Merge op".into()));
    };
    let Some((result, vector_blocks, scalar_tail)) =
        native_merge_bytes(img, target_mode, bands, mode)?
    else {
        return Err(simd_unsupported("Merge"));
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(result)
}

#[inline]
fn native_expand_fill_sample(fill: (u8, u8, u8, u8), channels: usize, index: usize) -> u8 {
    match channels {
        1 => fill.0,
        2 => {
            if index % 2 == 0 {
                fill.0
            } else {
                fill.3
            }
        }
        3 => match index % 3 {
            0 => fill.0,
            1 => fill.1,
            _ => fill.2,
        },
        4 => match index % 4 {
            0 => fill.0,
            1 => fill.1,
            2 => fill.2,
            _ => fill.3,
        },
        _ => 0,
    }
}

fn native_fill_row(
    row: &mut [u8],
    fill: (u8, u8, u8, u8),
    channels: usize,
) -> Option<(u64, u64)> {
    if !(1..=4).contains(&channels) {
        return None;
    }
    let vector_len = row.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for start in (0..vector_len).step_by(16) {
        let block = u8x16::new(std::array::from_fn(|lane| {
            native_expand_fill_sample(fill, channels, start + lane)
        }));
        row[start..start + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let scalar_tail = row.len() - vector_len;
    if scalar_tail != 0 {
        let block = u8x16::new(std::array::from_fn(|lane| {
            native_expand_fill_sample(fill, channels, vector_len + lane)
        }));
        row[vector_len..].copy_from_slice(&block.to_array()[..scalar_tail]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, scalar_tail as u64))
}

/// Expand an image in its native byte layout using vectorized fill and row
/// copies. The output border is intentionally handled as two data-plane
/// kernels: a repeated `u8x16` fill pattern, followed by direct source-row
/// copies. This keeps P/PA indices raw and keeps CMYK's K sample in the fourth
/// byte; no intermediate RGBA image is created.
fn native_expand_bytes(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    let Some((channels, output_width, output_height)) =
        native_expand_contract_for_image(img, border, mode)
    else {
        return Ok(None);
    };
    let source_width = img.width() as usize;
    let source_height = img.height() as usize;
    let output_width_usize = output_width as usize;
    let output_height_usize = output_height as usize;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand source stride overflow".into()))?;
    let output_stride = output_width_usize
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand output stride overflow".into()))?;
    let output_len = output_stride
        .checked_mul(output_height_usize)
        .ok_or_else(|| PilError::ValueError("SIMD expand output length overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_stride * source_height {
        return Ok(None);
    }
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..output_height_usize {
        let output_start = y * output_stride;
        let (blocks, tail) = native_fill_row(
            &mut output[output_start..output_start + output_stride],
            fill,
            channels,
        )
        .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let border_usize = border as usize;
    for y in 0..source_height {
        let source_start = y * source_stride;
        let output_start = (y + border_usize) * output_stride + border_usize * channels;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_start + source_stride],
            &mut output[output_start..output_start + source_stride],
        )
        .ok_or_else(|| PilError::InternalError("SIMD expand copy shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        output_width,
        output_height,
        output,
        channels,
    )?;
    Ok(Some((result, vector_blocks, scalar_tail)))
}

pub fn simd_expand(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Expand { border, fill } = op else {
        return Err(PilError::ValueError("expected Expand op".into()));
    };
    let Some((result, vector_blocks, scalar_tail)) =
        native_expand_bytes(img, *border, *fill, mode)?
    else {
        return Err(simd_unsupported("Expand"));
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(result)
}

/// Fill the ImageChops.constant result with a repeated SIMD byte block.
///
/// Pillow always returns a new single-band `L` image for this operation; the
/// source samples and logical source mode do not affect the result. A partial
/// final block is loaded and stored through the vector type, with only the
/// unwritten lanes discarded at the boundary.
pub fn simd_constant(
    img: &DynamicImage,
    op: &PipelineOp,
    _mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Constant { value } = op else {
        return Err(PilError::ValueError("expected Constant op".into()));
    };
    let width = img.width();
    let height = img.height();
    if width == 0 || height == 0 {
        return Err(simd_unsupported("Constant"));
    }
    let output_len = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD constant output length overflow".into()))?;
    if output_len == 0 {
        return Err(simd_unsupported("Constant"));
    }
    let mut output = vec![0u8; output_len];
    let vector_len = output_len / 16 * 16;
    let block = u8x16::splat(*value).to_array();
    for start in (0..vector_len).step_by(16) {
        output[start..start + 16].copy_from_slice(&block);
    }
    let scalar_tail = output_len - vector_len;
    if scalar_tail != 0 {
        output[vector_len..].copy_from_slice(&block[..scalar_tail]);
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(
        (vector_len / 16 + usize::from(scalar_tail != 0)) as u64,
    );
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::image_utils::raw_bytes_to_image(width, height, output, 1)
}

/// Duplicate an image through its native byte layout.
///
/// Unlike the packed scalar adapter, this preserves raw P/PA samples and the
/// fourth CMYK byte. The copy kernel also handles a short final block without
/// routing the operation through a CPU pixel loop.
pub fn simd_duplicate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::Duplicate) {
        return Err(PilError::ValueError("expected Duplicate op".into()));
    }
    let Some(channels) = native_copy_layout(img, mode) else {
        return Err(simd_unsupported("Duplicate"));
    };
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Err(simd_unsupported("Duplicate"));
    }
    let expected_len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD duplicate image length overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != expected_len {
        return Err(simd_unsupported("Duplicate"));
    }
    let mut output = vec![0u8; expected_len];
    let (vector_blocks, scalar_tail) = copy_native_bytes(source, &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD duplicate buffer shape mismatch".into()))?;
    let result = crate::image_utils::raw_bytes_to_image(width, height, output, channels)?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(preserve_mode(img, result))
}

/// Copy an identity-like operation in the source's native byte layout.
///
/// Callers validate their complete public contract before this fast path is
/// selected. Once that validation proves that every output pixel maps to the
/// same source pixel, the data plane is simply a contiguous copy. Keep it as
/// a real SIMD operation so strict SIMD never enters a scalar implementation
/// by accident.
fn native_copy_image_bytes(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_copy_layout(img, mode) else {
        return Ok(None);
    };
    let (width, height) = img.dimensions();
    let Some(expected_len) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return Ok(None);
    };
    let source = img.as_bytes();
    if expected_len < 16 || source.len() != expected_len {
        return Ok(None);
    }

    let mut output = vec![0u8; source.len()];
    let (vector_blocks, scalar_tail) = copy_native_bytes(source, &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD identity copy shape mismatch".into()))?;

    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels).map(Some)
}

/// Copy an identity-like operation through the native vector type when the
/// payload is shorter than one full vector. The regular identity helper keeps
/// its sixteen-byte admission threshold for operations whose capability
/// contract requires a complete block; EffectSpread distance 0/1 is a
/// different contract and must remain SIMD-capable for tiny images so its
/// process-global RNG consumption stays aligned with Pillow.
fn native_copy_image_bytes_allow_short(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_copy_layout(img, mode) else {
        return Ok(None);
    };
    let (width, height) = img.dimensions();
    let Some(expected_len) = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return Ok(None);
    };
    let source = img.as_bytes();
    if expected_len == 0 || source.len() != expected_len {
        return Ok(None);
    }

    let mut output = vec![0u8; source.len()];
    let (vector_blocks, scalar_tail) = copy_native_bytes(source, &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD short identity copy shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width, height, output, channels).map(Some)
}

fn native_identity_rotate(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    native_copy_image_bytes(img, mode)
}

/// Execute the exact nearest-neighbor affine contract for ordinary byte
/// layouts. Coordinate progression follows Pillow's signed 16.16 affine
/// sampler; each complete group of pixels is gathered into native `u8x16`
/// blocks and written without an RGBA expansion.
fn simd_nearest_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    if destination_width == 0 || destination_height == 0 {
        return Ok(None);
    }
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let mut output = vec![0u8; destination_len];
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let fixed = |value: f64| (value.mul_add(65_536.0, 0.5).floor()) as i64;
    let [a, b, c, d, e, f] = geometry.affine;
    let step_x_x = fixed(a);
    let step_y_x = fixed(b);
    let step_x_y = fixed(d);
    let step_y_y = fixed(e);
    let origin_x = fixed(c + a * 0.5 + b * 0.5);
    let origin_y = fixed(f + d * 0.5 + e * 0.5);
    let pixels_per_vector = (SIMD_RANK_FILTER_LANES / channels).max(1);
    let block_bytes = pixels_per_vector * channels;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;

    let destination_pixels = destination_width * destination_height;
    let mut destination_index = 0usize;
    while destination_index + pixels_per_vector <= destination_pixels {
        let mut block = [0u8; SIMD_RANK_FILTER_LANES];
        for pixel in 0..pixels_per_vector {
            let index = destination_index + pixel;
            let destination_y = index / destination_width;
            let destination_x = index % destination_width;
            let source_x = origin_x
                + destination_x as i64 * step_x_x
                + destination_y as i64 * step_y_x;
            let source_y = origin_y
                + destination_x as i64 * step_x_y
                + destination_y as i64 * step_y_y;
            let input_x = source_x >> 16;
            let input_y = source_y >> 16;
            let block_start = pixel * channels;
            if input_x >= 0
                && input_x < width as i64
                && input_y >= 0
                && input_y < height as i64
            {
                let source_start = (input_y as usize * width + input_x as usize) * channels;
                block[block_start..block_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            } else {
                for channel in 0..channels {
                    block[block_start + channel] = rotate_fill_sample(fill, channels, channel);
                }
            }
        }
        let output_start = destination_index * channels;
        output[output_start..output_start + block_bytes]
            .copy_from_slice(&u8x16::new(block).to_array()[..block_bytes]);
        vector_blocks = vector_blocks.saturating_add(1);
        destination_index += pixels_per_vector;
    }
    while destination_index < destination_pixels {
        let destination_y = destination_index / destination_width;
        let destination_x = destination_index % destination_width;
        let source_x = origin_x
            + destination_x as i64 * step_x_x
            + destination_y as i64 * step_y_x;
        let source_y = origin_y
            + destination_x as i64 * step_x_y
            + destination_y as i64 * step_y_y;
        let input_x = source_x >> 16;
        let input_y = source_y >> 16;
        let output_start = destination_index * channels;
        if input_x >= 0
            && input_x < width as i64
            && input_y >= 0
            && input_y < height as i64
        {
            let source_start = (input_y as usize * width + input_x as usize) * channels;
            output[output_start..output_start + channels]
                .copy_from_slice(&source[source_start..source_start + channels]);
        } else {
            for channel in 0..channels {
                output[output_start + channel] = rotate_fill_sample(fill, channels, channel);
            }
        }
        destination_index += 1;
        scalar_tail = scalar_tail.saturating_add(1);
    }

    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(
        geometry.width,
        geometry.height,
        output,
        channels,
    )
    .map(|result| Some(preserve_mode(img, result)))
}

#[derive(Clone, Copy, Default)]
struct SimdRotateSample {
    valid: bool,
    x0: usize,
    x1: usize,
    y0: usize,
    y1: usize,
    fx: f64,
    fy: f64,
}

#[inline]
fn simd_rotate_sample_coordinates(
    affine: [f64; 6],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
) -> SimdRotateSample {
    simd_rotate_sample_coordinates_with_mode(affine, width, height, x, y, false)
}

#[inline]
fn simd_rotate_sample_coordinates_with_mode(
    affine: [f64; 6],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    pa_mode: bool,
) -> SimdRotateSample {
    let [a, b, c, d, e, f] = affine;
    let source_x = a * (x as f64 + 0.5) + b * (y as f64 + 0.5) + c;
    let source_y = d * (x as f64 + 0.5) + e * (y as f64 + 0.5) + f;
    let (source_x, source_y) = if pa_mode {
        (source_x, source_y)
    } else {
        (source_x - 0.5, source_y - 0.5)
    };
    let outside = if pa_mode {
        source_x < 0.0
            || source_x >= width as f64
            || source_y < 0.0
            || source_y >= height as f64
    } else {
        source_x < -0.5
            || source_x >= width as f64 - 0.5
            || source_y < -0.5
            || source_y >= height as f64 - 0.5
    };
    if outside {
        return SimdRotateSample::default();
    }
    let source_x = if pa_mode {
        source_x
    } else {
        source_x.clamp(0.0, width as f64 - 1.0)
    };
    let source_y = if pa_mode {
        source_y
    } else {
        source_y.clamp(0.0, height as f64 - 1.0)
    };
    let x0 = source_x.floor() as usize;
    let y0 = source_y.floor() as usize;
    SimdRotateSample {
        valid: true,
        x0,
        x1: (x0 + 1).min(width - 1),
        y0,
        y1: (y0 + 1).min(height - 1),
        fx: source_x - x0 as f64,
        fy: source_y - y0 as f64,
    }
}

#[inline]
fn bilinear_rotate_scalar(p00: f64, p10: f64, p01: f64, p11: f64, fx: f64, fy: f64) -> f64 {
    (1.0 - fy) * ((1.0 - fx) * p00 + fx * p10)
        + fy * ((1.0 - fx) * p01 + fx * p11)
}

#[inline]
fn bilinear_rotate_vector(
    p00: f64x8,
    p10: f64x8,
    p01: f64x8,
    p11: f64x8,
    fx: f64x8,
    fy: f64x8,
) -> f64x8 {
    let one = f64x8::splat(1.0);
    (one - fy) * ((one - fx) * p00 + fx * p10)
        + fy * ((one - fx) * p01 + fx * p11)
}

/// Gather a channel's four bilinear neighbors in native byte storage.
///
/// `alpha_channel` is present only for a color channel in LA/RGBA. Source
/// samples are premultiplied with a vector multiply before the required
/// per-lane byte rounding. Invalid lanes retain the unpremultiplied fill
/// value, matching Pillow's fill-then-unpremultiply sequence.
#[inline]
fn gather_rotate_neighborhood_vectors(
    source: &[u8],
    width: usize,
    channels: usize,
    channel: usize,
    fill: (u8, u8, u8, u8),
    samples: &[SimdRotateSample; SIMD_F64_LANES],
    alpha_channel: Option<usize>,
) -> [f64x8; 4] {
    let fill_value = rotate_fill_sample(fill, channels, channel) as f64;
    let mut values = [[fill_value; SIMD_F64_LANES]; 4];
    let mut alphas = [[0.0; SIMD_F64_LANES]; 4];
    let premultiply = alpha_channel.is_some_and(|alpha| alpha != channel);
    for (lane, sample) in samples.iter().enumerate() {
        if !sample.valid {
            if let Some(alpha_channel) = alpha_channel {
                let alpha_fill = rotate_fill_sample(fill, channels, alpha_channel) as f64;
                alphas[0][lane] = alpha_fill;
                alphas[1][lane] = alpha_fill;
                alphas[2][lane] = alpha_fill;
                alphas[3][lane] = alpha_fill;
            }
            continue;
        }
        let coordinates = [
            (sample.x0, sample.y0),
            (sample.x1, sample.y0),
            (sample.x0, sample.y1),
            (sample.x1, sample.y1),
        ];
        for (neighbor, &(source_x, source_y)) in coordinates.iter().enumerate() {
            let index = (source_y * width + source_x) * channels;
            values[neighbor][lane] = source[index + channel] as f64;
            if let Some(alpha_channel) = alpha_channel {
                alphas[neighbor][lane] = source[index + alpha_channel] as f64;
            }
        }
    }
    if premultiply {
        let divisor = f64x8::splat(255.0);
        let bias = f64x8::splat(0.5);
        for neighbor in 0..4 {
            let premultiplied =
                (f64x8::new(values[neighbor]) * f64x8::new(alphas[neighbor]) / divisor + bias)
                    .to_array();
            for (lane, value) in premultiplied.into_iter().enumerate() {
                if samples[lane].valid {
                    values[neighbor][lane] = (value as u8) as f64;
                }
            }
        }
    }
    [
        f64x8::new(values[0]),
        f64x8::new(values[1]),
        f64x8::new(values[2]),
        f64x8::new(values[3]),
    ]
}

#[inline]
fn rotate_unpremultiply_vector(
    premultiplied: f64x8,
    alpha: [u8; SIMD_F64_LANES],
) -> [u8; SIMD_F64_LANES] {
    let premultiplied_bytes = premultiplied.to_array().map(|value| value as u8);
    let alpha_vector = f64x8::new(alpha.map(f64::from));
    let safe_alpha = alpha_vector.max(f64x8::splat(1.0));
    let restored = (f64x8::new(premultiplied_bytes.map(f64::from))
        * f64x8::splat(255.0)
        / safe_alpha)
        .to_array();
    std::array::from_fn(|lane| {
        if alpha[lane] == 0 {
            premultiplied_bytes[lane]
        } else {
            restored[lane] as u8
        }
    })
}

const SIMD_F64_LANES: usize = 8;

/// Vectorized ordinary-byte bilinear rotation.
///
/// Affine coordinate construction and source gathers are scalar control;
/// neighbor interpolation runs eight output pixels at a time in `f64x8`.
/// LA/RGBA retain Pillow's premultiplied-alpha byte-rounding boundaries while
/// avoiding a whole-frame mode conversion.
fn simd_bilinear_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let Some(geometry) =
        simd_rotate_geometry(img.width(), img.height(), angle, expand, center, translate)
    else {
        return Ok(None);
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    let destination_width = geometry.width as usize;
    let destination_height = geometry.height as usize;
    if width == 0 || height == 0 || destination_width < SIMD_F64_LANES {
        return Ok(None);
    }
    let source_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate source dimensions overflow".into()))?;
    let destination_len = destination_width
        .checked_mul(destination_height)
        .and_then(|pixels| pixels.checked_mul(channels))
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let pa_mode = mode == Some("PA");
    // Pillow's rotate/transform path treats RGBa as already-premultiplied
    // stored samples and RGBX/CMYK as raw four-byte planes. Only RGBA has a
    // straight-alpha channel that needs premultiply/interpolate/unpremultiply.
    let alpha_channel = match (channels, mode) {
        (2, _) if !pa_mode => Some(1),
        (4, None | Some("RGBA")) => Some(3),
        _ => None,
    };
    let mut output = vec![0u8; destination_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..destination_height {
        let output_row = y * destination_width * channels;
        let vector_width = destination_width / SIMD_F64_LANES * SIMD_F64_LANES;
        let mut x = 0usize;
        while x < vector_width {
            let samples = std::array::from_fn(|lane| {
                simd_rotate_sample_coordinates_with_mode(
                    geometry.affine,
                    width,
                    height,
                    x + lane,
                    y,
                    pa_mode,
                )
            });
            let fx = f64x8::new(samples.map(|sample| sample.fx));
            let fy = f64x8::new(samples.map(|sample| sample.fy));
            let mut block = [0u8; SIMD_F64_LANES * 4];
            let mut alpha = [0u8; SIMD_F64_LANES];
            if let Some(alpha_channel) = alpha_channel {
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    width,
                    channels,
                    alpha_channel,
                    fill,
                    &samples,
                    None,
                );
                alpha = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                )
                .to_array()
                .map(|value| value as u8);
                for (lane, &value) in alpha.iter().enumerate() {
                    block[lane * channels + alpha_channel] = value;
                }
            }
            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let neighbors = gather_rotate_neighborhood_vectors(
                    source,
                    width,
                    channels,
                    channel,
                    fill,
                    &samples,
                    alpha_channel,
                );
                let interpolated = bilinear_rotate_vector(
                    neighbors[0],
                    neighbors[1],
                    neighbors[2],
                    neighbors[3],
                    fx,
                    fy,
                );
                let values = if alpha_channel.is_some() {
                    rotate_unpremultiply_vector(interpolated, alpha)
                } else {
                    interpolated.to_array().map(|value| value as u8)
                };
                for (lane, &value) in values.iter().enumerate() {
                    block[lane * channels + channel] = value;
                }
            }
            let output_start = output_row + x * channels;
            let block_bytes = SIMD_F64_LANES * channels;
            output[output_start..output_start + block_bytes]
                .copy_from_slice(&block[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            x += SIMD_F64_LANES;
        }
        while x < destination_width {
            let sample = simd_rotate_sample_coordinates_with_mode(
                geometry.affine,
                width,
                height,
                x,
                y,
                pa_mode,
            );
            let output_start = output_row + x * channels;
            let mut alpha = 0u8;
            if let Some(alpha_channel) = alpha_channel {
                let alpha_value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * width + source_x) * channels + alpha_channel] as f64
                    };
                    bilinear_rotate_scalar(
                        read(sample.x0, sample.y0),
                        read(sample.x1, sample.y0),
                        read(sample.x0, sample.y1),
                        read(sample.x1, sample.y1),
                        sample.fx,
                        sample.fy,
                    )
                } else {
                    rotate_fill_sample(fill, channels, alpha_channel) as f64
                };
                alpha = alpha_value as u8;
                output[output_start + alpha_channel] = alpha;
            }
            for channel in 0..channels {
                if Some(channel) == alpha_channel {
                    continue;
                }
                let value = if sample.valid {
                    let read = |source_x: usize, source_y: usize| {
                        source[(source_y * width + source_x) * channels + channel] as f64
                    };
                    let mut p00 = read(sample.x0, sample.y0);
                    let mut p10 = read(sample.x1, sample.y0);
                    let mut p01 = read(sample.x0, sample.y1);
                    let mut p11 = read(sample.x1, sample.y1);
                    if alpha_channel.is_some() {
                        let alpha_channel = alpha_channel.expect("checked above");
                        let read_alpha = |source_x: usize, source_y: usize| {
                            source[(source_y * width + source_x) * channels + alpha_channel] as f64
                        };
                        let premultiply = |value: f64, alpha: f64| {
                            (value * alpha / 255.0 + 0.5) as u8 as f64
                        };
                        p00 = premultiply(p00, read_alpha(sample.x0, sample.y0));
                        p10 = premultiply(p10, read_alpha(sample.x1, sample.y0));
                        p01 = premultiply(p01, read_alpha(sample.x0, sample.y1));
                        p11 = premultiply(p11, read_alpha(sample.x1, sample.y1));
                    }
                    bilinear_rotate_scalar(p00, p10, p01, p11, sample.fx, sample.fy)
                } else {
                    rotate_fill_sample(fill, channels, channel) as f64
                };
                let value = value as u8;
                output[output_start + channel] = if alpha_channel.is_some() && alpha != 0 {
                    (value as f64 * 255.0 / alpha as f64) as u8
                } else {
                    value
                };
            }
            x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(
        geometry.width,
        geometry.height,
        output,
        channels,
    )
    .map(|result| Some(preserve_mode(img, result)))
}

/// Execute Pillow's right-angle fast paths in the source's native layout.
///
/// Expanded rotations are direct transposes. Non-expanded 90/270-degree
/// rotations use the same centered clipping offsets as the CPU path and fill
/// only the exposed canvas. The operation is bandwidth-bound, so it is
/// classified as `native-copy`; the non-expanded path additionally records
/// its complete native-byte vector blocks.
fn simd_right_angle_rotate_native(
    img: &DynamicImage,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(degree) = rotate_discrete_fast_angle(angle, center, translate) else {
        return Ok(None);
    };
    let Some(channels) = native_rotate_layout(img, mode) else {
        return Ok(None);
    };
    let (width, height) = img.dimensions();
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let method = match degree {
        90 => TransposeMethod::Rotate90,
        180 => TransposeMethod::Rotate180,
        270 => TransposeMethod::Rotate270,
        _ => return Ok(None),
    };
    if expand || degree == 180 {
        let Some((result, vector_blocks, scalar_tail)) = native_transpose(img, mode, method) else {
            return Ok(None);
        };
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(Some(result));
    }

    let width = width as usize;
    let height = height as usize;
    let row_bytes = width
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD rotate row dimensions overflow".into()))?;
    let output_len = row_bytes
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("SIMD rotate output dimensions overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != output_len {
        return Err(PilError::InternalError(
            "SIMD rotate source buffer shape mismatch".into(),
        ));
    }
    let fill = fill.unwrap_or((0, 0, 0, 0));
    let width_i32 = i32::try_from(width)
        .map_err(|_| PilError::ValueError("SIMD rotate width exceeds coordinate range".into()))?;
    let height_i32 = i32::try_from(height)
        .map_err(|_| PilError::ValueError("SIMD rotate height exceeds coordinate range".into()))?;
    let width_gap = width_i32 - height_i32;
    let height_gap = height_i32 - width_i32;
    let (x_offset, y_offset) = if degree == 90 {
        (
            (f64::from(width_gap) / 2.0).floor() as i32,
            (f64::from(height_gap) / 2.0).ceil() as i32,
        )
    } else {
        (
            (f64::from(width_gap) / 2.0).ceil() as i32,
            (f64::from(height_gap) / 2.0).floor() as i32,
        )
    };
    let pixels_per_vector = (SIMD_RANK_FILTER_LANES / channels).max(1);
    let block_bytes = pixels_per_vector * channels;
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for destination_y in 0..height {
        let output_row = destination_y * row_bytes;
        let mut destination_x = 0usize;
        let vector_width = width / pixels_per_vector * pixels_per_vector;
        while destination_x < vector_width {
            let mut block = [0u8; SIMD_RANK_FILTER_LANES];
            for pixel in 0..pixels_per_vector {
                let x = destination_x + pixel;
                let (source_x, source_y) = if degree == 90 {
                    (
                        width_i32 - 1 - destination_y as i32 + y_offset,
                        x as i32 - x_offset,
                    )
                } else {
                    (
                        destination_y as i32 - y_offset,
                        height_i32 - 1 - x as i32 + x_offset,
                    )
                };
                let block_start = pixel * channels;
                if source_x >= 0
                    && source_x < width_i32
                    && source_y >= 0
                    && source_y < height_i32
                {
                    let source_start =
                        (source_y as usize * width + source_x as usize) * channels;
                    block[block_start..block_start + channels]
                        .copy_from_slice(&source[source_start..source_start + channels]);
                } else {
                    for channel in 0..channels {
                        block[block_start + channel] =
                            rotate_fill_sample(fill, channels, channel);
                    }
                }
            }
            let output_start = output_row + destination_x * channels;
            output[output_start..output_start + block_bytes]
                .copy_from_slice(&u8x16::new(block).to_array()[..block_bytes]);
            vector_blocks = vector_blocks.saturating_add(1);
            destination_x += pixels_per_vector;
        }
        while destination_x < width {
            let x = destination_x;
            let (source_x, source_y) = if degree == 90 {
                (
                    width_i32 - 1 - destination_y as i32 + y_offset,
                    x as i32 - x_offset,
                )
            } else {
                (
                    destination_y as i32 - y_offset,
                    height_i32 - 1 - x as i32 + x_offset,
                )
            };
            let output_start = output_row + x * channels;
            if source_x >= 0
                && source_x < width_i32
                && source_y >= 0
                && source_y < height_i32
            {
                let source_start = (source_y as usize * width + source_x as usize) * channels;
                output[output_start..output_start + channels]
                    .copy_from_slice(&source[source_start..source_start + channels]);
            } else {
                for channel in 0..channels {
                    output[output_start + channel] = rotate_fill_sample(fill, channels, channel);
                }
            }
            destination_x += 1;
            scalar_tail = scalar_tail.saturating_add(1);
        }
    }
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(width as u32, height as u32, output, channels)
        .map(|result| Some(preserve_mode(img, result)))
}

/// Execute the identity contracts for `distance <= 1` and one-pixel images.
///
/// The C implementation consumes two process-global random values per pixel
/// for every positive distance. For a one-pixel image the sampled destination
/// is either the same pixel or out of bounds, so the result is still an
/// identity copy; preserve that scalar control sequence before the native
/// vector copy. For larger images and distances above one, two pixels can
/// target the same destination; preserving last-writer ordering would require
/// a scatter/gather kernel, so contextual preflight leaves those inputs on the
/// explicit CPU fallback.
pub fn simd_effect_spread(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::EffectSpread { distance } = op else {
        return Err(PilError::ValueError("expected EffectSpread op".into()));
    };
    if *distance > 1 && (img.width() != 1 || img.height() != 1) {
        return Err(simd_unsupported("EffectSpread"));
    }
    if *distance != 0 {
        let pixel_count = (img.width() as usize)
            .checked_mul(img.height() as usize)
            .ok_or_else(|| PilError::ValueError("SIMD EffectSpread pixel count overflow".into()))?;
        crate::compute::pool_cpu::ops::effects::with_process_rng(|rng| {
            for _ in 0..pixel_count {
                let _ = rng.next();
                let _ = rng.next();
            }
        })?;
    }
    native_copy_image_bytes_allow_short(img, mode)?.ok_or_else(|| simd_unsupported("EffectSpread"))
}

pub fn simd_rotate(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Rotate {
        angle,
        expand,
        fill,
        center,
        translate,
        nearest,
    } = op
    else {
        return Err(PilError::ValueError("expected Rotate op".into()));
    };
    // Indexed Pillow images are defined to rotate with nearest-neighbor
    // sampling. The operation flag records only the explicit resample
    // request, so include the logical mode before choosing the data kernel.
    let nearest = *nearest || matches!(mode, Some("1" | "P"));
    if rotate_identity_contract(*angle, *center, *translate) {
        if let Some(result) = native_identity_rotate(img, mode)? {
            return Ok(preserve_mode(img, result));
        }
    }
    if rotate_uses_discrete_fast_path(*angle, *center, *translate)
        && let Some(result) = simd_right_angle_rotate_native(
            img,
            *angle,
            *expand,
            *fill,
            *center,
            *translate,
            mode,
        )?
    {
        return Ok(result);
    }
    if nearest
        && !rotate_uses_discrete_fast_path(*angle, *center, *translate)
        && let Some(result) = simd_nearest_rotate_native(
            img,
            *angle,
            *expand,
            *fill,
            *center,
            *translate,
            mode,
        )?
    {
        return Ok(result);
    }
    if !nearest
        && !rotate_uses_discrete_fast_path(*angle, *center, *translate)
        && let Some(result) = simd_bilinear_rotate_native(
            img,
            *angle,
            *expand,
            *fill,
            *center,
            *translate,
            mode,
        )?
    {
        return Ok(result);
    }
    Err(simd_unsupported("Rotate"))
}

// ═══════════════════════════════════════════════════════════════════════
// Section H: Special/mutating ops
// ═══════════════════════════════════════════════════════════════════════

pub fn simd_put_pixel(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutPixel { x, y, color, .. } = op else {
        return Err(PilError::ValueError("expected PutPixel op".into()));
    };
    let Some(channels) = native_draw_layout(img, mode) else {
        return Err(simd_unsupported("PutPixel"));
    };
    simd_put_pixel_native(img, *x, *y, *color, channels)?
        .ok_or_else(|| simd_unsupported("PutPixel"))
}

#[inline]
fn simd_put_alpha_block(
    source_block: [u8; 16],
    mask_block: [u8; 16],
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
) -> Option<[u8; 16]> {
    match (source_channels, output_channels) {
        (1, 2) => {
            let mut inputs = [0u8; 16];
            inputs[..8].copy_from_slice(&source_block[..8]);
            inputs[8..].copy_from_slice(&mask_block[..8]);
            Some(
                u8x16::new(inputs)
                    .swizzle_relaxed(u8x16::new([
                        0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15,
                    ]))
                    .to_array(),
            )
        }
        (2, 2) => {
            let values = u8x16::new(source_block);
            let alpha = u8x16::new(mask_block).swizzle_relaxed(u8x16::new([
                0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7,
            ]));
            let alpha_mask = u8x16::new([
                0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
            ]);
            let preserved = values & (u8x16::splat(255) ^ alpha_mask);
            Some((preserved | (alpha & alpha_mask)).to_array())
        }
        (3, 4) => {
            let mut inputs = source_block;
            inputs[12..16].copy_from_slice(&mask_block[..4]);
            Some(
                u8x16::new(inputs)
                    .swizzle_relaxed(u8x16::new([
                        0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15,
                    ]))
                    .to_array(),
            )
        }
        (4, 4) if cmyk_source => {
            let values = u8x16::new(source_block);
            let c = u16x16::from(values.swizzle_relaxed(u8x16::new([
                0, 4, 8, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let m = u16x16::from(values.swizzle_relaxed(u8x16::new([
                1, 5, 9, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let y = u16x16::from(values.swizzle_relaxed(u8x16::new([
                2, 6, 10, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let k = u16x16::from(values.swizzle_relaxed(u8x16::new([
                3, 7, 11, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ])));
            let nk = u16x16::splat(255) - k;
            let muldiv255 = |value: u16x16| {
                let value = value + u16x16::splat(128);
                ((value >> 8u32) + value) >> 8u32
            };
            let red = simd_pack_u16x16(nk - muldiv255(c * nk)).to_array();
            let green = simd_pack_u16x16(nk - muldiv255(m * nk)).to_array();
            let blue = simd_pack_u16x16(nk - muldiv255(y * nk)).to_array();
            Some(
                u8x16::new([
                    red[0], red[1], red[2], red[3], green[0], green[1], green[2], green[3],
                    blue[0], blue[1], blue[2], blue[3], mask_block[0], mask_block[1],
                    mask_block[2], mask_block[3],
                ])
                .swizzle_relaxed(u8x16::new([
                    0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15,
                ]))
                .to_array(),
            )
        }
        (4, 4) => {
            let values = u8x16::new(source_block);
            let alpha = u8x16::new(mask_block).swizzle_relaxed(u8x16::new([
                0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3,
            ]));
            let alpha_mask = u8x16::new([
                0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
            ]);
            let preserved = values & (u8x16::splat(255) ^ alpha_mask);
            Some((preserved | (alpha & alpha_mask)).to_array())
        }
        _ => None,
    }
}

#[inline]
fn write_put_alpha_scalar(
    source: &[u8],
    source_start: usize,
    output: &mut [u8],
    output_start: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
    alpha: u8,
) -> Option<()> {
    if source_start.checked_add(source_channels)? > source.len()
        || output_start.checked_add(output_channels)? > output.len()
    {
        return None;
    }
    if cmyk_source {
        let c = u32::from(source[source_start]);
        let m = u32::from(source[source_start + 1]);
        let y = u32::from(source[source_start + 2]);
        let k = u32::from(source[source_start + 3]);
        let nk = 255u32.saturating_sub(k);
        let red = (nk as i32 - crate::color::muldiv255(c, nk) as i32).clamp(0, 255) as u8;
        let green = (nk as i32 - crate::color::muldiv255(m, nk) as i32).clamp(0, 255) as u8;
        let blue = (nk as i32 - crate::color::muldiv255(y, nk) as i32).clamp(0, 255) as u8;
        output[output_start..output_start + 4].copy_from_slice(&[red, green, blue, alpha]);
    } else {
        output[output_start..output_start + source_channels]
            .copy_from_slice(&source[source_start..source_start + source_channels]);
        output[output_start + output_channels - 1] = alpha;
    }
    Some(())
}

/// Replace alpha samples from an L/1 mask using the source's native byte
/// layout.  This is a data-plane operation, not an RGBA promotion: L/ P
/// inputs become two-byte LA/PA samples, RGB inputs become four-byte RGBA,
/// and existing LA/RGBA alpha bytes are overwritten in place in the new
/// output buffer.  The four shuffle/merge cases below all process complete
/// pixel groups with `u8x16`; only the final incomplete group is scalar.
fn simd_put_alpha_data_bytes(
    source: &[u8],
    mask: &[u8],
    pixels: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
) -> Option<(Vec<u8>, u64, u64)> {
    let pixels_per_vector = match (source_channels, output_channels) {
        (1, 2) | (2, 2) => 8,
        (3, 4) | (4, 4) => 4,
        _ => return None,
    };
    let source_len = pixels.checked_mul(source_channels)?;
    let output_len = pixels.checked_mul(output_channels)?;
    if source.len() != source_len || mask.len() != pixels {
        return None;
    }

    let mut output = vec![0u8; output_len];
    let mut pixel = 0usize;
    while pixel + pixels_per_vector <= pixels {
        let source_start = pixel * source_channels;
        let source_bytes = pixels_per_vector * source_channels;
        let output_start = pixel * output_channels;
        let output_bytes = pixels_per_vector * output_channels;
        let mut source_block = [0u8; 16];
        source_block[..source_bytes]
            .copy_from_slice(&source[source_start..source_start + source_bytes]);
        let mut mask_block = [0u8; 16];
        mask_block[..pixels_per_vector]
            .copy_from_slice(&mask[pixel..pixel + pixels_per_vector]);

        let result = match (source_channels, output_channels) {
            (1, 2) => {
                let mut inputs = [0u8; 16];
                inputs[..pixels_per_vector].copy_from_slice(&source_block[..pixels_per_vector]);
                inputs[pixels_per_vector..2 * pixels_per_vector]
                    .copy_from_slice(&mask_block[..pixels_per_vector]);
                let indices = [
                    0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15,
                ];
                u8x16::new(inputs)
                    .swizzle_relaxed(u8x16::new(indices))
                    .to_array()
            }
            (2, 2) => {
                let values = u8x16::new(source_block);
                let alpha = u8x16::new(mask_block).swizzle_relaxed(u8x16::new([
                    0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7,
                ]));
                let alpha_mask = u8x16::new([
                    0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255, 0, 255,
                ]);
                let preserved = values & (u8x16::splat(255) ^ alpha_mask);
                (preserved | (alpha & alpha_mask)).to_array()
            }
            (3, 4) => {
                let mut inputs = source_block;
                inputs[12..16].copy_from_slice(&mask_block[..4]);
                let indices = [
                    0, 1, 2, 12, 3, 4, 5, 13, 6, 7, 8, 14, 9, 10, 11, 15,
                ];
                u8x16::new(inputs)
                    .swizzle_relaxed(u8x16::new(indices))
                    .to_array()
            }
            (4, 4) if cmyk_source => {
                // Pillow's CMYK putalpha path first converts C/M/Y/K to RGB
                // using MULDIV255, then writes the L mask as the new alpha.
                // Keep the four source samples native and do the three
                // channel arithmetic in sixteen-wide lanes. The final
                // interleave is only a byte shuffle; the scalar tail below
                // handles fewer than four pixels.
                let values = u8x16::new(source_block);
                let c = u16x16::from(values.swizzle_relaxed(u8x16::new([
                    0, 4, 8, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])));
                let m = u16x16::from(values.swizzle_relaxed(u8x16::new([
                    1, 5, 9, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])));
                let y = u16x16::from(values.swizzle_relaxed(u8x16::new([
                    2, 6, 10, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])));
                let k = u16x16::from(values.swizzle_relaxed(u8x16::new([
                    3, 7, 11, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                ])));
                let nk = u16x16::splat(255) - k;
                let muldiv255 = |value: u16x16| {
                    let value = value + u16x16::splat(128);
                    ((value >> 8u32) + value) >> 8u32
                };
                let red = simd_pack_u16x16(nk - muldiv255(c * nk)).to_array();
                let green = simd_pack_u16x16(nk - muldiv255(m * nk)).to_array();
                let blue = simd_pack_u16x16(nk - muldiv255(y * nk)).to_array();
                u8x16::new([
                    red[0], red[1], red[2], red[3],
                    green[0], green[1], green[2], green[3],
                    blue[0], blue[1], blue[2], blue[3],
                    mask_block[0], mask_block[1], mask_block[2], mask_block[3],
                ])
                .swizzle_relaxed(u8x16::new([
                    0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15,
                ]))
                .to_array()
            }
            (4, 4) => {
                let values = u8x16::new(source_block);
                let alpha = u8x16::new(mask_block).swizzle_relaxed(u8x16::new([
                    0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3,
                ]));
                let alpha_mask = u8x16::new([
                    0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255,
                ]);
                let preserved = values & (u8x16::splat(255) ^ alpha_mask);
                (preserved | (alpha & alpha_mask)).to_array()
            }
            _ => return None,
        };
        output[output_start..output_start + output_bytes].copy_from_slice(&result[..output_bytes]);
        pixel += pixels_per_vector;
    }

    for index in pixel..pixels {
        let source_start = index * source_channels;
        let output_start = index * output_channels;
        write_put_alpha_scalar(
            source,
            source_start,
            &mut output,
            output_start,
            source_channels,
            output_channels,
            cmyk_source,
            mask[index],
        )?;
    }

    Some((
        output,
        (pixel / pixels_per_vector) as u64,
        (pixels - pixel) as u64,
    ))
}

/// Replace alpha with one constant byte without first materializing a full
/// mask image. The output-changing promotion cases use the same native
/// interleave/conversion block as image-backed putalpha.
fn simd_put_alpha_constant_bytes(
    source: &[u8],
    pixels: usize,
    source_channels: usize,
    output_channels: usize,
    cmyk_source: bool,
    alpha: u8,
) -> Option<(Vec<u8>, u64, u64)> {
    let pixels_per_vector = match (source_channels, output_channels) {
        (1, 2) | (2, 2) => 8,
        (3, 4) | (4, 4) => 4,
        _ => return None,
    };
    let source_len = pixels.checked_mul(source_channels)?;
    let output_len = pixels.checked_mul(output_channels)?;
    if source.len() != source_len {
        return None;
    }

    let mut output = vec![0u8; output_len];
    let mut pixel = 0usize;
    while pixel + pixels_per_vector <= pixels {
        let source_start = pixel * source_channels;
        let source_bytes = pixels_per_vector * source_channels;
        let output_start = pixel * output_channels;
        let output_bytes = pixels_per_vector * output_channels;
        let mut source_block = [0u8; 16];
        source_block[..source_bytes]
            .copy_from_slice(&source[source_start..source_start + source_bytes]);
        let result = simd_put_alpha_block(
            source_block,
            [alpha; 16],
            source_channels,
            output_channels,
            cmyk_source,
        )?;
        output[output_start..output_start + output_bytes].copy_from_slice(&result[..output_bytes]);
        pixel += pixels_per_vector;
    }

    for index in pixel..pixels {
        write_put_alpha_scalar(
            source,
            index * source_channels,
            &mut output,
            index * output_channels,
            source_channels,
            output_channels,
            cmyk_source,
            alpha,
        )?;
    }

    Some((
        output,
        (pixel / pixels_per_vector) as u64,
        (pixels - pixel) as u64,
    ))
}

pub fn simd_put_alpha(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutAlpha { alpha, mode: alpha_mode } = op else {
        return Err(PilError::ValueError("expected PutAlpha op".into()));
    };
    let Some((source_channels, output_channels, _pixels_per_vector, cmyk_source)) =
        put_alpha_shape(img, *alpha_mode, mode)
    else {
        return Err(simd_unsupported("PutAlpha"));
    };
    let pixels = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutAlpha pixel count overflow".into()))?;
    let (output, vector_blocks, scalar_tail) = simd_put_alpha_constant_bytes(
        img.as_bytes(),
        pixels,
        source_channels,
        output_channels,
        cmyk_source,
        *alpha,
    )
    .ok_or_else(|| simd_unsupported("PutAlpha"))?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, output_channels)
}

pub fn simd_put_alpha_data(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutAlphaData { mask, mode: alpha_mode } = op else {
        return Err(PilError::ValueError("expected PutAlphaData op".into()));
    };
    let mask = mask.as_ref();
    let Some((source_channels, output_channels, _pixels_per_vector, cmyk_source)) =
        put_alpha_data_shape(img, mask, *alpha_mode, mode)
    else {
        return Err(simd_unsupported("PutAlphaData"));
    };
    let DynamicImage::ImageLuma8(mask) = mask else {
        return Err(simd_unsupported("PutAlphaData"));
    };
    let pixels = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutAlphaData pixel count overflow".into()))?;
    let (output, vector_blocks, scalar_tail) = simd_put_alpha_data_bytes(
        img.as_bytes(),
        mask.as_raw(),
        pixels,
        source_channels,
        output_channels,
        cmyk_source,
    )
    .ok_or_else(|| simd_unsupported("PutAlphaData"))?;
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    }
    if scalar_tail != 0 {
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    }
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, output_channels)
}

pub fn simd_eval(
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

pub fn simd_convert(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Convert {
        mode: target,
        matrix,
        dither: _,
    } = op
    else {
        return Err(PilError::ValueError("expected Convert op".into()));
    };
    if let Some((output, vector_blocks, scalar_tail)) =
        native_convert_luma16_bytes(img, target, mode)
    {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        crate::compute::record_pipeline_operation_path("vector");
        let channels = match target {
            ColorMode::L => 1,
            ColorMode::LA => 2,
            ColorMode::RGB => 3,
            ColorMode::RGBA | ColorMode::CMYK => 4,
            _ => return Err(simd_unsupported("Convert")),
        };
        return crate::image_utils::raw_bytes_to_image(
            img.width(),
            img.height(),
            output,
            channels,
        );
    }
    let Some(layout) = native_convert_layout(img, target, mode) else {
        return Err(simd_unsupported("Convert"));
    };
    if !native_convert_supported_for_image(img, target, matrix.as_deref(), mode) {
        return Err(simd_unsupported("Convert"));
    }
    let Some((output, vector_blocks, scalar_tail)) = native_convert_bytes(img, layout) else {
        return Err(simd_unsupported("Convert"));
    };
    if vector_blocks == 0 {
        return Err(simd_unsupported("Convert"));
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(
        img.width(),
        img.height(),
        output,
        layout.target_channels,
    )
}

#[inline]
fn alpha_composite_div255(value: u32x8) -> u32x8 {
    // Pillow's SHIFTFORDIV255(a) is ((a >> 8) + a) >> 8.  This is the
    // exact fixed-point division used by libImaging/AlphaComposite.c, not a
    // mathematically equivalent floating-point rounding operation.
    ((value >> 8u32) + value) >> 8u32
}

#[inline]
fn alpha_composite_channel_vector(
    source: [u32; 8],
    destination: [u32; 8],
    coefficient_source: u32x8,
    coefficient_destination: u32x8,
) -> [u8; 8] {
    // AlphaComposite.c uses PRECISION_BITS=7. All intermediates fit in u32:
    // the coefficient product is below 2^31 and the channel accumulator is
    // below 2^24, so widening is lossless before the vectorized SHIFTFORDIV255.
    let blended = u32x8::new(source) * coefficient_source
        + u32x8::new(destination) * coefficient_destination;
    let rounded = alpha_composite_div255(blended + u32x8::splat(0x80 << 7)) >> 7u32;
    let rounded = rounded.to_array();
    std::array::from_fn(|lane| rounded[lane].min(255) as u8)
}

#[inline]
fn alpha_composite_vector_block(source: &[u8], output: &mut [u8], channels: usize) -> bool {
    let Some(block_bytes) = channels.checked_mul(8) else {
        return false;
    };
    if !matches!(channels, 2 | 4)
        || source.len() < block_bytes
        || output.len() < block_bytes
    {
        return false;
    }

    let source_alpha = std::array::from_fn(|lane| {
        u32::from(source[lane * channels + channels - 1])
    });
    let destination_alpha = std::array::from_fn(|lane| {
        u32::from(output[lane * channels + channels - 1])
    });
    let source_alpha_vector = u32x8::new(source_alpha);
    let destination_alpha_vector = u32x8::new(destination_alpha);
    let blend = destination_alpha_vector * (u32x8::splat(255) - source_alpha_vector);
    let out_alpha_255 = source_alpha_vector * u32x8::splat(255) + blend;
    let coefficient_numerator = source_alpha_vector
        * u32x8::splat(255)
        * u32x8::splat(255)
        * u32x8::splat(1 << 7);
    // There is no portable integer divide instruction in `wide`. Convert
    // only this coefficient-control calculation to f64 lanes, take the exact
    // floor required by C, then return the hot per-channel path to u32 lanes.
    // Every operand is an exactly representable integer below 2^32.
    let coefficient_source = f64x8::new(
        coefficient_numerator
            .to_array()
            .map(f64::from),
    ) / f64x8::new(out_alpha_255.to_array().map(f64::from).map(|value| value.max(1.0)));
    let coefficient_source = u32x8::new(
        coefficient_source
            .floor()
            .to_array()
            .map(|value| value as u32),
    );
    let coefficient_destination = u32x8::splat(255 << 7) - coefficient_source;
    let output_alpha = alpha_composite_div255(out_alpha_255 + u32x8::splat(0x80)).to_array();
    let out_alpha_255_values = out_alpha_255.to_array();
    let source_luma = std::array::from_fn(|lane| {
        u32::from(source[lane * channels])
    });
    let destination_luma = std::array::from_fn(|lane| {
        u32::from(output[lane * channels])
    });
    let luma = alpha_composite_channel_vector(
        source_luma,
        destination_luma,
        coefficient_source,
        coefficient_destination,
    );

    let rgb = if channels == 4 {
        let source_green =
            std::array::from_fn(|lane| u32::from(source[lane * channels + 1]));
        let destination_green =
            std::array::from_fn(|lane| u32::from(output[lane * channels + 1]));
        let source_blue = std::array::from_fn(|lane| u32::from(source[lane * channels + 2]));
        let destination_blue =
            std::array::from_fn(|lane| u32::from(output[lane * channels + 2]));
        Some((
            alpha_composite_channel_vector(
                source_green,
                destination_green,
                coefficient_source,
                coefficient_destination,
            ),
            alpha_composite_channel_vector(
                source_blue,
                destination_blue,
                coefficient_source,
                coefficient_destination,
            ),
        ))
    } else {
        None
    };

    for lane in 0..8 {
        // Pillow leaves the destination pixel untouched when both alpha
        // values are zero. The vector arithmetic uses a denominator of one
        // only to avoid a divide-by-zero lane; this branch restores the
        // observable transparent RGB/LA payload exactly.
        if source_alpha[lane] == 0 || out_alpha_255_values[lane] == 0 {
            continue;
        }
        let offset = lane * channels;
        output[offset] = luma[lane];
        if let Some((green, blue)) = &rgb {
            output[offset + 1] = green[lane];
            output[offset + 2] = blue[lane];
        }
        output[offset + channels - 1] = output_alpha[lane].min(255) as u8;
    }
    true
}

#[inline]
fn alpha_composite_scalar_pixel(source: &[u8], output: &mut [u8], channels: usize) {
    let source_alpha = u32::from(source[channels - 1]);
    let destination_alpha = u32::from(output[channels - 1]);
    if source_alpha == 0 {
        return;
    }
    let blend = destination_alpha * (255 - source_alpha);
    let out_alpha_255 = source_alpha * 255 + blend;
    let coefficient_source = source_alpha * 255 * 255 * (1 << 7) / out_alpha_255;
    let coefficient_destination = (255 << 7) - coefficient_source;
    let div255 = |value: u32| ((value >> 8) + value) >> 8;
    let channel = |source: u8, destination: u8| {
        (div255(
            u32::from(source) * coefficient_source
                + u32::from(destination) * coefficient_destination
                + (0x80 << 7),
        ) >> 7)
            .min(255) as u8
    };
    output[0] = channel(source[0], output[0]);
    if channels == 4 {
        output[1] = channel(source[1], output[1]);
        output[2] = channel(source[2], output[2]);
    }
    output[channels - 1] = div255(out_alpha_255 + 0x80).min(255) as u8;
}

fn simd_alpha_composite_native(
    img: &DynamicImage,
    source: &Image,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !simd_alpha_composite_operands_supported(img, source, mode) {
        return Ok(None);
    }
    let source_image = source.materialized_shared()?;
    let channels = match (img, source_image.as_ref()) {
        (DynamicImage::ImageLumaA8(_), DynamicImage::ImageLumaA8(_)) => 2,
        (DynamicImage::ImageRgba8(_), DynamicImage::ImageRgba8(_)) => 4,
        _ => return Ok(None),
    };
    let source_bytes = source_image.as_bytes();
    let destination_bytes = img.as_bytes();
    if source_bytes.len() != destination_bytes.len() {
        return Ok(None);
    }
    let row_stride = (img.width() as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD AlphaComposite row stride overflow".into()))?;
    if row_stride.checked_mul(img.height() as usize) != Some(destination_bytes.len()) {
        return Ok(None);
    }

    let mut output = destination_bytes.to_vec();
    let total_pixels = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .ok_or_else(|| PilError::ValueError("SIMD AlphaComposite pixel count overflow".into()))?;
    let vector_pixels = total_pixels / 8 * 8;
    let vector_blocks = vector_pixels / 8;
    let scalar_tail = total_pixels - vector_pixels;
    // The image crate stores each image row contiguously, so a vector block is
    // allowed to cross a row boundary. This matters for small but non-empty
    // Pillow images: a 3×3 image has one complete eight-pixel block even
    // though no individual row is wide enough to hold one.
    for pixel in (0..vector_pixels).step_by(8) {
        let byte_start = pixel * channels;
        let byte_end = byte_start + channels * 8;
        if !alpha_composite_vector_block(
            &source_bytes[byte_start..byte_end],
            &mut output[byte_start..byte_end],
            channels,
        ) {
            return Ok(None);
        }
    }
    for pixel in vector_pixels..total_pixels {
        let byte_start = pixel * channels;
        let byte_end = byte_start + channels;
        alpha_composite_scalar_pixel(
            &source_bytes[byte_start..byte_end],
            &mut output[byte_start..byte_end],
            channels,
        );
    }
    if vector_blocks == 0 {
        // Empty images have no pixel data to vectorize. They are still a
        // valid SIMD-capable operation after scalar validation and produce an
        // empty native buffer without entering a CPU implementation.
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_path("vector");
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::image_utils::raw_bytes_to_image_allow_empty(img.width(), img.height(), output, channels)
        .map(|result| preserve_mode(img, result))
        .map(Some)
}

pub fn simd_alpha_composite(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::AlphaComposite { source, dest, src } = op else {
        return Err(PilError::ValueError(
            "expected AlphaComposite op".to_string(),
        ));
    };
    if *dest != (0, 0) || *src != (0, 0) {
        return Err(simd_unsupported("AlphaComposite"));
    }
    simd_alpha_composite_native(img, source, mode)?.ok_or_else(|| simd_unsupported("AlphaComposite"))
}
