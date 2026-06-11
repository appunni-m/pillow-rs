//! Channel split operations — these are IMMEDIATE operations (not pipeline ops).

use crate::error::PilError;
use crate::image::Image;
use image::GrayImage;

/// Channel split using pre-allocation pattern.
fn split_channels(raw: &[u8], channels: usize, n: usize, w: u32, h: u32) -> Vec<Image> {
    let mut bufs: Vec<Vec<u8>> = (0..channels)
        .map(|_| {
            let mut v = Vec::with_capacity(n);
            unsafe {
                v.set_len(n);
            }
            v
        })
        .collect();

    for (i, chunk) in raw.chunks_exact(channels).enumerate() {
        for c in 0..channels {
            bufs[c][i] = chunk[c];
        }
    }

    bufs
        .into_iter()
        .map(|buf| {
            Image::Loaded(image::DynamicImage::ImageLuma8(
                GrayImage::from_raw(w, h, buf).expect("split: buffer size mismatch"),
            ))
        })
        .collect()
}

impl Image {
    /// Split the image into individual bands (immediate operation).
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        let img = self.materialize()?;
        let (w, h) = (img.width(), img.height());
        let n = (w * h) as usize;

        let bands = match img {
            image::DynamicImage::ImageLuma8(gray) => {
                vec![Image::Loaded(image::DynamicImage::ImageLuma8(
                    gray.clone(),
                ))]
            }
            image::DynamicImage::ImageLumaA8(ga) => {
                split_channels(ga.as_raw(), 2, n, w, h)
            }
            image::DynamicImage::ImageRgb8(rgb) => {
                split_channels(rgb.as_raw(), 3, n, w, h)
            }
            image::DynamicImage::ImageRgba8(rgba) => {
                split_channels(rgba.as_raw(), 4, n, w, h)
            }
            _ => {
                let rgba = img.to_rgba8();
                split_channels(rgba.as_raw(), 4, n, w, h)
            }
        };

        Ok(bands)
    }
}
