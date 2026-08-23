//! Color/Convert CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible color mode conversion, quantization, and palette remapping.

use crate::checked_dims::CheckedDims;
use crate::color::{pil_grayscale, pil_grayscale_truncate};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::quantize::median_cut_quantize_rgb;
use crate::pipeline::{ColorMode, DitherMethod};
use crate::raster::DynamicImage;
use crate::raster::GenericImageView;

/// Convert image to a specified color mode.
/// Matches PIL's Image.convert() behavior exactly.
/// `explicit_mode` is the PIL mode string on the source image.
pub fn op_convert(
    img: &DynamicImage,
    mode: &ColorMode,
    dither: Option<&DitherMethod>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match mode {
        ColorMode::L => Ok(DynamicImage::ImageLuma8(pil_grayscale(img)?)),
        ColorMode::LA => {
            let gray = pil_grayscale(img)?;
            let (w, h) = gray.dimensions();
            let mut ga = crate::raster::GrayAlphaImage::new(w, h);
            // Pillow's convert.c carries the source alpha through an RGBA
            // to LA conversion.  RGBX uses an unused byte instead of alpha,
            // while CMYK/I/F are four-byte storage formats rather than alpha
            // images; those modes therefore retain the opaque default.
            let source_alpha = if matches!(img.color(), crate::raster::ColorType::Rgba8)
                && !matches!(explicit_mode, Some("RGBX" | "CMYK" | "I" | "F"))
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
            if explicit_mode == Some("RGBX") {
                // RGBX uses the same four-byte storage as RGBA, but its X byte
                // is padding rather than opacity. Pillow expands it to an
                // opaque RGBA pixel during conversion.
                let rgb = img.to_rgb8();
                let (w, h) = rgb.dimensions();
                let mut out = crate::raster::RgbaImage::new(w, h);
                for (output, input) in out.pixels_mut().zip(rgb.pixels()) {
                    *output = crate::raster::Rgba([input[0], input[1], input[2], 255]);
                }
                Ok(DynamicImage::ImageRgba8(out))
            } else {
                Ok(DynamicImage::ImageRgba8(img.to_rgba8()))
            }
        }
        ColorMode::Mode1 => {
            // DEPRECATED deferred compatibility arm: the public Image::convert
            // path materializes binary threshold/dither conversions eagerly.
            // PIL uses TRUNCATED grayscale for convert("1") (dither or no dither)
            // while convert("L") uses ROUNDED grayscale.
            // CMYK mode: proper CMYK→RGB→L conversion before thresholding.
            let gray = if explicit_mode == Some("CMYK") {
                crate::color::cmyk_to_grayscale_truncate(img)?
            } else {
                pil_grayscale_truncate(img)?
            };
            let (w, h) = gray.dimensions();
            let mut out = crate::raster::GrayImage::new(w, h);
            match dither {
                Some(DitherMethod::None) => {
                    // Threshold at 128 (no dither)
                    for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                        op[0] = if gp[0] >= 128 { 255 } else { 0 };
                    }
                }
                _ => {
                    // PIL-compatible Floyd-Steinberg dither using PIL's scaled-error pattern.
                    // Single errors array [w+1]; running l0/l1 carry error between rows.
                    // Truncation-toward-zero division, no intermediate clipping.
                    let mut errors = vec![0i32; (w + 1) as usize];
                    let src: Vec<i32> = gray.pixels().map(|p| p[0] as i32).collect();
                    let mut fs_out = CheckedDims::new(w, h, 1)?.alloc_buffer();
                    let wu = w as usize;
                    for y in 0..h as usize {
                        let mut l = 0i32;
                        let mut l0: i32 = 0;
                        let mut l1: i32 = 0;
                        for x in 0..wu {
                            let idx = y * wu + x;
                            let acc = l + errors[x + 1];
                            let v = src[idx] + acc / 16;
                            let v = v.clamp(0, 255);
                            let new = if v > 128 { 255i32 } else { 0i32 };
                            fs_out[idx] = new as u8;
                            l = v - new;
                            let l2 = l;
                            let d2 = l + l;
                            l += d2;
                            errors[x] = l + l0;
                            l += d2;
                            l0 = l + l1;
                            l1 = l2;
                            l += d2;
                        }
                    }
                    for (op, &gp) in out.pixels_mut().zip(fs_out.iter()) {
                        op[0] = gp;
                    }
                }
            }
            Ok(DynamicImage::ImageLuma8(out))
        }
        ColorMode::P => {
            // DEPRECATED deferred compatibility arm: public palette conversion
            // is owned by Image::convert/quantize before pipeline dispatch.
            // convert("P") = quantize(256) with dither
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let n = CheckedDims::new(w, h, 1)?.total_pixels();
            let rgb_raw = rgb.into_raw();
            let (indices, _palette) = median_cut_quantize_rgb(&rgb_raw, 256);
            let mut out = crate::raster::GrayImage::new(w, h);
            for (i, pixel) in out.pixels_mut().enumerate().take(n) {
                pixel[0] = indices.get(i).copied().unwrap_or(0);
            }
            Ok(DynamicImage::ImageLuma8(out))
        }
        ColorMode::I => {
            // Convert to int32 mode: PIL stores rounded grayscale as int32 LE in RGBA.
            // Use the luma formula directly (no intermediate u8 truncation).
            // CMYK is stored in the RGBA container as C/M/Y/K, not as an
            // ordinary RGBA pixel. Convert it through Pillow's CMYK->RGB
            // path before deriving luma; otherwise a zero CMYK sample is
            // incorrectly treated as black instead of white.
            // DEPRECATED deferred compatibility arm: public CMYK->I conversion
            // is materialized by Image::convert before this operation runs.
            let rgb = if explicit_mode == Some("CMYK") {
                crate::color::cmyk_to_rgb(img).to_rgb8()
            } else {
                img.to_rgb8()
            };
            let (w, h) = rgb.dimensions();
            let mut out = crate::raster::RgbaImage::new(w, h);
            for (op, px) in out.pixels_mut().zip(rgb.pixels()) {
                let r = px[0] as i32;
                let g = px[1] as i32;
                let b = px[2] as i32;
                // PIL's rounded luma: (19595*R + 38470*G + 7471*B + 32768) >> 16
                let val = (19595i32 * r + 38470i32 * g + 7471i32 * b + 32768) >> 16;
                let le = val.to_le_bytes();
                *op = crate::raster::Rgba([le[0], le[1], le[2], le[3]]);
            }
            Ok(DynamicImage::ImageRgba8(out))
        }
        ColorMode::F => {
            // Convert to float32 mode using PIL's exact formula from rgb2f:
            //   v = (r*299 + g*587 + b*114) / 1000.0F
            // This computes the sum in integer arithmetic (matching PIL's `L` macro)
            // then divides by 1000.0F as float, matching PIL pixel-for-pixel.
            // DEPRECATED deferred compatibility arm: public CMYK->F conversion
            // is materialized by Image::convert before this operation runs.
            let rgb = if explicit_mode == Some("CMYK") {
                crate::color::cmyk_to_rgb(img).to_rgb8()
            } else {
                img.to_rgb8()
            };
            let (w, h) = rgb.dimensions();
            let mut out = crate::raster::RgbaImage::new(w, h);
            for (op, px) in out.pixels_mut().zip(rgb.pixels()) {
                let sum = px[0] as i32 * 299 + px[1] as i32 * 587 + px[2] as i32 * 114;
                let val = sum as f32 / 1000.0_f32;
                let le = val.to_le_bytes();
                *op = crate::raster::Rgba([le[0], le[1], le[2], le[3]]);
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
                // "1" mode is stored as raw 0/1 bytes; Pillow converts it to
                // "L" first (1 -> 255) before the gray->K branch.
                // DEPRECATED deferred compatibility arm: Image::convert
                // normalizes public mode-1 sources before deferred dispatch.
                let gray = if explicit_mode == Some("1") {
                    let mut out = crate::raster::GrayImage::new(w, h);
                    for (op, ip) in out.pixels_mut().zip(img.to_luma8().pixels()) {
                        op[0] = if ip[0] != 0 { 255 } else { 0 };
                    }
                    crate::raster::DynamicImage::ImageLuma8(out)
                } else {
                    crate::raster::DynamicImage::ImageLuma8(crate::color::pil_grayscale(img)?)
                };
                let mut out = crate::raster::RgbaImage::new(w, h);
                for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                    *op = crate::raster::Rgba([0u8, 0u8, 0u8, 255u8.wrapping_sub(gp.2[0])]);
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
    }
}

/// Legacy registry quantization implementation.
///
/// The supported public [`Image::quantize`](crate::Image::quantize) path owns
/// palette construction, alpha handling, and method validation. This simpler
/// deferred operation is retained only for internally constructed
/// `PipelineOp::Quantize` values; no supported public pipeline creates one.
#[deprecated(note = "legacy deferred quantization; use Image::quantize instead")]
pub fn op_quantize(
    img: &DynamicImage,
    colors: usize,
    _dither: Option<&DitherMethod>,
) -> Result<DynamicImage, PilError> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let n = CheckedDims::new(w, h, 1)?.total_pixels();
    if n == 0 {
        return Err(PilError::ValueError("quantize: empty image".into()));
    }
    let colors = colors.clamp(2, 256);
    let rgb_raw = rgb.into_raw();
    if rgb_raw.len() < colors * 3 {
        return Err(PilError::ValueError(
            "quantize: not enough pixel data".into(),
        ));
    }
    // Use median-cut quantization instead of NeuQuant.
    let (indices, _palette) = median_cut_quantize_rgb(&rgb_raw, colors);
    let mut out = crate::raster::GrayImage::new(w, h);
    for (i, pixel) in out.pixels_mut().enumerate().take(n) {
        pixel[0] = indices.get(i).copied().unwrap_or(0);
    }
    Ok(DynamicImage::ImageLuma8(out))
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
