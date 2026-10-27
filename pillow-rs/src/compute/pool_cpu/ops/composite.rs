use super::effects::{apply_effect_rows, blend_row};
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::pipeline::ColorMode;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};
use std::sync::Arc;

#[path = "composite/tests.rs"]
#[cfg(test)]
mod tests;

// ── AlphaComposite ──

#[inline]
fn alpha_composite_div255(value: u32) -> u32 {
    // Pillow's ImagingUtils.h SHIFTFORDIV255 macro. AlphaComposite.c relies
    // on this fixed-point division rather than real-number rounding.
    ((value >> 8) + value) >> 8
}

#[inline]
fn alpha_composite_pixel<const CHANNELS: usize>(source: &[u8], destination: &mut [u8]) {
    let source_alpha = u32::from(source[CHANNELS - 1]);
    if source_alpha == 0 {
        // The entire destination, including invisible color payload, survives.
        return;
    }
    let destination_alpha = u32::from(destination[CHANNELS - 1]);
    let blend = destination_alpha * (255 - source_alpha);
    let out_alpha_255 = source_alpha * 255 + blend;
    let coefficient_source = source_alpha * 255 * 255 * (1 << 7) / out_alpha_255;
    let coefficient_destination = (255 << 7) - coefficient_source;
    for channel in 0..CHANNELS - 1 {
        let blended = u32::from(source[channel]) * coefficient_source
            + u32::from(destination[channel]) * coefficient_destination;
        destination[channel] = (alpha_composite_div255(blended + (0x80 << 7)) >> 7) as u8;
    }
    destination[CHANNELS - 1] = alpha_composite_div255(out_alpha_255 + 0x80) as u8;
}

pub fn op_alpha_composite(
    img: &DynamicImage,
    source: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let src_img = source.materialized_shared()?;
    // Image::alpha_composite validates mode and dimensions before queuing this
    // operation; this executor only receives matching source and destination
    // images from supported public inputs.

    // LA mode: composite on native LA canvas, return LA (PIL behavior)
    if matches!(img.color(), crate::raster::ColorType::La8) {
        let mut dest_la = img.to_luma_alpha8();
        let DynamicImage::ImageLumaA8(src_la) = src_img.as_ref() else {
            return Err(PilError::ValueError("images do not match".into()));
        };
        let (sw, sh) = src_la.dimensions();
        let width = sw.min(dest_la.width()) as usize;
        let height = sh.min(dest_la.height()) as usize;
        let source = src_la.as_raw();
        let source_stride = sw as usize * 2;
        apply_effect_rows(dest_la.as_mut(), width, height, 2, |row_index, row| {
            let source_start = row_index * source_stride;
            let source_row = &source[source_start..source_start + width * 2];
            for (sp, dp) in source_row.chunks_exact(2).zip(row.chunks_exact_mut(2)) {
                alpha_composite_pixel::<2>(sp, dp);
            }
        });
        return Ok(DynamicImage::ImageLumaA8(dest_la));
    }

    let mut dest_rgba = img.to_rgba8();
    let DynamicImage::ImageRgba8(src_rgba) = src_img.as_ref() else {
        return Err(PilError::ValueError("images do not match".into()));
    };
    let (sw, sh) = src_rgba.dimensions();
    let width = sw.min(dest_rgba.width()) as usize;
    let height = sh.min(dest_rgba.height()) as usize;
    let source = src_rgba.as_raw();
    let source_stride = sw as usize * 4;
    apply_effect_rows(dest_rgba.as_mut(), width, height, 4, |row_index, row| {
        let source_start = row_index * source_stride;
        let source_row = &source[source_start..source_start + width * 4];
        for (sp, dp) in source_row.chunks_exact(4).zip(row.chunks_exact_mut(4)) {
            alpha_composite_pixel::<4>(sp, dp);
        }
    });
    Ok(DynamicImage::ImageRgba8(dest_rgba))
}

/// Composite full-frame RGBA inputs and mirror the result in one output pass.
/// The fused scan reads each input at its mirrored coordinate and writes the
/// final row-major output, avoiding an intermediate composite image and the
/// subsequent mirror copy. Uniform image pairs need only one exact Pillow
/// pixel calculation; they are still scanned and every output sample is
/// materialized.
pub fn op_alpha_composite_mirror(
    img: &DynamicImage,
    source: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(destination) = img else {
        return Err(PilError::ValueError("images do not match".into()));
    };
    let source_owner = source.materialized_shared()?;
    let DynamicImage::ImageRgba8(source_image) = source_owner.as_ref() else {
        return Err(PilError::ValueError("images do not match".into()));
    };
    if source_image.dimensions() != img.dimensions() {
        return Err(PilError::ValueError("images do not match".into()));
    }

    let (width, height) = img.dimensions();
    let dims = crate::checked_dims::CheckedDims::new(width, height, 4)?;
    let destination_bytes = destination.as_raw();
    let source_bytes = source_image.as_raw();
    if destination_bytes.len() != dims.total_bytes() || source_bytes.len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBA alpha composite mirror storage does not match its dimensions".into(),
        ));
    }

    let mut output = dims.alloc_buffer();
    let uniform_pixel = |bytes: &[u8]| {
        let first: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
        bytes
            .chunks_exact(4)
            .all(|pixel| pixel == first.as_slice())
            .then_some(first)
    };
    if let (Some(source_pixel), Some(destination_pixel)) = (
        uniform_pixel(source_bytes),
        uniform_pixel(destination_bytes),
    ) {
        let mut result_pixel = destination_pixel;
        alpha_composite_pixel::<4>(&source_pixel, &mut result_pixel);
        for pixel in output.chunks_exact_mut(4) {
            pixel.copy_from_slice(&result_pixel);
        }
    } else {
        let row_stride = dims.row_stride();
        for (row_index, output_row) in output.chunks_exact_mut(row_stride).enumerate() {
            let row_start = row_index * row_stride;
            for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
                let source_x = width as usize - 1 - output_x;
                let source_offset = row_start + source_x * 4;
                output_pixel.copy_from_slice(&destination_bytes[source_offset..source_offset + 4]);
                alpha_composite_pixel::<4>(
                    &source_bytes[source_offset..source_offset + 4],
                    output_pixel,
                );
            }
        }
    }

    let result = crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, 4)?;
    Ok(preserve_mode(img, result))
}

// ── Merge ──

#[cfg(not(feature = "parallel"))]
fn interleave_rgb_luma_bands(red: &[u8], green: &[u8], blue: &[u8]) -> Result<Vec<u8>, PilError> {
    if red.len() != green.len() || red.len() != blue.len() {
        return Err(PilError::InternalError(
            "RGB merge band lengths do not match".into(),
        ));
    }
    let output_len = red
        .len()
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("merge: output length overflow".into()))?;
    const PIXELS_PER_CHUNK: usize = 64;
    let mut output = Vec::with_capacity(output_len);
    let mut red_chunks = red.chunks_exact(PIXELS_PER_CHUNK);
    let mut green_chunks = green.chunks_exact(PIXELS_PER_CHUNK);
    let mut blue_chunks = blue.chunks_exact(PIXELS_PER_CHUNK);
    for ((red_chunk, green_chunk), blue_chunk) in red_chunks
        .by_ref()
        .zip(green_chunks.by_ref())
        .zip(blue_chunks.by_ref())
    {
        let mut pixels = [[0; 3]; PIXELS_PER_CHUNK];
        for ((pixel, &red), (&green, &blue)) in pixels
            .iter_mut()
            .zip(red_chunk)
            .zip(green_chunk.iter().zip(blue_chunk))
        {
            *pixel = [red, green, blue];
        }
        output.extend_from_slice(pixels.as_flattened());
    }
    for ((&red, &green), &blue) in red_chunks
        .remainder()
        .iter()
        .zip(green_chunks.remainder())
        .zip(blue_chunks.remainder())
    {
        output.extend([red, green, blue]);
    }
    Ok(output)
}

/// Merge three native L bands into interleaved RGB without cloning their
/// complete source images.
fn try_merge_rgb_luma_bands(
    img: &DynamicImage,
    mode: &ColorMode,
    bands: &[Arc<Image>],
    logical_mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !matches!(mode, ColorMode::RGB) || logical_mode != Some("RGB") || bands.len() != 3 {
        return Ok(None);
    }
    let DynamicImage::ImageLuma8(red) = img else {
        return Ok(None);
    };
    let green_image = bands[1].materialized_shared_for_ops()?;
    let blue_image = bands[2].materialized_shared_for_ops()?;
    let (DynamicImage::ImageLuma8(green), DynamicImage::ImageLuma8(blue)) =
        (green_image.as_ref(), blue_image.as_ref())
    else {
        return Ok(None);
    };
    let (w, h) = red.dimensions();
    let pixels = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| PilError::ValueError("merge: image dimensions overflow".into()))?;
    if red.as_raw().len() != pixels
        || green.as_raw().len() != pixels
        || blue.as_raw().len() != pixels
    {
        return Ok(None);
    }
    let red = red.as_raw();
    let green = green.as_raw();
    let blue = blue.as_raw();

    #[cfg(feature = "parallel")]
    let rgb = {
        let output_len = pixels
            .checked_mul(3)
            .ok_or_else(|| PilError::ValueError("merge: output length overflow".into()))?;
        let mut rgb = vec![0u8; output_len];
        apply_effect_rows(&mut rgb, w as usize, h as usize, 3, |row_index, row| {
            let source_start = row_index * w as usize;
            for (x, pixel) in row.chunks_exact_mut(3).enumerate() {
                let source_index = source_start + x;
                pixel[0] = red[source_index];
                pixel[1] = green[source_index];
                pixel[2] = blue[source_index];
            }
        });
        rgb
    };
    #[cfg(not(feature = "parallel"))]
    let rgb = interleave_rgb_luma_bands(red, green, blue)?;
    crate::compute::record_pipeline_allocation(rgb.len());
    let image = RgbImage::from_raw(w, h, rgb)
        .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
    Ok(Some(DynamicImage::ImageRgb8(image)))
}

pub fn op_merge(
    img: &DynamicImage,
    mode: &ColorMode,
    bands: &[Arc<Image>],
    logical_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // Image::merge validates the supported mode, band count, band modes, and
    // dimensions before queuing this operation. The executor therefore only
    // handles the validated mode matrix below.
    // A single I/F band is already the complete native destination buffer.
    // Pillow's ImagingMerge accepts only the same typed source mode here, so
    // retaining the first buffer preserves signed integer/IEEE-754 bytes
    // exactly instead of reducing them through an 8-bit luma conversion.
    if matches!(mode, ColorMode::I | ColorMode::F) {
        return Ok(img.clone());
    }

    // The native-L fast path avoids cloning each complete DynamicImage.
    if let Some(image) = try_merge_rgb_luma_bands(img, mode, bands, logical_mode)? {
        return Ok(image);
    }

    // Get pixel data from each byte band.
    let mut band_pixels: Vec<Vec<u8>> = Vec::new();
    let first_gray = img.to_luma8();
    let (w, h) = first_gray.dimensions();
    band_pixels.push(first_gray.into_raw());
    for band in bands.iter().skip(1) {
        let b_img = band.materialize_for_ops()?;
        let b_gray = b_img.to_luma8();
        band_pixels.push(b_gray.into_raw());
    }
    let n = (w * h) as usize;
    match mode {
        ColorMode::RGB | ColorMode::YCbCr | ColorMode::HSV => {
            let mut rgb = vec![0u8; n * 3];
            let lab = logical_mode == Some("LAB");
            apply_effect_rows(&mut rgb, w as usize, h as usize, 3, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(3).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = if lab {
                        band_pixels[1][source_index].wrapping_add(128)
                    } else {
                        band_pixels[1][source_index]
                    };
                    pixel[2] = if lab {
                        band_pixels[2][source_index].wrapping_add(128)
                    } else {
                        band_pixels[2][source_index]
                    };
                }
            });
            let img = RgbImage::from_raw(w, h, rgb)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(img))
        }
        ColorMode::RGBA => {
            let mut rgba = vec![0u8; n * 4];
            apply_effect_rows(&mut rgba, w as usize, h as usize, 4, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = band_pixels[1][source_index];
                    pixel[2] = band_pixels[2][source_index];
                    pixel[3] = band_pixels[3][source_index];
                }
            });
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorMode::CMYK => {
            let mut rgba = vec![0u8; n * 4];
            apply_effect_rows(&mut rgba, w as usize, h as usize, 4, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(4).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = band_pixels[1][source_index];
                    pixel[2] = band_pixels[2][source_index];
                    pixel[3] = band_pixels[3][source_index];
                }
            });
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorMode::LA => {
            let mut la = vec![0u8; n * 2];
            apply_effect_rows(&mut la, w as usize, h as usize, 2, |row_index, row| {
                let source_start = row_index * w as usize;
                for (pixel_index, pixel) in row.chunks_exact_mut(2).enumerate() {
                    let source_index = source_start + pixel_index;
                    pixel[0] = band_pixels[0][source_index];
                    pixel[1] = band_pixels[1][source_index];
                }
            });
            let img = GrayAlphaImage::from_raw(w, h, la)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLumaA8(img))
        }
        ColorMode::L | ColorMode::Mode1 | ColorMode::P => {
            let img = GrayImage::from_raw(w, h, band_pixels.remove(0))
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(img))
        }
        // Exhaustiveness for ColorMode variants that Image::merge rejects
        // before it queues PipelineOp::Merge; not a supported input path.
        _ => Err(PilError::ValueError("Unsupported merge mode".into())),
    }
}

// ── BlendModule ──

pub fn op_blend_module(
    img: &DynamicImage,
    other: &Arc<Image>,
    alpha: f64,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let first_lab = explicit_mode == Some("LAB");
    let second_lab = other.mode()?.as_str() == "LAB";
    let other_img = other.materialized_shared()?;
    // Public blend validates the mode family and dimensions before queuing.
    // Every stored byte participates, including LA/RGBA alpha and CMYK K.
    // Borrow both native buffers instead of cloning them through conversions;
    // L does not need an RGB expansion followed by a grayscale conversion.
    let channels = match img {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return Err(PilError::ValueError("image has wrong mode".into())),
    };
    let (width, height) = img.dimensions();
    let first = img.as_bytes();
    let second = other_img.as_bytes();
    if first.len() != second.len() || img.dimensions() != other_img.dimensions() {
        return Err(PilError::ValueError("images do not match".into()));
    }
    if first_lab || second_lab {
        // Pillow blends its logical LAB samples. Our native LAB A/B bytes
        // use the raw encoder's 128 bias; remove it on each LAB operand and
        // restore it only when the first image (the result mode) is LAB.
        let alpha = alpha as f32;
        let output = first
            .iter()
            .zip(second)
            .enumerate()
            .map(|(index, (&left, &right))| {
                let bias = if index % 3 == 0 { 0 } else { 128 };
                let left = left ^ if first_lab { bias } else { 0 };
                let right = right ^ if second_lab { bias } else { 0 };
                let value = alpha
                    .mul_add(f32::from(right) - f32::from(left), f32::from(left))
                    .clamp(0.0, 255.0) as u8;
                value ^ if first_lab { bias } else { 0 }
            })
            .collect();
        return crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels);
    }
    let row_bytes = width as usize * channels;
    let mut output = vec![0u8; first.len()];
    apply_effect_rows(
        &mut output,
        width as usize,
        height as usize,
        channels,
        |row_index, row| {
            let start = row_index * row_bytes;
            blend_row(
                &first[start..start + row_bytes],
                &second[start..start + row_bytes],
                row,
                alpha,
            );
        },
    );
    // Blend preserves zero-sized images, unlike the ordinary allocation API.
    crate::image_utils::raw_bytes_to_image_allow_empty(width, height, output, channels)
}

// ── CompositeModule ──

fn composite_mask(mask: &DynamicImage, mask_alpha: bool) -> GrayImage {
    if !mask_alpha {
        return mask.to_luma8();
    }

    let rgba = mask.to_rgba8();
    GrayImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        crate::raster::Luma([rgba.get_pixel(x, y)[3]])
    })
}

#[inline]
fn composite_blend(source: u8, destination: u8, mask: u8) -> u8 {
    let mask = u16::from(mask);
    let inverse = 255u16 - mask;
    // Pillow 12.2.0 Paste.c applies ImagingUtils.h's BLEND/DIV255 macro to
    // every active destination band.
    ((u16::from(source) * mask + u16::from(destination) * inverse + 127) / 255) as u8
}

#[inline]
fn composite_l_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len(), mask.len());
    for index in 0..source.len() {
        destination[index] = composite_blend(source[index], destination[index], mask[index]);
    }
}

/// Blend the native byte layouts used by the common L/LA/RGB/RGBA composite
/// path. Pillow keeps image2's canvas and changes only the source/mask
/// intersection, so starting with its bytes also handles clipping and empty
/// overlap without materializing RGB conversions or per-pixel image wrappers.
fn composite_native_byte_fast_path(
    img: &DynamicImage,
    other: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
) -> Option<DynamicImage> {
    let channels = match img {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return None,
    };
    if img.color() != other.color() {
        return None;
    }
    let (mask_channels, mask_channel) = match mask {
        DynamicImage::ImageLuma8(_) if !mask_alpha => (1, 0),
        DynamicImage::ImageLumaA8(_) if mask_alpha => (2, 1),
        DynamicImage::ImageRgba8(_) if mask_alpha => (4, 3),
        _ => return None,
    };

    let (source_width, source_height) = img.dimensions();
    let (destination_width, destination_height) = other.dimensions();
    let (mask_width, mask_height) = mask.dimensions();
    let source_width = source_width as usize;
    let source_height = source_height as usize;
    let destination_width = destination_width as usize;
    let destination_height = destination_height as usize;
    let mask_width = mask_width as usize;
    let mask_height = mask_height as usize;
    let overlap_width = source_width.min(destination_width).min(mask_width);
    let overlap_height = source_height.min(destination_height).min(mask_height);
    let source_stride = source_width.checked_mul(channels)?;
    let destination_stride = destination_width.checked_mul(channels)?;
    let mask_stride = mask_width.checked_mul(mask_channels)?;
    let overlap_bytes = overlap_width.checked_mul(channels)?;
    let overlap_mask_bytes = overlap_width.checked_mul(mask_channels)?;
    let source = img.as_bytes();
    let mask = mask.as_bytes();
    let mut output = other.as_bytes().to_vec();
    if source.len() != source_stride.checked_mul(source_height)?
        || output.len() != destination_stride.checked_mul(destination_height)?
        || mask.len() != mask_stride.checked_mul(mask_height)?
    {
        return None;
    }

    apply_effect_rows(
        &mut output,
        destination_width,
        destination_height,
        channels,
        |row_index, output_row| {
            if row_index >= overlap_height {
                return;
            }
            let source_start = row_index * source_stride;
            let mask_start = row_index * mask_stride;
            let source_row = &source[source_start..source_start + overlap_bytes];
            let mask_row = &mask[mask_start..mask_start + overlap_mask_bytes];
            if channels == 1 && mask_channels == 1 && mask_channel == 0 {
                composite_l_row(source_row, &mut output_row[..overlap_bytes], mask_row);
                return;
            }
            for ((output_pixel, source_pixel), mask_pixel) in output_row[..overlap_bytes]
                .chunks_exact_mut(channels)
                .zip(source_row.chunks_exact(channels))
                .zip(mask_row.chunks_exact(mask_channels))
            {
                let mask_value = mask_pixel[mask_channel];
                for (destination, &source) in output_pixel.iter_mut().zip(source_pixel) {
                    *destination = composite_blend(source, *destination, mask_value);
                }
            }
        },
    );

    crate::image_utils::raw_bytes_to_image_allow_empty(
        destination_width as u32,
        destination_height as u32,
        output,
        channels,
    )
    .ok()
    .map(|result| preserve_mode(img, result))
}

pub fn op_composite_module(
    img: &DynamicImage,
    other: &Arc<Image>,
    mask: &Arc<Image>,
    mask_alpha: bool,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL composite: copy image2, then paste image1 onto it with mask at (0,0).
    // The output uses image2's size. Smaller images are pasted into the top-left.
    // Paste.c uses the alpha byte for LA/RGBA/RGBa masks and the luma byte for
    // 1/L masks. The choice is captured before backend dispatch. Keep the
    // source storage intact for the byte fast path's selected mask channel.
    let mask_image = mask.materialize_for_ops()?;

    // P-mode: composite on palette indices (PIL operates on indices, not colors)
    if explicit_mode == Some("P") {
        let mask_gray = composite_mask(&mask_image, mask_alpha);
        let gray1 = img.to_luma8();
        let other_indices = other.materialize_indices()?;
        let gray2 = other_indices.to_luma8();
        let mut out = gray2.clone();
        let overlap_w = gray1.width().min(gray2.width()).min(mask_gray.width());
        let overlap_h = gray1.height().min(gray2.height()).min(mask_gray.height());
        for y in 0..overlap_h {
            for x in 0..overlap_w {
                let value = composite_blend(
                    gray1.get_pixel(x, y)[0],
                    gray2.get_pixel(x, y)[0],
                    mask_gray.get_pixel(x, y)[0],
                );
                out.put_pixel(x, y, crate::raster::Luma([value]));
            }
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }

    let other_img = other.materialize_for_ops()?;

    if let Some(result) = composite_native_byte_fast_path(img, &other_img, &mask_image, mask_alpha)
    {
        return Ok(result);
    }
    let mask_gray = composite_mask(&mask_image, mask_alpha);

    // RGBA and four-byte compatibility modes (CMYK/I/F) blend every stored
    // band. In particular, RGBA alpha is output data, not merely metadata.
    if matches!(img.color(), crate::raster::ColorType::Rgba8) {
        let rgba1 = img.to_rgba8();
        let rgba2 = other_img.to_rgba8();
        let mut out = rgba2.clone();
        let overlap_w = rgba1.width().min(rgba2.width()).min(mask_gray.width());
        let overlap_h = rgba1.height().min(rgba2.height()).min(mask_gray.height());
        for y in 0..overlap_h {
            for x in 0..overlap_w {
                let p1 = rgba1.get_pixel(x, y);
                let p2 = rgba2.get_pixel(x, y);
                let m = mask_gray.get_pixel(x, y)[0];
                out.put_pixel(
                    x,
                    y,
                    crate::raster::Rgba([
                        composite_blend(p1[0], p2[0], m),
                        composite_blend(p1[1], p2[1], m),
                        composite_blend(p1[2], p2[2], m),
                        composite_blend(p1[3], p2[3], m),
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }
    // LA mode: composite both L and A channels natively
    if matches!(img.color(), crate::raster::ColorType::La8) {
        let la1 = img.to_luma_alpha8();
        let la2 = other_img.to_luma_alpha8();
        let mut out = la2.clone();
        let overlap_w = la1.width().min(la2.width()).min(mask_gray.width());
        let overlap_h = la1.height().min(la2.height()).min(mask_gray.height());
        for y in 0..overlap_h {
            for x in 0..overlap_w {
                let p1 = la1.get_pixel(x, y);
                let p2 = la2.get_pixel(x, y);
                let m = mask_gray.get_pixel(x, y)[0];
                out.put_pixel(
                    x,
                    y,
                    crate::raster::LumaA([
                        composite_blend(p1[0], p2[0], m),
                        composite_blend(p1[1], p2[1], m),
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageLumaA8(out));
    }
    let rgb1 = img.to_rgb8();
    let rgb2 = other_img.to_rgb8();
    let mut out = rgb2.clone();
    let overlap_w = rgb1.width().min(rgb2.width()).min(mask_gray.width());
    let overlap_h = rgb1.height().min(rgb2.height()).min(mask_gray.height());
    for y in 0..overlap_h {
        for x in 0..overlap_w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            let m = mask_gray.get_pixel(x, y)[0];
            out.put_pixel(
                x,
                y,
                crate::raster::Rgb([
                    composite_blend(p1[0], p2[0], m),
                    composite_blend(p1[1], p2[1], m),
                    composite_blend(p1[2], p2[2], m),
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

// ── Eval ──
