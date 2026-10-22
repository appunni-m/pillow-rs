use super::*;

pub(super) fn gpu_thumbnail_output_dims(
    source_width: u32,
    source_height: u32,
    target_width: u32,
    target_height: u32,
) -> Option<(u32, u32)> {
    if source_width == 0 || source_height == 0 || target_width == 0 || target_height == 0 {
        return None;
    }
    // `Image::thumbnail` computes the aspect-preserving final dimensions
    // before queuing its lazy operation. Recomputing the ratio here would
    // apply the adjustment twice and make backend shape planning disagree
    // with the public image size.
    Some((
        target_width.min(source_width).max(1),
        target_height.min(source_height).max(1),
    ))
}

pub(super) fn gpu_thumbnail_requires_exact_host_control(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    image: &DynamicImage,
    mode: Option<&str>,
) -> bool {
    let PipelineOp::Thumbnail { w, h, filter } = op else {
        return false;
    };
    let Some((output_width, output_height)) =
        gpu_thumbnail_output_dims(source_dimensions.0, source_dimensions.1, *w, *h)
    else {
        return true;
    };
    let effective_filter = if matches!(mode, Some("1" | "P")) {
        ResampleFilter::Nearest
    } else {
        *filter
    };
    if matches!(effective_filter, ResampleFilter::Nearest) {
        return false;
    }
    // Thumbnail's reducing_gap=2.0 pass is skipped for alpha images and for
    // scalar F/I contracts.  A plain Resize is exact in those cases.  For
    // ordinary byte images the pool expands the divisible integer-reduce
    // case into native Reduce + Resize below; only partial edge blocks still
    // require the host's fractional resize box.
    let has_alpha = matches!(
        image,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) && !matches!(mode, Some("F" | "I" | "CMYK" | "RGBa" | "RGBX"));
    let factor_x = ((f64::from(source_dimensions.0) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y =
        ((f64::from(source_dimensions.1) / f64::from(output_height) / 2.0) as u32).max(1);
    // When both reducing-gap factors are one, Pillow's F thumbnail is exactly
    // the ordinary filtered Resize. Let the geometry expander lower it to
    // that path so marker-9 can prove heterogeneous/non-dyadic words instead
    // of needlessly publishing an exact-host receipt.
    if mode == Some("F") && factor_x == 1 && factor_y == 1 {
        return false;
    }
    // A finite constant F source is invariant under both Thumbnail's
    // reducing-gap pass and its final resize. Lower this narrow case to the
    // exact constant Resize marker instead of materializing the scalar
    // reducing pass on the host. Typed I still needs its integer rounding
    // contract and remains on exact host semantic control.
    if mode == Some("F") && gpu_f_thumbnail_constant_is_exact(op, source_dimensions, image, mode) {
        return false;
    }
    if matches!(mode, Some("F" | "I")) {
        return true;
    }
    if has_alpha {
        return false;
    }
    (factor_x > 1 || factor_y > 1)
        && (source_dimensions.0 % factor_x != 0 || source_dimensions.1 % factor_y != 0)
}

/// Return whether a constant F Thumbnail can skip the scalar reducing-gap
/// pass without changing Pillow's observable word.
///
/// A zero source remains zero for every finite block average. Nonzero
/// constants are admitted only when both reducing factors are one; Pillow's
/// f32 reduction can otherwise overflow or round the repeated sum before the
/// final resize (even though a direct normalized resize would preserve the
/// constant word).
pub(super) fn gpu_f_thumbnail_constant_is_exact(
    op: &PipelineOp,
    source_dimensions: (u32, u32),
    image: &DynamicImage,
    mode: Option<&str>,
) -> bool {
    let Some(bits) = gpu_f_source_constant_bits(image, mode) else {
        return false;
    };
    if bits == 0 {
        return true;
    }
    let PipelineOp::Thumbnail { w, h, .. } = op else {
        return false;
    };
    let Some((output_width, output_height)) =
        gpu_thumbnail_output_dims(source_dimensions.0, source_dimensions.1, *w, *h)
    else {
        return false;
    };
    let factor_x = ((f64::from(source_dimensions.0) / f64::from(output_width) / 2.0) as u32).max(1);
    let factor_y =
        ((f64::from(source_dimensions.1) / f64::from(output_height) / 2.0) as u32).max(1);
    factor_x == 1 && factor_y == 1
}
