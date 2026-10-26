use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::raster::{DynamicImage, GenericImageView};
use std::sync::Arc;

use crate::checked_dims::CheckedDims;

#[cfg(feature = "parallel")]
use super::effects::EFFECT_PARALLEL_PIXEL_THRESHOLD;

#[path = "paste/kernels.rs"]
mod kernels;
#[path = "paste/layout.rs"]
mod layout;
#[cfg(test)]
#[path = "paste/tests.rs"]
mod tests;

use kernels::{
    paste_native_masked_cmyk_row, paste_native_masked_hsv_l_row, paste_native_masked_l_row,
    paste_native_masked_pa_row, paste_native_masked_rgb_l_row, paste_native_masked_rgba_l_row,
    paste_native_masked_two_channel_l_row, paste_native_rows, paste_rgb_into_rgba_rows,
};
use layout::{
    PasteMaskLayout, PasteMaskPixels, paste_luma16_storage_byte_shift, paste_mask_pixels,
    paste_native_channels,
};

struct NativeMaskedRows<'a> {
    mode: &'a str,
    channels: usize,
    mask_layout: PasteMaskLayout,
    source_bytes: &'a [u8],
    mask_bytes: &'a [u8],
    source_stride: usize,
    source_top: usize,
    source_x: usize,
    destination_top: usize,
    destination_x: usize,
    destination_x_end: usize,
    copy_bytes: usize,
    mask_stride: usize,
    mask_x: usize,
    mask_copy_bytes: usize,
}

impl NativeMaskedRows<'_> {
    fn transform_row(&self, destination_y: usize, row: &mut [u8]) {
        let source_y = self
            .source_top
            .saturating_add(destination_y.saturating_sub(self.destination_top));
        let source_start = source_y
            .saturating_mul(self.source_stride)
            .saturating_add(self.source_x);
        let mask_start = source_y
            .saturating_mul(self.mask_stride)
            .saturating_add(self.mask_x);
        let source_row =
            &self.source_bytes[source_start..source_start.saturating_add(self.copy_bytes)];
        let destination_row = &mut row[self.destination_x..self.destination_x_end];
        let mask_row =
            &self.mask_bytes[mask_start..mask_start.saturating_add(self.mask_copy_bytes)];
        paste_native_masked_apply_row(
            self.mode,
            self.channels,
            self.mask_layout,
            source_row,
            destination_row,
            mask_row,
        );
    }
}

#[inline(always)]
fn paste_native_masked_rows(
    output_bytes: &mut [u8],
    destination_top: usize,
    destination_stride: usize,
    _copy_width: usize,
    copy_height: usize,
    transform_rows: NativeMaskedRows<'_>,
) -> Option<()> {
    #[cfg(feature = "parallel")]
    if _copy_width.saturating_mul(copy_height) >= EFFECT_PARALLEL_PIXEL_THRESHOLD {
        let output_row_start = destination_top.checked_mul(destination_stride)?;
        let output_row_end = destination_top
            .checked_add(copy_height)?
            .checked_mul(destination_stride)?;
        let output_rows = output_bytes.get_mut(output_row_start..output_row_end)?;
        crate::par_rows_mut!(
            output_rows,
            destination_stride,
            copy_height,
            |_row_start, _row_end, row_index, row| {
                let destination_y = destination_top.saturating_add(row_index as usize);
                transform_rows.transform_row(destination_y, row);
            }
        );
    } else {
        for row_index in 0..copy_height {
            let destination_y = destination_top.saturating_add(row_index);
            let row_start = destination_y.saturating_mul(destination_stride);
            transform_rows.transform_row(
                destination_y,
                &mut output_bytes[row_start..row_start.saturating_add(destination_stride)],
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for row_index in 0..copy_height {
        let destination_y = destination_top.saturating_add(row_index);
        let row_start = destination_y.saturating_mul(destination_stride);
        transform_rows.transform_row(
            destination_y,
            &mut output_bytes[row_start..row_start.saturating_add(destination_stride)],
        );
    }
    Some(())
}

/// Blend an exact same-mode byte layout directly. This covers common modes
/// whose storage is already native; tagged, indexed, scalar, and cross-mode
/// cases retain the established conversion path until their contracts have
/// separate kernels.
fn paste_native_masked(
    destination: &DynamicImage,
    source: &Arc<Image>,
    x: i64,
    y: i64,
    mask: &Arc<Image>,
    mask_alpha: bool,
    mode: &str,
) -> Option<DynamicImage> {
    if !native_masked_mode_supported(mode) {
        return None;
    }
    let channels = paste_native_channels(mode, destination)?;
    if source.mode().ok()?.as_str() != mode {
        return None;
    }
    let source_image = source.materialized_shared().ok()?;
    if paste_native_channels(mode, source_image.as_ref())? != channels {
        return None;
    }
    let (source_width, source_height) = source_image.dimensions();
    let (destination_width, destination_height) = destination.dimensions();
    let source_stride = usize::try_from(source_width).ok()?.checked_mul(channels)?;
    let destination_stride = usize::try_from(destination_width)
        .ok()?
        .checked_mul(channels)?;
    let source_len = source_stride.checked_mul(usize::try_from(source_height).ok()?)?;
    let destination_len =
        destination_stride.checked_mul(usize::try_from(destination_height).ok()?)?;
    let source_bytes = source_image.as_bytes();
    let destination_bytes = destination.as_bytes();
    if source_bytes.len() != source_len || destination_bytes.len() != destination_len {
        return None;
    }

    let source_left = u32::try_from(x.saturating_neg().max(0).min(i64::from(source_width))).ok()?;
    let source_top = u32::try_from(y.saturating_neg().max(0).min(i64::from(source_height))).ok()?;
    let destination_left = u32::try_from(x.max(0).min(i64::from(destination_width))).ok()?;
    let destination_top = u32::try_from(y.max(0).min(i64::from(destination_height))).ok()?;
    let copy_width = source_width
        .saturating_sub(source_left)
        .min(destination_width.saturating_sub(destination_left));
    let copy_height = source_height
        .saturating_sub(source_top)
        .min(destination_height.saturating_sub(destination_top));
    if copy_width == 0 || copy_height == 0 {
        return Some(destination.clone());
    }

    let mask_pixels = paste_mask_pixels(mask, mask_alpha).ok()??;
    if matches!(mode, "1" | "P") && !mask_pixels.layout.binary {
        return None;
    }
    if (mask_pixels.width, mask_pixels.height) != (source_width as usize, source_height as usize) {
        return None;
    }

    let copy_width = usize::try_from(copy_width).ok()?;
    let copy_height = usize::try_from(copy_height).ok()?;
    let source_x = usize::try_from(source_left).ok()?.checked_mul(channels)?;
    let destination_x = usize::try_from(destination_left)
        .ok()?
        .checked_mul(channels)?;
    let copy_bytes = copy_width.checked_mul(channels)?;
    let mask_channels = mask_pixels.layout.channels;
    let mask_stride = mask_pixels.width.checked_mul(mask_channels)?;
    let mask_x = usize::try_from(source_left)
        .ok()?
        .checked_mul(mask_channels)?;
    let mask_copy_bytes = copy_width.checked_mul(mask_channels)?;
    let mask_bytes = mask_pixels.image.as_bytes();
    let source_x_end = source_x.checked_add(copy_bytes)?;
    let destination_x_end = destination_x.checked_add(copy_bytes)?;
    let mask_x_end = mask_x.checked_add(mask_copy_bytes)?;
    let mask_len = mask_stride.checked_mul(mask_pixels.height)?;
    if source_x_end > source_stride
        || destination_x_end > destination_stride
        || mask_x_end > mask_stride
        || mask_bytes.len() != mask_len
    {
        return None;
    }

    let mut output = destination.clone();
    let output_bytes = output.as_bytes_mut()?;
    let source_top = usize::try_from(source_top).ok()?;
    let destination_top = usize::try_from(destination_top).ok()?;
    let transform_rows = NativeMaskedRows {
        mode,
        channels,
        mask_layout: mask_pixels.layout,
        source_bytes,
        mask_bytes,
        source_stride,
        source_top,
        source_x,
        destination_top,
        destination_x,
        destination_x_end,
        copy_bytes,
        mask_stride,
        mask_x,
        mask_copy_bytes,
    };

    paste_native_masked_rows(
        output_bytes,
        destination_top,
        destination_stride,
        copy_width,
        copy_height,
        transform_rows,
    )?;
    Some(output)
}

fn paste_native_masked_apply_row(
    mode: &str,
    channels: usize,
    mask_layout: PasteMaskLayout,
    source_row: &[u8],
    destination_row: &mut [u8],
    mask_row: &[u8],
) {
    if !paste_native_masked_specialized_row(
        mode,
        channels,
        mask_layout,
        source_row,
        destination_row,
        mask_row,
    ) {
        paste_native_masked_blend_pixels(
            channels,
            mask_layout,
            source_row,
            destination_row,
            mask_row,
        );
    }
}

fn paste_native_masked_specialized_row(
    mode: &str,
    channels: usize,
    mask_layout: PasteMaskLayout,
    source_row: &[u8],
    destination_row: &mut [u8],
    mask_row: &[u8],
) -> bool {
    if mask_layout.binary {
        paste_native_masked_binary_row(
            source_row,
            destination_row,
            mask_row,
            channels,
            mask_layout.channels,
            mask_layout.value_index,
        );
        return true;
    }

    let plain_l_mask =
        mask_layout.channels == 1 && mask_layout.value_index == 0 && !mask_layout.premultiplied;
    match (mode, plain_l_mask) {
        ("LA", true) | ("PA", true) => {
            paste_native_masked_two_channel_l_row(source_row, destination_row, mask_row);
        }
        ("L", true) => paste_native_masked_l_row(source_row, destination_row, mask_row),
        ("RGB", true) => paste_native_masked_rgb_l_row(source_row, destination_row, mask_row),
        ("RGBA", true) => paste_native_masked_rgba_l_row(source_row, destination_row, mask_row),
        ("HSV", true) => paste_native_masked_hsv_l_row(source_row, destination_row, mask_row),
        ("CMYK", true) => paste_native_masked_cmyk_row(source_row, destination_row, mask_row),
        ("PA", false) => {
            paste_native_masked_pa_row(source_row, destination_row, mask_row, mask_layout)
        }
        _ => return false,
    }
    true
}

fn paste_native_masked_blend_pixels(
    channels: usize,
    mask_layout: PasteMaskLayout,
    source_row: &[u8],
    destination_row: &mut [u8],
    mask_row: &[u8],
) {
    for ((source_pixel, destination_pixel), mask_pixel) in source_row
        .chunks_exact(channels)
        .zip(destination_row.chunks_exact_mut(channels))
        .zip(mask_row.chunks_exact(mask_layout.channels))
    {
        let mask_value = mask_pixel[mask_layout.value_index];
        if mask_value == 0 && !mask_layout.premultiplied {
            continue;
        }
        if mask_value == 255 {
            destination_pixel.copy_from_slice(source_pixel);
            continue;
        }

        let mask = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - mask;
        for (source_value, destination_value) in
            source_pixel.iter().zip(destination_pixel.iter_mut())
        {
            *destination_value = if mask_layout.premultiplied {
                // Pillow's RGBa-mask path uses PREBLEND: add the incoming
                // premultiplied sample after scaling the destination.
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "byte products and PREBLEND intermediates are bounded below u32::MAX"
                )]
                let scaled = u32::from(*destination_value) * u32::from(inverse) + 128;
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "the proven PREBLEND intermediate is bounded below u32::MAX"
                )]
                let blended = (((scaled >> 8) + scaled) >> 8) + u32::from(*source_value);
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "Pillow stores PREBLEND's sum modulo 256 in the byte channel"
                )]
                let blended = blended as u8;
                blended
            } else {
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
                )]
                let weighted =
                    u16::from(*source_value) * mask + u16::from(*destination_value) * inverse + 127;
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "DIV255 yields an 8-bit convex blend of byte channels"
                )]
                let blended = (weighted / 255) as u8;
                blended
            };
        }
    }
}

fn native_masked_mode_supported(mode: &str) -> bool {
    matches!(
        mode,
        "1" | "L" | "P" | "LA" | "PA" | "RGB" | "HSV" | "RGBA" | "CMYK"
    )
}

#[inline]
fn paste_native_masked_binary_row(
    source_row: &[u8],
    destination_row: &mut [u8],
    mask_row: &[u8],
    channels: usize,
    mask_channels: usize,
    value_index: usize,
) {
    for ((source_pixel, destination_pixel), mask_pixel) in source_row
        .chunks_exact(channels)
        .zip(destination_row.chunks_exact_mut(channels))
        .zip(mask_row.chunks_exact(mask_channels))
    {
        if mask_pixel[value_index] != 0 {
            destination_pixel.copy_from_slice(source_pixel);
        }
    }
}

#[derive(Clone, Copy)]
struct PasteRegion {
    source_left: u32,
    source_top: u32,
    dest_left: u32,
    dest_top: u32,
    copy_width: u32,
    copy_height: u32,
}

enum Luma16PasteMask {
    Native(PasteMaskPixels),
    Luma(crate::raster::GrayImage),
    Alpha(crate::raster::RgbaImage),
}

enum PasteMask {
    Native(PasteMaskPixels),
    Luma(crate::raster::GrayImage),
    Alpha(crate::raster::RgbaImage),
}

#[inline(always)]
fn paste_luma16_fallback(
    img: &DynamicImage,
    src_img: &DynamicImage,
    mask: &Option<Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
    region: PasteRegion,
) -> Result<DynamicImage, PilError> {
    // Pillow's masked I;16 Paste updates the first byte in the mode's
    // storage order, preserving the other byte. The unmasked path still
    // copies full unsigned 16-bit samples.
    let source_luma = src_img.to_luma16();
    let mut destination = img.to_luma16();
    let mask_pixels = match mask {
        Some(mask_image) => {
            if let Some(pixels) = paste_mask_pixels(mask_image, mask_alpha)? {
                Some(Luma16PasteMask::Native(pixels))
            } else {
                let materialized = mask_image.materialize()?;
                if mask_alpha {
                    Some(Luma16PasteMask::Alpha(materialized.to_rgba8()))
                } else {
                    Some(Luma16PasteMask::Luma(materialized.to_luma8()))
                }
            }
        }
        None => None,
    };
    let mask_mode = mask.as_ref().map(|image| image.mode()).transpose()?;
    let binary_mask = mask_mode.as_deref() == Some("1");

    for offset_y in 0..region.copy_height {
        let source_y = region.source_top + offset_y;
        let dest_y = region.dest_top + offset_y;
        for offset_x in 0..region.copy_width {
            let source_x = region.source_left + offset_x;
            let dest_x = region.dest_left + offset_x;
            let Some(mask_image) = mask_pixels.as_ref() else {
                let source_value = source_luma.get_pixel(source_x, source_y)[0];
                destination.put_pixel(dest_x, dest_y, crate::raster::Luma([source_value]));
                continue;
            };
            let mask_value = match mask_image {
                Luma16PasteMask::Native(pixels) => pixels.value(source_x, source_y),
                Luma16PasteMask::Luma(pixels) => pixels.get_pixel(source_x, source_y)[0],
                Luma16PasteMask::Alpha(pixels) => pixels.get_pixel(source_x, source_y)[3],
            };
            let premultiplied_mask = matches!(
                mask_image,
                Luma16PasteMask::Native(pixels) if pixels.layout.premultiplied
            );
            if binary_mask {
                if mask_value != 0 {
                    let source_value = source_luma.get_pixel(source_x, source_y)[0];
                    destination.put_pixel(dest_x, dest_y, crate::raster::Luma([source_value]));
                }
                continue;
            }

            // Pillow's 8-bit L/RGBA/RGBa mask loops advance the I;16
            // image8 pointer by one byte per image pixel. Preserve this
            // odd byte stride exactly; RGBa also applies PREBLEND at a
            // zero mask value. This is Pillow's observable contract.
            if mask_value == 0 && !premultiplied_mask {
                continue;
            }
            let source_byte_offset = source_x;
            let destination_byte_offset = dest_x;
            let source_byte_x = source_byte_offset / 2;
            let destination_byte_x = destination_byte_offset / 2;
            let source_value = source_luma.get_pixel(source_byte_x, source_y)[0];
            let destination_value = destination.get_pixel(destination_byte_x, dest_y)[0];
            let source_shift = paste_luma16_storage_byte_shift(mode, source_byte_offset);
            let destination_shift = paste_luma16_storage_byte_shift(mode, destination_byte_offset);
            let source_byte = ((source_value >> source_shift) & 0xff) as u8;
            let destination_byte = ((destination_value >> destination_shift) & 0xff) as u8;
            let mask = u32::from(mask_value);
            let inverse = 255 - mask;
            let blended = if premultiplied_mask {
                let scaled = u32::from(destination_byte) * inverse + 128;
                ((((scaled >> 8) + scaled) >> 8) + u32::from(source_byte)) as u8
            } else {
                ((u32::from(source_byte) * mask + u32::from(destination_byte) * inverse + 127)
                    / 255) as u8
            };
            let byte_mask = 0xffu16 << destination_shift;
            let updated =
                (destination_value & !byte_mask) | (u16::from(blended) << destination_shift);
            destination.put_pixel(destination_byte_x, dest_y, crate::raster::Luma([updated]));
        }
    }

    Ok(DynamicImage::ImageLuma16(destination))
}

#[inline(always)]
fn paste_rgba_fallback(
    img: &DynamicImage,
    src_img: &DynamicImage,
    mask: &Option<Arc<Image>>,
    mask_alpha: bool,
    region: PasteRegion,
) -> Result<DynamicImage, PilError> {
    let source_rgba = src_img.to_rgba8();
    let mut destination = img.to_rgba8();
    let mask_pixels = match mask {
        Some(mask_image) => {
            if let Some(pixels) = paste_mask_pixels(mask_image, mask_alpha)? {
                Some(PasteMask::Native(pixels))
            } else {
                let materialized = mask_image.materialize()?;
                if mask_alpha {
                    Some(PasteMask::Alpha(materialized.to_rgba8()))
                } else {
                    Some(PasteMask::Luma(materialized.to_luma8()))
                }
            }
        }
        None => None,
    };

    for offset_y in 0..region.copy_height {
        let source_y = region.source_top + offset_y;
        let dest_y = region.dest_top + offset_y;
        for offset_x in 0..region.copy_width {
            let source_x = region.source_left + offset_x;
            let dest_x = region.dest_left + offset_x;
            let source_pixel = *source_rgba.get_pixel(source_x, source_y);
            let Some(mask_image) = mask_pixels.as_ref() else {
                destination.put_pixel(dest_x, dest_y, source_pixel);
                continue;
            };
            let mask_value = match mask_image {
                PasteMask::Native(pixels) => pixels.value(source_x, source_y),
                PasteMask::Luma(pixels) => pixels.get_pixel(source_x, source_y)[0],
                PasteMask::Alpha(pixels) => pixels.get_pixel(source_x, source_y)[3],
            };
            let premultiplied_mask = matches!(
                mask_image,
                PasteMask::Native(pixels) if pixels.layout.premultiplied
            );
            if mask_value == 0 && !premultiplied_mask {
                continue;
            }
            if mask_value == 255 {
                destination.put_pixel(dest_x, dest_y, source_pixel);
                continue;
            }

            // Pillow libImaging uses BLEND/DIV255 for every active band:
            // DIV255(src * mask + dst * (255 - mask)). Its integer macro is
            // equivalent to round-to-nearest for this 8-bit input range.
            let destination_pixel = *destination.get_pixel(dest_x, dest_y);
            let mask = u16::from(mask_value);
            let inverse = 255u16 - mask;
            let blend = |src: u8, dst: u8| -> u8 {
                if premultiplied_mask {
                    let scaled = u32::from(dst) * u32::from(inverse) + 128;
                    ((((scaled >> 8) + scaled) >> 8) + u32::from(src)) as u8
                } else {
                    ((u16::from(src) * mask + u16::from(dst) * inverse + 127) / 255) as u8
                }
            };
            destination.put_pixel(
                dest_x,
                dest_y,
                crate::raster::Rgba([
                    blend(source_pixel[0], destination_pixel[0]),
                    blend(source_pixel[1], destination_pixel[1]),
                    blend(source_pixel[2], destination_pixel[2]),
                    blend(source_pixel[3], destination_pixel[3]),
                ]),
            );
        }
    }

    Ok(preserve_mode(img, DynamicImage::ImageRgba8(destination)))
}

fn native_paste_source_mode<'a>(
    source: &Image,
    mask: &Option<Arc<Image>>,
    mode: Option<&'a str>,
) -> Result<Option<&'a str>, PilError> {
    let source_mode = if mask.is_none()
        && matches!(
            mode,
            None | Some(
                "1" | "L"
                    | "P"
                    | "LA"
                    | "La"
                    | "PA"
                    | "RGB"
                    | "HSV"
                    | "YCbCr"
                    | "LAB"
                    | "RGBA"
                    | "RGBa"
                    | "RGBX"
                    | "CMYK"
                    | "I"
                    | "F"
            )
        ) {
        Some(source.mode()?)
    } else {
        None
    };
    Ok(match (mode, source_mode.as_deref()) {
        (Some(mode), Some(source_mode)) if mode == source_mode => Some(mode),
        (None, Some("RGB")) => Some("RGB"),
        _ => None,
    })
}

pub fn op_paste(
    img: &DynamicImage,
    source: &Arc<Image>,
    x: i64,
    y: i64,
    mask: &Option<Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let expanded_source = expand_compact_masked_source(source, mask, mask_alpha, mode)?;
    let source = expanded_source.as_ref().unwrap_or(source);
    let native_mode = native_paste_source_mode(source, mask, mode)?;
    let native_source = if let Some(native_mode) = native_mode {
        if matches!(native_mode, "P" | "PA") {
            Some(Arc::new(source.materialize_indices()?))
        } else {
            Some(source.materialized_shared()?)
        }
    } else {
        None
    };
    if mask.is_none()
        && let (Some(native_mode), Some(source_image)) = (native_mode, native_source.as_deref())
        && let Some(output) = paste_native_rows(img, source_image, x, y, native_mode)
    {
        return Ok(output);
    }

    if let (Some(mode), Some(mask_image)) = (mode, mask)
        && let Some(output) = paste_native_masked(img, source, x, y, mask_image, mask_alpha, mode)
    {
        return Ok(output);
    }

    let src_img = if matches!(mode, Some("P" | "PA")) {
        source.materialize_indices()?
    } else if let Some(source_image) = native_source {
        source_image.as_ref().clone()
    } else {
        source.materialize_for_ops()?
    };
    let (src_w, src_h) = (src_img.width(), src_img.height());
    let (dest_w, dest_h) = img.dimensions();
    let region = PasteRegion {
        source_left: x.saturating_neg().max(0).min(i64::from(src_w)) as u32,
        source_top: y.saturating_neg().max(0).min(i64::from(src_h)) as u32,
        dest_left: x.max(0).min(i64::from(dest_w)) as u32,
        dest_top: y.max(0).min(i64::from(dest_h)) as u32,
        copy_width: 0,
        copy_height: 0,
    };
    let region = PasteRegion {
        copy_width: src_w
            .saturating_sub(region.source_left)
            .min(dest_w.saturating_sub(region.dest_left)),
        copy_height: src_h
            .saturating_sub(region.source_top)
            .min(dest_h.saturating_sub(region.dest_top)),
        ..region
    };
    if region.copy_width == 0 || region.copy_height == 0 {
        return Ok(img.clone());
    }
    // Image::paste validates mask dimensions against the paste region before
    // queuing this operation, so every loop coordinate is valid for the mask.
    // Keep malformed PipelineOp fallback arms out of this public-input path.

    if img.color() == crate::raster::ColorType::L16 {
        return paste_luma16_fallback(img, &src_img, mask, mask_alpha, mode, region);
    }

    if mask.is_none()
        && mode == Some("RGBA")
        && let Some(output) = paste_rgb_into_rgba_rows(img, &src_img, x, y)
    {
        crate::compute::record_pipeline_operation_path("native-rgb-to-rgba-copy");
        return Ok(output);
    }

    paste_rgba_fallback(img, &src_img, mask, mask_alpha, region)
}

fn expand_compact_masked_source(
    source: &Arc<Image>,
    mask: &Option<Arc<Image>>,
    mask_alpha: bool,
    mode: Option<&str>,
) -> Result<Option<Arc<Image>>, PilError> {
    // If GPU admission or device limits select the ordinary CPU Paste path,
    // restore the one-pixel bitmap fill to the region size before blending.
    let Some(mask) = mask else {
        return Ok(None);
    };
    let source_mode = source.mode()?;
    let (source_width, source_height) = source.size()?;
    let (mask_width, mask_height) = mask.size()?;
    let mask_mode = mask.mode()?;
    let mode_supported = if mask_mode == "1" {
        matches!(
            source_mode.as_str(),
            "1" | "P" | "L" | "LA" | "RGB" | "HSV" | "RGBA" | "CMYK"
        )
    } else {
        mask_mode == "L" && matches!(source_mode.as_str(), "L" | "RGB" | "HSV" | "RGBA" | "CMYK")
    };
    if mask_alpha
        || !mode_supported
        || (source_width, source_height) != (1, 1)
        || (mask_width, mask_height) == (1, 1)
        || mode.is_some_and(|mode| mode != source_mode && !(mode == "RGBA" && source_mode == "RGB"))
    {
        return Ok(None);
    }

    let source_pixels = source.materialized_shared()?;
    let sample = source_pixels.as_bytes();
    let channels = u8::try_from(sample.len())
        .map_err(|_| PilError::ValueError("invalid compact Paste source".into()))?;
    if channels == 0 {
        return Err(PilError::ValueError("invalid compact Paste source".into()));
    }
    let dimensions = CheckedDims::new(mask_width, mask_height, channels)?;
    let mut bytes = dimensions.alloc_buffer();
    for destination_sample in bytes.chunks_exact_mut(sample.len()) {
        destination_sample.copy_from_slice(sample);
    }
    Ok(Some(Arc::new(Image::frombytes(
        &source_mode,
        (mask_width, mask_height),
        &bytes,
    )?)))
}
