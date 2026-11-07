//! Native-format flip, rotation, and transpose operations.

use crate::raster::buffer::ImageBuffer;
use crate::raster::traits::Pixel;

use super::DynamicImage;

fn carry_hidden_lanes<P>(
    source: &ImageBuffer<P, Vec<P::Subpixel>>,
    output: &mut ImageBuffer<P, Vec<P::Subpixel>>,
    source_index: impl FnMut(usize) -> usize,
) where
    P: Pixel,
{
    output
        .remap_hidden_pixel_lanes_from(source, source_index)
        .expect("pixel-preserving transform maps valid source pixels");
}

fn flip_horizontal<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    let mut output = ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel(width.saturating_sub(1).saturating_sub(x), y)
    });
    let width = width as usize;
    carry_hidden_lanes(image, &mut output, |index| {
        let x = index % width;
        let y = index / width;
        y * width + width.saturating_sub(1).saturating_sub(x)
    });
    output
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
    let mut output = ImageBuffer::from_raw(width, height, output)
        .expect("vertical flip preserves the source sample count");
    let width = width as usize;
    let height = height as usize;
    carry_hidden_lanes(image, &mut output, |index| {
        let x = index % width;
        let y = index / width;
        height.saturating_sub(1).saturating_sub(y) * width + x
    });
    output
}

fn rotate_clockwise<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    let mut output = ImageBuffer::from_fn(height, width, |x, y| {
        *image.get_pixel(y, height.saturating_sub(1).saturating_sub(x))
    });
    let input_width = width as usize;
    let input_height = height as usize;
    carry_hidden_lanes(image, &mut output, |index| {
        let x = index % input_height;
        let y = index / input_height;
        input_height.saturating_sub(1).saturating_sub(x) * input_width + y
    });
    output
}

fn rotate_half_turn<P>(image: &ImageBuffer<P, Vec<P::Subpixel>>) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    let mut output = ImageBuffer::from_fn(width, height, |x, y| {
        *image.get_pixel(
            width.saturating_sub(1).saturating_sub(x),
            height.saturating_sub(1).saturating_sub(y),
        )
    });
    let pixel_count = (width as usize) * (height as usize);
    carry_hidden_lanes(image, &mut output, |index| {
        pixel_count.saturating_sub(1).saturating_sub(index)
    });
    output
}

fn rotate_counter_clockwise<P>(
    image: &ImageBuffer<P, Vec<P::Subpixel>>,
) -> ImageBuffer<P, Vec<P::Subpixel>>
where
    P: Pixel,
{
    let (width, height) = image.dimensions();
    let mut output = ImageBuffer::from_fn(height, width, |x, y| {
        *image.get_pixel(width.saturating_sub(1).saturating_sub(y), x)
    });
    let input_width = width as usize;
    let input_height = height as usize;
    carry_hidden_lanes(image, &mut output, |index| {
        let x = index % input_height;
        let y = index / input_height;
        x * input_width + input_width.saturating_sub(1).saturating_sub(y)
    });
    output
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

}
