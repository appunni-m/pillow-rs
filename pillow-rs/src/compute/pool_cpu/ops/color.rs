//! Color/Convert CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible color mode conversion, quantization, and palette remapping.

use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::pipeline::ColorMode;
use crate::raster::DynamicImage;
use crate::raster::GenericImageView;

/// Convert image to a specified color mode.
/// Matches PIL's Image.convert() behavior exactly.
///
/// `source_mode` is needed because Pillow's `RGBX` layout shares the native
/// four-byte RGBA storage with `RGBA`, but its fourth byte is padding rather
/// than an alpha sample. The image buffer alone cannot distinguish those two
/// public modes at this executor boundary.
pub fn op_convert(
    img: &DynamicImage,
    mode: &ColorMode,
    source_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match mode {
        ColorMode::L => Ok(DynamicImage::ImageLuma8(pil_grayscale(img)?)),
        ColorMode::LA => {
            let gray = pil_grayscale(img)?;
            let (w, h) = gray.dimensions();
            let mut ga = crate::raster::GrayAlphaImage::new(w, h);
            // Pillow's convert.c carries the source alpha through an RGBA
            // to LA conversion. Non-RGBA source layouts use opaque alpha.
            let source_alpha = if matches!(img.color(), crate::raster::ColorType::Rgba8)
                && source_mode != Some("RGBX")
            {
                Some(
                    img.as_bytes()
                        .chunks_exact(4)
                        .map(|pixel| pixel[3])
                        .collect::<Vec<_>>(),
                )
            } else {
                None
            };
            for (index, (gap, gp)) in ga.pixels_mut().zip(gray.pixels()).enumerate() {
                gap[0] = gp[0];
                gap[1] = source_alpha
                    .as_ref()
                    .and_then(|alpha| alpha.get(index))
                    .copied()
                    .unwrap_or(255);
            }
            Ok(DynamicImage::ImageLumaA8(ga))
        }
        ColorMode::RGB => Ok(DynamicImage::ImageRgb8(img.to_rgb8())),
        ColorMode::RGBA => {
            let mut rgba = img.to_rgba8();
            if source_mode == Some("RGBX") {
                // Pillow treats RGBX's fourth byte as padding on conversion;
                // RGBA output receives a newly supplied opaque alpha band.
                for pixel in rgba.pixels_mut() {
                    pixel[3] = 255;
                }
            }
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        ColorMode::I => {
            // RGB-family sources use the deferred exact converter for I.
            // CMYK/I/F source normalization is handled by Image::convert
            // before this operation is queued.
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = crate::raster::RgbaImage::new(w, h);
            for (op, px) in out.pixels_mut().zip(rgb.pixels()) {
                let r = px[0] as i32;
                let g = px[1] as i32;
                let b = px[2] as i32;
                // PIL's rounded luma: (19595*R + 38470*G + 7471*B + 32768) >> 16
                let val = (19595i32 * r + 38470i32 * g + 7471i32 * b + 32768) >> 16;
                *op = crate::raster::Rgba(val.to_le_bytes());
            }
            Ok(DynamicImage::ImageRgba8(out))
        }
        ColorMode::F => {
            // RGB-family sources use the deferred exact converter for F.
            // CMYK/I/F source normalization is handled by Image::convert
            // before this operation is queued.
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = crate::raster::RgbaImage::new(w, h);
            for (op, px) in out.pixels_mut().zip(rgb.pixels()) {
                let sum = px[0] as i32 * 299 + px[1] as i32 * 587 + px[2] as i32 * 114;
                let val = sum as f32 / 1000.0_f32;
                *op = crate::raster::Rgba(val.to_le_bytes());
            }
            Ok(DynamicImage::ImageRgba8(out))
        }
        ColorMode::CMYK => {
            // Pillow's Convert.c routes luma sources ("L", "LA") through the
            // gray branch: C=M=Y=0 and K=255-gray.  Every other source goes
            // through the RGB inverse: C=255-R, M=255-G, Y=255-B, K=0
            // (ImagingConvertCMYK with INVERSE=1).
            if matches!(
                img,
                DynamicImage::ImageLuma8(_) | DynamicImage::ImageLumaA8(_)
            ) {
                let (w, h) = img.dimensions();
                let mut out = crate::raster::RgbaImage::new(w, h);
                // Convert.c's luma-to-CMYK branch copies L directly before
                // subtracting it from 255 for K.  The generic grayscale
                // helper would first allocate an RGB image and a second L
                // image even though that conversion is an identity for L/LA;
                // read the native luma byte directly while preserving the
                // exact C=M=Y=0, K=255-L result.
                match img {
                    DynamicImage::ImageLuma8(luma) => {
                        for (op, gp) in out.pixels_mut().zip(luma.pixels()) {
                            *op = crate::raster::Rgba([0u8, 0u8, 0u8, 255u8.wrapping_sub(gp[0])]);
                        }
                    }
                    DynamicImage::ImageLumaA8(la) => {
                        for (op, gp) in out.pixels_mut().zip(la.pixels()) {
                            *op = crate::raster::Rgba([0u8, 0u8, 0u8, 255u8.wrapping_sub(gp[0])]);
                        }
                    }
                    _ => unreachable!("luma CMYK conversion matched an invalid layout"),
                }
                Ok(DynamicImage::ImageRgba8(out))
            } else {
                let rgb = img.to_rgb8();
                let (w, h) = rgb.dimensions();
                let mut out = crate::raster::RgbaImage::new(w, h);
                for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                    *op = crate::raster::Rgba([
                        255u8.wrapping_sub(ip[0]),
                        255u8.wrapping_sub(ip[1]),
                        255u8.wrapping_sub(ip[2]),
                        0u8,
                    ]);
                }
                Ok(DynamicImage::ImageRgba8(out))
            }
        }
        ColorMode::HSV => {
            // Convert to HSV: RGB→HSV using PIL's exact algorithm.
            // HSV is stored in an Rgb8 container (H→R, S→G, V→B).
            Ok(crate::color::rgb_to_hsv(img))
        }
        ColorMode::YCbCr => {
            // Convert to YCbCr: RGB→YCbCr using PIL's BT.601 fixed-point.
            // YCbCr is stored in an Rgb8 container (Y→R, Cb→G, Cr→B).
            // Pillow's l2ycbcr/la2ycbcr converters copy the luma byte and
            // install neutral chroma; routing L through the RGB lookup table
            // would turn some grayscale values (for example 11) into 10.
            if matches!(
                img.color(),
                crate::raster::ColorType::L8 | crate::raster::ColorType::La8
            ) {
                Ok(crate::color::luma_to_ycbcr(img))
            } else {
                Ok(crate::color::rgb_to_ycbcr(img))
            }
        }
        // The public conversion path materializes binary and palette targets
        // before queuing a deferred operation. Keep the descriptor variants
        // for registry compatibility, but reject their duplicate CPU kernels.
        ColorMode::Mode1 | ColorMode::P => Err(PilError::ValueError(
            "deferred conversion mode must be materialized first".into(),
        )),
    }
}

/// Remap palette indices according to a destination map.
/// PIL builds inverse lookup: inverse[dest_map[i]] = i, all else -> 0
pub fn op_remap_palette(
    img: &DynamicImage,
    dest_map: &[u8],
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL: dest_map maps position-in-list → old index.
    // inverse[old_idx] = position. Only iterate actual entries, not padding.
    let mut inverse = [0u8; 256];
    for (i, &old_pos) in dest_map.iter().enumerate() {
        let old_idx = old_pos as usize;
        if old_idx < 256 {
            inverse[old_idx] = i as u8;
        }
    }
    // P-mode: operate on palette indices directly.
    if explicit_mode == Some("P") {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = crate::raster::GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = inverse[ip[0] as usize];
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    // L-mode: operate on each luma value, returning P-mode output
    if img.color() == crate::raster::ColorType::L8 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = crate::raster::GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = inverse[ip[0] as usize];
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    // Non-P, non-L: operate on each RGB channel.
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = crate::raster::RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        op[0] = inverse[ip[0] as usize];
        op[1] = inverse[ip[1] as usize];
        op[2] = inverse[ip[2] as usize];
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

/// Extract a single band/channel from the image as an L-mode output.
/// index: 0=R, 1=G, 2=B, 3=A (for RGBA), 0=only band for L/LA
pub fn op_extract_band(img: &DynamicImage, index: u8) -> Result<DynamicImage, PilError> {
    let (w, h) = img.dimensions();
    let mut gray = crate::raster::GrayImage::new(w, h);
    let idx = index as usize;
    // Extract band from native format to avoid RGBA round-trip losing channels.
    // LA mode stored as La8: [L, A] at bytes 0, 1 per pixel.
    // RGB/RGBA/CMYK stored in their respective formats.
    match img {
        DynamicImage::ImageLumaA8(la) => {
            // La8: [L, A] per pixel, stride 2
            for (gp, lp) in gray.pixels_mut().zip(la.pixels()) {
                gp[0] = lp[idx.min(1)];
            }
        }
        DynamicImage::ImageRgba8(rgba) => {
            let ch = idx.min(3);
            for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
                gp[0] = rp[ch];
            }
        }
        _ => {
            // Fallback: convert to RGBA and extract
            let rgba = img.to_rgba8();
            let ch = idx.min(3);
            for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
                gp[0] = rp[ch];
            }
        }
    }
    Ok(DynamicImage::ImageLuma8(gray))
}
