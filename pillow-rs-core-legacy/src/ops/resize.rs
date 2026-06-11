use crate::error::PilError;
use crate::image::Image;
use image::imageops::FilterType;

/// Parse a resample filter string into image::imageops::FilterType.
/// Defaults to Bilinear (Triangle) matching Pillow's BILINEAR default.
pub fn parse_resample(s: Option<&str>) -> Result<FilterType, PilError> {
    match s {
        None | Some("BILINEAR") | Some("bilinear") => Ok(FilterType::Triangle),
        Some("NEAREST") | Some("nearest") => Ok(FilterType::Nearest),
        Some("BICUBIC") | Some("bicubic") => Ok(FilterType::CatmullRom),
        Some("LANCZOS") | Some("lanczos") => Ok(FilterType::Lanczos3),
        Some("BOX") | Some("box") => Ok(FilterType::Gaussian),
        Some("HAMMING") | Some("hamming") => Ok(FilterType::Lanczos3),
        Some(other) => Err(PilError::ValueError(format!(
            "Unknown resample filter: {}",
            other
        ))),
    }
}

impl Image {
    pub fn resize(&self, size: (u32, u32), filter: Option<&str>) -> Result<Image, PilError> {
        let (w, h) = size;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "resize dimensions must be > 0".into(),
            ));
        }
        let filter_type = parse_resample(filter)?;
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let (cur_w, cur_h) = (img.width(), img.height());
        if cur_w == w && cur_h == h {
            return Ok(clone);
        }
        let resized = img.resize_exact(w, h, filter_type);
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(resized),
            format: self.format,
        })
    }
}
