//! Image module-level functions — merge, blend, composite, eval.
//! These correspond to PIL.Image.merge(), PIL.Image.blend(), PIL.Image.composite().

use crate::error::PilError;
use crate::image::Image;
use image::{DynamicImage, GenericImage, GenericImageView};

/// Merge single-band images into a multi-band image.
/// PIL: `Image.merge(mode, bands)` where mode determines the band count.
pub fn merge(mode: &str, bands: &[Image]) -> Result<Image, PilError> {
    let n_expected = match mode {
        "RGB" => 3,
        "RGBA" => 4,
        "LA" => 2,
        "L" => 1,
        _ => return Err(PilError::ValueError(format!("Unsupported merge mode: {}", mode))),
    };

    if bands.len() != n_expected {
        return Err(PilError::ValueError(format!(
            "Wrong number of bands for mode {}: expected {}, got {}",
            mode,
            n_expected,
            bands.len()
        )));
    }

    // Get dimensions from first band
    let mut band_clones: Vec<Image> = bands.iter().map(|b| b.clone()).collect();
    let w = {
        let b0 = band_clones[0].ensure_loaded()?;
        b0.width()
    };
    let h = {
        let b0 = band_clones[0].ensure_loaded()?;
        b0.height()
    };

    // Extract raw data from each band
    let mut band_data: Vec<Vec<u8>> = Vec::new();
    for band in band_clones.iter_mut() {
        let img = band.ensure_loaded()?;
        let gray = img.to_luma8();
        if gray.width() != w || gray.height() != h {
            return Err(PilError::ValueError("All bands must have the same dimensions".into()));
        }
        band_data.push(gray.into_raw());
    }

    let n = (w * h) as usize;
    match mode {
        "RGB" => {
            let mut rgb = vec![0u8; n * 3];
            for i in 0..n {
                rgb[i * 3] = band_data[0][i];
                rgb[i * 3 + 1] = band_data[1][i];
                rgb[i * 3 + 2] = band_data[2][i];
            }
            let img = image::RgbImage::from_raw(w, h, rgb)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(img)),
                format: None,
            })
        }
        "RGBA" => {
            let mut rgba = vec![0u8; n * 4];
            for i in 0..n {
                rgba[i * 4] = band_data[0][i];
                rgba[i * 4 + 1] = band_data[1][i];
                rgba[i * 4 + 2] = band_data[2][i];
                rgba[i * 4 + 3] = band_data[3][i];
            }
            let img = image::RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgba8(img)),
                format: None,
            })
        }
        "LA" => {
            let mut la = vec![0u8; n * 2];
            for i in 0..n {
                la[i * 2] = band_data[0][i];
                la[i * 2 + 1] = band_data[1][i];
            }
            let img = image::GrayAlphaImage::from_raw(w, h, la)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageLumaA8(img)),
                format: None,
            })
        }
        "L" => {
            let img = image::GrayImage::from_raw(w, h, band_data.remove(0))
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(Image {
                inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageLuma8(img)),
                format: None,
            })
        }
        _ => unreachable!(),
    }
}

/// Linear interpolation between two images.
/// PIL: `Image.blend(im1, im2, alpha)` → (1-alpha)*im1 + alpha*im2
/// Uses integer arithmetic for exact PIL parity.
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    let mut c1 = image1.clone();
    let mut c2 = image2.clone();
    let img1 = c1.ensure_loaded()?;
    let img2 = c2.ensure_loaded()?;

    let (w, h) = (img1.width().min(img2.width()), img1.height().min(img2.height()));
    let rgb1 = img1.to_rgb8();
    let rgb2 = img2.to_rgb8();

    // PIL uses: int(p1 * (1-alpha) + p2 * alpha) — float multiply + truncation
    let a = alpha.clamp(0.0, 1.0);
    let mut out = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            out.put_pixel(x, y, image::Rgb([
                (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
            ]));
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
        format: image1.format,
    })
}

/// Composite image1 over image2 using mask.
/// PIL: `Image.composite(image1, image2, mask)`
pub fn composite(image1: &Image, image2: &Image, mask: &Image) -> Result<Image, PilError> {
    let mut c1 = image1.clone();
    let mut c2 = image2.clone();
    let mut cm = mask.clone();
    let img1 = c1.ensure_loaded()?;
    let img2 = c2.ensure_loaded()?;
    let mask_img = cm.ensure_loaded()?;

    let (w, h) = (img1.width().min(img2.width()).min(mask_img.width()),
                   img1.height().min(img2.height()).min(mask_img.height()));
    let rgb1 = img1.to_rgb8();
    let rgb2 = img2.to_rgb8();
    let mask_gray = mask_img.to_luma8();

    let mut out = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            let m = mask_gray.get_pixel(x, y)[0] as f64 / 255.0;
            out.put_pixel(x, y, image::Rgb([
                ((p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round() as u8),
                ((p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round() as u8),
                ((p1[2] as f64 * m + p2[2] as f64 * (1.0 - m)).round() as u8),
            ]));
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(out)),
        format: image1.format,
    })
}
