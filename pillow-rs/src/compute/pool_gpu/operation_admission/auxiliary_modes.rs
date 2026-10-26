use super::{AuxiliaryImages, DynamicImage, PipelineOp};

pub fn gpu_auxiliary_modes_are_safe(
    op: &PipelineOp,
    image: &DynamicImage,
    auxiliary: &AuxiliaryImages,
) -> bool {
    gpu_auxiliary_modes_are_safe_for_color(op, image.color(), auxiliary, None)
}

pub fn gpu_auxiliary_modes_are_safe_for_color(
    op: &PipelineOp,
    current_color: crate::raster::ColorType,
    auxiliary: &AuxiliaryImages,
    logical_mode: Option<&str>,
) -> bool {
    if matches!(
        op,
        PipelineOp::Autocontrast { mask: Some(_), .. } | PipelineOp::EqualizeMasked { .. }
    ) {
        // ImageOps.autocontrast validates masks as mode 1/L. Both are
        // represented by the Luma8 transport, whose first byte is exactly the
        // nonzero selector consumed by the histogram gather shader.
        return matches!(
            auxiliary.third.as_deref(),
            Some(DynamicImage::ImageLuma8(_))
        );
    }
    if let PipelineOp::Paste { mask_alpha, .. } = op
        && current_color == crate::raster::ColorType::L16
    {
        return gpu_luma16_paste_auxiliary_modes_are_safe(*mask_alpha, auxiliary);
    }
    if let PipelineOp::CompositeModule { mask_alpha, .. } = op {
        return gpu_composite_auxiliary_modes_are_safe(*mask_alpha, current_color, auxiliary);
    }
    let requires_matching_source = matches!(
        op,
        PipelineOp::Add { .. }
            | PipelineOp::Subtract { .. }
            | PipelineOp::Multiply { .. }
            | PipelineOp::Screen { .. }
            | PipelineOp::Darker { .. }
            | PipelineOp::Lighter { .. }
            | PipelineOp::Difference { .. }
            | PipelineOp::AddModulo { .. }
            | PipelineOp::SubtractModulo { .. }
            | PipelineOp::LogicalAnd { .. }
            | PipelineOp::LogicalOr { .. }
            | PipelineOp::LogicalXor { .. }
            | PipelineOp::BlendModule { .. }
            | PipelineOp::Paste { .. }
            | PipelineOp::AlphaComposite { .. }
    );
    if requires_matching_source
        && auxiliary
            .second
            .as_ref()
            .is_some_and(|second| second.color() != current_color)
    {
        let native_rgb_to_rgba_paste = matches!(
            (op, logical_mode, current_color, auxiliary.second.as_deref()),
            (
                PipelineOp::Paste { mask: None, .. },
                Some("RGBA"),
                crate::raster::ColorType::Rgba8,
                Some(DynamicImage::ImageRgb8(_))
            )
        );
        if !native_rgb_to_rgba_paste {
            return false;
        }
    }
    if let PipelineOp::Paste {
        mask_alpha: false, ..
    } = op
    {
        // Paste's luma-mask path calls DynamicImage::to_luma8(), which uses
        // weighted RGB conversion for RGB/RGBA masks. The shader samples byte
        // 0 directly, so only native L/LA masks are exact.
        return !auxiliary.third.as_ref().is_some_and(|mask| {
            !matches!(
                mask.as_ref(),
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
            )
        });
    }
    true
}

#[inline]
fn gpu_luma16_paste_auxiliary_modes_are_safe(
    mask_alpha: bool,
    auxiliary: &AuxiliaryImages,
) -> bool {
    let source_is_luma16 = auxiliary
        .second
        .as_deref()
        .is_some_and(|source| matches!(source, DynamicImage::ImageLuma16(_)));
    let mask_is_supported = matches!(
        (mask_alpha, auxiliary.third.as_deref()),
        (false, None)
            | (false, Some(DynamicImage::ImageLuma8(_)))
            | (true, Some(DynamicImage::ImageRgba8(_)))
    );
    source_is_luma16 && mask_is_supported
}

#[inline]
fn gpu_composite_auxiliary_modes_are_safe(
    mask_alpha: bool,
    current_color: crate::raster::ColorType,
    auxiliary: &AuxiliaryImages,
) -> bool {
    let Some(destination) = auxiliary.second.as_ref() else {
        return false;
    };
    let Some(mask) = auxiliary.third.as_ref() else {
        return false;
    };
    // Image.composite converts image1 to image2's mode before the operation
    // is queued. The shader therefore needs both sources in one native layout.
    if destination.color() != current_color {
        return false;
    }
    // Pillow selects byte 0 for 1/L and byte 3 for LA/RGBA/RGBa. Reject RGB
    // luma masks: CPU computes weighted luma while the packed shader reads R.
    if mask_alpha {
        matches!(
            mask.as_ref(),
            DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
        )
    } else {
        matches!(mask.as_ref(), DynamicImage::ImageLuma8(_))
    }
}
