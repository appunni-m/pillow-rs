//! Image analysis operations — getbbox, histogram, getextrema.
//! These are IMMEDIATE operations that materialize the pipeline first.

use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Return the bounding box of non-zero regions.
    /// PIL: `getbbox(*, alpha_only=True) -> (left, top, right, bottom) | None`
    pub fn getbbox(&self, alpha_only: bool) -> Result<Option<(u32, u32, u32, u32)>, PilError> {
        let img = self.materialize()?;
        let (img_w, img_h) = (img.width(), img.height());

        let mut left = img_w;
        let mut top = img_h;
        let mut right = 0u32;
        let mut bottom = 0u32;

        let has_alpha = matches!(
            img.color(),
            image::ColorType::La8
                | image::ColorType::La16
                | image::ColorType::Rgba8
                | image::ColorType::Rgba16
                | image::ColorType::Rgba32F
        );
        let rgba = img.to_rgba8();
        for y in 0..img_h {
            for x in 0..img_w {
                let px = rgba.get_pixel(x, y);
                let is_nonzero = if alpha_only && has_alpha {
                    px[3] > 0
                } else {
                    px[0] > 0 || px[1] > 0 || px[2] > 0
                };
                if is_nonzero {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            }
        }

        if left > right || top > bottom {
            Ok(None)
        } else {
            Ok(Some((left, top, right + 1, bottom + 1)))
        }
    }

    /// Return min/max pixel values per band.
    /// PIL: `getextrema() -> tuple[float, float] | tuple[tuple[int,int],...]`
    pub fn getextrema(&self) -> Result<Vec<(u8, u8)>, PilError> {
        let img = self.materialize()?;

        let bands = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => 1,
            image::ColorType::La8 | image::ColorType::La16 => 2,
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
            _ => 4,
        };

        let mut extrema = vec![(255u8, 0u8); bands];
        // Use the appropriate pixel reader based on color type
        match img.color() {
            image::ColorType::L8 => {
                let luma = img.to_luma8();
                for px in luma.pixels() {
                    extrema[0].0 = extrema[0].0.min(px[0]);
                    extrema[0].1 = extrema[0].1.max(px[0]);
                }
            }
            image::ColorType::La8 => {
                let la = img.to_luma_alpha8();
                for px in la.pixels() {
                    extrema[0].0 = extrema[0].0.min(px[0]);
                    extrema[0].1 = extrema[0].1.max(px[0]);
                    extrema[1].0 = extrema[1].0.min(px[1]);
                    extrema[1].1 = extrema[1].1.max(px[1]);
                }
            }
            _ => {
                let rgba = img.to_rgba8();
                for px in rgba.pixels() {
                    for b in 0..bands.min(4) {
                        extrema[b].0 = extrema[b].0.min(px[b]);
                        extrema[b].1 = extrema[b].1.max(px[b]);
                    }
                }
            }
        }
        Ok(extrema)
    }

    /// Compute image histogram per band.
    /// PIL: `histogram(mask=None, extrema=None) -> list[int]`
    /// Returns 256 values per band, concatenated.
    pub fn histogram(&self) -> Result<Vec<u32>, PilError> {
        let img = self.materialize()?;
        let rgba = img.to_rgba8();

        let n_bands = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => 1,
            image::ColorType::La8 | image::ColorType::La16 => 2,
            image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
            _ => 4,
        };

        let mut hist = vec![0u32; 256 * n_bands];
        for px in rgba.pixels() {
            for b in 0..n_bands {
                hist[b * 256 + px[b] as usize] += 1;
            }
        }
        Ok(hist)
    }
}
