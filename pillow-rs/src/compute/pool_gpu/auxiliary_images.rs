use super::*;

pub(super) fn put_alpha_output(
    result: DynamicImage,
    mode: PixelMode,
) -> Result<DynamicImage, PilError> {
    if matches!(
        mode,
        PixelMode::L | PixelMode::LA | PixelMode::P | PixelMode::PA
    ) {
        let rgba = match result {
            // Native LA/PA byte dispatch already returned the public
            // two-band representation. Do not widen it to RGBA and pack it
            // straight back into the same two bytes.
            DynamicImage::ImageLumaA8(image) => return Ok(DynamicImage::ImageLumaA8(image)),
            // The generic GPU path returns an owned RGBA readback; consume
            // that carrier so the fallback does not clone it before packing.
            result => result.into_rgba8(),
        };
        let (w, h) = rgba.dimensions();
        let samples = rgba
            .pixels()
            .flat_map(|pixel| [pixel[0], pixel[3]])
            .collect();
        crate::raster::GrayAlphaImage::from_raw(w, h, samples)
            .map(DynamicImage::ImageLumaA8)
            .ok_or_else(|| {
                PilError::InternalError("GPU putalpha buffer shape mismatch".to_string())
            })
    } else {
        Ok(DynamicImage::ImageRgba8(result.into_rgba8()))
    }
}

/// Extract the second (right-hand) image from a dual-input PipelineOp, if present.
/// Returns shared materialized pixels ready for GPU upload.
fn extract_effect_spread_or_lut_auxiliary(
    op: &PipelineOp,
    primary_dimensions: Option<(u32, u32)>,
) -> Option<Result<Arc<DynamicImage>, PilError>> {
    match op {
        PipelineOp::EffectSpread { distance } => Some((|| {
            let (width, height) = primary_dimensions.ok_or_else(|| {
                PilError::InternalError("GPU EffectSpread is missing primary dimensions".into())
            })?;
            let mapping = crate::compute::pool_cpu::ops::effects::effect_spread_mapping(
                width, height, *distance,
            )?;
            let mut bytes = Vec::with_capacity(mapping.len().saturating_mul(4));
            for value in mapping {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            let synthetic = RgbaImage::from_raw(width.saturating_mul(height), 1, bytes)
                .ok_or_else(|| {
                    PilError::InternalError("GPU EffectSpread map shape mismatch".into())
                })?;
            Ok(Arc::new(DynamicImage::ImageRgba8(synthetic)))
        })()),
        PipelineOp::Color3DLut { table, .. } => Some((|| {
            // Color3DLut table entries are prepared once on the host using the
            // exact signed 10.6 conversion from the CPU implementation. Store one
            // signed i16 value per synthetic pixel; the shader reads the low 16
            // bits through the normal packed RGBA upload path.
            let values = color3dlut_table_words(table)?;
            let width = u32::try_from(values.len()).map_err(|_| {
                PilError::ValueError("GPU Color3DLut table exceeds u32 dimensions".into())
            })?;
            let mut bytes = Vec::with_capacity(values.len().saturating_mul(4));
            for value in values {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            let synthetic = RgbaImage::from_raw(width, 1, bytes).ok_or_else(|| {
                PilError::InternalError("GPU Color3DLut auxiliary table shape mismatch".into())
            })?;
            Ok(Arc::new(DynamicImage::ImageRgba8(synthetic)))
        })()),
        _ => None,
    }
}

pub(super) fn extract_second_image(
    op: &PipelineOp,
    primary_dimensions: Option<(u32, u32)>,
    draw_source: Option<&DynamicImage>,
) -> Result<Option<Arc<DynamicImage>>, PilError> {
    if matches!(op, PipelineOp::ConvertLab { .. }) {
        return Ok(Some(crate::lab::gpu_table_image()));
    }
    if crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
        let rendered = if let Some(rendered) = draw_source {
            rendered.clone()
        } else {
            return Err(PilError::InternalError(
                "GPU draw operation is missing its control canvas".into(),
            ));
        };
        return Ok(Some(Arc::new(DynamicImage::ImageRgba8(
            rendered.to_rgba8(),
        ))));
    }
    if let Some(auxiliary) = extract_effect_spread_or_lut_auxiliary(op, primary_dimensions) {
        return auxiliary.map(Some);
    }
    if let PipelineOp::EffectNoise { sigma } = op {
        let (width, height) = primary_dimensions.ok_or_else(|| {
            PilError::InternalError("GPU EffectNoise is missing primary dimensions".into())
        })?;
        let values =
            crate::compute::pool_cpu::ops::effects::effect_noise_values(width, height, *sigma)?;
        let mut bytes = Vec::with_capacity(values.len().saturating_mul(4));
        for value in values {
            bytes.extend_from_slice(&[value, 0, 0, 255]);
        }
        let synthetic = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
            PilError::InternalError("GPU EffectNoise buffer shape mismatch".into())
        })?;
        return Ok(Some(Arc::new(DynamicImage::ImageRgba8(synthetic))));
    }
    if let PipelineOp::Merge { bands, .. } = op {
        // Merge's first band is the current pipeline image. Pack the remaining
        // single-band values consecutively into a synthetic Luma image so the
        // ordinary dual-input binding can carry all bands without a new ABI.
        let first = bands
            .first()
            .ok_or_else(|| PilError::ValueError("GPU Merge requires at least one band".into()))?;
        let (w, h) = first.size()?;
        let pixels = CheckedDims::new(w, h, 1)?.total_pixels();
        let extras = bands.len().saturating_sub(1);
        if extras == 0 {
            return Ok(None);
        }
        let packed_width = pixels.checked_mul(extras).ok_or_else(|| {
            PilError::ValueError("GPU Merge auxiliary band dimensions overflow".into())
        })?;
        let packed_width = u32::try_from(packed_width).map_err(|_| {
            PilError::ValueError("GPU Merge auxiliary band dimensions exceed u32".into())
        })?;
        let mut values = Vec::with_capacity(packed_width as usize);
        for band in bands.iter().skip(1) {
            let image = band.materialize_for_ops()?;
            if image.dimensions() != (w, h) {
                return Err(PilError::ValueError(
                    "GPU Merge band dimensions do not match".into(),
                ));
            }
            values.extend(image.to_luma8().into_raw());
        }
        let synthetic = crate::raster::GrayImage::from_raw(packed_width, 1, values)
            .ok_or_else(|| PilError::InternalError("GPU Merge auxiliary shape mismatch".into()))?;
        return Ok(Some(Arc::new(DynamicImage::ImageLuma8(synthetic))));
    }
    if let PipelineOp::Paste { source, .. } = op {
        // Paste has already converted its source to the destination mode. Keep
        // P-mode sources as their one-byte indices here: expanding them through
        // the palette before packing the GPU upload would make the GPU lane
        // disagree with the CPU and SIMD paste implementations.
        return source.materialized_shared().map(Some);
    }
    if let PipelineOp::CompositeModule { other, .. } = op
        && matches!(other.mode()?.as_str(), "P" | "PA")
    {
        // Image.composite blends indexed samples and gives the result
        // image2's palette. Upload image2's raw index/(index, alpha)
        // bytes, not its visible RGB(A) expansion.
        return other.materialized_shared().map(Some);
    }

    let arc_img: Option<&std::sync::Arc<crate::image::Image>> = match op {
        PipelineOp::Add { other, .. }
        | PipelineOp::Subtract { other, .. }
        | PipelineOp::Multiply { other }
        | PipelineOp::Screen { other }
        | PipelineOp::Darker { other }
        | PipelineOp::Lighter { other }
        | PipelineOp::Difference { other }
        | PipelineOp::Overlay { other }
        | PipelineOp::HardLight { other }
        | PipelineOp::SoftLight { other }
        | PipelineOp::AddModulo { other }
        | PipelineOp::SubtractModulo { other }
        | PipelineOp::LogicalAnd { other }
        | PipelineOp::LogicalOr { other }
        | PipelineOp::LogicalXor { other }
        | PipelineOp::BlendModule { other, .. }
        | PipelineOp::CompositeModule { other, .. } => Some(other),
        PipelineOp::AlphaComposite { source, .. } => Some(source),
        _ => None,
    };
    arc_img
        .map(|image| image.materialized_shared().map(Some))
        .unwrap_or(Ok(None))
}

/// Extract the third image (mask) from a 3-input PipelineOp, if present.
/// Returns shared materialized pixels ready for GPU upload.
pub(super) fn extract_third_image(op: &PipelineOp) -> Result<Option<Arc<DynamicImage>>, PilError> {
    match op {
        PipelineOp::CompositeModule { mask, .. } | PipelineOp::EqualizeMasked { mask } => {
            mask.materialized_shared().map(Some)
        }
        PipelineOp::Paste { mask, .. } => mask
            .as_ref()
            .map(|image| image.materialized_shared())
            .transpose(),
        PipelineOp::Autocontrast { mask, .. } => mask
            .as_ref()
            .map(|image| image.materialized_shared())
            .transpose(),
        // PutAlphaData owns an operation-ready L mask already. Reuse its Arc
        // directly so adding the GPU mask binding does not create a second
        // host image or trigger another materialization.
        PipelineOp::PutAlphaData { mask, .. } => Ok(Some(Arc::clone(mask))),
        _ => Ok(None),
    }
}

/// Count the samples participating in an Autocontrast histogram. This is
/// scalar control-plane work only; the pixel histogram itself is accumulated
/// by the GPU gather pass. Keeping the count on the host lets the cutoff pass
/// use exact integer thresholds instead of depending on device f32 rounding.
pub(super) fn gpu_autocontrast_selected_pixels(
    width: u32,
    height: u32,
    mask: Option<&DynamicImage>,
) -> Result<usize, PilError> {
    let image_pixels = CheckedDims::new(width, height, 1)?.total_pixels();
    let Some(mask) = mask else {
        return Ok(image_pixels);
    };
    if mask.dimensions() != (width, height) {
        return Err(PilError::ValueError(
            "Autocontrast mask dimensions do not match image".into(),
        ));
    }
    Ok(mask
        .to_luma8()
        .pixels()
        .filter(|pixel| pixel[0] != 0)
        .count())
}

pub(super) fn gpu_autocontrast_cutoff_indices(selected_pixels: usize, cutoff: f64) -> (u32, u32) {
    if selected_pixels == 0 {
        return (0, 0);
    }
    let total = selected_pixels as f64;
    let low = (total * cutoff / 100.0) as usize;
    let high = (total * (100.0 - cutoff) / 100.0) as usize;
    (
        low.min(u32::MAX as usize) as u32,
        high.min(selected_pixels.saturating_sub(1))
            .min(u32::MAX as usize) as u32,
    )
}

pub(super) struct AuxiliaryImages {
    pub(super) second: Option<Arc<DynamicImage>>,
    pub(super) third: Option<Arc<DynamicImage>>,
}

pub(super) fn extract_auxiliary_images(
    op: &PipelineOp,
    primary_dimensions: (u32, u32),
    draw_source: Option<&DynamicImage>,
) -> Result<AuxiliaryImages, PilError> {
    // Pillow resolves each operation's source before its mask, then advances
    // to the next operation. Preserve that observable error order instead of
    // collecting one auxiliary slot across the whole batch at a time.
    Ok(AuxiliaryImages {
        second: extract_second_image(op, Some(primary_dimensions), draw_source)?,
        third: extract_third_image(op)?,
    })
}

pub(super) fn resolve_batch_auxiliary_images(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Result<Vec<AuxiliaryImages>, PilError> {
    let mut dimensions = image.dimensions();
    let mut images = Vec::with_capacity(ops.len());
    // Non-drawing batches only need dimensions and auxiliary handles. Borrow
    // the source until drawing produces a preview, avoiding a discarded
    // full-frame clone for ordinary batches.
    let mut draw_preview: Option<DynamicImage> = None;
    let mut draw_preview_valid = true;
    for op in ops {
        let rendered = if draw_preview_valid && crate::compute::pool_cpu::ops::draw::is_draw_op(op)
        {
            Some(crate::compute::pool_cpu::ops::draw::execute_draw_batch(
                draw_preview.as_ref().unwrap_or(image),
                std::slice::from_ref(op),
                mode,
            )?)
        } else {
            None
        };
        images.push(extract_auxiliary_images(op, dimensions, rendered.as_ref())?);
        if let Some(rendered) = rendered {
            draw_preview = Some(rendered);
        } else if !crate::compute::pool_cpu::ops::draw::is_draw_op(op) {
            // A draw after an arbitrary GPU operation needs the preceding
            // native result as its canvas; do not render against stale pixels.
            draw_preview_valid = false;
        }
        if let Some(next) = op_output_dims(op, dimensions.0, dimensions.1) {
            dimensions = next;
        }
    }
    Ok(images)
}
