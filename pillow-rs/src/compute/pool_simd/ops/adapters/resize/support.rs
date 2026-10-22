//! Shared Resize, aspect-resize, and native-layout admission contracts.

use super::super::{SIMD_RESIZE_NEAREST_BYTES, SimdImageShape, SimdLayout};
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

pub(super) const SIMD_FIT_PARALLEL_PIXEL_THRESHOLD: usize = 32 * 32;

/// Round a positive Pillow dimension with Python's ties-to-even rule.
///
/// `ImageOps.pad` performs this calculation in its scalar control plane
/// before the resize and placement kernels run. Keeping the rule here avoids
/// importing a CPU pixel implementation while retaining the public
/// dimension contract.
pub(in crate::compute::pool_simd::ops::adapters) fn native_pad_round_dimension(
    value: f64,
) -> Option<u32> {
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

pub(in crate::compute::pool_simd::ops::adapters) fn native_pad_contained_dimensions(
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
                f64::from(source_height) / f64::from(source_width) * f64::from(target_width),
            )?,
        ))
    } else {
        Some((
            native_pad_round_dimension(
                f64::from(source_width) / f64::from(source_height) * f64::from(target_height),
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
pub(in crate::compute::pool_simd::ops::adapters) fn native_cover_dimensions(
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
                f64::from(source_height) / f64::from(source_width) * f64::from(target_width),
            )?,
        ))
    } else {
        Some((
            native_pad_round_dimension(
                f64::from(source_width) / f64::from(source_height) * f64::from(target_height),
            )?,
            target_height,
        ))
    }
}

// Keep feature-gated aspect-resize block paths limited to images with enough
// pixels to amortize their setup; SIMD row scheduling remains serial.
pub(super) const SIMD_ASPECT_RESIZE_PARALLEL_PIXEL_THRESHOLD: usize = 32 * 32;

/// Compute the source box used by `ImageOps.fit` without touching pixels.
///
/// Pillow keeps this calculation in the ImageOps layer and passes the four
/// coordinates to its boxed resampler. Keeping the same scalar control plane
/// here lets the SIMD data plane consume the exact crop, including fractional
/// bleed and centering values.
pub(super) fn native_fit_box(
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
    // Pillow retains the exact positive live dimensions after subtracting
    // bleed; clamping either axis to one pixel changes small-image crop boxes.
    let live_width = source_width - 2.0 * bleed_width;
    let live_height = source_height - 2.0 * bleed_height;
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

/// Return whether the boxed fit target is valid for the zero-dimension
/// contract that Pillow's `Image.resize` enforces after ImageOps.fit's ratio
/// calculations.  A zero-width source may produce a positive black result,
/// but an empty-width output is only a same-size, no-bleed copy.
pub(super) fn native_fit_dimensions_supported(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
    bleed: f64,
) -> bool {
    if source_height == 0 || target_height == 0 {
        return false;
    }
    if target_width == 0 {
        return source_width == 0 && target_height == source_height && bleed == 0.0;
    }
    !(source_width == 0 && target_height < source_height)
}

/// Return the raw byte layout and alpha contract accepted by the boxed SIMD
/// resampler. P/PA are included as raw indexed samples: P forces nearest
/// sampling in Pillow, while PA filters its index and alpha bytes without RGBA
/// premultiplication.
pub(super) fn native_fit_layout_for_image(
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
        DynamicImage::ImageRgba8(_) if matches!(mode, Some("RGBa" | "CMYK")) => Some((4, false)),
        _ => None,
    }
}

pub(super) fn native_fit_layout_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some((1, false)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, true)),
        SimdLayout::LumaA8 if mode == Some("PA") => Some((2, false)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB")) => Some((3, false)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "RGBX")) => Some((4, true)),
        SimdLayout::Rgba8 if matches!(mode, Some("RGBa" | "CMYK")) => Some((4, false)),
        _ => None,
    }
}

pub(super) fn native_fit_filter(mode: Option<&str>, filter: ResampleFilter) -> ResampleFilter {
    if matches!(mode, Some("1" | "P")) {
        ResampleFilter::Nearest
    } else {
        filter
    }
}

/// Match the float32 box validation boundary used by `Image.resize` and the
/// CPU boxed resampler before admitting a source box to native SIMD kernels.
pub(super) fn native_resize_boxed_bounds(
    source_width: u32,
    source_height: u32,
    bounds: (f64, f64, f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    let bounds = (
        f64::from(bounds.0 as f32),
        f64::from(bounds.1 as f32),
        f64::from(bounds.2 as f32),
        f64::from(bounds.3 as f32),
    );
    if !bounds.0.is_finite()
        || !bounds.1.is_finite()
        || !bounds.2.is_finite()
        || !bounds.3.is_finite()
        || bounds.0 < 0.0
        || bounds.1 < 0.0
        || bounds.2 > f64::from(source_width as f32)
        || bounds.3 > f64::from(source_height as f32)
        || bounds.2 < bounds.0
        || bounds.3 < bounds.1
    {
        return None;
    }
    Some(bounds)
}

pub(super) fn native_resize_boxed_layout_for_image(
    img: &DynamicImage,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("F" | "I")) {
        if !matches!(img, DynamicImage::ImageRgba8(_))
            || (mode == Some("I") && !matches!(filter, ResampleFilter::Nearest))
        {
            return None;
        }
        return Some((4, false));
    }
    native_fit_layout_for_image(img, mode)
}

pub(super) fn native_resize_boxed_layout_for_shape(
    shape: SimdImageShape,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    if matches!(mode, Some("F" | "I")) {
        if shape.layout != SimdLayout::Rgba8
            || (mode == Some("I") && !matches!(filter, ResampleFilter::Nearest))
        {
            return None;
        }
        return Some((4, false));
    }
    native_fit_layout_for_shape(shape, mode)
}

pub(super) fn native_resize_boxed_supported_for_dimensions(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    channels: usize,
) -> bool {
    if output_width == 0 || output_height == 0 {
        return false;
    }
    let Some((left, top, right, bottom)) =
        native_resize_boxed_bounds(source_width, source_height, bounds)
    else {
        return false;
    };

    // Pillow changes pass order for extremely tall images. The existing
    // boxed SIMD kernel is horizontal-first, so leave this case on the CPU
    // until a vertical-first vector implementation can preserve its rounding.
    if u64::from(source_height) > u64::from(source_width) * 100 && output_height < source_height {
        return false;
    }

    let full_source_identity = source_width == output_width
        && source_height == output_height
        && left == 0.0
        && top == 0.0
        && right == f64::from(source_width as f32)
        && bottom == f64::from(source_height as f32);
    full_source_identity
        || native_resize_supported_for_dimensions(
            source_width,
            source_height,
            output_width,
            output_height,
            filter,
            channels,
        )
}

pub(super) fn native_resize_boxed_supported_for_image(
    img: &DynamicImage,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> bool {
    let effective_filter = native_fit_filter(mode, filter);
    let Some((channels, _)) = native_resize_boxed_layout_for_image(img, effective_filter, mode)
    else {
        return false;
    };
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
        && native_resize_boxed_supported_for_dimensions(
            img.width(),
            img.height(),
            output_width,
            output_height,
            effective_filter,
            bounds,
            channels,
        )
}

pub(super) fn native_resize_boxed_supported_for_shape(
    shape: SimdImageShape,
    output_width: u32,
    output_height: u32,
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    mode: Option<&str>,
) -> bool {
    let effective_filter = native_fit_filter(mode, filter);
    native_resize_boxed_layout_for_shape(shape, effective_filter, mode).is_some_and(
        |(channels, _)| {
            native_resize_boxed_supported_for_dimensions(
                shape.width,
                shape.height,
                output_width,
                output_height,
                effective_filter,
                bounds,
                channels,
            )
        },
    )
}

pub(super) fn native_fit_float_supported_for_image(
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

pub(super) fn native_fit_float_supported_for_shape(
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

pub(super) fn native_fit_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    mode: Option<&str>,
) -> bool {
    if !native_fit_dimensions_supported(
        img.width(),
        img.height(),
        target_width,
        target_height,
        bleed,
    ) {
        return false;
    }
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

pub(super) fn native_fit_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    mode: Option<&str>,
) -> bool {
    if !native_fit_dimensions_supported(
        shape.width,
        shape.height,
        target_width,
        target_height,
        bleed,
    ) {
        return false;
    }
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

/// Pillow keeps `P` and `PA` in their native indexed sample layouts during
/// `ImageOps.pad`; `F` and `I` keep one scalar sample in four raw bytes, and
/// HSV/CMYK use their ordinary packed byte layouts, and RGBX uses its
/// four-byte carrier while preserving the X sample. The adapter must admit
/// those logical modes without pretending that the stored bytes are RGBA.
pub(in crate::compute::pool_simd::ops::adapters) fn native_pad_channels_for_image(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some(3)
        }
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "RGBX" | "CMYK" | "I" | "F")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_pad_channels_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "RGBX" | "CMYK" | "I" | "F")) => {
            Some(4)
        }
        _ => None,
    }
}

pub(super) fn native_contain_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    if img.width() == 0 || img.height() == 0 {
        let expected_bytes = (img.width() as usize)
            .checked_mul(img.height() as usize)
            .and_then(|pixels| pixels.checked_mul(channels));
        return target_width != 0
            && target_height != 0
            && expected_bytes == Some(img.as_bytes().len());
    }
    let Some((output_width, output_height)) =
        native_pad_contained_dimensions(img.width(), img.height(), target_width, target_height)
    else {
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

pub(super) fn native_contain_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    if shape.width == 0 || shape.height == 0 {
        return target_width != 0 && target_height != 0;
    }
    let Some((output_width, output_height)) =
        native_pad_contained_dimensions(shape.width, shape.height, target_width, target_height)
    else {
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

pub(super) fn native_cover_supported_for_image(
    img: &DynamicImage,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_image(img, mode) else {
        return false;
    };
    let Some((output_width, output_height)) =
        native_cover_dimensions(img.width(), img.height(), target_width, target_height)
    else {
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

pub(super) fn native_cover_supported_for_shape(
    shape: SimdImageShape,
    target_width: u32,
    target_height: u32,
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let Some(channels) = native_pad_channels_for_shape(shape, mode) else {
        return false;
    };
    let Some((output_width, output_height)) =
        native_cover_dimensions(shape.width, shape.height, target_width, target_height)
    else {
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

/// Return the dimensions Pillow's scalar `ImageOps.scale` validation computes.
///
/// Scale is scalar control work followed by the same native resize data plane
/// as `Image.resize`.  Keeping the ties-to-even rounding here makes SIMD
/// preflight agree with the public operation without moving validation into a
/// CPU adapter.
pub(in crate::compute::pool_simd::ops::adapters) fn native_scale_dimensions(
    source_width: u32,
    source_height: u32,
    factor: f64,
) -> Option<(u32, u32)> {
    if !factor.is_finite() || factor <= 0.0 {
        return None;
    }
    let round = |dimension: u32| {
        native_pad_round_dimension(f64::from(dimension) * factor).filter(|rounded| *rounded > 0)
    };
    Some((round(source_width)?, round(source_height)?))
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_resize_supported_for_dimensions(
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
        ResampleFilter::Nearest => super::resize_nearest_vectorizable(
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

pub(in crate::compute::pool_simd::ops::adapters) fn native_resize_supported_for_image(
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
    } else if let Some(channels) = super::resize_native_channels_for_image(img, mode) {
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

pub(in crate::compute::pool_simd::ops::adapters) fn native_resize_supported_for_shape(
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
        match super::native_resize_byte_layout_for_shape(shape, mode) {
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
