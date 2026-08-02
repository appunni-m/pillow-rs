use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};

/// Parses a Pillow resampling filter name.
///
/// `None` defaults to [`ResampleFilter::Bicubic`], matching Pillow's default
/// for resize-like methods.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `s` is not a supported filter name.
pub fn parse_resample(s: Option<&str>) -> Result<ResampleFilter, PilError> {
    match s {
        None | Some("BICUBIC") | Some("bicubic") => Ok(ResampleFilter::Bicubic),
        Some("NEAREST") | Some("nearest") => Ok(ResampleFilter::Nearest),
        Some("BILINEAR") | Some("bilinear") => Ok(ResampleFilter::Bilinear),
        Some("LANCZOS") | Some("lanczos") => Ok(ResampleFilter::Lanczos),
        Some("BOX") | Some("box") => Ok(ResampleFilter::Box),
        Some("HAMMING") | Some("hamming") => Ok(ResampleFilter::Hamming),
        Some(other) => Err(PilError::ValueError(format!(
            "Unknown resample filter: {}",
            other
        ))),
    }
}

impl Image {
    /// Returns a resized image with dimensions `size`.
    ///
    /// The original image is unchanged. Indexed modes, including `"PA"`, force
    /// nearest sampling to avoid interpolating palette indices or alpha bytes.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] for zero dimensions or unknown filters.
    pub fn resize(&self, size: (u32, u32), filter: Option<&str>) -> Result<Image, PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
        let mut filter = parse_resample(filter)?;
        // PIL forces NEAREST for indexed samples, including PA's raw
        // index/alpha pairs, to avoid interpolating palette indices or alpha
        // bytes. The result remains PA rather than expanding to RGBA.
        if self.has_palette_mode() || matches!(self.explicit_mode(), Some("1") | Some("PA")) {
            filter = ResampleFilter::Nearest;
        }
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }

    /// Queues an in-place Pillow-style thumbnail resize.
    ///
    /// Indexed modes, including `"PA"`, force nearest sampling to preserve the
    /// raw sample layout.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when either requested dimension is zero.
    pub fn thumbnail(
        &mut self,
        size: (u32, u32),
        filter: Option<ResampleFilter>,
    ) -> Result<(), PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError("thumbnail size must be > 0".into()));
        }
        let mut filter = filter.unwrap_or(ResampleFilter::Bicubic);
        // PIL forces NEAREST for indexed samples, including PA's raw
        // index/alpha pairs, to preserve the sample layout.
        if self.has_palette_mode() || matches!(self.explicit_mode(), Some("1") | Some("PA")) {
            filter = ResampleFilter::Nearest;
        }
        let new_self = Image::push_op(self, PipelineOp::Thumbnail { w, h, filter });
        *self = new_self;
        Ok(())
    }
}
