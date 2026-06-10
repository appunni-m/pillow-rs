use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Crop expects (x, y, width, height) — matching crop_imm format.
    /// Python wrapper converts Pillow's (left, top, right, bottom) to this format.
    pub fn crop(&self, box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        let (x, y, w, h) = box_coords;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "crop box must have positive dimensions".into(),
            ));
        }
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let (img_w, img_h) = (img.width(), img.height());

        if x + w > img_w || y + h > img_h {
            return Err(PilError::ValueError(format!(
                "crop box (x={}, y={}, w={}, h={}) exceeds image bounds ({}x{})",
                x, y, w, h, img_w, img_h
            )));
        }

        let cropped = img.crop_imm(x, y, w, h);
        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(cropped),
            format: self.format,
        })
    }
}
