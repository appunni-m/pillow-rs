use super::*;

fn resolve_centering(input: CenteringInput) -> Result<(f64, f64), PilError> {
    match input {
        CenteringInput::Default => Ok((0.5, 0.5)),
        CenteringInput::Scalar(_) => Err(PilError::TypeError(
            "cannot unpack non-iterable float object".into(),
        )),
        CenteringInput::Values(values) if values.len() == 2 => Ok((
            normalize_fit_centering(values[0]),
            normalize_fit_centering(values[1]),
        )),
        CenteringInput::Values(values) if values.len() < 2 => Err(PilError::ValueError(format!(
            "not enough values to unpack (expected 2, got {})",
            values.len()
        ))),
        CenteringInput::Values(_) => Err(PilError::ValueError(
            "too many values to unpack (expected 2)".into(),
        )),
        CenteringInput::Invalid => Err(PilError::TypeError(
            "cannot unpack non-iterable NoneType object".into(),
        )),
    }
}

/// Pillow replaces each out-of-range `ImageOps.fit` centering coordinate with
/// `0.5` before computing its crop box. This also handles NaN and infinities,
/// for which Rust's floating-point `clamp` would otherwise preserve a value
/// that can make boxed coefficient spans invalid.
fn normalize_fit_centering(value: f64) -> f64 {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        value
    } else {
        0.5
    }
}

/// Resolve `ImageOps.pad` centering only after `contain` has determined that
/// padding is needed.
///
/// Pillow indexes only the coordinate for the axis that receives padding. A
/// one-element sequence is therefore valid when width is padded, and extra
/// sequence values are ignored. This is deliberately separate from
/// `ImageOps.fit`, which unpacks both coordinates before doing any work.
fn resolve_pad_centering(
    input: CenteringInput,
    width_padded: bool,
    height_padded: bool,
) -> Result<(f64, f64), PilError> {
    match input {
        CenteringInput::Default => Ok((0.5, 0.5)),
        CenteringInput::Scalar(_) => Err(PilError::TypeError(
            "'float' object is not subscriptable".into(),
        )),
        CenteringInput::Invalid => Err(PilError::TypeError(
            "'NoneType' object is not subscriptable".into(),
        )),
        CenteringInput::Values(values) => {
            let x = if width_padded {
                values
                    .first()
                    .copied()
                    .ok_or_else(|| PilError::IndexError("tuple index out of range".into()))?
            } else {
                0.5
            };
            let y = if height_padded {
                values
                    .get(1)
                    .copied()
                    .ok_or_else(|| PilError::IndexError("tuple index out of range".into()))?
            } else {
                0.5
            };
            Ok((x, y))
        }
    }
}

/// Python's `round()` for the positive dimensions used by `ImageOps.contain`.
fn round_positive_ties_even(value: f64) -> u32 {
    let floor = value.floor();
    let fraction = value - floor;
    let rounded = if fraction < 0.5 {
        floor
    } else if fraction > 0.5 || (floor as u64) % 2 == 1 {
        floor + 1.0
    } else {
        floor
    };
    rounded.max(0.0).min(f64::from(u32::MAX)) as u32
}

/// Return the exact contain dimensions Pillow computes before `Image.resize`.
/// A rounded axis of zero is intentionally preserved: Pillow passes that
/// result to `resize`, which raises `ValueError("height and width must be > 0")`
/// rather than silently clamping the axis to one.
fn pad_containment_dimensions(
    source_w: u32,
    source_h: u32,
    target_w: u32,
    target_h: u32,
) -> Option<(u32, u32)> {
    if source_h == 0 || target_h == 0 {
        return None;
    }
    let source_ratio = f64::from(source_w) / f64::from(source_h);
    let destination_ratio = f64::from(target_w) / f64::from(target_h);
    if (source_ratio - destination_ratio).abs() < 1e-10 {
        Some((target_w, target_h))
    } else if source_ratio > destination_ratio {
        Some((
            target_w,
            round_positive_ties_even(
                f64::from(source_h) / f64::from(source_w) * f64::from(target_w),
            ),
        ))
    } else {
        Some((
            round_positive_ties_even(
                f64::from(source_w) / f64::from(source_h) * f64::from(target_h),
            ),
            target_h,
        ))
    }
}

/// Return the dimensions that `ImageOps.contain` passes to `Image.resize`.
///
/// Pillow evaluates both aspect-ratio divisions in `ImageOps.contain` before
/// it calls `Image.resize`, so zero source/target heights must be reported as
/// `ZeroDivisionError` at this public boundary.  Rounded-zero output axes are
/// deliberately returned for the later resize validation step: Pillow parses
/// the resampling filter before `Image.resize` rejects those dimensions.
fn contain_dimensions(image: &Image, w: u32, h: u32) -> Result<((u32, u32), (u32, u32)), PilError> {
    let source = image.size()?;
    if source.1 == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let dimensions = pad_containment_dimensions(source.0, source.1, w, h)
        .expect("non-zero contain heights were validated above");
    Ok((source, dimensions))
}

/// Validate the dimensions produced by `ImageOps.contain` or `ImageOps.cover`
/// before queuing the resize. Pillow's empty-width source is the one valid
/// zero-axis result: when its height is unchanged, the resize request equals
/// the source and `Image.resize` returns a copy without rejecting width zero.
fn validate_aspect_resize_dimensions(
    (source_w, source_h): (u32, u32),
    (new_w, new_h): (u32, u32),
) -> Result<(), PilError> {
    let empty_width_copy = source_w == 0 && source_h != 0 && new_w == 0 && new_h == source_h;
    if (new_w == 0 || new_h == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

/// Return the dimensions that `ImageOps.cover` passes to `Image.resize`.
///
/// Unlike `contain`, a zero-width source can only reach the resize call when
/// the destination ratio is also zero. If the destination has positive width,
/// Pillow divides by the source width while calculating the covering height
/// and raises `ZeroDivisionError` before filter validation.
fn cover_dimensions(image: &Image, w: u32, h: u32) -> Result<((u32, u32), (u32, u32)), PilError> {
    let source = image.size()?;
    if source.1 == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source.0) / f64::from(source.1);
    let destination_ratio = f64::from(w) / f64::from(h);
    let dimensions = if (image_ratio - destination_ratio).abs() < 1e-10 {
        (w, h)
    } else if image_ratio < destination_ratio {
        if source.0 == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        (
            w,
            round_positive_ties_even(f64::from(source.1) / f64::from(source.0) * f64::from(w)),
        )
    } else {
        (
            round_positive_ties_even(f64::from(source.0) / f64::from(source.1) * f64::from(h)),
            h,
        )
    };
    Ok((source, dimensions))
}

/// Return which axes `ImageOps.pad` will fill after its `contain` step.
/// `None` preserves deferred error handling for zero-sized inputs, where
/// Pillow evaluates the aspect-ratio division before it can inspect color or
/// centering.
fn pad_containment_axes(image: &Image, w: u32, h: u32) -> Result<Option<(bool, bool)>, PilError> {
    let (iw, ih) = image.size()?;
    Ok(pad_containment_dimensions(iw, ih, w, h).map(|(new_w, new_h)| (new_w != w, new_h != h)))
}
fn parse_imageops_filter(
    input: Option<ResampleInput>,
) -> Result<crate::pipeline::ResampleFilter, PilError> {
    match input {
        // Pillow's ImageOps ``method=`` parameter does not accept a string;
        // the parity adapter only materializes enum names for ``resample=``.
        // Keep this distinction visible instead of making the four method
        // error cases accidentally succeed.
        Some(ResampleInput::Name(name)) => Err(PilError::ValueError(format!(
            "Unknown resampling filter ({name}). Use Image.Resampling.NEAREST (0), \
             Image.Resampling.LANCZOS (1), Image.Resampling.BILINEAR (2), \
             Image.Resampling.BICUBIC (3), Image.Resampling.BOX (4) or \
             Image.Resampling.HAMMING (5)"
        ))),
        input => parse_resample_input(input),
    }
}

/// Validates the resampling argument accepted by `ImageOps.deform`.
///
/// Normalize the target facade's symbolic names and validate numeric values;
/// the current mesh backend uses its established nearest-neighbor sampling.
pub fn validate_deform_resample(input: Option<ResampleInput>) -> Result<(), PilError> {
    parse_resample_input(input).map(|_| ())
}
/// Adds a border around an image.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn expand(image: &Image, border: u32, fill: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Expand { border, fill }))
}

/// Expands an image after resolving the fill against its native Pillow mode.
///
/// Keeping the host value intact until the mode is known preserves native
/// component counts and meanings: CMYK's fourth component is K, RGBX's is X,
/// and a three-component color receives Pillow's opaque fourth byte for both.
pub fn expand_with_input(
    image: &Image,
    border: u32,
    fill: ImageOpsColor,
) -> Result<Image, PilError> {
    let mode = image.mode()?;
    let fill = resolve_imageops_color(fill, &mode)?.unwrap_or((0, 0, 0, 0));
    expand(image, border, fill)
}

/// Resizes an image to fit within `(w, h)` while preserving aspect ratio.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn contain(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let (source, dimensions) = contain_dimensions(image, w, h)?;
    let filter = parse_resample(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// `ImageOps.contain` with a host-neutral public filter value.
pub fn contain_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    if filter.is_none() {
        return contain(image, w, h, None);
    }
    let (source, dimensions) = contain_dimensions(image, w, h)?;
    let filter = parse_imageops_filter(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// Resizes an image to cover `(w, h)` while preserving its aspect ratio.
///
/// The returned dimensions may exceed `(w, h)`; overflow is not cropped.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn cover(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let (source, dimensions) = cover_dimensions(image, w, h)?;
    let filter = parse_resample(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Cover { w, h, filter }))
}

/// `ImageOps.cover` with a host-neutral public filter value.
pub fn cover_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    if filter.is_none() {
        return cover(image, w, h, None);
    }
    let (source, dimensions) = cover_dimensions(image, w, h)?;
    let filter = parse_imageops_filter(filter)?;
    validate_aspect_resize_dimensions(source, dimensions)?;
    Ok(Image::push_op(image, PipelineOp::Cover { w, h, filter }))
}

/// Resizes and crops an image to exactly fit `(w, h)`.
///
/// `bleed` and `centering` follow Pillow `ImageOps.fit` semantics.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn fit(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    bleed: f64,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let bleed = normalize_fit_bleed(bleed);
    let centering = (
        normalize_fit_centering(centering.0),
        normalize_fit_centering(centering.1),
    );
    // Pillow computes both aspect ratios inside ImageOps.fit before calling
    // Image.resize (and therefore before Image.resize parses its filter).
    // Keep zero-height and zero-target errors at this eager boundary instead
    // of letting a deferred backend clamp them to one pixel.
    validate_fit_geometry(image, w, h, bleed)?;
    let filter = parse_resample(filter)?;
    validate_fit_resize_dimensions(image, w, h, bleed)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        },
    ))
}

/// `ImageOps.fit` with filter and centering validation owned by core.
pub fn fit_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
    bleed: f64,
    centering: CenteringInput,
) -> Result<Image, PilError> {
    let filter_was_none = filter.is_none();
    // Pillow treats an explicit `(0.5, 0.5)` pair exactly like the omitted
    // default, but it still preserves an explicitly supplied resampling
    // method. Normalize the centering in core so that the default path cannot
    // discard that method before building the pipeline operation.
    let centering = match centering {
        CenteringInput::Values(values)
            if values.len() == 2 && values[0] == 0.5 && values[1] == 0.5 =>
        {
            CenteringInput::Default
        }
        centering => centering,
    };
    let centering = resolve_centering(centering)?;
    let bleed = normalize_fit_bleed(bleed);
    // ImageOps.fit performs its centering unpack and crop-ratio divisions
    // before delegating to Image.resize. In particular, a zero source height
    // or target height must win over an invalid resampling value, while a
    // zero-width source can still produce a valid black positive result.
    validate_fit_geometry(image, w, h, bleed)?;
    let filter = parse_imageops_filter(filter)?;
    validate_fit_resize_dimensions(image, w, h, bleed)?;
    if filter_was_none && centering == (0.5, 0.5) {
        return fit(image, w, h, None, bleed, centering);
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        },
    ))
}

// Pillow's ImageOps.fit normalizes invalid bleed values before calculating the
// crop box. Keep this at the public core boundary so CPU, SIMD, and GPU receive
// the same deferred operation value instead of each reimplementing the rule.
fn normalize_fit_bleed(bleed: f64) -> f64 {
    if 0.0 <= bleed && bleed < 0.5 {
        bleed
    } else {
        0.0
    }
}

/// Validate the geometry and error ordering performed by `ImageOps.fit` before
/// its lazy boxed resize is queued. Pillow evaluates the source/live aspect
/// ratio and output aspect ratio in Python before parsing the resize filter.
fn validate_fit_geometry(
    image: &Image,
    target_width: u32,
    target_height: u32,
    bleed: f64,
) -> Result<(), PilError> {
    let (source_width, source_height) = image.size()?;
    if source_height == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    let bleed_height = bleed * f64::from(source_height);
    let live_height = f64::from(source_height) - 2.0 * bleed_height;
    if live_height == 0.0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    // This is the first division in Pillow's fit implementation.
    let live_width = f64::from(source_width) - 2.0 * bleed * f64::from(source_width);
    let _live_ratio = live_width / live_height;
    // The output ratio is evaluated next, before Image.resize parses its
    // filter or validates a zero-width target.
    if target_height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let _output_ratio = f64::from(target_width) / f64::from(target_height);

    Ok(())
}

/// Validate the dimensions that Pillow's `Image.resize` checks after its
/// filter has been parsed. Keeping this separate from [`validate_fit_geometry`]
/// preserves the native precedence of an invalid filter over a zero-width
/// resize, while source/target height divisions still win earlier.
fn validate_fit_resize_dimensions(
    image: &Image,
    target_width: u32,
    target_height: u32,
    bleed: f64,
) -> Result<(), PilError> {
    let (source_width, source_height) = image.size()?;
    if target_width == 0 {
        // Image.resize can return an empty copy only when the source and
        // requested dimensions remain identical and no bleed changes the
        // boxed source height.
        let empty_width_copy = source_width == 0 && target_height == source_height && bleed == 0.0;
        if !empty_width_copy {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    } else if source_width == 0 && target_height < source_height {
        // A zero-width boxed source is accepted by Pillow only when the
        // positive destination has at least as many rows as the source.
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

/// Resizes and pads an image to exactly `(w, h)`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `filter` is unknown.
pub fn pad(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    if w > 0 && h > 0 && image.size()? == (w, h) {
        // `ImageOps.pad` returns an independent copy when no resize or border
        // is needed. Skip a queued Pad pass so every backend avoids the
        // identity placement work, including callers below the Python facade.
        return Ok(image.copy());
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    ))
}

/// `ImageOps.pad` with filter and centering validation owned by core.
pub fn pad_with_input(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<ResampleInput>,
    color: ImageOpsColor,
    centering: CenteringInput,
) -> Result<Image, PilError> {
    let filter_was_none = filter.is_none();
    let color_was_none = matches!(&color, ImageOpsColor::None);
    let filter = parse_imageops_filter(filter)?;

    // Pillow's `pad` calls `contain` before touching either optional boundary
    // argument. If the resized image already has the requested dimensions,
    // malformed color/centering values are intentionally ignored.
    let containment_axes = pad_containment_axes(image, w, h)?;
    // Pillow evaluates ``contain`` before constructing the padded canvas. A
    // zero-height source or destination therefore raises division by zero at
    // the public call boundary. A zero-width destination normally reaches
    // ``Image.new`` and raises its non-positive-dimension ValueError, except
    // when an empty-width source keeps the same height: that contain result
    // remains empty and can be padded. Keep these checks after
    // ``pad_containment_axes`` so the aspect-ratio guard remains exercised by
    // valid public zero-dimension inputs.
    let (_, source_height) = image.size()?;
    if source_height == 0 || h == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let (source_width, _) = image.size()?;
    if w == 0 && !(source_width == 0 && source_height == h) {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    if let Some((new_w, new_h)) = pad_containment_dimensions(source_width, source_height, w, h) {
        let zero_axis = new_w == 0 || new_h == 0;
        let zero_width_source_resize = source_width == 0 && new_h == source_height;
        if zero_axis && !zero_width_source_resize {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    }
    if containment_axes == Some((false, false)) {
        if (source_width, source_height) == (w, h) {
            // ImageOps.pad still returns an independent copy when the input
            // already has the requested size. Match Image.copy semantics
            // (including clearing source format metadata) without scheduling
            // a no-op Pad pass on CPU, SIMD, or GPU.
            return Ok(image.copy());
        }
        return Ok(Image::push_op(
            image,
            PipelineOp::Pad {
                w,
                h,
                filter,
                color: None,
                centering: (0.5, 0.5),
            },
        ));
    }

    let (width_padded, height_padded) = containment_axes.unwrap_or((true, true));
    let centering = resolve_pad_centering(centering, width_padded, height_padded)?;
    let color = resolve_imageops_color(color, &image.mode()?)?;
    if filter_was_none && color_was_none && centering == (0.5, 0.5) {
        return pad(image, w, h, None, color, centering);
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    ))
}

/// `ImageOps.scale` with the integer/enum resampling value exposed by Pillow.
pub fn scale_with_input(
    image: &Image,
    factor: f64,
    filter: Option<ResampleInput>,
) -> Result<Image, PilError> {
    // Pillow's ``factor <= 0`` check intentionally does not catch NaN.  The
    // later Python ``round`` then reports the conversion error for NaN, while
    // positive infinity reports its distinct overflow error. Preserve those
    // public distinctions before constructing the lazy resize.
    if factor.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".to_owned(),
        ));
    }
    // Pillow returns ``image.copy()`` before it inspects ``resample`` when
    // the factor is exactly one. Keep this fast path ahead of filter parsing,
    // including for empty images and invalid filter values.
    if factor == 1.0 {
        return Ok(image.copy());
    }
    if factor <= 0.0 {
        return Err(PilError::ValueError(
            "the factor must be greater than 0".to_owned(),
        ));
    }

    // ImageOps.scale computes dimensions with Python's ties-to-even round and
    // immediately delegates to Image.resize.  Resize rejects a rounded zero;
    // do that check here so the lazy target has the same public call boundary
    // as Pillow instead of allowing its backend's minimum-size clamp to turn
    // the request into a 1x1 image.
    let (width, height) = image.size()?;
    let rounded_dimension = |dimension: u32| {
        let value = f64::from(dimension) * factor;
        // ``round(inf)`` and ``round(nan)`` fail before Pillow delegates to
        // Image.resize. In particular, ``inf * 0`` is NaN, so an empty image
        // receives the NaN conversion error rather than the infinity one.
        if value.is_nan() {
            return Err(PilError::ValueError(
                "cannot convert float NaN to integer".to_owned(),
            ));
        }
        if value.is_infinite() {
            return Err(PilError::OverflowError(
                "cannot convert float infinity to integer".to_owned(),
            ));
        }
        let lower = value.floor();
        let fraction = value - lower;
        let rounded = if fraction < 0.5 {
            lower
        } else if fraction > 0.5 || (lower as u64) % 2 == 1 {
            lower + 1.0
        } else {
            lower
        };
        if rounded > f64::from(u32::MAX) {
            Err(PilError::OverflowError(
                "signed integer is greater than maximum".into(),
            ))
        } else {
            Ok(rounded as u32)
        }
    };
    let rounded_width = rounded_dimension(width)?;
    let rounded_height = rounded_dimension(height)?;

    // The input language records Image.Resampling enum members by name. The
    // Python source side materializes that name as an IntEnum; normalize the
    // equivalent target-facade representation here for this `resample=`
    // parameter only. ImageOps method parameters intentionally use the
    // stricter parser above.
    let filter = parse_resample_input(filter)?;

    // Pillow's resize accepts the all-zero image/result pair. Other rounded
    // zero dimensions still fail at the resize boundary; do not let the
    // backend's minimum-size clamp turn them into a one-pixel image.
    if rounded_width == 0 || rounded_height == 0 {
        if width == 0 && height == 0 {
            return Ok(image.copy());
        }
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }

    Ok(Image::push_op(image, PipelineOp::Scale { factor, filter }))
}

/// Crops the same border width from every image edge.
///
/// # Errors
///
/// Returns Pillow's width-first or height-second crop-coordinate error when
/// the requested border is strictly larger than half of the corresponding
/// dimension.  Pillow validates the derived box in `ImageOps.crop` before it
/// returns a lazy image; doing the same here prevents an invalid request from
/// queueing a `CropBorder` operation and then failing only at materialization.
pub fn crop(image: &Image, border: u32) -> Result<Image, PilError> {
    let (width, height) = image.size()?;
    // Use division instead of `2 * border` so an oversized u32 border cannot
    // wrap before the comparison.  Equality is valid and produces Pillow's
    // empty image; only a strictly larger border is rejected.
    if border > width / 2 {
        return Err(PilError::ValueError(
            "Coordinate 'right' is less than 'left'".into(),
        ));
    }
    if border > height / 2 {
        return Err(PilError::ValueError(
            "Coordinate 'lower' is less than 'upper'".into(),
        ));
    }
    Ok(Image::push_op(image, PipelineOp::CropBorder { border }))
}
