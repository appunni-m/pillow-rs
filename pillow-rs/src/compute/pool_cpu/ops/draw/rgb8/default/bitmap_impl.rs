use crate::draw::pil_blend;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::helpers::bytes::preblend;
use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};
use image::GenericImageView;

enum NativeMask<'a> {
    One(&'a [u8]),
    Luma(&'a [u8]),
    Rgba(&'a [u8]),
    RgbPremultiplied(&'a [u8]),
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
            (Mode::ImageRgbPremultiplied8, DynamicImage::ImageRgbPremultiplied8(image)) => {
                Some(Self::RgbPremultiplied(image.as_raw()))
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
            Self::RgbPremultiplied(bytes) => bytes[pixel_index * 4 + 3],
        }
    }

    #[inline]
    fn is_premultiplied(&self) -> bool {
        matches!(self, Self::RgbPremultiplied(_))
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
) -> Result<Option<DynamicImage>, PilError> {
    let DynamicImage::ImageRgb8(source) = image else {
        return Ok(None);
    };
    let Some(mask) = NativeMask::from_image(mask_mode, mask_image) else {
        return Ok(None);
    };
    let (image_width, image_height) = source.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let mut output = source.clone();
    let mut hidden_lanes = output.take_hidden_pixel_lanes();
    let coverage_at = |pixel_index| mask.coverage(pixel_index);
    if mask.is_premultiplied() {
        draw_mask::<true, _>(
            &mut output,
            mask_width,
            region,
            x,
            y,
            color,
            coverage_at,
            &mut hidden_lanes,
        )?;
    } else {
        draw_mask::<false, _>(
            &mut output,
            mask_width,
            region,
            x,
            y,
            color,
            coverage_at,
            &mut hidden_lanes,
        )?;
    }
    super::super::super::helpers::hidden_lanes::restore(&mut output, hidden_lanes)?;
    Ok(Some(DynamicImage::ImageRgb8(output)))
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
    let mut hidden_lanes = output.take_hidden_pixel_lanes();
    let (image_width, image_height) = output.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let coverage_at = |pixel_index| mask.coverage_at(pixel_index);
    if mask.is_premultiplied() {
        draw_mask::<true, _>(
            &mut output,
            mask_width,
            region,
            x,
            y,
            color,
            coverage_at,
            &mut hidden_lanes,
        )?;
    } else {
        draw_mask::<false, _>(
            &mut output,
            mask_width,
            region,
            x,
            y,
            color,
            coverage_at,
            &mut hidden_lanes,
        )?;
    }
    super::super::super::helpers::hidden_lanes::restore(&mut output, hidden_lanes)?;
    Ok(DynamicImage::ImageRgb8(output))
}

fn draw_mask<const PREMULTIPLIED: bool, C>(
    image: &mut RgbImage,
    mask_width: u32,
    region: BitmapRegion,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
    coverage_at: C,
    hidden_lanes: &mut Option<crate::raster::buffer::HiddenPixelLanes>,
)
 -> Result<(), PilError>
where
    C: Fn(usize) -> u8,
{
    let width = image.width();
    let row_stride = width as usize * 3;
    for source_y in region.y_start..region.y_end {
        let image_y = (i64::from(y) + i64::from(source_y)) as usize;
        let destination_row = image_y * row_stride;
        for source_x in region.x_start..region.x_end {
            let pixel_index = source_y as usize * mask_width as usize + source_x as usize;
            let coverage = coverage_at(pixel_index);
            if coverage == 0 && !PREMULTIPLIED {
                continue;
            }
            let image_x = (i64::from(x) + i64::from(source_x)) as usize;
            let destination_offset = destination_row + image_x * 3;
            let destination_index = image_y * width as usize + image_x;
            let previous_hidden = hidden_lanes_for_pixel(
                hidden_lanes,
                destination_index,
                1,
                [u8::MAX, 0],
            )?;
            {
                let destination = image.as_mut();
                blend_pixel::<PREMULTIPLIED>(
                    destination,
                    destination_offset,
                    coverage,
                    color,
                );
            }
            let lane = if coverage == 255 && !PREMULTIPLIED {
                color.3
            } else if PREMULTIPLIED {
                preblend::apply(color.3, previous_hidden[0], coverage)
            } else {
                pil_blend(color.3, previous_hidden[0], coverage)
            };
            update_hidden_lanes(
                image,
                hidden_lanes,
                destination_index,
                1,
                [lane, 0],
                [u8::MAX, 0],
                &|_| [u8::MAX, 0],
            )?;
        }
    }
    Ok(())
}

#[inline]
fn blend_pixel<const PREMULTIPLIED: bool>(
    destination: &mut [u8],
    destination_offset: usize,
    coverage: u8,
    color: (u8, u8, u8, u8),
) {
    if coverage == 255 && !PREMULTIPLIED {
        destination[destination_offset] = color.0;
        destination[destination_offset + 1] = color.1;
        destination[destination_offset + 2] = color.2;
    } else if PREMULTIPLIED {
        destination[destination_offset] =
            preblend::apply(color.0, destination[destination_offset], coverage);
        destination[destination_offset + 1] = preblend::apply(
            color.1,
            destination[destination_offset + 1],
            coverage,
        );
        destination[destination_offset + 2] = preblend::apply(
            color.2,
            destination[destination_offset + 2],
            coverage,
        );
    } else {
        destination[destination_offset] =
            pil_blend(color.0, destination[destination_offset], coverage);
        destination[destination_offset + 1] =
            pil_blend(color.1, destination[destination_offset + 1], coverage);
        destination[destination_offset + 2] =
            pil_blend(color.2, destination[destination_offset + 2], coverage);
    }
}
