use crate::compute::pool_cpu::ops::draw::BitmapRegion;
use crate::error::PilError;
use crate::raster::DynamicImage;
use image::GenericImageView;

pub(crate) fn apply<const RGBA_MASK: bool>(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    mask: &[u8],
    ink: u8,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImagePalette8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = canvas.dimensions();
    // ImagingFill2 clips against the destination and advances the mask origin
    // when x or y is negative; clipping the source dimensions first loses
    // covered pixels when the mask begins before the image.
    let region = BitmapRegion::clipped(x, y, image_width, image_height, width, height);
    let samples = canvas.as_mut();

    for mask_y in region.y_start..region.y_end {
        let image_y = (i64::from(y) + i64::from(mask_y)) as usize;
        let mask_row = mask_y as usize * width as usize;
        let image_row = image_y * image_width as usize;
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
            samples[image_row + image_x] = ink;
        }
    }

    Ok(image)
}
