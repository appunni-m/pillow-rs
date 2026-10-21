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
        ColorMode::L => super::grayscale::op_grayscale(img, source_mode),
        ColorMode::LA => {
            let gray = pil_grayscale(img)?;
            let (w, h) = gray.dimensions();
            let mut ga = crate::raster::GrayAlphaImage::new(w, h);
            if ga.as_raw().capacity() != 0 {
                crate::compute::record_pipeline_allocation(ga.as_raw().capacity());
            }
            // Pillow retains alpha for both LA -> LA and RGBA -> LA. The
            // public same-mode shortcut usually avoids a queued LA conversion,
            // but an explicit PipelineOp must preserve the same samples.
            // PA's two bytes are palette indices/alpha, so its raw transport
            // must still pass through the palette-expansion boundary first.
            let source_alpha = match img {
                DynamicImage::ImageLumaA8(_) if matches!(source_mode, None | Some("LA")) => Some(
                    img.as_bytes()
                        .chunks_exact(2)
                        .map(|pixel| pixel[1])
                        .collect::<Vec<_>>(),
                ),
                DynamicImage::ImageRgba8(_) if source_mode != Some("RGBX") => Some(
                    img.as_bytes()
                        .chunks_exact(4)
                        .map(|pixel| pixel[3])
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            };
            if let Some(alpha) = &source_alpha {
                if alpha.capacity() != 0 {
                    crate::compute::record_pipeline_allocation(alpha.capacity());
                }
            }
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
        ColorMode::RGB if source_mode == Some("CMYK") => {
            // CMYK occupies the same four-byte transport as RGBA, but its
            // samples are C/M/Y/K. Do not discard K as if it were alpha.
            Ok(crate::color::cmyk_to_rgb(img))
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
        ColorMode::I => Ok(convert_rgb_to_i(img)),
        ColorMode::F => Ok(convert_rgb_to_f(img)),
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

/// Convert an RGB-family source to Pillow's signed integer luma storage.
#[inline(always)]
fn convert_rgb_to_i(img: &DynamicImage) -> DynamicImage {
    // CMYK/I/F normalization happens before this deferred operation is queued.
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
    DynamicImage::ImageRgba8(out)
}

/// Convert an RGB-family source to Pillow's floating point luma storage.
#[inline(always)]
fn convert_rgb_to_f(img: &DynamicImage) -> DynamicImage {
    // CMYK/I/F normalization happens before this deferred operation is queued.
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = crate::raster::RgbaImage::new(w, h);
    for (op, px) in out.pixels_mut().zip(rgb.pixels()) {
        let sum = px[0] as i32 * 299 + px[1] as i32 * 587 + px[2] as i32 * 114;
        let val = sum as f32 / 1000.0_f32;
        *op = crate::raster::Rgba(val.to_le_bytes());
    }
    DynamicImage::ImageRgba8(out)
}

#[cfg(test)]
#[path = "color_convert_allocation_tests.rs"]
mod la_conversion_allocation_receipts;

/// Convert normalized RGB8 pixels to Pillow's packed LAB representation.
pub fn op_convert_lab(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    crate::lab::convert_rgb_image(img)
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
    let idx = index as usize;
    let pixel_count = crate::checked_dims::CheckedDims::new_allow_empty(w, h, 1)?.total_pixels();

    // One-channel images already have the exact output representation. Copy
    // only the logical raster: ImageBuffer can retain trailing samples, while
    // Pillow's getchannel result contains exactly width * height bytes.
    if let DynamicImage::ImageLuma8(luma) = img {
        let samples = if idx < 3 {
            luma.as_raw().iter().copied().take(pixel_count).collect()
        } else {
            vec![u8::MAX; pixel_count]
        };
        if samples.capacity() != 0 {
            crate::compute::record_pipeline_allocation(samples.capacity());
        }
        return crate::image_utils::raw_bytes_to_image_allow_empty(w, h, samples, 1);
    }

    // RGB is stored as three interleaved bytes. Expanding it to RGBA before
    // selecting one byte performs a full-frame conversion and then discards
    // three quarters of that temporary. Build the one-byte output directly.
    if let DynamicImage::ImageRgb8(rgb) = img {
        let channel = idx.min(3);
        let samples = if channel < 3 {
            let source = rgb.as_raw();
            source
                .chunks_exact(3)
                .take(pixel_count)
                .map(|pixel| pixel[channel])
                .collect()
        } else {
            vec![u8::MAX; pixel_count]
        };
        if samples.capacity() != 0 {
            crate::compute::record_pipeline_allocation(samples.capacity());
        }
        return crate::image_utils::raw_bytes_to_image_allow_empty(w, h, samples, 1);
    }

    let mut gray = crate::raster::GrayImage::new(w, h);
    if gray.as_raw().capacity() != 0 {
        crate::compute::record_pipeline_allocation(gray.as_raw().capacity());
    }
    // Extract band from native format to avoid RGBA round-trip losing channels.
    // LA mode stored as La8: [L, A] at bytes 0, 1 per pixel.
    // RGB/RGBA/CMYK stored in their respective formats.
    match img {
        DynamicImage::ImageLumaA8(la) => {
            // Gather four La8 pixels per iteration while preserving the
            // logical raster boundary for buffers with trailing samples.
            let band = idx.min(1);
            let source = la.as_raw();
            let output = gray.as_mut();
            let grouped_pixels = pixel_count / 4 * 4;
            for (dst, pixels) in output[..grouped_pixels]
                .chunks_exact_mut(4)
                .zip(source.chunks_exact(8).take(grouped_pixels / 4))
            {
                dst[0] = pixels[band];
                dst[1] = pixels[band + 2];
                dst[2] = pixels[band + 4];
                dst[3] = pixels[band + 6];
            }
            let source_tail = source.get(grouped_pixels * 2..).unwrap_or_default();
            for (dst, pixel) in output[grouped_pixels..].iter_mut().zip(
                source_tail
                    .chunks_exact(2)
                    .take(pixel_count - grouped_pixels),
            ) {
                *dst = pixel[band];
            }
        }
        DynamicImage::ImageRgba8(rgba) => {
            // RGBA8 channels are interleaved at a four-byte stride. Process
            // complete groups directly and handle the logical tail separately.
            let band = idx.min(3);
            let source = rgba.as_raw();
            let output = gray.as_mut();
            let grouped_pixels = pixel_count / 4 * 4;
            for (dst, pixels) in output[..grouped_pixels]
                .chunks_exact_mut(4)
                .zip(source.chunks_exact(16).take(grouped_pixels / 4))
            {
                dst[0] = pixels[band];
                dst[1] = pixels[band + 4];
                dst[2] = pixels[band + 8];
                dst[3] = pixels[band + 12];
            }
            let source_tail = source.get(grouped_pixels * 4..).unwrap_or_default();
            for (dst, pixel) in output[grouped_pixels..].iter_mut().zip(
                source_tail
                    .chunks_exact(4)
                    .take(pixel_count - grouped_pixels),
            ) {
                *dst = pixel[band];
            }
        }
        _ => {
            // Fallback: convert to RGBA and extract
            let rgba = img.to_rgba8();
            if rgba.as_raw().capacity() != 0 {
                crate::compute::record_pipeline_allocation(rgba.as_raw().capacity());
            }
            let ch = idx.min(3);
            for (gp, rp) in gray.pixels_mut().zip(rgba.pixels()) {
                gp[0] = rp[ch];
            }
        }
    }
    Ok(DynamicImage::ImageLuma8(gray))
}

#[cfg(test)]
mod extract_band_tests {
    use super::op_extract_band;
    use crate::compute::Backend;
    use crate::image::Image;
    use crate::raster::{DynamicImage, GenericImageView, RgbImage};
    use std::sync::Arc;

    fn rgb_with_trailing_storage(width: u32, height: u32) -> DynamicImage {
        let logical_bytes = usize::try_from(width * height * 3).expect("small RGB shape");
        let samples = (0..logical_bytes + 18)
            .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
            .collect();
        DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, samples).expect("valid overlong RGB storage"),
        )
    }

    #[test]
    fn rgb_extract_band_ignores_trailing_storage_for_every_tail_and_band() {
        let shapes = [
            (0, 7),
            (7, 0),
            (1, 1),
            (2, 1),
            (3, 1),
            (5, 1),
            (6, 1),
            (7, 1),
        ];
        for (width, height) in shapes {
            let source = rgb_with_trailing_storage(width, height);
            let pixel_count = usize::try_from(width * height).expect("small RGB shape");
            for channel in [0, 1, 2, 3, u8::MAX] {
                let output = op_extract_band(&source, channel).expect("CPU RGB extraction");
                let expected: Vec<u8> = if channel < 3 {
                    source.as_bytes()[..pixel_count * 3]
                        .chunks_exact(3)
                        .map(|pixel| pixel[usize::from(channel)])
                        .collect()
                } else {
                    vec![u8::MAX; pixel_count]
                };
                assert_eq!(output.dimensions(), (width, height));
                assert_eq!(output.color(), crate::raster::ColorType::L8);
                assert_eq!(
                    output.as_bytes(),
                    expected,
                    "shape={width}x{height}, band={channel}"
                );
            }
        }
    }

    #[test]
    fn public_cpu_getchannel_ignores_overlong_loaded_rgb_storage() {
        // Public Rust callers can replace LoadedData.image with a valid
        // overlong ImageBuffer; Python frombytes truncates this storage first.
        // Counts above the eager threshold exercise the deferred CPU path.
        for width in [257, 258, 259, 260] {
            let source = rgb_with_trailing_storage(width, 1);
            let mut image = Image::new(width, 1, "RGB", (0, 0, 0, 255)).expect("RGB image");
            let Image::Loaded(data) = &mut image else {
                panic!("new RGB image must have loaded storage");
            };
            data.image = Arc::new(source.clone());
            for channel in 0..3 {
                let result = image
                    .getchannel(channel)
                    .expect("public valid RGB band")
                    .use_backend(Backend::Cpu);
                let actual = result.tobytes().expect("CPU band materialization");
                let expected: Vec<u8> = source.as_bytes()
                    [..usize::try_from(width * 3).expect("small RGB shape")]
                    .chunks_exact(3)
                    .map(|pixel| pixel[usize::try_from(channel).expect("valid band")])
                    .collect();
                assert_eq!(actual, expected, "width={width}, band={channel}");
                assert_eq!(result.mode().expect("output mode"), "L");
            }
            assert!(image.getchannel(-1).is_err());
            assert!(image.getchannel(3).is_err());
        }
    }
}
