use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn transpose(&self, method: &str) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;

        let transposed = match method {
            "FLIP_LEFT_RIGHT" => img.fliph(),
            "FLIP_TOP_BOTTOM" => img.flipv(),
            "ROTATE_90" => img.rotate90(),
            "ROTATE_180" => img.rotate180(),
            "ROTATE_270" => img.rotate270(),
            "TRANSPOSE" => {
                let t = img.rotate90();
                t.fliph()
            }
            "TRANSVERSE" => {
                let t = img.rotate90();
                t.flipv()
            }
            _ => {
                return Err(PilError::ValueError(format!(
                    "Unknown transpose method: {}. Use FLIP_LEFT_RIGHT, FLIP_TOP_BOTTOM, ROTATE_90, ROTATE_180, ROTATE_270, TRANSPOSE, or TRANSVERSE.",
                    method
                )));
            }
        };

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(transposed),
            format: self.format,
        })
    }
}
