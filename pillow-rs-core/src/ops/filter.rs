use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Apply a named filter. Supports built-in kernels: BLUR, CONTOUR, DETAIL,
    /// EDGE_ENHANCE, EDGE_ENHANCE_MORE, EMBOSS, FIND_EDGES, SHARPEN, SMOOTH, SMOOTH_MORE.
    /// Parameterized filters (GaussianBlur, etc.) not yet implemented.
    pub fn filter(&self, filter_type: &str) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;

        let filtered = match filter_type {
            "BLUR" => img.blur(1.0),
            "CONTOUR" => img.filter3x3(&[0.0, -1.0, 0.0, -1.0, 4.0, -1.0, 0.0, -1.0, 0.0]),
            "DETAIL" => img.filter3x3(&[0.0, -1.0, 0.0, -1.0, 10.0, -1.0, 0.0, -1.0, 0.0]),
            "EDGE_ENHANCE" => img.filter3x3(&[0.0, 0.0, 0.0, -1.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
            "EDGE_ENHANCE_MORE" => img.filter3x3(&[-1.0, -1.0, -1.0, -1.0, 9.0, -1.0, -1.0, -1.0, -1.0]),
            "EMBOSS" => img.filter3x3(&[-1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
            "FIND_EDGES" => img.filter3x3(&[-1.0, -1.0, -1.0, -1.0, 8.0, -1.0, -1.0, -1.0, -1.0]),
            "SHARPEN" => img.filter3x3(&[0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0]),
            "SMOOTH" => img.filter3x3(&[1.0, 1.0, 1.0, 1.0, 5.0, 1.0, 1.0, 1.0, 1.0]),
            "SMOOTH_MORE" => img.filter3x3(&[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]),
            _ => {
                return Err(PilError::NotImplementedError(format!(
                    "Filter '{}' not yet implemented",
                    filter_type
                )));
            }
        };

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(filtered),
            format: self.format,
        })
    }
}
