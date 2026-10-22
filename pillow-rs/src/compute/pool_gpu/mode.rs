use super::*;

pub(super) fn gpu_resize_channel_count(mode: u32) -> u32 {
    match mode {
        0 => 1,
        1 => 2,
        2 => 3,
        _ => 4,
    }
}

pub(super) fn gpu_resize_should_premultiply(
    mode: u32,
    logical_mode: Option<&str>,
    filter: ResampleFilter,
) -> bool {
    // Pillow's nearest affine path copies samples directly. Alpha is
    // premultiplied only by the separable convolution filters.
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }
    match logical_mode {
        Some("PA" | "RGBa" | "CMYK" | "RGBX" | "P" | "1" | "I" | "F") => false,
        Some("LA" | "RGBA") => true,
        Some("L" | "RGB" | "HSV" | "YCbCr") => false,
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => false,
        None => matches!(mode, 1 | 3),
        Some(_) => matches!(mode, 1 | 3),
    }
}

pub(super) fn gpu_transform_uses_nearest(
    logical_mode: Option<&str>,
    filter: ResampleFilter,
) -> bool {
    matches!(filter, ResampleFilter::Nearest) || matches!(logical_mode, Some("P" | "1"))
}

pub(super) fn gpu_transform_should_premultiply(
    mode: u32,
    logical_mode: Option<&str>,
    filter: ResampleFilter,
    method: TransformMethod,
) -> bool {
    // Perspective/Quad use the direct ImagingTransformProjective byte path.
    // Its filtered LA/RGBA relocation envelope still performs Pillow's
    // premultiplied round trip in the shader, while raw L/RGB/PA paths remain
    // unpremultiplied. The bounded Mesh relocation path follows the same
    // projective contract.
    if gpu_transform_uses_nearest(logical_mode, filter) {
        return false;
    }
    if !matches!(method, TransformMethod::Affine) {
        return matches!(logical_mode, Some("LA" | "RGBA"))
            || (logical_mode.is_none() && matches!(mode, 1 | 3));
    }
    match logical_mode {
        Some("LA" | "RGBA") => true,
        Some("PA" | "RGBa" | "CMYK" | "I" | "F" | "P" | "1") => false,
        Some("I;16" | "I;16L" | "I;16B" | "I;16N") => false,
        None => matches!(mode, 1 | 3),
        Some(_) => matches!(mode, 1 | 3),
    }
}

pub(super) fn gpu_transform_fill(op: &PipelineOp, logical_mode: Option<&str>, mode: u32) -> u32 {
    let PipelineOp::Transform {
        fill, palette_fill, ..
    } = op
    else {
        return 0;
    };
    let has_alpha =
        matches!(logical_mode, Some("LA" | "La" | "PA" | "RGBA" | "RGBa")) || matches!(mode, 1 | 3);
    // Pillow's default rotate fill for CMYK is the complete zero C/M/Y/K
    // sample. It is not the opaque-alpha default used by RGB-like modes.
    let default_fill = if matches!(logical_mode, Some("CMYK" | "F")) {
        // F uses a zero floating-point word, even though its packed
        // transport has four bytes and no logical alpha channel. CMYK uses
        // the same all-zero default for its four raw channels.
        (0, 0, 0, 0)
    } else {
        (0, 0, 0, if has_alpha { 0 } else { 255 })
    };
    let resolved = palette_fill
        .map(|index| (index, 0, 0, 255))
        .or(*fill)
        .unwrap_or(default_fill);
    if mode == 5 {
        // Geometry upload/readback keeps B/N samples in big-endian byte
        // order even on little-endian hosts. Fill must use that same transport.
        let value = u16::from_le_bytes([resolved.0, resolved.1]);
        let bytes = if matches!(logical_mode, Some("I;16B" | "I;16N")) {
            value.to_be_bytes()
        } else {
            value.to_ne_bytes()
        };
        return u32::from(bytes[0]) | (u32::from(bytes[1]) << 8);
    }
    // Core normalizes LA/PA colors as `(luma, alpha, 0, 0)` because that is
    // the native two-band representation.  The GPU transport is packed
    // RGBA, so move the logical alpha into byte three and replicate luma in
    // the unused color lanes just as `to_rgba8()` does for the source image.
    let packed = if matches!(logical_mode, Some("LA" | "La" | "PA")) {
        (resolved.0, resolved.0, resolved.0, resolved.1)
    } else {
        resolved
    };
    u32::from(packed.0)
        | (u32::from(packed.1) << 8)
        | (u32::from(packed.2) << 16)
        | (u32::from(packed.3) << 24)
}

/// Return the packed transport mode used by the exact four-mode Convert
/// shader.  Non-byte targets are deliberately excluded: their public sample
/// representation needs a different buffer layout and therefore remains an
/// explicit preflight failure.
pub(super) fn gpu_standard_color_mode_code(mode: &ColorMode) -> Option<u32> {
    match mode {
        ColorMode::L => Some(0),
        ColorMode::LA => Some(1),
        ColorMode::RGB => Some(2),
        ColorMode::RGBA => Some(3),
        _ => None,
    }
}

/// Return the packed target-mode code understood by `convert.wgsl`.
/// Non-standard Pillow modes remain packed byte buffers at this executor
/// boundary, but their output representation is restored by the final
/// color-type conversion and the public explicit mode on the lazy image.
pub(super) fn gpu_convert_target_mode_code(mode: &ColorMode) -> Option<u32> {
    match mode {
        ColorMode::L => Some(0),
        ColorMode::LA => Some(1),
        ColorMode::RGB => Some(2),
        ColorMode::RGBA => Some(3),
        ColorMode::CMYK => Some(4),
        ColorMode::YCbCr => Some(5),
        ColorMode::HSV => Some(6),
        ColorMode::I => Some(7),
        ColorMode::F => Some(8),
        ColorMode::P | ColorMode::Mode1 => None,
    }
}

/// Update the packed mode word after a mode-changing operation that the GPU
/// batch can represent.  Keeping this scalar state beside the uniform builder
/// lets a Convert feed a later shader without a host readback: the first
/// dispatch writes the new packed layout and the next dispatch receives the
/// corresponding mode code.
pub(super) fn gpu_mode_after_op(mode: u32, op: &PipelineOp) -> u32 {
    match op {
        PipelineOp::Convert { mode: target, .. } => {
            gpu_convert_target_mode_code(target).unwrap_or(mode)
        }
        PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. } => 0,
        PipelineOp::Colorize { .. } => 2,
        PipelineOp::EffectNoise { .. } => 0,
        PipelineOp::Color3DLut { target_mode, .. } => {
            gpu_pixel_mode_code(*target_mode).unwrap_or(mode)
        }
        // LAB shares RGB's native three-byte storage after its fixed-point
        // transform, so later packed-byte shaders use the RGB mode code.
        PipelineOp::ConvertLab { .. } => 2,
        _ => mode,
    }
}

pub(super) fn gpu_pixel_mode_code(mode: PixelMode) -> Option<u32> {
    match mode {
        PixelMode::RGB => Some(2),
        PixelMode::RGBA => Some(3),
        PixelMode::CMYK => Some(4),
        _ => None,
    }
}

pub(super) fn gpu_mode_after_ops(mut mode: u32, ops: &[PipelineOp]) -> u32 {
    for op in ops {
        mode = gpu_mode_after_op(mode, op);
    }
    mode
}

/// Compute Pillow's rounded Contrast midpoint without materializing a
/// grayscale image. This is scalar control-plane work: the original native
/// byte layout is scanned once, then the complete per-pixel blend is executed
/// by `contrast.wgsl`. CMYK keeps its C/M/Y/K interpretation instead of
/// treating the fourth packed byte as alpha.
pub(super) fn gpu_contrast_mean(img: &DynamicImage, logical_mode: Option<&str>) -> Option<u8> {
    let base = crate::ops::enhance::contrast_base(img, logical_mode)?;
    (!img.as_bytes().is_empty()).then_some(base.mean)
}

/// Compute Contrast's midpoint after a narrowly provable host prefix.
///
/// `Image.putpixel` is an exact byte-layout update, while Contrast's midpoint
/// belongs to the image produced by that update rather than to the original
/// source.  The GPU batch already executes both operations in order; this
/// helper mirrors only the one exact prefix needed to supply the scalar
/// midpoint.  Keep the proof deliberately small: palette writes, additional
/// operations, and a later Contrast still require the ordinary host-control
/// path because their current-image semantics are not represented by one
/// static uniform.
pub(super) fn gpu_contrast_mean_after_exact_prefix(
    img: &DynamicImage,
    ops: &[PipelineOp],
    logical_mode: Option<&str>,
) -> Option<u8> {
    let first_contrast = ops
        .iter()
        .position(|op| matches!(op, PipelineOp::Contrast { .. }))?;
    if ops
        .iter()
        .skip(first_contrast.saturating_add(1))
        .any(|op| matches!(op, PipelineOp::Contrast { .. }))
    {
        return None;
    }

    if first_contrast == 0 {
        return gpu_contrast_mean(img, logical_mode);
    }

    // A single PutPixel is the only prefix whose exact result can be formed
    // without invoking the CPU batch executor (which would publish a second
    // backend/operation receipt).  Palette-index writes need palette state and
    // therefore remain on the existing exact host-control path.
    if first_contrast != 1 {
        return None;
    }
    let PipelineOp::PutPixel {
        x,
        y,
        color,
        palette_index: false,
    } = &ops[0]
    else {
        return None;
    };
    let prefixed =
        crate::compute::pool_cpu::ops::effects::op_put_pixel(img, *x, *y, *color).ok()?;
    gpu_contrast_mean(&prefixed, logical_mode)
}

/// Return the concrete byte-buffer color type produced by a supported GPU
/// mode-changing operation. The shader always writes packed RGBA8, but the
/// public result must expose the requested number of bands rather than the
/// source image's type.
pub(super) fn gpu_output_color_type(mode: &ColorMode) -> Option<crate::raster::ColorType> {
    match mode {
        ColorMode::L => Some(crate::raster::ColorType::L8),
        ColorMode::LA => Some(crate::raster::ColorType::La8),
        ColorMode::RGB => Some(crate::raster::ColorType::Rgb8),
        ColorMode::RGBA => Some(crate::raster::ColorType::Rgba8),
        ColorMode::CMYK | ColorMode::I | ColorMode::F => Some(crate::raster::ColorType::Rgba8),
        ColorMode::YCbCr | ColorMode::HSV => Some(crate::raster::ColorType::Rgb8),
        _ => None,
    }
}

pub(super) fn gpu_pixel_mode_color_type(mode: PixelMode) -> Option<crate::raster::ColorType> {
    match mode {
        PixelMode::RGB => Some(crate::raster::ColorType::Rgb8),
        PixelMode::RGBA | PixelMode::CMYK => Some(crate::raster::ColorType::Rgba8),
        _ => None,
    }
}

/// Convert the packed GPU result to the requested standard byte mode without
/// applying a second luma conversion. GPU L/LA shaders intentionally keep the
/// luma sample in byte 0; calling `to_luma8()` on that RGBA transport would
/// weight the zeroed G/B bytes again and change every sample.
pub(super) fn gpu_result_as_color_type(
    result: DynamicImage,
    color_type: crate::raster::ColorType,
) -> Result<DynamicImage, PilError> {
    // GPU readback already returns the packed RGBA transport. Consume it so
    // narrowing L/LA/RGB outputs does not first clone a full frame.
    let rgba = result.into_rgba8();
    let (w, h) = rgba.dimensions();
    match color_type {
        crate::raster::ColorType::L8 => {
            let luma = rgba.pixels().map(|pixel| pixel[0]).collect();
            crate::raster::GrayImage::from_raw(w, h, luma)
                .map(DynamicImage::ImageLuma8)
                .ok_or_else(|| PilError::InternalError("GPU L output shape mismatch".into()))
        }
        crate::raster::ColorType::La8 => {
            let samples = rgba
                .pixels()
                .flat_map(|pixel| [pixel[0], pixel[3]])
                .collect();
            crate::raster::GrayAlphaImage::from_raw(w, h, samples)
                .map(DynamicImage::ImageLumaA8)
                .ok_or_else(|| PilError::InternalError("GPU LA output shape mismatch".into()))
        }
        crate::raster::ColorType::Rgb8 => Ok(DynamicImage::ImageRgb8(
            DynamicImage::ImageRgba8(rgba).to_rgb8(),
        )),
        crate::raster::ColorType::Rgba8 => Ok(DynamicImage::ImageRgba8(rgba)),
        _ => Err(PilError::InternalError(
            "unsupported GPU output color type".into(),
        )),
    }
}

/// Return whether a batch contains a mode-changing operation that cannot feed
/// a later shader in-place. Even byte-to-byte Convert changes the source
/// layout used by subsequent image-aware checks, so it terminates a segment
/// just like the other mode transitions.
pub(super) fn gpu_batch_has_nonterminal_mode_change(ops: &[PipelineOp]) -> bool {
    ops.iter().enumerate().any(|(index, op)| {
        index + 1 < ops.len()
            && matches!(
                op,
                PipelineOp::Grayscale
                    | PipelineOp::ExtractBand { .. }
                    | PipelineOp::Constant { .. }
                    | PipelineOp::Colorize { .. }
                    | PipelineOp::PutAlpha { .. }
                    | PipelineOp::PutAlphaData { .. }
                    | PipelineOp::Convert { .. }
                    | PipelineOp::ConvertLab { .. }
                    | PipelineOp::EffectNoise { .. }
            )
    })
}

/// Find the first operation whose public result changes the packed logical
/// layout while another operation still follows it. A host-visible native
/// result keeps the following operation's mode checks tied to the converted
/// image rather than the original source layout.
pub(super) fn gpu_first_nonterminal_mode_change(ops: &[PipelineOp]) -> Option<usize> {
    ops.iter().enumerate().find_map(|(index, op)| {
        (index + 1 < ops.len()
            && matches!(
                op,
                PipelineOp::Grayscale
                    | PipelineOp::ExtractBand { .. }
                    | PipelineOp::Constant { .. }
                    | PipelineOp::Colorize { .. }
                    | PipelineOp::PutAlpha { .. }
                    | PipelineOp::PutAlphaData { .. }
                    | PipelineOp::Convert { .. }
                    | PipelineOp::ConvertLab { .. }
                    | PipelineOp::EffectNoise { .. }
            ))
        .then_some(index)
    })
}

/// Return the logical mode that follows a mode-changing GPU segment. The
/// image result is used as a conservative fallback for direct backend callers
/// that do not provide an explicit logical-mode tag.
pub(super) fn gpu_logical_mode_after_op(
    previous: Option<&str>,
    op: &PipelineOp,
    result: &DynamicImage,
) -> Option<String> {
    let known = match op {
        PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } | PipelineOp::Constant { .. } => {
            Some("L")
        }
        PipelineOp::Colorize { .. } => Some("RGB"),
        PipelineOp::Convert { mode, .. } => match mode {
            ColorMode::L => Some("L"),
            ColorMode::LA => Some("LA"),
            ColorMode::RGB => Some("RGB"),
            ColorMode::RGBA => Some("RGBA"),
            ColorMode::CMYK => Some("CMYK"),
            ColorMode::YCbCr => Some("YCbCr"),
            ColorMode::HSV => Some("HSV"),
            ColorMode::I => Some("I"),
            ColorMode::F => Some("F"),
            ColorMode::P => Some("P"),
            ColorMode::Mode1 => Some("1"),
        },
        PipelineOp::ConvertLab { .. } => Some("LAB"),
        PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => match mode {
            PixelMode::L | PixelMode::LA => Some("LA"),
            PixelMode::RGB | PixelMode::RGBA | PixelMode::YCbCr | PixelMode::HSV => Some("RGBA"),
            PixelMode::P | PixelMode::PA => Some("PA"),
            PixelMode::CMYK => Some("CMYK"),
            PixelMode::Mode1 | PixelMode::I | PixelMode::F => None,
        },
        _ => None,
    };
    known
        .map(str::to_owned)
        .or_else(|| previous.map(str::to_owned))
        .or_else(|| match result {
            DynamicImage::ImageLuma8(_) => Some("L".to_owned()),
            DynamicImage::ImageLumaA8(_) => Some("LA".to_owned()),
            DynamicImage::ImageRgb8(_) => Some("RGB".to_owned()),
            DynamicImage::ImageRgba8(_) => Some("RGBA".to_owned()),
            DynamicImage::ImageLuma16(_) => None,
            _ => None,
        })
}
