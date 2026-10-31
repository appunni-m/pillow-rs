use crate::draw::pil_blend;
use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use image::GenericImageView;

enum NativeMask<'a> {
    One(&'a [u8]),
    Luma(&'a [u8]),
    Rgba(&'a [u8]),
    Opaque,
}

impl<'a> NativeMask<'a> {
    fn from_image(mode: Mode, image: &'a DynamicImage) -> Option<Self> {
        match (mode, image) {
            (Mode::One, DynamicImage::One(image)) => Some(Self::One(image.as_raw())),
            (Mode::ImageLuma8, DynamicImage::ImageLuma8(image)) => {
                Some(Self::Luma(image.as_raw()))
            }
            (Mode::ImageRgba8, DynamicImage::ImageRgba8(image)) => {
                Some(Self::Rgba(image.as_raw()))
            }
            (Mode::ImageRgbPremultiplied8, DynamicImage::ImageRgbPremultiplied8(_)) => {
                Some(Self::Opaque)
            }
            _ => None,
        }
    }

    #[inline]
    fn coverage(&self, pixel_index: usize) -> u8 {
        match self {
            Self::One(bytes) => u8::from(bytes[pixel_index] > 0) * 255,
            Self::Luma(bytes) => bytes[pixel_index],
            Self::Rgba(bytes) => bytes[pixel_index * 4 + 3],
            Self::Opaque => 255,
        }
    }
}

pub(crate) fn apply(
    image: &DynamicImage,
    mask_image: &DynamicImage,
    mask_mode: Mode,
    mask_width: u32,
    mask_height: u32,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
) -> Option<DynamicImage> {
    let DynamicImage::ImageRgb8(source) = image else {
        return None;
    };
    let mask = NativeMask::from_image(mask_mode, mask_image)?;
    let (image_width, image_height) = source.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let mut output = source.clone();
    draw_mask(
        &mut output,
        mask_width,
        region,
        x,
        y,
        color,
        |pixel_index| mask.coverage(pixel_index),
    );
    Some(DynamicImage::ImageRgb8(output))
}

pub(crate) fn apply_raw(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgb8(mut output) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = output.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    draw_mask(
        &mut output,
        mask_width,
        region,
        x,
        y,
        color,
        |pixel_index| mask.coverage_at(pixel_index),
    );
    Ok(DynamicImage::ImageRgb8(output))
}

fn draw_mask<C>(
    image: &mut image::RgbImage,
    mask_width: u32,
    region: BitmapRegion,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    coverage_at: C,
)
where
    C: Fn(usize) -> u8,
{
    let row_stride = image.width() as usize * 3;
    let destination = image.as_mut();
    for source_y in region.y_start..region.y_end {
        let destination_row = (i64::from(y) + i64::from(source_y)) as usize * row_stride;
        for source_x in region.x_start..region.x_end {
            let pixel_index = source_y as usize * mask_width as usize + source_x as usize;
            let coverage = coverage_at(pixel_index);
            if coverage == 0 {
                continue;
            }
            blend_pixel(
                destination,
                destination_row,
                coverage,
                i64::from(x) + i64::from(source_x),
                color,
            );
        }
    }
}

#[inline]
fn blend_pixel(
    destination: &mut [u8],
    destination_row: usize,
    coverage: u8,
    x: i64,
    color: (u8, u8, u8, u8),
) {
    let offset = destination_row + x as usize * 3;
    if coverage == 255 {
        destination[offset] = color.0;
        destination[offset + 1] = color.1;
        destination[offset + 2] = color.2;
    } else {
        destination[offset] = pil_blend(color.0, destination[offset], coverage);
        destination[offset + 1] = pil_blend(color.1, destination[offset + 1], coverage);
        destination[offset + 2] = pil_blend(color.2, destination[offset + 2], coverage);
    }
}
