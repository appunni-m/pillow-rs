use super::ffi;
pub(super) fn positive_dimension_collapsed(base: i32, adjusted: i32) -> bool {
    // Pillow 12.2.0 `_imagingft.c::font_render_impl` accepts an exactly
    // zero-sized mask dimension when `start` consumes the base dimension;
    // only a negative result raises "bad image size".  `Image::from_luma_mask`
    // likewise preserves a legal `(width, 0)` mask with no pixel bytes.
    base > 0 && adjusted < 0
}

pub(super) fn bitmap_coverage(bitmap: &ffi::FT_Bitmap, row: usize, column: usize) -> u8 {
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    // `fontdone` preserves FreeType's signed-pitch convention: with a
    // negative pitch the buffer starts at the last logical row and rows are
    // addressed backwards.  `_imagingft.c` consumes that same descriptor
    // directly, so do not normalize the bitmap by reversing its owned bytes
    // here.  This matters for embedded bitmap strikes and any future
    // fontdone route that returns a bottom-up glyph bitmap.
    let physical_row = if bitmap.pitch < 0 {
        bitmap.rows.saturating_sub(1).saturating_sub(row as u32) as usize
    } else {
        row
    };
    let row_start = physical_row.saturating_mul(pitch);
    match bitmap.pixel_mode {
        ffi::FT_PIXEL_MODE_MONO => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 8)
                .copied()
                .unwrap_or(0);
            if byte & (0x80 >> (column & 7)) != 0 {
                255
            } else {
                0
            }
        }
        ffi::FT_PIXEL_MODE_GRAY2 => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 4)
                .copied()
                .unwrap_or(0);
            let value = (byte >> (6 - 2 * (column & 3))) & 0x03;
            value.saturating_mul(85)
        }
        ffi::FT_PIXEL_MODE_GRAY4 => {
            let byte = bitmap
                .buffer
                .get(row_start + column / 2)
                .copied()
                .unwrap_or(0);
            let value = if column & 1 == 0 {
                byte >> 4
            } else {
                byte & 0x0F
            };
            value.saturating_mul(17)
        }
        ffi::FT_PIXEL_MODE_BGRA => {
            let offset = row_start + column * 4;
            // `fontdone` SBIT decoding mirrors FreeType's BGRA bitmap shape:
            // pitch is `width * 4`, buffer length is `pitch * rows`, and this
            // helper is called only for pixels inside `bitmap.width/rows`.
            // Malformed embedded bitmap tables must be rejected in the lower
            // FreeType-compatible decoder, not hidden by Pillow `_imagingft`.
            debug_assert!(offset + 4 <= bitmap.buffer.len());
            let bgra = &bitmap.buffer[offset..offset + 4];
            gray_for_premultiplied_srgb_bgra(bgra)
        }
        _ => {
            debug_assert_eq!(
                bitmap.pixel_mode,
                ffi::FT_PIXEL_MODE_GRAY,
                "Pillow ImageFont BASIC rendering reaches imagingft as MONO or GRAY"
            );
            bitmap.buffer.get(row_start + column).copied().unwrap_or(0)
        }
    }
}

pub(super) fn bitmap_rgba_pixel(
    bitmap: &ffi::FT_Bitmap,
    row: usize,
    column: usize,
    foreground: (u8, u8, u8),
) -> [u8; 4] {
    if bitmap.pixel_mode == ffi::FT_PIXEL_MODE_BGRA {
        let pitch = bitmap.pitch.unsigned_abs() as usize;
        let physical_row = if bitmap.pitch < 0 {
            bitmap.rows.saturating_sub(1).saturating_sub(row as u32) as usize
        } else {
            row
        };
        let offset = physical_row
            .saturating_mul(pitch)
            .saturating_add(column.saturating_mul(4));
        debug_assert!(offset + 4 <= bitmap.buffer.len());
        if let Some(bgra) = bitmap.buffer.get(offset..offset.saturating_add(4)) {
            return rgba_for_premultiplied_srgb_bgra(bgra);
        }
    }
    let coverage = bitmap_coverage(bitmap, row, column);
    if coverage == 0 {
        [0, 0, 0, 0]
    } else {
        [foreground.0, foreground.1, foreground.2, coverage]
    }
}

pub(super) fn rgba_foreground_from_ink(ink: Option<i64>) -> (u8, u8, u8) {
    let bytes = ink.unwrap_or(0).to_le_bytes();
    (bytes[0], bytes[1], bytes[2])
}

pub(super) fn rgba_for_premultiplied_srgb_bgra(bgra: &[u8]) -> [u8; 4] {
    let alpha = u32::from(bgra[3]);
    if alpha == 0 {
        return [0, 0, 0, 0];
    }
    [
        (u32::from(bgra[2]) * 255 / alpha).min(255) as u8,
        (u32::from(bgra[1]) * 255 / alpha).min(255) as u8,
        (u32::from(bgra[0]) * 255 / alpha).min(255) as u8,
        bgra[3],
    ]
}

pub(super) fn gray_for_premultiplied_srgb_bgra(bgra: &[u8]) -> u8 {
    let alpha = u32::from(bgra[3]);
    if alpha == 0 {
        return 0;
    }
    let luminance = (4731u32 * u32::from(bgra[0]) * u32::from(bgra[0])
        + 46868u32 * u32::from(bgra[1]) * u32::from(bgra[1])
        + 13937u32 * u32::from(bgra[2]) * u32::from(bgra[2]))
        >> 16;
    alpha.wrapping_sub(luminance / alpha) as u8
}
