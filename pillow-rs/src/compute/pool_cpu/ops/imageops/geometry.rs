use super::*;
use crate::compute::{Backend, ExecutionContext};

/// Contain: resize to fit within (w, h) preserving aspect ratio.
/// PIL: adjusts one dimension using round(), does not truncate.
pub fn op_contain(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // Pillow's ImageOps.contain evaluates source and destination aspect
    // ratios before calling Image.resize, so either zero height raises
    // ZeroDivisionError instead of flowing into a deferred empty image.
    if ih == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio > dest_ratio {
        // Image is wider: adjust height
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is taller: adjust width
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    // Image.resize rejects rounded-zero dimensions.  The one valid exception
    // is an empty-width source whose contain height is unchanged: that
    // request equals the source and Pillow returns an empty copy.
    let empty_width_copy = iw == 0 && nw == 0 && nh == ih;
    if (nw == 0 || nh == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    // Pillow preserves a zero dimension when the source aspect-ratio math
    // rounds one axis to zero. `pil_resize` has an explicit empty-image path;
    // clamping here would turn a valid 0×N result into 1×N.
    let result = pil_resize(img, nw, nh, filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Cover: resize to cover (w, h) preserving aspect ratio.
/// PIL: adjusts one dimension using round(), does NOT crop.
pub fn op_cover(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // Pillow's ImageOps.cover evaluates source and destination aspect ratios
    // before calling Image.resize. Preserve its division errors instead of
    // allowing a deferred empty image or a zero target height to flow into
    // Rust's non-panicking floating-point arithmetic.
    if ih == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let im_ratio = iw as f64 / ih as f64;
    let dest_ratio = w as f64 / h as f64;
    let (nw, nh) = if (im_ratio - dest_ratio).abs() < 1e-10 {
        (w, h)
    } else if im_ratio < dest_ratio {
        // Image is taller: adjust height to cover
        if iw == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        let new_h = bankers_round(ih as f64 / iw as f64 * w as f64) as u32;
        (w, new_h)
    } else {
        // Image is wider: adjust width to cover
        let new_w = bankers_round(iw as f64 / ih as f64 * h as f64) as u32;
        (new_w, h)
    };
    // Image.resize rejects rounded-zero dimensions. The one valid exception
    // is an empty-width source whose cover height is unchanged, for which the
    // resize request equals the source and Pillow returns an empty copy.
    let empty_width_copy = iw == 0 && nw == 0 && nh == ih;
    if (nw == 0 || nh == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    let result = pil_resize(img, nw, nh, filter, explicit_mode);
    Ok(preserve_mode(img, result))
}

/// Fit: resize to fit within (w, h) with bleed and centering, then crop.
/// PIL: applies bleed to source, computes crop box, resize with box parameter.
/// Uses PIL's exact box-based resize to match pixel-perfect output.
pub fn op_fit(
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    op_fit_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        img,
        w,
        h,
        filter,
        bleed,
        centering,
        explicit_mode,
    )
}

pub(crate) fn op_fit_in_context(
    context: &ExecutionContext,
    img: &DynamicImage,
    w: u32,
    h: u32,
    filter: ResampleFilter,
    bleed: f64,
    centering: (f64, f64),
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (iw, ih) = (img.width(), img.height());
    // ImageOps.fit performs these divisions before Image.resize sees the
    // boxed target. Keep the eager public checks mirrored here for callers
    // that execute a deferred pipeline through an explicit CPU backend.
    if ih == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    if h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    if iw == 0 {
        // A zero-width source can be copied only when the fit request keeps
        // the empty dimensions and does not alter the boxed source height.
        if w == 0 && h == ih && bleed == 0.0 {
            return Ok(img.clone());
        }
        if w == 0 || h < ih {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    } else if w == 0 {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    // Bleed pixels (PIL: bleed * image.size)
    let bleed_w = bleed * iw as f64;
    let bleed_h = bleed * ih as f64;
    // Live size
    // ImageOps.fit keeps the live dimensions as the exact positive result of
    // subtracting the bleed. Pillow does not clamp either axis to one pixel;
    // doing so changes the crop box for small images with non-zero bleed.
    let live_w = iw as f64 - 2.0 * bleed_w;
    let live_h = ih as f64 - 2.0 * bleed_h;
    let live_ratio = live_w / live_h;
    let output_ratio = w as f64 / h as f64;
    // Compute crop dimensions (PIL: floats, no rounding)
    let (crop_w, crop_h) = if (live_ratio - output_ratio).abs() < 1e-10 {
        (live_w, live_h)
    } else if live_ratio >= output_ratio {
        // Live is wider: crop sides
        (output_ratio * live_h, live_h)
    } else {
        // Live is taller: crop top/bottom
        (live_w, live_w / output_ratio)
    };
    // Compute crop position with centering (PIL: floats, no rounding)
    let crop_left = bleed_w + (live_w - crop_w) * centering.0;
    let crop_top = bleed_h + (live_h - crop_h) * centering.1;
    // Use PIL's box-based resize (maps source box to target size)
    // Pillow's P resize path forces nearest-neighbour sampling even when
    // ImageOps.fit received another method. PA is different: its two raw
    // bands are passed through the requested filter and stay indexed.
    let resize_filter = if explicit_mode == Some("P") {
        ResampleFilter::Nearest
    } else {
        filter
    };
    let result = crate::ops::pil_resize::pil_resize_boxed_in_context(
        context,
        img,
        w,
        h,
        crop_left,
        crop_top,
        crop_left + crop_w,
        crop_top + crop_h,
        resize_filter,
        explicit_mode,
    );
    Ok(preserve_mode(img, result))
}

/// Build the padded output in the exact byte layout of canonical L, LA, RGB,
/// HSV, RGBA, RGBX, or CMYK images. Other logical modes keep the general compatibility path.
pub fn op_crop_border(img: &DynamicImage, border: u32) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width(), img.height());
    // Pillow permits a border exactly half the image size and returns a
    // zero-sized image; only a strictly oversized border is invalid.
    if border > w / 2 {
        // Pillow delegates this invalid box to Image.crop(), whose public
        // contract reports the right edge being left of the left edge when
        // the width is the first invalid dimension.
        return Err(PilError::ValueError(
            "Coordinate 'right' is less than 'left'".into(),
        ));
    }
    if border > h / 2 {
        // Keep the height-specific crop error observable for rectangular
        // inputs instead of collapsing it into the width diagnostic above.
        return Err(PilError::ValueError(
            "Coordinate 'lower' is less than 'upper'".into(),
        ));
    }

    // DynamicImage::crop_imm dispatches through generic pixel access, which
    // makes this byte-preserving operation several times slower than Pillow
    // for large rasters. Copy complete native rows instead. Typed and
    // floating-point images retain the image crate's existing behavior.
    let channels = match img {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return Ok(img.crop_imm(border, border, w - 2 * border, h - 2 * border)),
    };
    let output_width = w - 2 * border;
    let output_height = h - 2 * border;
    let output_dims = CheckedDims::new_allow_empty(output_width, output_height, channels as u8)?;
    let source_stride = (w as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("CropBorder source stride overflow".into()))?;
    let output_stride = (output_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("CropBorder output stride overflow".into()))?;
    // The crop overwrites every output byte; reserve without zero-filling the
    // destination so each pixel is written only once.
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let source = img.as_bytes();
    let x_offset = border as usize * channels;
    for y in 0..output_height as usize {
        let source_start = (border as usize + y)
            .checked_mul(source_stride)
            .and_then(|offset| offset.checked_add(x_offset))
            .ok_or_else(|| PilError::ValueError("CropBorder source offset overflow".into()))?;
        let source_end = source_start
            .checked_add(output_stride)
            .ok_or_else(|| PilError::ValueError("CropBorder source range overflow".into()))?;
        let source_row = source.get(source_start..source_end).ok_or_else(|| {
            PilError::InternalError("CropBorder source buffer shape mismatch".into())
        })?;
        output.extend_from_slice(source_row);
    }
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        output_width,
        output_height,
        output,
        channels,
    )?;
    Ok(preserve_mode(img, result))
}

/// Scale: resize by a floating-point factor.
pub fn op_scale(
    img: &DynamicImage,
    factor: f64,
    filter: ResampleFilter,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // ImageOps.scale uses Python's round(image_dimension * factor), whose
    // ties-to-even behavior is observable at .5 dimensions (13 * 1.5 -> 20
    // while 11 * 1.5 -> 16).
    let new_w = bankers_round(img.width() as f64 * factor) as u32;
    let new_h = bankers_round(img.height() as f64 * factor) as u32;
    let result = pil_resize(img, new_w.max(1), new_h.max(1), filter, explicit_mode);
    Ok(preserve_mode(img, result))
}
