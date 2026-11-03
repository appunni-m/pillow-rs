use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{GenericImageView, Mode};

mod reducing_gap;
use reducing_gap::apply_reducing_gap;

/// Public resampling input accepted by Pillow resize-like methods.
#[derive(Debug, Clone)]
pub enum ResampleInput {
    /// A Pillow `Image.Resampling` integer code.
    Code(i64),
    /// A Pillow resampling filter name or an invalid name to report.
    Name(String),
}

/// Geometry selected by Pillow's thumbnail size preflight.
///
/// Keep this plan across host-side input coercion. Python callbacks can mutate
/// the wrapped image between preflight and execution, but Pillow has already
/// selected the output dimensions by then.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThumbnailPlan {
    dimensions: (u32, u32),
}

impl ThumbnailPlan {
    /// Returns the target size selected by preflight.
    pub const fn output_size(self) -> (u32, u32) {
        self.dimensions
    }
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

/// Check Pillow's cumulative boxed-nearest coordinates for an identity axis.
fn nearest_box_axis_maps_identity(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> bool {
    if source_size == 0
        || source_size != output_size
        || !box_start.is_finite()
        || !box_end.is_finite()
    {
        return false;
    }
    let box_start = f64::from(box_start as f32);
    let box_end = f64::from(box_end as f32);
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    if !scale.is_finite() {
        return false;
    }
    let last = i64::from(source_size - 1);
    let mut coordinate = box_start + scale * 0.5;
    for expected in 0..output_size {
        if (coordinate.floor() as i64).clamp(0, last) as u32 != expected {
            return false;
        }
        coordinate += scale;
    }
    true
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
        let mode = self.mode_identity()?;
        let full_box = (0.0, 0.0, f64::from(source_w), f64::from(source_h));
        let mut bounds = box_coords.unwrap_or(full_box);
        if (w, h) == (source_w, source_h) && bounds == full_box {
            // The resize executors already copy identity geometry before
            // filtering. Retain their ordinary backend execution boundary.
            return self.resize_with_filter((w, h), filter, mode);
        }
        if matches!(mode, Mode::One | Mode::ImagePalette8) {
            filter = ResampleFilter::Nearest;
        }
        if let Some(identity_resize) =
            self.identity_boxed_nearest_resize(filter, (w, h), (source_w, source_h), mode, bounds)?
        {
            return Ok(identity_resize);
        }
        let mut source = self.clone();
        // Pillow's alpha recursion omits reducing_gap after premultiplying.
        // Applying reduction to ordinary RGBA/LA first changes its rounding.
        if let Some(gap) = reducing_gap
            && !matches!(filter, ResampleFilter::Nearest)
            && !matches!(mode, Mode::ImageLumaA8 | Mode::ImageRgba8)
        {
            (source, bounds) = apply_reducing_gap(
                source,
                bounds,
                gap,
                filter,
                mode,
                (w, h),
                (source_w, source_h),
            )?;
        }
        let source_size = source.size()?;
        let bounds = validate_resize_box(bounds, source_size)?;
        let full_source = (0.0, 0.0, f64::from(source_size.0), f64::from(source_size.1));
        let vertical_first = u64::from(source_size.1) > u64::from(source_size.0) * 100
            && h < source_size.1
            && w != source_size.0;
        if bounds == full_source && !vertical_first {
            return source.resize_with_filter((w, h), filter, mode);
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

    #[inline(always)]
    fn identity_boxed_nearest_resize(
        &self,
        filter: ResampleFilter,
        output_size: (u32, u32),
        source_size: (u32, u32),
        mode: Mode,
        bounds: (f64, f64, f64, f64),
    ) -> Result<Option<Image>, PilError> {
        if mode == Mode::ImageF32
            && matches!(filter, ResampleFilter::Nearest)
            && output_size == source_size
        {
            // Validate before the no-op check to keep Pillow's box errors.
            // A fractional box can still map every nearest coordinate to the
            // same sample; return a logically independent copy that shares
            // immutable storage instead of queuing a full pixel gather.
            let bounds = validate_resize_box(bounds, source_size)?;
            if nearest_box_axis_maps_identity(source_size.0, output_size.0, bounds.0, bounds.2)
                && nearest_box_axis_maps_identity(source_size.1, output_size.1, bounds.1, bounds.3)
            {
                return Ok(Some(self.copy()));
            }
        }
        Ok(None)
    }

    fn resize_with_filter(
        &self,
        (w, h): (u32, u32),
        filter: ResampleFilter,
        mode: Mode,
    ) -> Result<Image, PilError> {
        let filter = crate::compute::pool_cpu::ops::resize::ops::effective_filter(mode, filter);
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }

    /// Prepares a Pillow-style thumbnail from the image's current dimensions.
    ///
    /// This preflight reads the source dimensions, validates output geometry
    /// before resampling input, and returns `None` for Pillow's early no-op
    /// and for requests whose computed output equals the source. Known-size
    /// lazy sources stay unmaterialized. Bindings retain the plan while
    /// coercing host values, then pass it to [`Self::thumbnail_with_plan`].
    ///
    /// # Errors
    ///
    /// Returns the same size and geometry errors as the initial phase of
    /// [`Self::thumbnail`].
    pub fn prepare_thumbnail(
        &self,
        size: (i64, i64),
    ) -> Result<Option<ThumbnailPlan>, PilError> {
        let source_size = self.size()?;
        if size.0 >= i64::from(source_size.0)
            && size.1 >= i64::from(source_size.1)
            && size.0 >= 0
            && size.1 >= 0
        {
            return Ok(None);
        }
        let dimensions = Self::thumbnail_dimensions(size, source_size)?;
        if dimensions == (0, 0) || dimensions == source_size {
            return Ok(None);
        }
        Ok(Some(ThumbnailPlan { dimensions }))
    }

    /// Reports whether a Pillow-style thumbnail request reaches its resize path.
    ///
    /// Known-size lazy sources stay unmaterialized during this geometry check.
    pub fn thumbnail_needs_resize(&self, size: (i64, i64)) -> Result<bool, PilError> {
        Ok(self.prepare_thumbnail(size)?.is_some())
    }

    /// Queues an in-place Pillow-style thumbnail resize from public input.
    ///
    /// Indexed modes, including `"PA"`, force nearest sampling to preserve the
    /// raw sample layout.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for invalid computed dimensions or
    /// [`PilError::ZeroDivisionError`] when aspect-ratio calculation divides by
    /// zero. Requests that already contain the source return without resizing.
    pub fn thumbnail(
        &mut self,
        size: (i64, i64),
        filter: Option<ResampleInput>,
    ) -> Result<(), PilError> {
        self.thumbnail_with_options(size, filter, Some(2.0))
    }

    /// Queues a Pillow-style thumbnail with an optional reduction gap.
    ///
    /// `None` disables Pillow's reducing-gap pass. The default `thumbnail`
    /// method supplies Pillow's default gap of `2.0`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for invalid geometry, filters, or reduction gaps.
    pub fn thumbnail_with_options(
        &mut self,
        size: (i64, i64),
        filter: Option<ResampleInput>,
        reducing_gap: Option<f64>,
    ) -> Result<(), PilError> {
        let Some(plan) = self.prepare_thumbnail(size)? else {
            return Ok(());
        };
        self.thumbnail_with_plan(plan, filter, reducing_gap)
    }

    /// Queues a thumbnail using dimensions selected by [`Self::prepare_thumbnail`].
    ///
    /// Host bindings use this after converting Python values, which may run
    /// callbacks that mutate the image. Applying the saved plan preserves the
    /// dimensions Pillow selected before those callbacks.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] for an invalid filter, reduction gap, or source
    /// image that cannot be resized.
    pub fn thumbnail_with_plan(
        &mut self,
        plan: ThumbnailPlan,
        filter: Option<ResampleInput>,
        reducing_gap: Option<f64>,
    ) -> Result<(), PilError> {
        let deferable_putpixel_prefix = self.has_deferable_rgb_putpixel_prefix();
        let source_size = self.size()?;
        let dimensions = plan.dimensions;
        let mut filter = parse_resample_input(filter)?;
        if reducing_gap.is_some_and(|gap| gap < 1.0) {
            return Err(PilError::ValueError(
                "reducing_gap must be 1.0 or greater".into(),
            ));
        }
        // Pillow's resize call can become an identity after an effectful host
        // conversion changes the image to the already selected target size.
        if source_size == dimensions {
            return Ok(());
        }
        if source_size.0 == 0 || source_size.1 == 0 {
            // Pillow's aspect-preserving size can be positive even when the
            // source has a zero dimension. Its subsequent Image.resize call
            // then rejects the zero-sized source before any pixels are produced.
            self.materialized_shared()?;
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
        let can_defer = deferable_putpixel_prefix
            && self.can_defer_rgb_putpixel_thumbnail(
                dimensions.0,
                dimensions.1,
                filter,
                reducing_gap,
            );
        if !can_defer {
            self.materialized_shared()?;
        }
        let mode = match self.known_mode_identity() {
            Some(mode) => mode,
            None => self.materialized_shared()?.mode(),
        };
        self.thumbnail_with_filter(dimensions, &mut filter, mode, reducing_gap)
    }

    fn thumbnail_with_filter(
        &mut self,
        (w, h): (u32, u32),
        filter: &mut ResampleFilter,
        mode: Mode,
        reducing_gap: Option<f64>,
    ) -> Result<(), PilError> {
        // PIL forces NEAREST for indexed samples, including PA's raw
        // index/alpha pairs, to preserve the sample layout.
        if matches!(mode, Mode::One | Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
            *filter = ResampleFilter::Nearest;
        }
        let new_self = Image::push_op(
            self,
            PipelineOp::Thumbnail {
                w,
                h,
                filter: *filter,
                reducing_gap,
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
    crate::compute::pool_cpu::ops::resize::helpers::boxed_crop::validate_bounds(bounds, size)
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
    use crate::compute::Backend;
    use crate::image::Image as CoreImage;
    use crate::pipeline::{PipelineOp, ResampleFilter};

    #[test]
    fn cpu_rgb_thumbnail_keeps_valid_putpixel_prefix_in_the_same_batch() {
        let mut image = CoreImage::new(1024, 768, crate::raster::Mode::ImageRgb8, (0, 0, 0, 255)).unwrap();
        image.putpixel(2, 3, 180, 120, 60, 255).unwrap();
        let CoreImage::Pipeline {
            source,
            ops,
            backend,
            mode,
            ..
        } = &image
        else {
            panic!("putpixel must create a pipeline");
        };
        assert_eq!(*backend, None);
        assert!(matches!(
            crate::image::declared_pipeline_mode(mode),
            None | Some(crate::raster::Mode::ImageRgb8)
        ));
        assert!(matches!(source.as_ref(), CoreImage::Loaded(_)));
        assert!(ops.iter().all(|op| matches!(
            op,
            PipelineOp::PutPixel {
                palette_index: false,
                ..
            }
        )));
        assert!(image.has_deferable_rgb_putpixel_prefix());

        let mut image = image;
        image
            .thumbnail((256, 256), Some(super::ResampleInput::Code(2)))
            .unwrap();
        let CoreImage::Pipeline {
            ops, materialized, ..
        } = &image
        else {
            panic!("thumbnail must remain a lazy pipeline until observed");
        };
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], PipelineOp::PutPixel { .. }));
        assert!(matches!(ops[1], PipelineOp::Thumbnail { .. }));
        assert!(materialized.get().is_none());

        let actual = image.materialize().unwrap();
        let mut reference = CoreImage::new(1024, 768, crate::raster::Mode::ImageRgb8, (0, 0, 0, 255)).unwrap();
        reference.putpixel(2, 3, 180, 120, 60, 255).unwrap();
        let materialized_reference = reference.materialize().unwrap();
        let mut expected = CoreImage::from_dynamic(materialized_reference);
        expected
            .thumbnail((256, 256), Some(super::ResampleInput::Code(2)))
            .unwrap();
        let expected = expected.materialize().unwrap();
        assert_eq!(actual.as_bytes(), expected.as_bytes());
    }

    #[test]
    fn simd_rgb_thumbnail_keeps_fusible_putpixel_prefix_until_materialization() {
        let mut image = CoreImage::new(1024, 768, crate::raster::Mode::ImageRgb8, (0, 0, 0, 255)).unwrap();
        image.putpixel(2, 3, 180, 120, 60, 255).unwrap();
        let image = image.use_backend(Backend::Simd);
        assert!(image.has_deferable_rgb_putpixel_prefix());
        assert!(image.can_defer_rgb_putpixel_thumbnail(
            256,
            192,
            ResampleFilter::Bicubic,
            Some(2.0),
        ));

        let mut image = image;
        image
            .thumbnail((256, 256), Some(super::ResampleInput::Code(2)))
            .unwrap();
        let CoreImage::Pipeline {
            ops, materialized, ..
        } = &image
        else {
            panic!("thumbnail must remain a lazy pipeline until observed");
        };
        assert_eq!(ops.len(), 2);
        assert!(matches!(ops[0], PipelineOp::PutPixel { .. }));
        assert!(matches!(ops[1], PipelineOp::Thumbnail { .. }));
        assert!(materialized.get().is_none());
    }

    #[test]
    fn cpu_rgb_thumbnail_noop_and_invalid_request_keep_eager_prefix_load() {
        let mut noop = CoreImage::new(1024, 768, crate::raster::Mode::ImageRgb8, (0, 0, 0, 255)).unwrap();
        noop.putpixel(2, 3, 180, 120, 60, 255).unwrap();
        let mut noop = noop.use_backend(Backend::Cpu);
        noop.thumbnail((1024, 768), None).unwrap();
        let CoreImage::Pipeline { materialized, .. } = &noop else {
            panic!("putpixel must remain represented by a pipeline");
        };
        assert!(materialized.get().is_some());

        let mut invalid = CoreImage::new(1024, 768, crate::raster::Mode::ImageRgb8, (0, 0, 0, 255)).unwrap();
        invalid.putpixel(2, 3, 180, 120, 60, 255).unwrap();
        let mut invalid = invalid.use_backend(Backend::Cpu);
        let error = invalid.thumbnail((-1, 256), None).unwrap_err();
        assert!(matches!(error, PilError::ValueError(_)));
        let CoreImage::Pipeline { materialized, .. } = &invalid else {
            panic!("putpixel must remain represented by a pipeline");
        };
        assert!(materialized.get().is_some());
    }

    #[test]
    fn thumbnail_empty_source_nonnegative_bounds_is_noop() {
        let mut image = Image::new(0, 0, crate::raster::Mode::ImageRgb8, (1, 2, 3, 255)).unwrap();

        image.thumbnail((0, 2), None).unwrap();

        assert_eq!(image.size().unwrap(), (0, 0));
    }

    #[test]
    fn thumbnail_zero_width_source_matches_resize_validation() {
        let mut image = Image::new(0, 2, crate::raster::Mode::ImageRgb8, (1, 2, 3, 255)).unwrap();

        let error = image.thumbnail((1, 1), None).unwrap_err();

        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
        assert_eq!(image.size().unwrap(), (0, 2));
    }

    #[test]
    fn thumbnail_zero_width_source_preserves_aspect_division_order() {
        let mut image = Image::new(0, 2, crate::raster::Mode::ImageRgb8, (1, 2, 3, 255)).unwrap();

        let error = image.thumbnail((-1, 1), None).unwrap_err();

        assert!(matches!(
            error,
            PilError::ZeroDivisionError(message) if message == "float division by zero"
        ));
        assert_eq!(image.size().unwrap(), (0, 2));
    }

    #[test]
    fn thumbnail_zero_height_source_preserves_aspect_division_order() {
        let mut image = Image::new(2, 0, crate::raster::Mode::ImageRgb8, (1, 2, 3, 255)).unwrap();

        let error = image.thumbnail((1, 1), None).unwrap_err();

        assert!(matches!(
            error,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
        assert_eq!(image.size().unwrap(), (2, 0));
    }
}
