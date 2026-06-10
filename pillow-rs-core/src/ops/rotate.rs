use crate::error::PilError;
use crate::image::Image;

impl Image {
    pub fn rotate(
        &self,
        angle: f64,
        expand: bool,
        _fillcolor: Option<(u8, u8, u8, u8)>,
    ) -> Result<Image, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let angle_norm = angle.rem_euclid(360.0);

        let rotated = if (angle_norm - 90.0).abs() < f64::EPSILON {
            if expand {
                let h = img.height();
                let w = img.width();
                let mut canvas = image::DynamicImage::new_rgba8(h, w);
                let r90 = img.rotate90();
                image::imageops::overlay(&mut canvas, &r90, 0, 0);
                canvas
            } else {
                img.rotate90()
            }
        } else if (angle_norm - 180.0).abs() < f64::EPSILON {
            if expand {
                img.rotate180()
            } else {
                img.rotate180()
            }
        } else if (angle_norm - 270.0).abs() < f64::EPSILON {
            if expand {
                let h = img.height();
                let w = img.width();
                let mut canvas = image::DynamicImage::new_rgba8(h, w);
                let r270 = img.rotate270();
                image::imageops::overlay(&mut canvas, &r270, 0, 0);
                canvas
            } else {
                img.rotate270()
            }
        } else if angle_norm == 0.0 {
            img.clone()
        } else {
            return Err(PilError::NotImplementedError(format!(
                "Arbitrary angle rotation ({}°) not yet supported. Use 90, 180, or 270.",
                angle
            )));
        };

        Ok(Image {
            inner: crate::lazy::LazyImage::Loaded(rotated),
            format: self.format,
        })
    }
}
