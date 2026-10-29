//! Native-format flip, rotation, transpose, and offset operations.

use crate::raster::buffer::{GrayAlphaImage, GrayImage, ImageBuffer, RgbImage, RgbaImage};
use crate::raster::color::Luma;
use crate::raster::traits::Pixel;

use super::DynamicImage;

fn flip_horizontal<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel(width.saturating_sub(1).saturating_sub(x), y)
    })
}

fn flip_vertical<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    let source = image.as_raw();
    if height == 0 {
        return image.clone();
    }
    let row_len = source.len() / height as usize;
    if row_len == 0 {
        return image.clone();
    }

    // A vertical flip changes only row order. Copy complete native sample
    // spans so packed RGB and typed 16-bit/float pixels never go through
    // per-pixel accessors or a color conversion. Growing from capacity also
    // avoids zero-initializing bytes that are immediately overwritten.
    let mut output = Vec::with_capacity(source.len());
    for row in source.chunks_exact(row_len).rev() {
        output.extend_from_slice(row);
    }
    ImageBuffer::from_raw(width, height, output)
        .expect("vertical flip preserves the source sample count")
}

fn rotate_clockwise<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    ImageBuffer::from_fn(height, width, |x, y| {
        *image.get_pixel(y, height.saturating_sub(1).saturating_sub(x))
    })
}

fn rotate_half_turn<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel(
            width.saturating_sub(1).saturating_sub(x),
            height.saturating_sub(1).saturating_sub(y),
        )
    })
}

fn rotate_counter_clockwise<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    ImageBuffer::from_fn(height, width, |x, y| {
        *image.get_pixel(width.saturating_sub(1).saturating_sub(y), x)
    })
}

fn transpose_diagonal<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    transverse: bool,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    ImageBuffer::from_fn(height, width, |x, y| {
        let (source_x, source_y) = if transverse {
            (
                width.saturating_sub(1).saturating_sub(y),
                height.saturating_sub(1).saturating_sub(x),
            )
        } else {
            (y, x)
        };
        *image.get_pixel(source_x, source_y)
    })
}

fn offset_image<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
    xoffset: i32,
    yoffset: i32,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return ImageBuffer::new(width, height);
    }

    let source_x = (-(i64::from(xoffset))).rem_euclid(i64::from(width)) as u32;
    let source_y = (-(i64::from(yoffset))).rem_euclid(i64::from(height)) as u32;
    ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel((x + source_x) % width, (y + source_y) % height)
    })
}

fn offset_luma16(
    image: &ImageBuffer<Luma<u16>, Vec<u16>>,
    xoffset: i32,
    yoffset: i32,
    big_endian: bool,
) -> ImageBuffer<Luma<u16>, Vec<u16>> {
    let (width, height) = image.dimensions();
    if width == 0 || height == 0 {
        return ImageBuffer::new(width, height);
    }

    let mut source = Vec::with_capacity(image.as_raw().len() * 2);
    for &sample in image.as_raw() {
        let bytes = if big_endian {
            sample.to_be_bytes()
        } else {
            sample.to_ne_bytes()
        };
        source.extend_from_slice(&bytes);
    }

    let row_bytes = width as usize * 2;
    let xshift = (-(i64::from(xoffset))).rem_euclid(i64::from(width)) as usize;
    let yshift = (-(i64::from(yoffset))).rem_euclid(i64::from(height)) as usize;
    let mut destination = vec![0_u8; source.len()];
    for y in 0..height as usize {
        let source_y = (y + yshift) % height as usize;
        let source_row = source_y * row_bytes;
        let destination_row = y * row_bytes;
        for x in 0..width as usize {
            let source_x = (x + xshift) % width as usize;
            destination[destination_row + x] = source[source_row + source_x];
        }
    }

    let pixels: Vec<u16> = destination
        .chunks_exact(2)
        .map(|bytes| {
            let bytes = [bytes[0], bytes[1]];
            if big_endian {
                u16::from_be_bytes(bytes)
            } else {
                u16::from_ne_bytes(bytes)
            }
        })
        .collect();
    ImageBuffer::from_vec(width, height, pixels).expect("offset preserves the source dimensions")
}

macro_rules! map_mode_preserving {
    ($image:expr, $operation:ident $(, $argument:expr)* $(,)?) => {
        match $image {
            DynamicImage::One(image) => DynamicImage::One($operation(image $(, $argument)*)),
            DynamicImage::ImageLuma8(image) => DynamicImage::ImageLuma8($operation(image $(, $argument)*)),
            DynamicImage::ImageLumaA8(image) => DynamicImage::ImageLumaA8($operation(image $(, $argument)*)),
            DynamicImage::ImageLaPremultiplied8(image) => DynamicImage::ImageLaPremultiplied8($operation(image $(, $argument)*)),
            DynamicImage::ImagePalette8(image) => DynamicImage::ImagePalette8($operation(image $(, $argument)*)),
            DynamicImage::ImagePaletteAlpha8(image) => DynamicImage::ImagePaletteAlpha8($operation(image $(, $argument)*)),
            DynamicImage::ImageRgb8(image) => DynamicImage::ImageRgb8($operation(image $(, $argument)*)),
            DynamicImage::ImageRgba8(image) => DynamicImage::ImageRgba8($operation(image $(, $argument)*)),
            DynamicImage::ImageRgbPremultiplied8(image) => DynamicImage::ImageRgbPremultiplied8($operation(image $(, $argument)*)),
            DynamicImage::ImageRgbx8(image) => DynamicImage::ImageRgbx8($operation(image $(, $argument)*)),
            DynamicImage::ImageCmyk8(image) => DynamicImage::ImageCmyk8($operation(image $(, $argument)*)),
            DynamicImage::ImageYCbCr8(image) => DynamicImage::ImageYCbCr8($operation(image $(, $argument)*)),
            DynamicImage::ImageLab8(image) => DynamicImage::ImageLab8($operation(image $(, $argument)*)),
            DynamicImage::ImageHsv8(image) => DynamicImage::ImageHsv8($operation(image $(, $argument)*)),
            DynamicImage::ImageI32(image) => DynamicImage::ImageI32($operation(image $(, $argument)*)),
            DynamicImage::ImageF32(image) => DynamicImage::ImageF32($operation(image $(, $argument)*)),
            DynamicImage::ImageI16(image) => DynamicImage::ImageI16($operation(image $(, $argument)*)),
            DynamicImage::ImageI16L(image) => DynamicImage::ImageI16L($operation(image $(, $argument)*)),
            DynamicImage::ImageI16B(image) => DynamicImage::ImageI16B($operation(image $(, $argument)*)),
            DynamicImage::ImageI16N(image) => DynamicImage::ImageI16N($operation(image $(, $argument)*)),
            DynamicImage::ImageLumaA16(image) => DynamicImage::ImageLumaA16($operation(image $(, $argument)*)),
            DynamicImage::ImageRgb16(image) => DynamicImage::ImageRgb16($operation(image $(, $argument)*)),
            DynamicImage::ImageRgba16(image) => DynamicImage::ImageRgba16($operation(image $(, $argument)*)),
            DynamicImage::ImageRgb32F(image) => DynamicImage::ImageRgb32F($operation(image $(, $argument)*)),
            DynamicImage::ImageRgba32F(image) => DynamicImage::ImageRgba32F($operation(image $(, $argument)*)),
        }
    };
}

impl DynamicImage {
    /// Matches Pillow's `libImaging/Offset.c` dispatch for 16-bit luma.
    ///
    /// Pillow exposes I;16 images through the byte-oriented `image8` storage
    /// branch of this operation, so each coordinate indexes one byte and the
    /// unwritten half of every output pixel remains zero. Other formats use
    /// normal pixel-coordinate wrapping.
    pub(crate) fn offset_with_mode(
        &self,
        xoffset: i32,
        yoffset: i32,
    ) -> DynamicImage {
        match self {
            DynamicImage::ImageI16(image) => DynamicImage::ImageI16(offset_luma16(
                image, xoffset, yoffset, false,
            )),
            DynamicImage::ImageI16L(image) => DynamicImage::ImageI16L(offset_luma16(
                image, xoffset, yoffset, false,
            )),
            DynamicImage::ImageI16B(image) => DynamicImage::ImageI16B(offset_luma16(
                image, xoffset, yoffset, true,
            )),
            DynamicImage::ImageI16N(image) => DynamicImage::ImageI16N(offset_luma16(
                image, xoffset, yoffset, false,
            )),
            image => map_mode_preserving!(image, offset_image, xoffset, yoffset),
        }
    }

    /// Flip the image horizontally (mirror).
    #[must_use]
    pub fn fliph(&self) -> DynamicImage {
        map_mode_preserving!(self, flip_horizontal)
    }

    /// Flip the image vertically.
    #[must_use]
    pub fn flipv(&self) -> DynamicImage {
        map_mode_preserving!(self, flip_vertical)
    }

    /// Rotate the image 90 degrees clockwise.
    #[must_use]
    pub fn rotate90(&self) -> DynamicImage {
        map_mode_preserving!(self, rotate_clockwise)
    }

    /// Rotate the image 180 degrees.
    #[must_use]
    pub fn rotate180(&self) -> DynamicImage {
        map_mode_preserving!(self, rotate_half_turn)
    }

    /// Rotate the image 270 degrees clockwise (90 degrees counter-clockwise).
    #[must_use]
    pub fn rotate270(&self) -> DynamicImage {
        map_mode_preserving!(self, rotate_counter_clockwise)
    }

    /// Apply a diagonal transpose while retaining the image's native pixel type.
    pub(crate) fn transpose_diagonal(&self, transverse: bool) -> DynamicImage {
        map_mode_preserving!(self, transpose_diagonal, transverse)
    }
}
