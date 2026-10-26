//! Shared SIMD image storage layouts and pipeline shape prediction.

use super::{effects, put_alpha, resize, thumbnail};
use crate::pipeline::{ColorMode, PipelineOp, PixelMode, TransposeMethod};
use crate::raster::DynamicImage;

pub(super) const SIMD_BYTE_VECTOR_LANES: usize = 16;

pub(super) fn native_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX")) => {
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
pub(super) fn native_filter_byte_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
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

pub(super) fn native_typed_filter_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    matches!(img, DynamicImage::ImageRgba8(_))
        .then_some(())
        .filter(|_| matches!(mode, Some("I" | "F")))
        .map(|_| 4)
}

fn native_expand_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
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

pub(super) fn native_expand_contract_for_image(
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
    if output_len == 0 || img.as_bytes().len() != source_len {
        return None;
    }
    Some((channels, width, height))
}

pub(super) fn native_expand_contract_for_shape(
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
    if output_len == 0 {
        return None;
    }
    Some((channels, width, height))
}

/// Return a native interleaved byte layout for operations whose data plane is
/// memory movement rather than color arithmetic. Indexed and color-space
/// tags are included here because their stored samples are still raw bytes;
/// callers that interpret alpha or color arithmetic must use the narrower
/// helpers above instead.
pub(super) fn native_copy_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
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
pub(super) fn native_luma16_transpose_layout(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

/// Return whether an `I;16*` image can use native-copy geometry operations.
/// The two-byte samples stay opaque throughout the copy; reconstructing the
/// `ImageLuma16` buffer at the boundary preserves the typed image contract
/// without widening it through an RGBA representation.
fn native_luma16_copy_layout(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

pub(super) fn has_valid_luma16_copy_bytes(img: &DynamicImage, mode: Option<&str>) -> bool {
    if !native_luma16_copy_layout(img, mode) {
        return false;
    }
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(2))
    else {
        return false;
    };
    img.as_bytes().len() == expected
}

/// Return whether a row-oriented byte kernel has at least one complete
/// sixteen-byte vector block per row.  A large image made of very narrow rows
/// can have many total bytes while still exercising only scalar tails; those
/// inputs must not be advertised as SIMD-capable.
pub(super) fn has_vectorized_byte_rows(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0
        && img
            .width()
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

/// A zero-pixel native image is a valid Chops data path even though it has no
/// vector block to execute. Returning its already-typed buffer is the native
/// no-work equivalent of Pillow's empty output; it must not be reported as a
/// scalar or arithmetic vector kernel.
pub(super) fn has_empty_native_bytes(img: &DynamicImage, channels: usize) -> bool {
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
pub(super) fn has_vectorized_offset_rows(
    img: &DynamicImage,
    channels: usize,
    xoffset: i32,
) -> bool {
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
/// byte row, leaving the second half zero-filled. Narrow images use a masked
/// vector block across adjacent rows, so a small frame does not need to be
/// handed to the scalar backend merely because one row is shorter than a
/// register.
pub(super) fn has_vectorized_luma16_offset(img: &DynamicImage, mode: Option<&str>) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && (has_empty_native_bytes(img, 2)
            || (img.width() != 0
                && img.height() != 0
                && img
                    .width()
                    .checked_mul(img.height())
                    .and_then(|pixels| pixels.checked_mul(2))
                    .is_some_and(|bytes| img.as_bytes().len() == bytes as usize)))
}

/// Mirror uses one padded vector group for a partial final pixel group, so
/// even narrow rows can execute the native reverse kernel. RGB still needs
/// three vectors for one complete sixteen-pixel group, while the other layouts
/// use one vector group sized to their channel stride.
pub(super) fn has_vectorized_mirror_rows(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0 && img.width() != 0 && matches!(channels, 1..=4)
}

/// A masked final block is still a real SIMD data path. Use this admission
/// predicate for byte-wise kernels whose operation is independent per sample;
/// it permits narrow rows without routing the whole operation to CPU merely
/// because the row has fewer than sixteen bytes.
pub(super) fn has_nonempty_byte_data(img: &DynamicImage, channels: usize) -> bool {
    img.height() != 0
        && img.width() != 0
        && img
            .width()
            .checked_mul(img.height())
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

/// Validate the native byte buffer without imposing a minimum vector width.
/// A partial final block is padded by the data kernel, and an empty image is
/// handled as a valid native no-work path by operations whose Pillow contract
/// preserves it.
pub(super) fn has_valid_byte_data(img: &DynamicImage, channels: usize) -> bool {
    let Some(expected) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected
}

/// Return whether an identity operation can use at least one complete native
/// vector copy.  Zero-radius blur is a valid Pillow no-op, but it must not be
/// admitted as SIMD merely because the image has byte storage: a one-byte
/// scalar-only copy is still an unsupported strict-SIMD data path.
pub(super) fn has_vectorized_native_identity_copy(img: &DynamicImage, mode: Option<&str>) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SimdLayout {
    Luma8,
    LumaA8,
    Rgb8,
    Rgba8,
    Luma16,
    Unsupported,
}

#[derive(Clone, Copy)]
pub(super) struct SimdImageShape {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) layout: SimdLayout,
}

pub(super) fn simd_shape(img: &DynamicImage) -> SimdImageShape {
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

pub(super) fn shape_channels(shape: SimdImageShape) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 => Some(1),
        SimdLayout::LumaA8 => Some(2),
        SimdLayout::Rgb8 => Some(3),
        SimdLayout::Rgba8 => Some(4),
        SimdLayout::Luma16 | SimdLayout::Unsupported => None,
    }
}

fn shape_mode_matches(shape: SimdImageShape, mode: Option<&str>) -> bool {
    match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("1" | "L" | "P")),
        SimdLayout::LumaA8 => matches!(mode, None | Some("LA" | "PA")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")),
        SimdLayout::Rgba8 => {
            matches!(
                mode,
                None | Some("RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F")
            )
        }
        SimdLayout::Luma16 => false,
        SimdLayout::Unsupported => false,
    }
}

pub(super) fn shape_native_byte_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
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

pub(super) fn shape_native_typed_filter_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    (shape.layout == SimdLayout::Rgba8 && matches!(mode, Some("I" | "F"))).then_some(4)
}

pub(super) fn shape_has_nonempty_byte_data(shape: SimdImageShape, channels: usize) -> bool {
    shape.width != 0
        && shape.height != 0
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            .is_some_and(|bytes| bytes != 0)
}

pub(super) fn shape_has_vectorized_native_identity_copy(
    shape: SimdImageShape,
    channels: usize,
) -> bool {
    shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some_and(|bytes| bytes >= 16)
}

pub(super) fn shape_native_copy_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = shape_channels(shape)?;
    shape_mode_matches(shape, mode).then_some(channels)
}

pub(super) fn shape_luma16_transpose_layout(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
}

pub(super) fn shape_luma16_copy_layout(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape_luma16_transpose_layout(shape, mode)
}

pub(super) fn shape_native_identity_copy_channels(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA" | "PA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => Some(3),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "CMYK" | "RGBa" | "I" | "F")) => {
            Some(4)
        }
        _ => None,
    }
}

pub(super) fn shape_has_empty_native_bytes(shape: SimdImageShape, channels: usize) -> bool {
    (shape.width == 0 || shape.height == 0)
        && shape
            .width
            .checked_mul(shape.height)
            .and_then(|pixels| pixels.checked_mul(channels as u32))
            == Some(0)
}

pub(super) fn shape_has_vector_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape.height != 0
        && shape
            .width
            .checked_mul(channels as u32)
            .is_some_and(|row_bytes| row_bytes >= 16)
}

pub(super) fn shape_has_vectorized_offset_rows(
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

pub(super) fn shape_has_vectorized_luma16_offset(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, None | Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && (shape_has_empty_native_bytes(shape, 2) || (shape.width != 0 && shape.height != 0))
}

pub(super) fn shape_has_vectorized_mirror_rows(shape: SimdImageShape, channels: usize) -> bool {
    shape.height != 0 && shape.width != 0 && matches!(channels, 1..=4)
}

fn update_shape_crop_geometry(
    next: &mut SimdImageShape,
    shape: SimdImageShape,
    op: &PipelineOp,
) -> Option<bool> {
    match op {
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            if *left >= *right || *top >= *bottom || *right > shape.width || *bottom > shape.height
            {
                return None;
            }
            next.width = *right - *left;
            next.height = *bottom - *top;
            Some(true)
        }
        PipelineOp::CropBorder { border } => {
            let twice = border.saturating_mul(2);
            if twice > shape.width || twice > shape.height {
                return None;
            }
            next.width = shape.width - twice;
            next.height = shape.height - twice;
            Some(true)
        }
        _ => Some(false),
    }
}

fn update_shape_geometry(
    next: &mut SimdImageShape,
    shape: SimdImageShape,
    op: &PipelineOp,
) -> Option<bool> {
    if update_shape_crop_geometry(next, shape, op)? {
        return Some(true);
    }
    if update_shape_fixed_geometry(next, shape, op)? {
        return Some(true);
    }
    update_shape_resampling_geometry(next, shape, op)
}

fn update_shape_fixed_geometry(
    next: &mut SimdImageShape,
    shape: SimdImageShape,
    op: &PipelineOp,
) -> Option<bool> {
    match op {
        PipelineOp::Expand { border, .. } => {
            let twice = border.checked_mul(2)?;
            next.width = shape.width.checked_add(twice)?;
            next.height = shape.height.checked_add(twice)?;
            Some(true)
        }
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
            Some(true)
        }
        PipelineOp::Reduce { x_factor, y_factor } => {
            next.width = shape.width.div_ceil((*x_factor).max(1));
            next.height = shape.height.div_ceil((*y_factor).max(1));
            Some(true)
        }
        PipelineOp::Resize { w, h, .. } => {
            next.width = *w;
            next.height = *h;
            Some(true)
        }
        PipelineOp::ResizeBoxed { w, h, .. } => {
            next.width = *w;
            next.height = *h;
            Some(true)
        }
        PipelineOp::Transform { w, h, .. } | PipelineOp::Pad { w, h, .. } => {
            next.width = *w;
            next.height = *h;
            Some(true)
        }
        _ => Some(false),
    }
}

fn update_shape_resampling_geometry(
    next: &mut SimdImageShape,
    shape: SimdImageShape,
    op: &PipelineOp,
) -> Option<bool> {
    match op {
        PipelineOp::Scale { factor, .. } => {
            (next.width, next.height) =
                resize::native_scale_dimensions(shape.width, shape.height, *factor)?;
            Some(true)
        }
        PipelineOp::Thumbnail { w, h, .. } => {
            let (output_width, output_height) =
                thumbnail::native_thumbnail_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
            Some(true)
        }
        PipelineOp::Contain { w, h, .. } => {
            if shape.width != 0 && shape.height != 0 {
                let (output_width, output_height) =
                    resize::native_pad_contained_dimensions(shape.width, shape.height, *w, *h)?;
                next.width = output_width;
                next.height = output_height;
            }
            Some(true)
        }
        PipelineOp::Cover { w, h, .. } => {
            let (output_width, output_height) =
                resize::native_cover_dimensions(shape.width, shape.height, *w, *h)?;
            next.width = output_width;
            next.height = output_height;
            Some(true)
        }
        PipelineOp::Fit { w, h, .. } => {
            next.width = (*w).max(1);
            next.height = (*h).max(1);
            Some(true)
        }
        _ => Some(false),
    }
}

fn layout_for_convert_mode(mode: &ColorMode) -> SimdLayout {
    match mode {
        ColorMode::L => SimdLayout::Luma8,
        ColorMode::LA => SimdLayout::LumaA8,
        ColorMode::RGB | ColorMode::YCbCr | ColorMode::HSV => SimdLayout::Rgb8,
        ColorMode::RGBA | ColorMode::CMYK => SimdLayout::Rgba8,
        _ => SimdLayout::Unsupported,
    }
}

fn layout_for_color_3d_lut_mode(mode: PixelMode) -> SimdLayout {
    match mode {
        PixelMode::RGB => SimdLayout::Rgb8,
        PixelMode::RGBA | PixelMode::CMYK => SimdLayout::Rgba8,
        _ => SimdLayout::Unsupported,
    }
}

fn layout_for_merge_mode(mode: &ColorMode) -> SimdLayout {
    match mode {
        ColorMode::L | ColorMode::Mode1 => SimdLayout::Luma8,
        ColorMode::LA => SimdLayout::LumaA8,
        ColorMode::RGB => SimdLayout::Rgb8,
        ColorMode::RGBA | ColorMode::CMYK => SimdLayout::Rgba8,
        _ => SimdLayout::Unsupported,
    }
}

fn layout_for_composite_mode(mode: &str) -> SimdLayout {
    match mode {
        "1" | "L" | "P" => SimdLayout::Luma8,
        "LA" | "PA" => SimdLayout::LumaA8,
        "RGB" | "HSV" | "YCbCr" => SimdLayout::Rgb8,
        "RGBA" | "CMYK" | "RGBa" | "RGBX" | "I" | "F" => SimdLayout::Rgba8,
        _ => SimdLayout::Unsupported,
    }
}

fn update_shape_layout(next: &mut SimdImageShape, op: &PipelineOp) -> Option<bool> {
    match op {
        PipelineOp::Constant { .. } | PipelineOp::Grayscale => {
            next.layout = SimdLayout::Luma8;
            Some(true)
        }
        PipelineOp::Colorize { .. } | PipelineOp::ConvertLab { .. } => {
            next.layout = SimdLayout::Rgb8;
            Some(true)
        }
        PipelineOp::ExtractBand { .. } => {
            next.layout = SimdLayout::Luma8;
            Some(true)
        }
        PipelineOp::EffectNoise { .. } => {
            next.layout = effects::output_layout();
            Some(true)
        }
        PipelineOp::Convert { mode, .. } => {
            next.layout = layout_for_convert_mode(mode);
            Some(true)
        }
        PipelineOp::Color3DLut { target_mode, .. } => {
            next.layout = layout_for_color_3d_lut_mode(*target_mode);
            Some(true)
        }
        PipelineOp::Merge { mode, .. } => {
            next.layout = layout_for_merge_mode(mode);
            Some(true)
        }
        PipelineOp::CompositeModule { other, .. } => {
            let output_mode = other.mode().ok()?;
            (next.width, next.height) = other.size().ok()?;
            next.layout = layout_for_composite_mode(output_mode.as_str());
            Some(true)
        }
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => {
            put_alpha::update_shape(next, op);
            Some(true)
        }
        _ => Some(false),
    }
}

pub(super) fn shape_after_simd_op(
    shape: SimdImageShape,
    op: &PipelineOp,
) -> Option<SimdImageShape> {
    let mut next = shape;
    if update_shape_geometry(&mut next, shape, op)? {
        return Some(next);
    }
    update_shape_layout(&mut next, op)?;
    Some(next)
}
