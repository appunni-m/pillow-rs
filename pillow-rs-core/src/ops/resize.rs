use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter};

/// Parse a resample filter string into ResampleFilter.
/// Defaults to Bilinear matching Pillow's BILINEAR default.
pub fn parse_resample(s: Option<&str>) -> Result<ResampleFilter, PilError> {
    match s {
        None | Some("BILINEAR") | Some("bilinear") => Ok(ResampleFilter::Bilinear),
        Some("NEAREST") | Some("nearest") => Ok(ResampleFilter::Nearest),
        Some("BICUBIC") | Some("bicubic") => Ok(ResampleFilter::Bicubic),
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
    /// Resize the image to the given size.
    /// Returns a new image (does not mutate self).
    pub fn resize(&self, size: (u32, u32), filter: Option<&str>) -> Result<Image, PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "resize dimensions must be > 0".into(),
            ));
        }
        let mut filter = parse_resample(filter)?;
        // PIL forces NEAREST for mode "1" and "P" to avoid non-binary gray values
        if let Some(m) = self.explicit_mode() {
            if m == "1" || m == "P" {
                filter = ResampleFilter::Nearest;
            }
        }
        Ok(Image::push_op(self, PipelineOp::Resize { w, h, filter }))
    }

    /// Resize the image to thumbnail size, mutating in place.
    /// Matching Pillow's Image.thumbnail() semantics.
    pub fn thumbnail(&mut self, size: (u32, u32)) -> Result<(), PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "thumbnail size must be > 0".into(),
            ));
        }
        let new_self = Image::push_op(
            self,
            PipelineOp::Thumbnail {
                w,
                h,
                filter: ResampleFilter::Bilinear,
            },
        );
        *self = new_self;
        Ok(())
    }
}
