use super::layout::{shape_after_simd_op, simd_shape};
use super::{effects, put_alpha, simd_supports_for_shape};
use crate::pipeline::{ColorMode, PipelineOp, PixelMode};
use crate::raster::DynamicImage;

fn color_mode_name(mode: &ColorMode) -> &'static str {
    match mode {
        ColorMode::L => "L",
        ColorMode::LA => "LA",
        ColorMode::LaPremultiplied => "La",
        ColorMode::RGB => "RGB",
        ColorMode::RGBA => "RGBA",
        ColorMode::RgbPremultiplied => "RGBa",
        ColorMode::Rgbx => "RGBX",
        ColorMode::CMYK => "CMYK",
        ColorMode::YCbCr => "YCbCr",
        ColorMode::HSV => "HSV",
        ColorMode::I => "I",
        ColorMode::F => "F",
        ColorMode::I16 => "I;16",
        ColorMode::P => "P",
        ColorMode::Mode1 => "1",
    }
}

pub(super) fn pixel_mode_name(mode: PixelMode) -> &'static str {
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

fn operation_target_mode(op: &PipelineOp) -> Option<&str> {
    match op {
        PipelineOp::Convert { mode, .. } => Some(color_mode_name(mode)),
        PipelineOp::ConvertLab { .. } => Some("LAB"),
        // A merge's storage family can differ from its logical mode (LAB
        // uses RGB storage). Subsequent operations need the semantic tag.
        PipelineOp::Merge { logical_mode, .. } => Some(logical_mode),
        PipelineOp::Color3DLut { target_mode, .. } => Some(pixel_mode_name(*target_mode)),
        PipelineOp::PutAlpha { .. } | PipelineOp::PutAlphaData { .. } => put_alpha::output_mode(op),
        PipelineOp::Grayscale => Some("L"),
        PipelineOp::EffectNoise { .. } => Some(effects::output_mode()),
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
        if matches!(op, PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }) {
            return Some(target.to_owned());
        }
        return Some(target.to_owned());
    }
    if matches!(op, PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. })
        && matches!(current, Some("P" | "PA"))
    {
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
