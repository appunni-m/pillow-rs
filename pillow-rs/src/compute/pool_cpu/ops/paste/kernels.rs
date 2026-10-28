use crate::raster::{DynamicImage, GenericImageView};

use super::layout::{PasteMaskLayout, paste_native_channels};

/// Copy an unmasked paste using an exact same-mode native byte layout.
///
/// `None` means the logical modes, physical layouts, or checked dimensions do
/// not satisfy this route; callers then keep the established general path.
pub(super) fn paste_native_rows(
    destination: &DynamicImage,
    source: &DynamicImage,
    x: i64,
    y: i64,
    mode: &str,
) -> Option<DynamicImage> {
    let channels = paste_native_channels(mode, destination)?;
    if paste_native_channels(mode, source)? != channels {
        return None;
    }
    let (source_width, source_height) = source.dimensions();
    let (destination_width, destination_height) = destination.dimensions();
    let source_stride = usize::try_from(source_width).ok()?.checked_mul(channels)?;
    let destination_stride = usize::try_from(destination_width)
        .ok()?
        .checked_mul(channels)?;
    let source_len = source_stride.checked_mul(usize::try_from(source_height).ok()?)?;
    let destination_len =
        destination_stride.checked_mul(usize::try_from(destination_height).ok()?)?;
    let source_bytes = source.as_bytes();
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

    let row_bytes = usize::try_from(copy_width).ok()?.checked_mul(channels)?;
    let source_x_bytes = usize::try_from(source_left).ok()?.checked_mul(channels)?;
    let destination_x_bytes = usize::try_from(destination_left)
        .ok()?
        .checked_mul(channels)?;
    let source_x_end = source_x_bytes.checked_add(row_bytes)?;
    let destination_x_end = destination_x_bytes.checked_add(row_bytes)?;
    let source_rows_start = usize::try_from(source_top)
        .ok()?
        .checked_mul(source_stride)?;
    let destination_rows_start = usize::try_from(destination_top)
        .ok()?
        .checked_mul(destination_stride)?;
    let source_rows_end = source_rows_start
        .checked_add(source_stride.checked_mul(usize::try_from(copy_height).ok()?)?)?;
    let destination_rows_end = destination_rows_start
        .checked_add(destination_stride.checked_mul(usize::try_from(copy_height).ok()?)?)?;
    let mut output = destination.clone();
    let output_bytes = output.as_bytes_mut()?;
    let source_rows = source_bytes.get(source_rows_start..source_rows_end)?;
    let destination_rows = output_bytes.get_mut(destination_rows_start..destination_rows_end)?;
    for (source_row, destination_row) in source_rows
        .chunks_exact(source_stride)
        .zip(destination_rows.chunks_exact_mut(destination_stride))
    {
        destination_row
            .get_mut(destination_x_bytes..destination_x_end)?
            .copy_from_slice(source_row.get(source_x_bytes..source_x_end)?);
    }
    Some(output)
}

/// Copy native RGB rows into an RGBA destination, supplying Pillow's opaque
/// alpha for each converted source pixel without building an RGBA source.
pub(super) fn paste_rgb_into_rgba_rows(
    destination: &DynamicImage,
    source: &DynamicImage,
    x: i64,
    y: i64,
) -> Option<DynamicImage> {
    let (DynamicImage::ImageRgba8(destination), DynamicImage::ImageRgb8(source)) =
        (destination, source)
    else {
        return None;
    };
    let (source_width, source_height) = source.dimensions();
    let (destination_width, destination_height) = destination.dimensions();
    let source_stride = usize::try_from(source_width).ok()?.checked_mul(3)?;
    let destination_stride = usize::try_from(destination_width).ok()?.checked_mul(4)?;
    let source_len = source_stride.checked_mul(usize::try_from(source_height).ok()?)?;
    let destination_len =
        destination_stride.checked_mul(usize::try_from(destination_height).ok()?)?;
    let source_bytes = source.as_raw();
    if source_bytes.len() != source_len || destination.as_raw().len() != destination_len {
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
        return Some(DynamicImage::ImageRgba8(destination.clone()));
    }

    let source_x_bytes = usize::try_from(source_left).ok()?.checked_mul(3)?;
    let destination_x_bytes = usize::try_from(destination_left).ok()?.checked_mul(4)?;
    let source_rows_start = usize::try_from(source_top)
        .ok()?
        .checked_mul(source_stride)?;
    let destination_rows_start = usize::try_from(destination_top)
        .ok()?
        .checked_mul(destination_stride)?;
    let source_rows_bytes = source_stride.checked_mul(usize::try_from(copy_height).ok()?)?;
    let destination_rows_bytes =
        destination_stride.checked_mul(usize::try_from(copy_height).ok()?)?;
    let source_rows =
        source_bytes.get(source_rows_start..source_rows_start.checked_add(source_rows_bytes)?)?;
    let mut output = destination.as_raw().clone();
    let destination_rows = output.get_mut(
        destination_rows_start..destination_rows_start.checked_add(destination_rows_bytes)?,
    )?;
    let source_row_bytes = usize::try_from(copy_width).ok()?.checked_mul(3)?;
    let destination_row_bytes = usize::try_from(copy_width).ok()?.checked_mul(4)?;
    let source_x_end = source_x_bytes.checked_add(source_row_bytes)?;
    let destination_x_end = destination_x_bytes.checked_add(destination_row_bytes)?;
    let copy_row = |source_row: &[u8], destination_row: &mut [u8]| {
        let source_pixels = &source_row[source_x_bytes..source_x_end];
        let destination_pixels = &mut destination_row[destination_x_bytes..destination_x_end];
        for (rgb, rgba) in source_pixels
            .chunks_exact(3)
            .zip(destination_pixels.chunks_exact_mut(4))
        {
            rgba[..3].copy_from_slice(rgb);
            rgba[3] = 255;
        }
    };

    for (source_row, destination_row) in source_rows
        .chunks_exact(source_stride)
        .zip(destination_rows.chunks_exact_mut(destination_stride))
    {
        copy_row(source_row, destination_row);
    }
    Some(DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(destination_width, destination_height, output)?,
    ))
}

/// Blend native L samples with an L mask. Endpoint weights are exact under
/// the byte formula, so the hot loop needs no per-pixel branches.
#[inline]
pub(super) fn paste_native_masked_l_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len(), mask.len());

    for ((source_value, destination_value), mask_value) in source
        .iter()
        .copied()
        .zip(destination.iter_mut())
        .zip(mask.iter().copied())
    {
        let weight = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - weight;
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
        )]
        let weighted =
            u16::from(source_value) * weight + u16::from(*destination_value) * inverse + 127;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "DIV255 yields an 8-bit convex blend of byte channels"
        )]
        let blended = (weighted / 255) as u8;
        *destination_value = blended;
    }
}

/// Blend PA's palette-index/alpha pairs in their native two-byte layout.
/// The palette index is a stored sample here, not a request to expand colors.
pub(super) fn paste_native_masked_pa_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
    mask_layout: PasteMaskLayout,
) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 2, 0);
    debug_assert_eq!(source.len() / 2 * mask_layout.channels, mask.len());

    for ((source_pixel, destination_pixel), mask_pixel) in source
        .chunks_exact(2)
        .zip(destination.chunks_exact_mut(2))
        .zip(mask.chunks_exact(mask_layout.channels))
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
        for channel in 0..2 {
            destination_pixel[channel] = if mask_layout.premultiplied {
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "byte products and PREBLEND intermediates are bounded below u32::MAX"
                )]
                let scaled = u32::from(destination_pixel[channel]) * u32::from(inverse) + 128;
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "the proven PREBLEND intermediate is bounded below u32::MAX"
                )]
                let blended = (((scaled >> 8) + scaled) >> 8) + u32::from(source_pixel[channel]);
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
                let weighted = u16::from(source_pixel[channel]) * mask
                    + u16::from(destination_pixel[channel]) * inverse
                    + 127;
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

/// Blend two independent native byte channels with an L mask without
/// per-channel premultiplication checks or a nested channel loop. This is
/// used by LA and PA storage: paste treats both stored samples as bytes and
/// does not reinterpret either one as color or premultiplied alpha.
#[inline(always)]
pub(super) fn paste_native_masked_two_channel_blend_byte(
    source: u8,
    destination: u8,
    mask: u16,
    inverse: u16,
) -> u8 {
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
    )]
    let weighted = u16::from(source) * mask + u16::from(destination) * inverse + 127;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "DIV255 yields an 8-bit convex blend of byte channels"
    )]
    let blended = (weighted / 255) as u8;
    blended
}

#[inline]
pub(super) fn paste_native_masked_two_channel_l_row(
    source: &[u8],
    destination: &mut [u8],
    mask: &[u8],
) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 2, 0);
    debug_assert_eq!(source.len() / 2, mask.len());

    for ((source_pixel, destination_pixel), mask_value) in source
        .chunks_exact(2)
        .zip(destination.chunks_exact_mut(2))
        .zip(mask.iter().copied())
    {
        let mask = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - mask;
        destination_pixel[0] = paste_native_masked_two_channel_blend_byte(
            source_pixel[0],
            destination_pixel[0],
            mask,
            inverse,
        );
        destination_pixel[1] = paste_native_masked_two_channel_blend_byte(
            source_pixel[1],
            destination_pixel[1],
            mask,
            inverse,
        );
    }
}

/// Blend native CMYK samples with an L mask. CMYK's fourth stored byte is K,
/// so this path applies the same independent byte formula to all four samples
/// without treating K as alpha or converting the image to RGBA colors.
#[inline]
pub(super) fn paste_native_masked_cmyk_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 4, 0);
    debug_assert_eq!(source.len() / 4, mask.len());

    for ((source_pixel, destination_pixel), mask_value) in source
        .chunks_exact(4)
        .zip(destination.chunks_exact_mut(4))
        .zip(mask.iter().copied())
    {
        // The exact blend formula already preserves both endpoints, so keep
        // noisy masks on one branch-free arithmetic path through all C/M/Y/K
        // samples rather than testing 0 and 255 for every pixel.
        let weight = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - weight;
        let blend = |source_value: u8, destination_value: u8| {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "byte products sum to at most 255*255 before the rounding bias"
            )]
            let weighted =
                u16::from(source_value) * weight + u16::from(destination_value) * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            blended
        };
        destination_pixel[0] = blend(source_pixel[0], destination_pixel[0]);
        destination_pixel[1] = blend(source_pixel[1], destination_pixel[1]);
        destination_pixel[2] = blend(source_pixel[2], destination_pixel[2]);
        destination_pixel[3] = blend(source_pixel[3], destination_pixel[3]);
    }
}

/// Blend native HSV samples with an L mask without branching on each mask
/// value. HSV is stored as three independent bytes here; Paste blends those
/// stored channels directly, just as it does for RGB.
#[inline]
pub(super) fn paste_native_masked_hsv_l_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 3, 0);
    debug_assert_eq!(source.len() / 3, mask.len());

    for ((source_pixel, destination_pixel), mask_value) in source
        .chunks_exact(3)
        .zip(destination.chunks_exact_mut(3))
        .zip(mask.iter().copied())
    {
        let weight = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - weight;
        let blend = |source_value: u8, destination_value: u8| {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
            )]
            let weighted =
                u16::from(source_value) * weight + u16::from(destination_value) * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            blended
        };
        destination_pixel[0] = blend(source_pixel[0], destination_pixel[0]);
        destination_pixel[1] = blend(source_pixel[1], destination_pixel[1]);
        destination_pixel[2] = blend(source_pixel[2], destination_pixel[2]);
    }
}

/// Blend native RGB samples with an L mask. The mask endpoints are already
/// exact under the byte formula, so this avoids the generic path's per-pixel
/// endpoint branches and per-channel premultiplication branch.
#[inline]
pub(super) fn paste_native_masked_rgb_l_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 3, 0);
    debug_assert_eq!(source.len() / 3, mask.len());

    for ((source_pixel, destination_pixel), mask_value) in source
        .chunks_exact(3)
        .zip(destination.chunks_exact_mut(3))
        .zip(mask.iter().copied())
    {
        let weight = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - weight;
        let blend = |source_value: u8, destination_value: u8| {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
            )]
            let weighted =
                u16::from(source_value) * weight + u16::from(destination_value) * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            blended
        };
        destination_pixel[0] = blend(source_pixel[0], destination_pixel[0]);
        destination_pixel[1] = blend(source_pixel[1], destination_pixel[1]);
        destination_pixel[2] = blend(source_pixel[2], destination_pixel[2]);
    }
}

/// Blend native RGBA samples with one L-mask byte per pixel. Alpha is a
/// stored channel and follows the same Pillow byte blend as red, green, and
/// blue; endpoint weights need no special cases in the arithmetic loop.
#[inline]
pub(super) fn paste_native_masked_rgba_l_row(source: &[u8], destination: &mut [u8], mask: &[u8]) {
    debug_assert_eq!(source.len(), destination.len());
    debug_assert_eq!(source.len() % 4, 0);
    debug_assert_eq!(source.len() / 4, mask.len());

    for ((source_pixel, destination_pixel), mask_value) in source
        .chunks_exact(4)
        .zip(destination.chunks_exact_mut(4))
        .zip(mask.iter().copied())
    {
        let weight = u16::from(mask_value);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "mask is a byte, so subtracting it from 255 cannot underflow"
        )]
        let inverse = 255 - weight;
        let blend = |source_value: u8, destination_value: u8| {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "the convex byte blend is bounded by 255*255 plus its rounding bias"
            )]
            let weighted =
                u16::from(source_value) * weight + u16::from(destination_value) * inverse + 127;
            #[expect(
                clippy::cast_possible_truncation,
                reason = "DIV255 yields an 8-bit convex blend of byte channels"
            )]
            let blended = (weighted / 255) as u8;
            blended
        };
        destination_pixel[0] = blend(source_pixel[0], destination_pixel[0]);
        destination_pixel[1] = blend(source_pixel[1], destination_pixel[1]);
        destination_pixel[2] = blend(source_pixel[2], destination_pixel[2]);
        destination_pixel[3] = blend(source_pixel[3], destination_pixel[3]);
    }
}
