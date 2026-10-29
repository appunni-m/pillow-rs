//! Serial native-sample Expand kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, ImageBuffer, Pixel};

pub(super) fn expand_native_bytes(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    channels: usize,
    new_width: u32,
    new_height: u32,
) -> Result<DynamicImage, PilError> {
    let source_dims = CheckedDims::new_allow_empty(img.width(), img.height(), channels as u8)?;
    let output_dims = CheckedDims::new(new_width, new_height, channels as u8)?;
    let source = img.as_bytes();
    if source.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "Expand source bytes do not match the selected mode layout".into(),
        ));
    }
    if border == 0 {
        return Ok(img.clone());
    }

    let fill_pixel = match channels {
        1 => [fill.0, 0, 0, 0],
        2 => [fill.0, fill.3, 0, 0],
        3 => [fill.0, fill.1, fill.2, 0],
        4 => [fill.0, fill.1, fill.2, fill.3],
        _ => {
            return Err(PilError::InternalError(
                "Expand selected an invalid byte channel count".into(),
            ));
        }
    };
    let fill_pixel = &fill_pixel[..channels];
    let fill_row = fill_pixel.repeat(new_width as usize);
    let fill_sides = fill_pixel.repeat(border as usize);
    let source_stride = source_dims.row_stride();
    let source_height = img.height() as usize;
    let offset_y = border as usize;
    let mut output = Vec::with_capacity(output_dims.total_bytes());

    for y in 0..new_height as usize {
        if y < offset_y || y >= offset_y + source_height {
            output.extend_from_slice(&fill_row);
        } else {
            output.extend_from_slice(&fill_sides);
            let source_y = y - offset_y;
            let source_start = source_y * source_stride;
            output.extend_from_slice(&source[source_start..source_start + source_stride]);
            output.extend_from_slice(&fill_sides);
        }
    }

    crate::image_utils::raw_bytes_to_image(new_width, new_height, output, channels)
}

fn expand_native_pixels<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    border: u32,
    new_width: u32,
    new_height: u32,
    fill_pixel: &[P::Subpixel],
) -> Result<ImageBuffer<P, Vec<P::Subpixel>>, PilError>
where
    P: Pixel,
{
    let channels = P::CHANNEL_COUNT as usize;
    if fill_pixel.len() != channels {
        return Err(PilError::InternalError(
            "Expand fill sample count does not match the selected mode".into(),
        ));
    }
    let source_dims = CheckedDims::new_allow_empty(image.width(), image.height(), channels as u8)?;
    let output_dims = CheckedDims::new(new_width, new_height, channels as u8)?;
    let source = image.as_raw();
    if source.len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "Expand source samples do not match the selected mode layout".into(),
        ));
    }
    if border == 0 {
        return Ok(image.clone());
    }

    let fill_row = fill_pixel.repeat(new_width as usize);
    let fill_sides = fill_pixel.repeat(border as usize);
    let source_stride = source_dims.row_stride();
    let source_height = image.height() as usize;
    let offset_y = border as usize;
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    for y in 0..new_height as usize {
        if y < offset_y || y >= offset_y + source_height {
            output.extend_from_slice(&fill_row);
        } else {
            output.extend_from_slice(&fill_sides);
            let source_y = y - offset_y;
            let source_start = source_y * source_stride;
            output.extend_from_slice(&source[source_start..source_start + source_stride]);
            output.extend_from_slice(&fill_sides);
        }
    }

    ImageBuffer::from_raw(new_width, new_height, output)
        .ok_or_else(|| PilError::InternalError("Expand native sample shape mismatch".into()))
}

pub(super) fn expand_u16(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    channels: usize,
    new_width: u32,
    new_height: u32,
) -> Result<DynamicImage, PilError> {
    let fill = [
        u16::from(fill.0),
        u16::from(fill.1),
        u16::from(fill.2),
        u16::from(fill.3),
    ];
    match img {
        DynamicImage::ImageLuma16(image) if channels == 1 => expand_native_pixels(
            image,
            border,
            new_width,
            new_height,
            &fill[..1],
        )
        .map(DynamicImage::ImageLuma16),
        // NO_PILLOW_SUPPORT: These native channel layouts extend Expand to
        // internal modes that Pillow does not expose.
        DynamicImage::ImageLumaA16(image) if channels == 2 => expand_native_pixels(
            image,
            border,
            new_width,
            new_height,
            &[fill[0], fill[3]],
        )
        .map(DynamicImage::ImageLumaA16),
        DynamicImage::ImageRgb16(image) if channels == 3 => expand_native_pixels(
            image,
            border,
            new_width,
            new_height,
            &fill[..3],
        )
        .map(DynamicImage::ImageRgb16),
        DynamicImage::ImageRgba16(image) if channels == 4 => expand_native_pixels(
            image,
            border,
            new_width,
            new_height,
            &fill,
        )
        .map(DynamicImage::ImageRgba16),
        _ => Err(PilError::InternalError(
            "Expand u16 storage changed after strategy selection".into(),
        )),
    }
}

pub(super) fn expand_f32(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    channels: usize,
    new_width: u32,
    new_height: u32,
) -> Result<DynamicImage, PilError> {
    // NO_PILLOW_SUPPORT: Native f32 Expand is an image-slash-star extension.
    let fill = [
        f32::from(fill.0),
        f32::from(fill.1),
        f32::from(fill.2),
        f32::from(fill.3),
    ];
    match img {
        DynamicImage::ImageRgb32F(image) if channels == 3 => expand_native_pixels(
            image,
            border,
            new_width,
            new_height,
            &fill[..3],
        )
        .map(DynamicImage::ImageRgb32F),
        DynamicImage::ImageRgba32F(image) if channels == 4 => expand_native_pixels(
            image,
            border,
            new_width,
            new_height,
            &fill,
        )
        .map(DynamicImage::ImageRgba32F),
        _ => Err(PilError::InternalError(
            "Expand f32 storage changed after strategy selection".into(),
        )),
    }
}

pub(super) fn expanded_dimensions(img: &DynamicImage, border: u32) -> Result<(u32, u32), PilError> {
    let border_twice = border
        .checked_mul(2)
        .ok_or_else(|| PilError::DimensionError("expand border overflows dimensions".into()))?;
    let width = img
        .width()
        .checked_add(border_twice)
        .ok_or_else(|| PilError::DimensionError("expanded image width overflows".into()))?;
    let height = img
        .height()
        .checked_add(border_twice)
        .ok_or_else(|| PilError::DimensionError("expanded image height overflows".into()))?;
    Ok((width, height))
}
