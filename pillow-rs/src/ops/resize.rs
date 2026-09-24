use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::GenericImageView;

/// Public resampling input accepted by Pillow resize-like methods.
#[derive(Debug, Clone)]
pub enum ResampleInput {
    /// A Pillow `Image.Resampling` integer code.
    Code(i64),
    /// A Pillow resampling filter name or an invalid name to report.
    Name(String),
}

const RESAMPLE_GUIDANCE: &str = concat!(
    "Use Image.Resampling.NEAREST (0), Image.Resampling.LANCZOS (1), ",
    "Image.Resampling.BILINEAR (2), Image.Resampling.BICUBIC (3), ",
    "Image.Resampling.BOX (4) or Image.Resampling.HAMMING (5)"
);

fn unknown_resample(value: impl std::fmt::Display) -> PilError {
    PilError::ValueError(format!(
        "Unknown resampling filter ({}). {}",
        value, RESAMPLE_GUIDANCE
    ))
}

fn parse_resample_name(name: &str) -> Result<ResampleFilter, PilError> {
    match name {
        "BICUBIC" | "bicubic" => Ok(ResampleFilter::Bicubic),
        "NEAREST" | "nearest" => Ok(ResampleFilter::Nearest),
        "BILINEAR" | "bilinear" => Ok(ResampleFilter::Bilinear),
        "LANCZOS" | "lanczos" => Ok(ResampleFilter::Lanczos),
        "BOX" | "box" => Ok(ResampleFilter::Box),
        "HAMMING" | "hamming" => Ok(ResampleFilter::Hamming),
        other => Err(unknown_resample(other)),
    }
}

/// Parses a public Pillow resampling value.
pub fn parse_resample_input(input: Option<ResampleInput>) -> Result<ResampleFilter, PilError> {
    match input {
        None => Ok(ResampleFilter::Bicubic),
        Some(ResampleInput::Code(code)) => match code {
            0 => Ok(ResampleFilter::Nearest),
            1 => Ok(ResampleFilter::Lanczos),
            2 => Ok(ResampleFilter::Bilinear),
            3 => Ok(ResampleFilter::Bicubic),
            4 => Ok(ResampleFilter::Box),
            5 => Ok(ResampleFilter::Hamming),
            other => Err(unknown_resample(other)),
        },
        Some(ResampleInput::Name(name)) => parse_resample_name(&name),
    }
}

/// Parses a Pillow resampling filter name.
///
/// `None` defaults to [`ResampleFilter::Bicubic`], matching Pillow's default
/// for resize-like methods.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `s` is not a supported filter name.
pub fn parse_resample(s: Option<&str>) -> Result<ResampleFilter, PilError> {
    parse_resample_input(s.map(|name| ResampleInput::Name(name.to_owned())))
}

impl Image {
    /// Returns a resized image from Pillow's public input representation.
    ///
    /// The original image is unchanged. Modes `"1"` and `"P"` force nearest
    /// sampling; `"PA"` filters its raw index and alpha bands independently.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for invalid dimensions or unknown filters.
    pub fn resize(
        &self,
        size: (i64, i64),
        filter: Option<ResampleInput>,
        box_coords: Option<(i32, i32, i32, i32)>,
    ) -> Result<Image, PilError> {
        self.resize_with_options(
            size,
            filter,
            box_coords.map(|(left, top, right, bottom)| {
                (
                    f64::from(left),
                    f64::from(top),
                    f64::from(right),
                    f64::from(bottom),
                )
            }),
            None,
        )
    }

    /// Resizes a floating source region, optionally reducing by integer factors first.
    ///
    /// `reducing_gap` follows Pillow: values below one are invalid, and
    /// nearest/indexed and filtered LA/RGBA paths ignore a valid gap.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for invalid dimensions, filter, gap, or source bounds.
    pub fn resize_with_options(
        &self,
        size: (i64, i64),
        filter: Option<ResampleInput>,
        box_coords: Option<(f64, f64, f64, f64)>,
        reducing_gap: Option<f64>,
    ) -> Result<Image, PilError> {
        let mut filter = parse_resample_input(filter)?;
        if reducing_gap.is_some_and(|gap| gap < 1.0) {
            return Err(PilError::ValueError(
                "reducing_gap must be 1.0 or greater".into(),
            ));
        }
        if matches!(self, Image::Bytes { .. }) {
            // Pillow loads an encoded source during resize(), so decoding
            // errors belong to this call, not a later output observation.
            // Keep the shared decode cache without cloning the pixel buffer.
            self.materialized_shared()?;
        }
        let (w, h) = positive_dimensions(size, "height and width must be > 0")?;
        let (source_w, source_h) = self.size()?;
        let full_box = (0.0, 0.0, f64::from(source_w), f64::from(source_h));
        let mut bounds = box_coords.unwrap_or(full_box);
        if (w, h) == (source_w, source_h) && bounds == full_box {
            // The resize executors already copy identity geometry before
            // filtering. Retain their ordinary backend execution boundary.
            return self.resize_with_filter((w, h), filter);
        }
        let mode = self.mode()?;
        if matches!(mode.as_str(), "1" | "P") {
            filter = ResampleFilter::Nearest;
        }
        let mut source = self.clone();
        // Pillow's alpha recursion omits reducing_gap after premultiplying.
        // Applying reduction to ordinary RGBA/LA first changes its rounding.
        if let Some(gap) = reducing_gap
            && !matches!(filter, ResampleFilter::Nearest)
            && !matches!(mode.as_str(), "LA" | "RGBA")
        {
            let factor = |extent: f64, output: u32| -> Result<u32, PilError> {
                let value = extent / f64::from(output) / gap;
                if value.is_nan() {
                    return Err(PilError::ValueError(
                        "cannot convert float NaN to integer".into(),
                    ));
                }
                if value.is_infinite() {
                    return Err(PilError::OverflowError(
                        "cannot convert float infinity to integer".into(),
                    ));
                }
                Ok((value as u32).max(1))
            };
            let factor_x = factor(bounds.2 - bounds.0, w)?;
            let factor_y = factor(bounds.3 - bounds.1, h)?;
            if factor_x > 1 || factor_y > 1 {
                // Pillow's integer reducer does not accept 16-bit modes,
                // although its ordinary filtered resampler does.
                if mode.starts_with("I;16") {
                    return Err(PilError::ValueError("image has wrong mode".into()));
                }
                validate_resize_box(bounds, (source_w, source_h))?;
                let (_, support) = crate::ops::pil_resize::filter_from_resample(filter);
                let support_x = (support - 0.5) * (bounds.2 - bounds.0) / f64::from(w);
                let support_y = (support - 0.5) * (bounds.3 - bounds.1) / f64::from(h);
                // Keep the filter halo when reducing an interior box. Pillow
                // truncates left/top and rounds right/bottom upward here.
                let safe = (
                    (bounds.0 - support_x).trunc().max(0.0) as u32,
                    (bounds.1 - support_y).trunc().max(0.0) as u32,
                    (bounds.2 + support_x).ceil().min(f64::from(source_w)) as u32,
                    (bounds.3 + support_y).ceil().min(f64::from(source_h)) as u32,
                );
                if safe != (0, 0, source_w, source_h) {
                    source = source.crop(Some((
                        safe.0 as i32,
                        safe.1 as i32,
                        safe.2 as i32,
                        safe.3 as i32,
                    )))?;
                }
                source = source.reduce(factor_x, factor_y)?;
                bounds = (
                    (bounds.0 - f64::from(safe.0)) / f64::from(factor_x),
                    (bounds.1 - f64::from(safe.1)) / f64::from(factor_y),
                    (bounds.2 - f64::from(safe.0)) / f64::from(factor_x),
                    (bounds.3 - f64::from(safe.1)) / f64::from(factor_y),
                );
            }
        }
        let source_size = source.size()?;
        let bounds = validate_resize_box(bounds, source_size)?;
        let full_source = (0.0, 0.0, f64::from(source_size.0), f64::from(source_size.1));
        let vertical_first =
            u64::from(source_size.1) > u64::from(source_size.0) * 100 && h < source_size.1;
        if bounds == full_source && !vertical_first {
            return source.resize_with_filter((w, h), filter);
        }
        Ok(Image::push_op(
            &source,
            PipelineOp::ResizeBoxed {
                w,
                h,
                filter,
                box_coords: bounds,
            },
        ))
    }

    fn resize_with_filter(
        &self,
        (w, h): (u32, u32),
        mut filter: ResampleFilter,
    ) -> Result<Image, PilError> {
        // Only single-band indexed samples force nearest. PA retains the
        // requested filter over its independent raw index and alpha bands.
        if self.has_palette_mode() || self.explicit_mode() == Some("1") {
            filter = ResampleFilter::Nearest;
        }
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }

    /// Queues an in-place Pillow-style thumbnail resize from public input.
    ///
    /// Indexed modes, including `"PA"`, force nearest sampling to preserve the
    /// raw sample layout.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for negative dimensions or
    /// [`PilError::ZeroDivisionError`] for a single zero dimension. An
    /// all-zero request is a Pillow no-op.
    pub fn thumbnail(
        &mut self,
        size: (i64, i64),
        filter: Option<ResampleInput>,
    ) -> Result<(), PilError> {
        // Pillow's in-place thumbnail loads the source before returning from
        // the public call. Materialize here so deferred codec failures are
        // reported at thumbnail(), rather than being delayed until a later
        // observation of the mutated image.
        let source_size = self.materialize()?.dimensions();
        // Pillow returns before evaluating the aspect-ratio division when
        // both requested bounds already contain the source. This also covers
        // empty images: every non-negative bound contains a (0, 0) source,
        // so thumbnail((0, 2)) is a no-op rather than division by zero.
        if size.0 >= i64::from(source_size.0)
            && size.1 >= i64::from(source_size.1)
            && size.0 >= 0
            && size.1 >= 0
        {
            return Ok(());
        }
        let dimensions = Self::thumbnail_dimensions(size, source_size)?;
        if dimensions == (0, 0) {
            return Ok(());
        }
        let mut filter = parse_resample_input(filter)?;
        if source_size.0 == 0 || source_size.1 == 0 {
            // Pillow's aspect-preserving size can be positive even when the
            // source has a zero dimension. Its subsequent Image.resize call
            // then rejects the zero-sized source before any pixels are produced.
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
        self.thumbnail_with_filter(dimensions, &mut filter)
    }

    fn thumbnail_with_filter(
        &mut self,
        (w, h): (u32, u32),
        filter: &mut ResampleFilter,
    ) -> Result<(), PilError> {
        // PIL forces NEAREST for indexed samples, including PA's raw
        // index/alpha pairs, to preserve the sample layout.
        if self.has_palette_mode() || matches!(self.explicit_mode(), Some("1") | Some("PA")) {
            *filter = ResampleFilter::Nearest;
        }
        let new_self = Image::push_op(
            self,
            PipelineOp::Thumbnail {
                w,
                h,
                filter: *filter,
            },
        );
        *self = new_self;
        Ok(())
    }

    fn thumbnail_dimensions(
        size: (i64, i64),
        (source_width, source_height): (u32, u32),
    ) -> Result<(u32, u32), PilError> {
        if source_height == 0 {
            // Pillow evaluates the source aspect ratio before validating the
            // requested bounds whenever the early no-op check did not fire.
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        if source_width == 0 {
            return Self::thumbnail_dimensions_zero_width(size);
        }
        // Pillow evaluates x / y before it rejects a negative width. Preserve
        // its division error for requests containing a zero dimension.
        if size.0 == 0 || size.1 == 0 {
            if size.0 == 0 && size.1 == 0 && source_width == 0 && source_height == 0 {
                return Ok((0, 0));
            }
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        if size.0 < 0 {
            return Err(PilError::ValueError("scale must be > 0".into()));
        }

        // Pillow accepts arbitrarily large positive bounds for thumbnail and
        // clamps the eventual result to the source dimensions. Keep the
        // public bound in i64 until after validation; converting the request
        // directly to u32 incorrectly rejected values such as 2**32.
        let requested_width = thumbnail_bound(size.0);
        let width = if source_width == 0 {
            requested_width
        } else {
            requested_width.min(source_width)
        };
        if size.1 < 0 {
            return Ok((
                width,
                Self::thumbnail_height_for_width(width, (source_width, source_height))?,
            ));
        }
        let requested_height = thumbnail_bound(size.1);
        let height = if source_height == 0 {
            // Pillow raises ZeroDivisionError for a positive thumbnail bound
            // when the source has no rows; do this at the public call instead
            // of queueing an operation that would fail only during execution.
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        } else {
            requested_height.min(source_height)
        };
        // Pillow stores the aspect-preserving dimensions in the thumbnail
        // operation itself. Keeping the caller's rectangular bounds here
        // makes `size` report the request rather than the eventual image, and
        // also causes deferred backends to plan the wrong geometry. Match
        // `Image.thumbnail`'s round_aspect floor/ceil choice before queuing.
        let aspect = source_width as f64 / source_height as f64;
        if width as f64 / height as f64 >= aspect {
            let adjusted = round_aspect(height as f64 * aspect, |candidate| {
                (aspect - candidate / height as f64).abs()
            });
            Ok((adjusted, height))
        } else {
            let adjusted = round_aspect(width as f64 / aspect, |candidate| {
                if candidate == 0.0 {
                    0.0
                } else {
                    (aspect - width as f64 / candidate).abs()
                }
            });
            Ok((width, adjusted))
        }
    }

    fn thumbnail_dimensions_zero_width(size: (i64, i64)) -> Result<(u32, u32), PilError> {
        // Pillow's preserve_aspect_ratio computes an aspect of 0.0 for a
        // zero-width, nonempty source and evaluates x / y before choosing its
        // branch. Keep those divisions observable: the branch that can resize
        // rounds x to one, after which Image.resize rejects the source.
        if size.1 == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        let ratio = size.0 as f64 / size.1 as f64;
        if ratio < 0.0 {
            return Err(PilError::ZeroDivisionError("float division by zero".into()));
        }
        if size.1 < 0 {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
        Ok((1, thumbnail_bound(size.1)))
    }

    fn thumbnail_height_for_width(
        width: u32,
        (source_width, source_height): (u32, u32),
    ) -> Result<u32, PilError> {
        if source_width == 0 {
            let message = if source_height == 0 {
                "division by zero"
            } else {
                "float division by zero"
            };
            return Err(PilError::ZeroDivisionError(message.into()));
        }
        if source_height == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        let aspect = source_width as f64 / source_height as f64;
        let number = width as f64 / aspect;
        Ok(round_aspect(number, |candidate| {
            if candidate == 0.0 {
                0.0
            } else {
                (aspect - width as f64 / candidate).abs()
            }
        }))
    }
}

fn validate_resize_box(
    bounds: (f64, f64, f64, f64),
    size: (u32, u32),
) -> Result<(f64, f64, f64, f64), PilError> {
    // _imaging.c reads a float32 box before checking its bounds. A double
    // just outside an edge may round back to that edge and remain valid.
    let bounds = (
        f64::from(bounds.0 as f32),
        f64::from(bounds.1 as f32),
        f64::from(bounds.2 as f32),
        f64::from(bounds.3 as f32),
    );
    if bounds.0 < 0.0 || bounds.1 < 0.0 {
        return Err(PilError::ValueError("box offset can't be negative".into()));
    }
    if bounds.2 > f64::from(size.0 as f32) || bounds.3 > f64::from(size.1 as f32) {
        return Err(PilError::ValueError(
            "box can't exceed original image size".into(),
        ));
    }
    if bounds.2 < bounds.0 || bounds.3 < bounds.1 {
        return Err(PilError::ValueError("box can't be empty".into()));
    }
    Ok(bounds)
}

fn positive_dimensions(size: (i64, i64), message: &str) -> Result<(u32, u32), PilError> {
    if size.0 <= 0 || size.1 <= 0 {
        return Err(PilError::ValueError(message.into()));
    }
    let width = u32::try_from(size.0)
        .map_err(|_| PilError::OverflowError("signed integer is greater than maximum".into()))?;
    let height = u32::try_from(size.1)
        .map_err(|_| PilError::OverflowError("signed integer is greater than maximum".into()))?;
    Ok((width, height))
}

fn thumbnail_bound(value: i64) -> u32 {
    value.min(i64::from(u32::MAX)) as u32
}

fn round_aspect(number: f64, key: impl Fn(f64) -> f64) -> u32 {
    let floor = number.floor();
    let ceil = number.ceil();
    let best = if key(floor) <= key(ceil) { floor } else { ceil };
    best.max(1.0) as u32
}

#[cfg(test)]
mod tests {
    use super::{Image, PilError};

    #[test]
    fn thumbnail_empty_source_nonnegative_bounds_is_noop() {
        let mut image = Image::new(0, 0, "RGB", (1, 2, 3, 255)).unwrap();

        image.thumbnail((0, 2), None).unwrap();

        assert_eq!(image.size().unwrap(), (0, 0));
    }

    #[test]
    fn thumbnail_zero_width_source_matches_resize_validation() {
        let mut image = Image::new(0, 2, "RGB", (1, 2, 3, 255)).unwrap();

        let error = image.thumbnail((1, 1), None).unwrap_err();

        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
        assert_eq!(image.size().unwrap(), (0, 2));
    }

    #[test]
    fn thumbnail_zero_width_source_preserves_aspect_division_order() {
        let mut image = Image::new(0, 2, "RGB", (1, 2, 3, 255)).unwrap();

        let error = image.thumbnail((-1, 1), None).unwrap_err();

        assert!(matches!(
            error,
            PilError::ZeroDivisionError(message) if message == "float division by zero"
        ));
        assert_eq!(image.size().unwrap(), (0, 2));
    }

    #[test]
    fn thumbnail_zero_height_source_preserves_aspect_division_order() {
        let mut image = Image::new(2, 0, "RGB", (1, 2, 3, 255)).unwrap();

        let error = image.thumbnail((1, 1), None).unwrap_err();

        assert!(matches!(
            error,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
        assert_eq!(image.size().unwrap(), (2, 0));
    }
}
