//! Pillow-compatible entropy methods for [`Image`].

use crate::error::PilError;

use super::Image;

impl Image {
    /// Returns Shannon entropy using Pillow-compatible per-band histograms.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn entropy(&self) -> Result<f64, PilError> {
        self.entropy_with_mask(None)
    }

    /// Computes entropy over pixels where an optional mask is non-zero,
    /// matching Pillow's masked-entropy semantics.
    pub fn entropy_with_mask(&self, mask: Option<&Image>) -> Result<f64, PilError> {
        let img = self.materialized_shared()?;
        let mask_image = if let Some(mask) = mask {
            let mask_image = mask.materialized_shared()?;
            if (mask_image.width(), mask_image.height()) != (img.width(), img.height()) {
                return Err(PilError::ValueError("images do not match".into()));
            }
            let mode = mask.mode()?;
            if mode != "1" && mode != "L" {
                return Err(PilError::ValueError("bad transparency mask".into()));
            }
            Some(mask_image)
        } else {
            None
        };
        // Borrow native L8 mask bytes instead of cloning the full mask. Other
        // valid mask layouts still use the same luma conversion as before.
        let owned_mask_luma =
            mask_image
                .as_ref()
                .and_then(|mask_image| match mask_image.as_ref() {
                    crate::raster::DynamicImage::ImageLuma8(_) => None,
                    _ => Some(mask_image.to_luma8().into_raw()),
                });
        let mask_raw: Option<&[u8]> = match (&mask_image, &owned_mask_luma) {
            (Some(mask_image), None) => match mask_image.as_ref() {
                crate::raster::DynamicImage::ImageLuma8(luma) => Some(luma.as_raw().as_slice()),
                _ => unreachable!("native mask must use L8 storage"),
            },
            (_, Some(raw)) => Some(raw.as_slice()),
            _ => None,
        };
        let width = img.width() as usize;
        let n_bands = match img.color() {
            crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => 1,
            crate::raster::ColorType::La8 | crate::raster::ColorType::La16 => 2,
            crate::raster::ColorType::Rgb8 | crate::raster::ColorType::Rgb16 => 3,
            _ => 4,
        };
        let hists = entropy_histograms(img.as_ref(), mask_raw, width, n_bands);
        let counted: u32 = hists.iter().map(|band| band.iter().sum::<u32>()).sum();
        let total = counted as f64;
        let mut entropy = 0.0f64;
        for band_hist in &hists {
            for &h in band_hist {
                if h > 0 {
                    let p = h as f64 / total;
                    // Pillow 12.2.0 `_entropy` accumulates
                    // `p * log(p) * M_LOG2E` and negates once at return
                    // (`src/_imaging.c:1401-1423`). Preserve that evaluation
                    // order, including the fused multiply-add emitted by its
                    // optimized C build, for bit-exact float and signed-zero
                    // parity.
                    entropy = (p * p.ln()).mul_add(std::f64::consts::LOG2_E, entropy);
                }
            }
        }
        Ok(-entropy)
    }
}

fn entropy_histograms(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    width: usize,
    n_bands: usize,
) -> Vec<[u32; 256]> {
    let mut hists = vec![[0u32; 256]; n_bands];
    // Use mode-aware pixel reading (to_rgba8 remaps LA channels incorrectly for histogram).
    match img.color() {
        crate::raster::ColorType::La8 => histogram_la8(img, mask_raw, &mut hists),
        crate::raster::ColorType::La16 => histogram_la16(img, mask_raw, width, &mut hists),
        crate::raster::ColorType::L8 | crate::raster::ColorType::L16 => {
            histogram_luma(img, mask_raw, width, &mut hists)
        }
        crate::raster::ColorType::Rgb8 => histogram_rgb8(img, mask_raw, width, &mut hists),
        crate::raster::ColorType::Rgba8 => histogram_rgba8(img, mask_raw, n_bands, &mut hists),
        _ => histogram_rgba_fallback(img, mask_raw, width, n_bands, &mut hists),
    }
    hists
}

fn histogram_la8(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    hists: &mut [[u32; 256]],
) {
    let crate::raster::DynamicImage::ImageLumaA8(la) = img else {
        unreachable!("La8 color must use native LA storage");
    };
    let raw = la.as_raw();
    match mask_raw {
        Some(mask) => {
            for (index, px) in raw.chunks_exact(2).enumerate() {
                if mask[index] == 0 {
                    continue;
                }
                hists[0][px[0] as usize] += 1;
                hists[1][px[1] as usize] += 1;
            }
        }
        None => {
            for px in raw.chunks_exact(2) {
                hists[0][px[0] as usize] += 1;
                hists[1][px[1] as usize] += 1;
            }
        }
    }
}

fn histogram_la16(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    width: usize,
    hists: &mut [[u32; 256]],
) {
    let la = img.to_luma_alpha8();
    let masked = |index: usize| match mask_raw {
        Some(mask) => mask[index] != 0,
        None => true,
    };
    for (y, row) in la.rows().enumerate() {
        for (x, px) in row.enumerate() {
            if !masked(y * width + x) {
                continue;
            }
            hists[0][px[0] as usize] += 1;
            hists[1][px[1] as usize] += 1;
        }
    }
}

fn histogram_luma(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    width: usize,
    hists: &mut [[u32; 256]],
) {
    let luma = img.to_luma8();
    let masked = |index: usize| match mask_raw {
        Some(mask) => mask[index] != 0,
        None => true,
    };
    for (y, row) in luma.rows().enumerate() {
        for (x, px) in row.enumerate() {
            if !masked(y * width + x) {
                continue;
            }
            hists[0][px[0] as usize] += 1;
        }
    }
}

fn histogram_rgb8(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    width: usize,
    hists: &mut [[u32; 256]],
) {
    // Masked RGB entropy reads the three logical color bands directly.
    let crate::raster::DynamicImage::ImageRgb8(rgb) = img else {
        unreachable!("Rgb8 color must use native RGB storage");
    };
    let masked = |index: usize| match mask_raw {
        Some(mask) => mask[index] != 0,
        None => true,
    };
    for (y, row) in rgb.rows().enumerate() {
        for (x, px) in row.enumerate() {
            if !masked(y * width + x) {
                continue;
            }
            hists[0][px[0] as usize] += 1;
            hists[1][px[1] as usize] += 1;
            hists[2][px[2] as usize] += 1;
        }
    }
}

fn histogram_rgba8(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    n_bands: usize,
    hists: &mut [[u32; 256]],
) {
    // Borrow native RGBA storage, avoiding a clone of the full image.
    let crate::raster::DynamicImage::ImageRgba8(rgba) = img else {
        unreachable!("Rgba8 color must use native RGBA storage");
    };
    let raw = rgba.as_raw();
    match mask_raw {
        Some(mask) => {
            for (index, px) in raw.chunks_exact(4).enumerate() {
                if mask[index] == 0 {
                    continue;
                }
                for band in 0..n_bands {
                    hists[band][px[band] as usize] += 1;
                }
            }
        }
        None => {
            for px in raw.chunks_exact(4) {
                for band in 0..n_bands {
                    hists[band][px[band] as usize] += 1;
                }
            }
        }
    }
}

fn histogram_rgba_fallback(
    img: &crate::raster::DynamicImage,
    mask_raw: Option<&[u8]>,
    width: usize,
    n_bands: usize,
    hists: &mut [[u32; 256]],
) {
    let rgba = img.to_rgba8();
    let masked = |index: usize| match mask_raw {
        Some(mask) => mask[index] != 0,
        None => true,
    };
    for (y, row) in rgba.rows().enumerate() {
        for (x, px) in row.enumerate() {
            if !masked(y * width + x) {
                continue;
            }
            for band in 0..n_bands {
                hists[band][px[band] as usize] += 1;
            }
        }
    }
}
