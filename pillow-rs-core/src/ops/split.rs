use crate::error::PilError;
use crate::image::Image;

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
                let raw = ga.as_raw();
                let (mut l, mut a) = unsafe {
                    let mut lv = Vec::with_capacity(n);
                    let mut av = Vec::with_capacity(n);
                    lv.set_len(n);
                    av.set_len(n);
                    (lv, av)
                };
                for (i, chunk) in raw.chunks_exact(2).enumerate() {
                    l[i] = chunk[0];
                    a[i] = chunk[1];
                }
                vec![
                    Image {
                        inner: crate::lazy::LazyImage::Loaded(
                            image::DynamicImage::ImageLuma8(
                                image::GrayImage::from_raw(w, h, l)
                                    .ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?,
                            ),
                        ),
                        format: None,
                    },
                    Image {
                        inner: crate::lazy::LazyImage::Loaded(
                            image::DynamicImage::ImageLuma8(
                                image::GrayImage::from_raw(w, h, a)
                                    .ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?,
                            ),
                        ),
                        format: None,
                    },
                ]
            }
            image::DynamicImage::ImageRgb8(rgb) => {
                let raw = rgb.as_raw();
                let (mut r, mut g, mut b) = unsafe {
                    let mut rv = Vec::with_capacity(n);
                    let mut gv = Vec::with_capacity(n);
                    let mut bv = Vec::with_capacity(n);
                    rv.set_len(n);
                    gv.set_len(n);
                    bv.set_len(n);
                    (rv, gv, bv)
                };
                for (i, chunk) in raw.chunks_exact(3).enumerate() {
                    r[i] = chunk[0];
                    g[i] = chunk[1];
                    b[i] = chunk[2];
                }
                vec![
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, r).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, g).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, b).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                ]
            }
            image::DynamicImage::ImageRgba8(rgba) => {
                let raw = rgba.as_raw();
                let (mut r, mut g, mut b, mut a) = unsafe {
                    let mut rv = Vec::with_capacity(n);
                    let mut gv = Vec::with_capacity(n);
                    let mut bv = Vec::with_capacity(n);
                    let mut av = Vec::with_capacity(n);
                    rv.set_len(n);
                    gv.set_len(n);
                    bv.set_len(n);
                    av.set_len(n);
                    (rv, gv, bv, av)
                };
                for (i, chunk) in raw.chunks_exact(4).enumerate() {
                    r[i] = chunk[0];
                    g[i] = chunk[1];
                    b[i] = chunk[2];
                    a[i] = chunk[3];
                }
                vec![
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, r).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, g).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, b).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, a).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                ]
            }
            _ => {
                let rgba = img.to_rgba8();
                let raw = rgba.as_raw();
                let (mut r, mut g, mut b, mut a) = unsafe {
                    let mut rv = Vec::with_capacity(n);
                    let mut gv = Vec::with_capacity(n);
                    let mut bv = Vec::with_capacity(n);
                    let mut av = Vec::with_capacity(n);
                    rv.set_len(n);
                    gv.set_len(n);
                    bv.set_len(n);
                    av.set_len(n);
                    (rv, gv, bv, av)
                };
                for (i, chunk) in raw.chunks_exact(4).enumerate() {
                    r[i] = chunk[0];
                    g[i] = chunk[1];
                    b[i] = chunk[2];
                    a[i] = chunk[3];
                }
                vec![
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, r).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, g).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, b).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                    Image { inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(w, h, a).ok_or_else(|| PilError::ValueError("split: buffer mismatch".into()))?)), format: None },
                ]
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
