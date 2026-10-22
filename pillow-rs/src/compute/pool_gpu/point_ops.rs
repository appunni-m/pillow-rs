use super::*;

mod native_byte_op;

/// Extract and pack LUT data from a PipelineOp into [u32; 256] for GPU upload.
/// Each u32 packs RGBA channels for one LUT entry (R in byte 0, G byte 1, B byte 2, A byte 3).
pub(super) fn extract_lut(op: &PipelineOp, mode: u32) -> Option<[u32; 256]> {
    if let PipelineOp::RemapPalette { dest_map } = op {
        let mut inverse = [0u8; 256];
        for (new_index, &old_index) in dest_map.iter().take(256).enumerate() {
            inverse[usize::from(old_index)] = new_index as u8;
        }
        let mut packed = [0u32; 256];
        for (entry, &value) in packed.iter_mut().zip(inverse.iter()) {
            let value = u32::from(value);
            *entry = value | (value << 8) | (value << 16) | (value << 24);
        }
        return Some(packed);
    }
    if let PipelineOp::Colorize {
        black,
        white,
        mid,
        blackpoint,
        midpoint,
        whitepoint,
    } = op
    {
        let channels = crate::compute::pool_cpu::ops::imageops::colorize_lut(
            black,
            white,
            *mid,
            *blackpoint,
            *midpoint,
            *whitepoint,
        );
        let mut packed = [0u32; 256];
        for (index, entry) in packed.iter_mut().enumerate() {
            *entry = u32::from(channels[0][index])
                | (u32::from(channels[1][index]) << 8)
                | (u32::from(channels[2][index]) << 16)
                | (u32::from(u8::MAX) << 24);
        }
        return Some(packed);
    }
    let lut_bytes: &[u8] = match op {
        PipelineOp::Eval { lut } => lut.as_ref(),
        _ => return None,
    };
    let channels = match mode {
        0 => 1usize,
        1 => 2,
        2 => 3,
        3 | 4 => 4,
        _ => return None,
    };
    if lut_bytes.len() != channels * 256 {
        return None;
    }
    let mut packed = [0u32; 256];
    // CPU Eval/PointOp stores one complete 256-entry table per logical band.
    // Pack those band-major tables into the per-index RGBA transport used by
    // the shader. Alpha is looked up for LA/RGBA; opaque modes use 255.
    for (i, p) in packed.iter_mut().enumerate() {
        let r = u32::from(lut_bytes[i]);
        let g = if channels >= 3 {
            u32::from(lut_bytes[256 + i])
        } else {
            r
        };
        let b = if channels >= 3 {
            u32::from(lut_bytes[512 + i])
        } else {
            r
        };
        let a = if channels == 2 {
            u32::from(lut_bytes[256 + i])
        } else if channels == 4 {
            u32::from(lut_bytes[768 + i])
        } else {
            255
        };
        *p = r | (g << 8) | (b << 16) | (a << 24);
    }
    Some(packed)
}

/// Build the byte LUT represented by a GPU-compatible point operation.
///
/// `Invert`, `InvertChops`, `Solarize`, and `Posterize` normally have their
/// own shaders.  For a contiguous run of point operations their per-channel
/// byte semantics are exactly representable by the generic LUT shader.  The
/// mode restrictions mirror the public ImageOps constructors: alpha-bearing
/// Solarize/Posterize operations stay on their existing shader path, while
/// explicit Eval/PointOp tables remain valid for every packed byte mode.
pub(super) fn gpu_point_lut(op: &PipelineOp, mode: u32) -> Option<Vec<u8>> {
    let channels = match mode {
        0 => 1usize,
        1 => 2,
        2 => 3,
        3 | 4 => 4,
        _ => return None,
    };
    if let PipelineOp::Eval { lut } = op {
        return (lut.len() == channels * 256).then(|| lut.to_vec());
    }
    if matches!(
        op,
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. }
    ) && matches!(mode, 1 | 3)
    {
        return None;
    }

    let map = |value: u8| -> Option<u8> {
        match op {
            PipelineOp::Invert | PipelineOp::InvertChops => Some(255 - value),
            PipelineOp::Solarize { threshold } => Some(if value >= *threshold {
                255 - value
            } else {
                value
            }),
            PipelineOp::Posterize { bits } if (1..=8).contains(bits) => {
                let mask = !((1u8 << (8 - bits)) - 1);
                Some(value & mask)
            }
            _ => None,
        }
    };
    let mut lut = Vec::with_capacity(channels * 256);
    for _ in 0..channels {
        for value in 0..=u8::MAX {
            lut.push(map(value)?);
        }
    }
    Some(lut)
}

/// Return whether a logical mode has the same byte-band contract as the
/// concrete image storage.  The point LUT composes complete byte-band maps,
/// so this is exact for ordinary L/LA/RGB/RGBA images even when the public
/// lazy image retains its explicit mode tag.  Palette and typed modes remain
/// excluded because their public point operations have additional sample
/// semantics beyond a packed byte lookup.
pub(super) fn gpu_byte_point_mode_allowed(image: &DynamicImage, mode: Option<&str>) -> bool {
    match image {
        DynamicImage::ImageLuma8(_) => matches!(mode, None | Some("L")),
        DynamicImage::ImageLumaA8(_) => matches!(mode, None | Some("LA")),
        DynamicImage::ImageRgb8(_) => matches!(mode, None | Some("RGB")),
        DynamicImage::ImageRgba8(_) => matches!(mode, None | Some("RGBA")),
        _ => false,
    }
}

#[cfg(target_endian = "little")]
pub(super) fn gpu_native_byte_op_channels(
    op: &PipelineOp,
    image: &DynamicImage,
    mode: Option<&str>,
) -> Option<u8> {
    match op {
        PipelineOp::Duplicate if image.width() > 0 && image.height() > 0 => match image {
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
        },
        PipelineOp::Brightness { factor }
            if registry::gpu_brightness_factor_int(*factor).is_some() =>
        {
            match image {
                DynamicImage::ImageLuma8(_)
                    if matches!(mode, None | Some("L"))
                        && image.width() > 0
                        && image.height() > 0 =>
                {
                    Some(1)
                }
                DynamicImage::ImageLumaA8(_)
                    if matches!(mode, None | Some("LA"))
                        && image.width() > 0
                        && image.height() > 0 =>
                {
                    Some(2)
                }
                DynamicImage::ImageRgb8(_)
                    if matches!(mode, None | Some("RGB"))
                        && image.width() > 0
                        && image.height() > 0 =>
                {
                    Some(3)
                }
                _ => None,
            }
        }
        PipelineOp::PutAlpha {
            mode: PixelMode::LA,
            ..
        } => match image {
            DynamicImage::ImageLumaA8(_)
                if matches!(mode, None | Some("LA")) && image.width() > 0 && image.height() > 0 =>
            {
                Some(2)
            }
            _ => None,
        },
        PipelineOp::PutAlpha {
            mode: PixelMode::PA,
            ..
        } => match image {
            DynamicImage::ImageLumaA8(_)
                if mode == Some("PA") && image.width() > 0 && image.height() > 0 =>
            {
                Some(2)
            }
            _ => None,
        },
        PipelineOp::Invert => match image {
            DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L" | "P")) => Some(1),
            DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
            DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
            DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some(4),
            _ => None,
        },
        // ImageChops.invert treats P pixels as raw palette indices. Keep that
        // one-byte sample domain through dispatch and readback instead of
        // expanding indices into a color carrier.
        PipelineOp::InvertChops => match image {
            DynamicImage::ImageLuma8(_)
                if mode == Some("P") && image.width() > 0 && image.height() > 0 =>
            {
                Some(1)
            }
            _ => None,
        },
        PipelineOp::Posterize { bits } if (1..=8).contains(bits) => match image {
            DynamicImage::ImageLuma8(_)
                if matches!(mode, None | Some("L")) && image.width() > 0 && image.height() > 0 =>
            {
                Some(1)
            }
            DynamicImage::ImageRgb8(_)
                if matches!(mode, None | Some("RGB"))
                    && image.width() > 0
                    && image.height() > 0 =>
            {
                Some(3)
            }
            _ => None,
        },
        PipelineOp::Multiply { .. }
        | PipelineOp::Screen { .. }
        | PipelineOp::Overlay { .. }
        | PipelineOp::HardLight { .. }
        | PipelineOp::SoftLight { .. }
        | PipelineOp::Difference { .. }
        | PipelineOp::Darker { .. }
        | PipelineOp::Lighter { .. }
        | PipelineOp::AddModulo { .. }
        | PipelineOp::SubtractModulo { .. } => gpu_native_multiply_channels(image, mode),
        PipelineOp::LogicalAnd { .. }
        | PipelineOp::LogicalOr { .. }
        | PipelineOp::LogicalXor { .. }
            if mode == Some("1") && matches!(image, DynamicImage::ImageLuma8(_)) =>
        {
            Some(1)
        }
        PipelineOp::BlendModule { .. } => match image {
            DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
            DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "La")) => Some(2),
            DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
                Some(3)
            }
            DynamicImage::ImageRgba8(_)
                if matches!(mode, None | Some("RGBA" | "RGBa" | "RGBX" | "CMYK")) =>
            {
                Some(4)
            }
            _ => None,
        },
        PipelineOp::Solarize { .. } => match image {
            DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
            DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
            _ => None,
        },
        PipelineOp::AlphaComposite {
            dest: (0, 0),
            src: (0, 0),
            ..
        } => match image {
            DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
            DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some(4),
            _ => None,
        },
        _ => None,
    }
}

/// Select native byte transport for full-size, same-mode ImageChops.composite
/// inputs. Other mask modes, mixed operands, clipping and empty geometry keep
/// their established packed-RGBA execution path.
#[cfg(target_endian = "little")]
pub(super) fn gpu_native_composite_layout(
    image: &DynamicImage,
    mode: Option<&str>,
    other: &crate::image::Image,
    other_image: &DynamicImage,
    mask: &crate::image::Image,
    mask_image: &DynamicImage,
    mask_alpha: bool,
) -> Option<(u8, u8, u8)> {
    let (logical_mode, channels) = match image {
        DynamicImage::ImageLuma8(_) => ("L", 1),
        DynamicImage::ImageLumaA8(_) => ("LA", 2),
        DynamicImage::ImageRgb8(_) => ("RGB", 3),
        DynamicImage::ImageRgba8(_) => ("RGBA", 4),
        _ => return None,
    };
    if mode.is_some_and(|mode| mode != logical_mode)
        || other.mode().ok()?.as_str() != logical_mode
        || image.dimensions() != other_image.dimensions()
        || image.dimensions() != other.size().ok()?
        || image.color() != other_image.color()
        || image.dimensions() != mask.size().ok()?
        || image.dimensions() != mask_image.dimensions()
    {
        return None;
    }

    let mask_mode = mask.mode().ok()?;
    let (mask_channels, mask_channel) = match (mask_mode.as_str(), mask_alpha, mask_image) {
        ("1" | "L", false, DynamicImage::ImageLuma8(_)) => (1, 0),
        ("LA", true, DynamicImage::ImageLumaA8(_)) => (2, 1),
        ("RGBA" | "RGBa", true, DynamicImage::ImageRgba8(_)) => (4, 3),
        _ => return None,
    };
    let pixels = (image.width() as usize).checked_mul(image.height() as usize)?;
    let input_length = pixels.checked_mul(usize::from(channels))?;
    let mask_length = pixels.checked_mul(usize::from(mask_channels))?;
    if image.as_bytes().len() != input_length
        || other_image.as_bytes().len() != input_length
        || mask_image.as_bytes().len() != mask_length
        || pixels == 0
    {
        return None;
    }
    Some((channels, mask_channels, mask_channel))
}

/// Raw Multiply treats every stored byte as an independent sample.
pub(super) fn gpu_native_multiply_channels(image: &DynamicImage, mode: Option<&str>) -> Option<u8> {
    match image {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("1" | "L" | "P")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA" | "La" | "PA")) => Some(2),
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

/// Collapse adjacent exact point operations into one generic LUT dispatch.
///
/// The public operation count remains the original count in the outer
/// telemetry receipt; only the GPU execution plan is rewritten.  Non-point
/// operations terminate a run, so geometry, neighborhood, and multi-image
/// ordering are unchanged.
pub(super) fn fuse_gpu_point_ops(ops: &[PipelineOp], mode: u32) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let Some(first) = gpu_point_lut(&ops[index], mode) else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut composed = first;
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let Some(next) = gpu_point_lut(&ops[index + consumed], mode) else {
                break;
            };
            for band in 0..(composed.len() / 256) {
                let offset = band * 256;
                for value in 0..256usize {
                    composed[offset + value] = next[offset + composed[offset + value] as usize];
                }
            }
            consumed += 1;
        }
        if consumed >= 2 {
            fused.push(PipelineOp::Eval {
                lut: composed.into(),
            });
        } else {
            fused.push(ops[index].clone());
        }
        index += consumed;
    }
    fused
}
