use crate::compute::pool_cpu::ops::draw::BitmapRegion;
use crate::raster::{ImageBuffer, Luma};
use image::GenericImageView;

type Luma16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

/// Blend the font mask directly into the two stored bytes of each I;16 sample.
///
/// Pillow's `fill_mask_L` uses both ink bytes at the native sample stride.
/// `fill_mask_RGBA` instead steps through the raw row byte prefix at one byte
/// per mask pixel. Reading and writing the native carrier as bytes preserves
/// both without projecting samples to 8-bit or rewriting the full canvas.
pub(crate) fn apply<const RGBA_MASK: bool>(
    canvas: &mut Luma16Image,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &[u8],
    ink: (u8, u8, u8, u8),
) -> bool {
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return false;
    }
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    if region.x_start == region.x_end || region.y_start == region.y_end {
        return true;
    }

    let bytes: &mut [u8] = bytemuck::cast_slice_mut(canvas.as_mut());
    let row_bytes = image_width as usize * 2;
    for mask_y in region.y_start..region.y_end {
        let image_y = (i64::from(y) + i64::from(mask_y)) as usize;
        let image_row = image_y * row_bytes;
        let mask_row = mask_y as usize * mask_width as usize;
        for mask_x in region.x_start..region.x_end {
            let mask_index = mask_row + mask_x as usize;
            let coverage = if RGBA_MASK {
                mask[mask_index * 4 + 3]
            } else {
                mask[mask_index]
            };
            if coverage == 0 {
                continue;
            }

            let image_x = (i64::from(x) + i64::from(mask_x)) as usize;
            if RGBA_MASK {
                let output = image_row + image_x;
                bytes[output] = crate::draw::pil_blend(ink.0, bytes[output], coverage);
            } else {
                let output = image_row + image_x * 2;
                bytes[output] = crate::draw::pil_blend(ink.0, bytes[output], coverage);
                bytes[output + 1] = crate::draw::pil_blend(ink.1, bytes[output + 1], coverage);
            }
        }
    }

    true
}
