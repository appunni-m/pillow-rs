//! Native-mode Paste admission and region planning.

use super::super::{SimdImageShape, SimdLayout};
use crate::image::Image;
use crate::raster::{DynamicImage, GenericImageView};
use std::sync::Arc;

#[derive(Clone, Copy)]
pub(in crate::compute::pool_simd::ops::adapters) struct NativePasteLayout {
    pub(in crate::compute::pool_simd::ops::adapters) channels: usize,
    pub(in crate::compute::pool_simd::ops::adapters) mode: &'static str,
}

#[derive(Clone, Copy)]
pub(in crate::compute::pool_simd::ops::adapters) struct NativePasteMaskLayout {
    pub(in crate::compute::pool_simd::ops::adapters) channels: usize,
    pub(in crate::compute::pool_simd::ops::adapters) value_index: usize,
    pub(in crate::compute::pool_simd::ops::adapters) premultiplied: bool,
    pub(in crate::compute::pool_simd::ops::adapters) binary: bool,
}

#[derive(Clone, Copy)]
pub(in crate::compute::pool_simd::ops::adapters) struct NativePasteRegion {
    pub(in crate::compute::pool_simd::ops::adapters) source_left: usize,
    pub(in crate::compute::pool_simd::ops::adapters) source_top: usize,
    pub(in crate::compute::pool_simd::ops::adapters) destination_left: usize,
    pub(in crate::compute::pool_simd::ops::adapters) destination_top: usize,
    pub(in crate::compute::pool_simd::ops::adapters) width: usize,
    pub(in crate::compute::pool_simd::ops::adapters) height: usize,
}

#[derive(Clone, Copy)]
pub(in crate::compute::pool_simd::ops::adapters) struct NativePastePlan {
    pub(in crate::compute::pool_simd::ops::adapters) layout: NativePasteLayout,
    pub(in crate::compute::pool_simd::ops::adapters) mask: Option<NativePasteMaskLayout>,
    pub(in crate::compute::pool_simd::ops::adapters) region: NativePasteRegion,
    pub(in crate::compute::pool_simd::ops::adapters) source_width: usize,
    pub(in crate::compute::pool_simd::ops::adapters) source_height: usize,
    pub(in crate::compute::pool_simd::ops::adapters) source_is_broadcast: bool,
    pub(in crate::compute::pool_simd::ops::adapters) luma16_mask_byte_shift: u32,
}

fn native_paste_luma16_mask_byte_shift(mode: Option<&str>) -> u32 {
    match mode {
        Some("I;16B") => 8,
        Some("I;16N" | "I;16") | None if cfg!(target_endian = "big") => 8,
        _ => 0,
    }
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
pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_layout(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
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

/// Return the native typed-sample contract used by `I;16*` Paste.
///
/// `DynamicImage` stores these modes as host-endian `u16` samples.  The
/// logical mode tag determines how the binding later serializes the samples,
/// but it does not change the SIMD data-plane contract: one unsigned 16-bit
/// value per pixel, with optional 8-bit mask blending.
fn native_paste_luma16_layout(img: &DynamicImage, mode: Option<&str>) -> Option<NativePasteLayout> {
    matches!(img, DynamicImage::ImageLuma16(_))
        .then_some(())
        .filter(|_| mode.is_none_or(|value| matches!(value, "I;16" | "I;16L" | "I;16B" | "I;16N")))
        .map(|_| NativePasteLayout {
            // This is the byte stride used by the actual-layout validator and
            // by the generic region planner. The typed kernel interprets it as
            // one u16 sample rather than two independent channels.
            channels: 2,
            mode: "I;16",
        })
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_shape_layout(
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

fn native_paste_luma16_shape_layout(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
    (shape.layout == SimdLayout::Luma16)
        .then_some(())
        .filter(|_| mode.is_none_or(|value| matches!(value, "I;16" | "I;16L" | "I;16B" | "I;16N")))
        .map(|_| NativePasteLayout {
            channels: 2,
            mode: "I;16",
        })
}

fn native_paste_mode_matches(actual: &str, expected: &str) -> bool {
    if expected == "I;16" {
        matches!(actual, "I;16" | "I;16L" | "I;16B" | "I;16N")
    } else {
        actual == expected
    }
}

fn native_paste_mask_layout(mode: &str, mask_alpha: bool) -> Option<NativePasteMaskLayout> {
    match (mode, mask_alpha) {
        ("1", false) => Some(NativePasteMaskLayout {
            channels: 1,
            value_index: 0,
            premultiplied: false,
            binary: true,
        }),
        ("L", false) => Some(NativePasteMaskLayout {
            channels: 1,
            value_index: 0,
            premultiplied: false,
            binary: false,
        }),
        ("LA", true) => Some(NativePasteMaskLayout {
            channels: 2,
            value_index: 1,
            premultiplied: false,
            binary: false,
        }),
        ("RGBA", true) => Some(NativePasteMaskLayout {
            channels: 4,
            value_index: 3,
            premultiplied: false,
            binary: false,
        }),
        ("RGBa", true) => Some(NativePasteMaskLayout {
            channels: 4,
            value_index: 3,
            premultiplied: true,
            binary: false,
        }),
        _ => None,
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_region(
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
    mode: Option<&str>,
) -> Option<NativePastePlan> {
    let source_width = u32::try_from(width).ok()?;
    let source_height = u32::try_from(height).ok()?;
    let source_mode = source.mode().ok()?;
    let source_size = source.size().ok()?;
    let compact_solid_source = source_size == (1, 1)
        && (source_width, source_height) != (1, 1)
        && mask.is_some()
        && !mask_alpha
        && matches!(layout.mode, "L" | "RGB" | "HSV" | "CMYK")
        && mask.and_then(|image| image.mode().ok()).as_deref() == Some("L");
    if !native_paste_mode_matches(&source_mode, layout.mode)
        || (source_size != (source_width, source_height) && !compact_solid_source)
    {
        return None;
    }
    let mask_layout = match mask {
        Some(mask) => {
            // These modes can be copied byte-for-byte, but their masked
            // Pillow contracts are not the same as L/LA/RGB/RGBA blending.
            // Reject them before execution rather than silently applying an
            // RGBA-shaped alpha formula to indexed, typed, or tagged data.
            if !matches!(
                layout.mode,
                "L" | "LA" | "PA" | "RGB" | "HSV" | "RGBA" | "CMYK" | "I;16"
            ) {
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
    // A zero-area intersection is a valid Pillow no-op, not an unsupported
    // layout. The apply kernels simply execute zero rows and preserve the
    // destination. Masked rows are also allowed to be narrower than a vector;
    // the padded tail kernel handles those bytes without scalar arithmetic.
    let region = native_paste_region(
        destination_width,
        destination_height,
        source_width,
        source_height,
        x,
        y,
    )
    .unwrap_or(NativePasteRegion {
        source_left: 0,
        source_top: 0,
        destination_left: 0,
        destination_top: 0,
        width: 0,
        height: 0,
    });
    Some(NativePastePlan {
        layout,
        mask: mask_layout,
        region,
        source_width: source_width as usize,
        source_height: source_height as usize,
        source_is_broadcast: compact_solid_source,
        luma16_mask_byte_shift: native_paste_luma16_mask_byte_shift(mode),
    })
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_plan_for_image(
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
    let layout =
        native_paste_luma16_layout(img, mode).or_else(|| native_paste_layout(img, mode))?;
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
        mode,
    )
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_plan_for_shape(
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
    let layout = native_paste_luma16_shape_layout(shape, mode)
        .or_else(|| native_paste_shape_layout(shape, mode))?;
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
        mode,
    )
}

/// RGB destinations intentionally accept an RGBA source in Pillow's Paste.c
/// path. The alpha byte is ignored for an unmasked paste; dropping it is a
/// fixed byte shuffle, not a reason to widen the operation through CPU/RGBA
/// materialization.
pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_rgba_to_rgb_supported(
    img: &DynamicImage,
    source: &Arc<Image>,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mode: Option<&str>,
) -> bool {
    let Some(source_width) = u32::try_from(width).ok() else {
        return false;
    };
    let Some(source_height) = u32::try_from(height).ok() else {
        return false;
    };
    matches!(img, DynamicImage::ImageRgb8(_))
        && mode.is_none_or(|value| value == "RGB")
        && mask.is_none()
        && source.mode().ok().as_deref() == Some("RGBA")
        && source.size().ok() == Some((source_width, source_height))
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_rgba_to_rgb_supported_for_shape(
    shape: SimdImageShape,
    source: &Arc<Image>,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mode: Option<&str>,
) -> bool {
    let Some(source_width) = u32::try_from(width).ok() else {
        return false;
    };
    let Some(source_height) = u32::try_from(height).ok() else {
        return false;
    };
    shape.layout == SimdLayout::Rgb8
        && mode.is_none_or(|value| value == "RGB")
        && mask.is_none()
        && source.mode().ok().as_deref() == Some("RGBA")
        && source.size().ok() == Some((source_width, source_height))
}

/// Unmasked RGB-to-RGBA paste writes opaque alpha while copying directly from
/// the three-byte source layout. The destination mode must be explicit because
/// several logical modes share the RGBA storage carrier.
pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_rgb_to_rgba_supported(
    img: &DynamicImage,
    source: &Arc<Image>,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mode: Option<&str>,
) -> bool {
    let Some(source_width) = u32::try_from(width).ok() else {
        return false;
    };
    let Some(source_height) = u32::try_from(height).ok() else {
        return false;
    };
    matches!(img, DynamicImage::ImageRgba8(_))
        && mode == Some("RGBA")
        && mask.is_none()
        && source.mode().ok().as_deref() == Some("RGB")
        && source.size().ok() == Some((source_width, source_height))
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_rgb_to_rgba_supported_for_shape(
    shape: SimdImageShape,
    source: &Arc<Image>,
    width: i32,
    height: i32,
    mask: Option<&Arc<Image>>,
    mode: Option<&str>,
) -> bool {
    let Some(source_width) = u32::try_from(width).ok() else {
        return false;
    };
    let Some(source_height) = u32::try_from(height).ok() else {
        return false;
    };
    shape.layout == SimdLayout::Rgba8
        && mode == Some("RGBA")
        && mask.is_none()
        && source.mode().ok().as_deref() == Some("RGB")
        && source.size().ok() == Some((source_width, source_height))
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
pub(in crate::compute::pool_simd::ops::adapters) fn native_composite_plan_for_image(
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
    // A disjoint source and image2 still produce image2.copy() in Pillow;
    // zero-area overlap is a valid no-op paste, not a reason to fall back.
    let region = native_paste_region(
        destination_width,
        destination_height,
        img.width(),
        img.height(),
        0,
        0,
    )
    .unwrap_or(NativePasteRegion {
        source_left: 0,
        source_top: 0,
        destination_left: 0,
        destination_top: 0,
        width: 0,
        height: 0,
    });
    Some(NativePastePlan {
        layout,
        mask: Some(mask_layout),
        region,
        source_width: img.width() as usize,
        source_height: img.height() as usize,
        source_is_broadcast: false,
        luma16_mask_byte_shift: 0,
    })
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_composite_plan_for_shape(
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
    )
    .unwrap_or(NativePasteRegion {
        source_left: 0,
        source_top: 0,
        destination_left: 0,
        destination_top: 0,
        width: 0,
        height: 0,
    });
    Some(NativePastePlan {
        layout,
        mask: Some(mask_layout),
        region,
        source_width: shape.width as usize,
        source_height: shape.height as usize,
        source_is_broadcast: false,
        luma16_mask_byte_shift: 0,
    })
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_actual_layout(
    image: &DynamicImage,
    layout: NativePasteLayout,
) -> bool {
    let layout_matches = if layout.mode == "I;16" {
        native_paste_luma16_layout(image, Some("I;16")).is_some()
    } else {
        native_paste_layout(image, Some(layout.mode)).is_some()
    };
    layout_matches && {
        let actual = layout;
        actual.channels == layout.channels
            && image
                .width()
                .checked_mul(image.height())
                .and_then(|pixels| pixels.checked_mul(layout.channels as u32))
                .is_some_and(|bytes| image.as_bytes().len() == bytes as usize)
    }
}

pub(in crate::compute::pool_simd::ops::adapters) fn native_paste_actual_mask_layout(
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
