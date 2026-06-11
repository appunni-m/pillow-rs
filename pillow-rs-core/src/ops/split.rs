use crate::error::PilError;
use crate::image::Image;
use image::GrayImage;

/// Channel split using puhu's pre-allocation pattern (set_len + chunks_exact).
fn split_channels(raw: &[u8], channels: usize, n: usize, w: u32, h: u32) -> Vec<Image> {
    let mut bufs: Vec<Vec<u8>> = (0..channels)
        .map(|_| unsafe { let mut v = Vec::with_capacity(n); v.set_len(n); v })
        .collect();

    for (i, chunk) in raw.chunks_exact(channels).enumerate() {
        for c in 0..channels { bufs[c][i] = chunk[c]; }
    }

    bufs.into_iter().map(|buf| {
        Image {
            inner: crate::lazy::LazyImage::Loaded(
                image::DynamicImage::ImageLuma8(
                    GrayImage::from_raw(w, h, buf)
                        .expect("split: buffer size mismatch"),
                ),
            ),
            format: None,
        }
    }).collect()
}

impl Image {
    pub fn split(&self) -> Result<Vec<Image>, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let (w, h) = (img.width(), img.height());
        let n = (w * h) as usize;

        let bands = match img {
            image::DynamicImage::ImageLuma8(gray) => {
                vec![Image {
                    inner: crate::lazy::LazyImage::Loaded(
                        image::DynamicImage::ImageLuma8(gray.clone()),
                    ),
                    format: None,
                }]
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

    pub fn getbands(&self) -> Result<Vec<String>, PilError> {
        let mut clone = self.clone();
        let img = clone.ensure_loaded()?;
        let bands = match img.color() {
            image::ColorType::L8 | image::ColorType::L16 => vec!["L".into()],
            image::ColorType::La8 | image::ColorType::La16 => vec!["L".into(), "A".into()],
            image::ColorType::Rgb8 | image::ColorType::Rgb16 | image::ColorType::Rgb32F => {
                vec!["R".into(), "G".into(), "B".into()]
            }
            image::ColorType::Rgba8 | image::ColorType::Rgba16 | image::ColorType::Rgba32F => {
                vec!["R".into(), "G".into(), "B".into(), "A".into()]
            }
            _ => vec!["R".into(), "G".into(), "B".into()],
        };
        Ok(bands)
    }
}
