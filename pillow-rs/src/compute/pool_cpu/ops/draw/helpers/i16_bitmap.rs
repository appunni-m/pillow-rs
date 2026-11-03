use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::raster::{ImageBuffer, Luma, Mode, Pixel};

type Luma16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

/// Apply Pillow's I;16 Bitmap byte-lane behavior to the native u16 storage.
///
/// Pillow's `Paste.c` addresses both bytes for an L mask. The `1`, `RGBA`, and
/// `RGBa` paths step through the raw row byte prefix at one byte per mask
/// pixel; RGBa uses alpha coverage with PREBLEND. The selected mask strategy
/// is fixed before the loop, with no sample projection or full-frame rewrite.
pub(crate) fn apply(
    canvas: &mut Luma16Image,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) {
    match mask.mode() {
        Mode::One => apply_mask::<true, false, false>(
            canvas,
            x,
            y,
            mask_width,
            mask_height,
            mask.data(),
            color,
        ),
        Mode::ImageLuma8 => apply_mask::<false, true, false>(
            canvas,
            x,
            y,
            mask_width,
            mask_height,
            mask.data(),
            color,
        ),
        Mode::ImageRgba8 => apply_mask::<false, false, false>(
            canvas,
            x,
            y,
            mask_width,
            mask_height,
            mask.data(),
            color,
        ),
        Mode::ImageRgbPremultiplied8 => apply_mask::<false, false, true>(
            canvas,
            x,
            y,
            mask_width,
            mask_height,
            mask.data(),
            color,
        ),
        _ => unreachable!("Bitmap mask mode was validated before I;16 execution"),
    }
}

fn apply_mask<const BINARY: bool, const LUMA: bool, const PREMULTIPLIED: bool>(
    canvas: &mut Luma16Image,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &[u8],
    color: (u8, u8, u8, u8),
) {
    let (image_width, image_height) = canvas.dimensions();
    let region = BitmapRegion::clipped(
        x,
        y,
        image_width,
        image_height,
        mask_width,
        mask_height,
    );
    let row_bytes = image_width as usize * 2;
    let bytes: &mut [u8] = bytemuck::cast_slice_mut(canvas.as_mut());

    for mask_y in region.y_start..region.y_end {
        let image_y = (i64::from(y) + i64::from(mask_y)) as usize;
        let row_start = image_y * row_bytes;
        for mask_x in region.x_start..region.x_end {
            let mask_index = mask_y as usize * mask_width as usize + mask_x as usize;
            let coverage = if BINARY {
                u8::from(mask[mask_index] != 0) * u8::MAX
            } else if LUMA {
                mask[mask_index]
            } else {
                mask[mask_index * 4 + 3]
            };
            if coverage == 0 && !PREMULTIPLIED {
                continue;
            }
            let image_x = (i64::from(x) + i64::from(mask_x)) as usize;
            if BINARY {
                bytes[row_start + image_x] = color.0;
            } else if LUMA {
                let offset = row_start + image_x * 2;
                bytes[offset] = crate::draw::pil_blend(color.0, bytes[offset], coverage);
                bytes[offset + 1] =
                    crate::draw::pil_blend(color.1, bytes[offset + 1], coverage);
            } else if PREMULTIPLIED {
                let offset = row_start + image_x;
                bytes[offset] = crate::compute::pool_cpu::helpers::bytes::preblend::apply(
                    color.0,
                    bytes[offset],
                    coverage,
                );
            } else {
                let offset = row_start + image_x;
                bytes[offset] = crate::draw::pil_blend(color.0, bytes[offset], coverage);
            }
        }
    }
}
