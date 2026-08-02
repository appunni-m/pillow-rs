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
    /// The original image is unchanged. Mode `"1"` and `"P"` force nearest
    /// sampling to avoid creating interpolated palette indices or binary pixels.
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
        // PIL forces NEAREST for mode "1" and "P" to avoid non-binary gray values
        if self.has_palette_mode() || self.explicit_mode() == Some("1") {
            filter = ResampleFilter::Nearest;
        }
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }

    /// Queues an in-place Pillow-style thumbnail resize.
    ///
    /// Mode `"1"` and `"P"` force nearest sampling to preserve binary pixels and
    /// palette indices.
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
        // PIL forces NEAREST for mode "1" and "P" to avoid non-binary/interpolated values
        if self.has_palette_mode() || self.explicit_mode() == Some("1") {
            filter = ResampleFilter::Nearest;
        }
        let new_self = Image::push_op(self, PipelineOp::Thumbnail { w, h, filter });
        *self = new_self;
        Ok(())
    }
}
