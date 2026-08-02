use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};

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
    /// The original image is unchanged. Indexed modes, including `"PA"`, force
    /// nearest sampling to avoid interpolating palette indices or alpha bytes.
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
        let filter = parse_resample_input(filter)?;
        let (w, h) = positive_dimensions(size, "height and width must be > 0")?;
        if let Some(box_coords) = box_coords {
            return self
                .crop(Some(box_coords))?
                .resize_with_filter((w, h), filter);
        }
        self.resize_with_filter((w, h), filter)
    }

    fn resize_with_filter(
        &self,
        (w, h): (u32, u32),
        mut filter: ResampleFilter,
    ) -> Result<Image, PilError> {
        // PIL forces NEAREST for indexed samples, including PA's raw
        // index/alpha pairs, to avoid interpolating palette indices or alpha
        // bytes. The result remains PA rather than expanding to RGBA.
        if self.has_palette_mode() || matches!(self.explicit_mode(), Some("1") | Some("PA")) {
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
        let dimensions = self.thumbnail_dimensions(size)?;
        if dimensions == (0, 0) {
            return Ok(());
        }
        let mut filter = parse_resample_input(filter)?;
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

    fn thumbnail_dimensions(&self, size: (i64, i64)) -> Result<(u32, u32), PilError> {
        // Pillow evaluates x / y before it rejects a negative width. Preserve
        // its division error for requests containing a zero dimension.
        if size.0 == 0 || size.1 == 0 {
            let (source_width, source_height) = self.size()?;
            if size.0 == 0 && size.1 == 0 && source_width == 0 && source_height == 0 {
                return Ok((0, 0));
            }
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        if size.0 < 0 {
            return Err(PilError::ValueError("scale must be > 0".into()));
        }

        let width = u32::try_from(size.0)
            .map_err(|_| PilError::ValueError("thumbnail size must be > 0".into()))?;
        if size.1 < 0 {
            return Ok((width, self.thumbnail_height_for_width(width)?));
        }
        let height = u32::try_from(size.1)
            .map_err(|_| PilError::ValueError("thumbnail size must be > 0".into()))?;
        Ok((width, height))
    }

    fn thumbnail_height_for_width(&self, width: u32) -> Result<u32, PilError> {
        let (source_width, source_height) = self.size()?;
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

fn positive_dimensions(size: (i64, i64), message: &str) -> Result<(u32, u32), PilError> {
    if size.0 <= 0 || size.1 <= 0 {
        return Err(PilError::ValueError(message.into()));
    }
    let width = u32::try_from(size.0).map_err(|_| PilError::ValueError(message.into()))?;
    let height = u32::try_from(size.1).map_err(|_| PilError::ValueError(message.into()))?;
    Ok((width, height))
}

fn round_aspect(number: f64, key: impl Fn(f64) -> f64) -> u32 {
    let floor = number.floor();
    let ceil = number.ceil();
    let best = if key(floor) <= key(ceil) { floor } else { ceil };
    best.max(1.0) as u32
}
