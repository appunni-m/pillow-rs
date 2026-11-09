//! Vertical-first boxed Resize composition for Pillow's very tall inputs.

// TODO(pillow-rs#3ptb): verify exact-mode child selections and output parity
// for tall boxed LA/RGBA across each CPU Architecture request.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

#[derive(Clone, Copy)]
enum StraightAlpha {
    Luma,
    Rgb,
}

/// Reports whether a boxed request needs Pillow's tall-image pass order.
pub(crate) fn required(
    image: &DynamicImage,
    output_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> bool {
    let source_width = image.width();
    let source_height = image.height();
    let (left, _, right, _) = bounds;
    let horizontal_work =
        output_size.0 != source_width || left != 0.0 || right != f64::from(source_width as f32);
    pillow_mode_uses_tall_pass_order(image.mode())
        && horizontal_work
        && u64::from(source_height) > u64::from(source_width) * 100
        && output_size.1 < source_height
}

pub(crate) fn execute(
    image: &DynamicImage,
    context: &ExecutionContext,
    output_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
) -> Result<Option<DynamicImage>, PilError> {
    if !required(image, output_size, bounds) {
        return Ok(None);
    }

    let (premultiplied, alpha) = premultiply_for_boxed_passes(image, filter);
    let source = premultiplied.as_ref().unwrap_or(image);
    let (_, top, _, bottom) = bounds;
    let vertical_bounds = (0.0, top, f64::from(source.width()), bottom);
    let vertical = resize_boxed_stage(
        source,
        context,
        (source.width(), output_size.1),
        filter,
        vertical_bounds,
    )?;

    let (left, _, right, _) = bounds;
    let horizontal_bounds = (left, 0.0, right, f64::from(output_size.1));
    let output = resize_boxed_stage(&vertical, context, output_size, filter, horizontal_bounds)?;
    let output = match alpha {
        Some(alpha) => unpremultiply_for_boxed_passes(output, alpha)?,
        None => output,
    };
    Ok(Some(output))
}

fn resize_boxed_stage(
    image: &DynamicImage,
    context: &ExecutionContext,
    output_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
) -> Result<DynamicImage, PilError> {
    let input = OperationInputs {
        image,
        palette: PaletteInput::default(),
    };
    super::super::ops::prepare_direct(
        input,
        output_size.0,
        output_size.1,
        filter,
        Some(bounds),
        context,
    )?
    .execute()
}

fn premultiply_for_boxed_passes(
    image: &DynamicImage,
    filter: ResampleFilter,
) -> (Option<DynamicImage>, Option<StraightAlpha>) {
    if matches!(filter, ResampleFilter::Nearest) {
        return (None, None);
    }
    match image {
        DynamicImage::ImageLumaA8(input) => {
            let mut output = input.clone();
            for pixel in output.pixels_mut() {
                pixel[0] = premultiply_sample(pixel[0], pixel[1]);
            }
            (
                Some(DynamicImage::ImageLaPremultiplied8(output)),
                Some(StraightAlpha::Luma),
            )
        }
        DynamicImage::ImageRgba8(input) => {
            let mut output = input.clone();
            for pixel in output.pixels_mut() {
                for channel in 0..3 {
                    pixel[channel] = premultiply_sample(pixel[channel], pixel[3]);
                }
            }
            (
                Some(DynamicImage::ImageRgbPremultiplied8(output)),
                Some(StraightAlpha::Rgb),
            )
        }
        _ => (None, None),
    }
}

fn premultiply_sample(sample: u8, alpha: u8) -> u8 {
    ((u16::from(sample) * u16::from(alpha) + 127) / 255) as u8
}

fn unpremultiply_for_boxed_passes(
    image: DynamicImage,
    alpha: StraightAlpha,
) -> Result<DynamicImage, PilError> {
    match (alpha, image) {
        (StraightAlpha::Luma, DynamicImage::ImageLaPremultiplied8(mut image)) => {
            for pixel in image.pixels_mut() {
                pixel[0] = unpremultiply_sample(pixel[0], pixel[1]);
            }
            Ok(DynamicImage::ImageLumaA8(image))
        }
        (StraightAlpha::Rgb, DynamicImage::ImageRgbPremultiplied8(mut image)) => {
            for pixel in image.pixels_mut() {
                for channel in 0..3 {
                    pixel[channel] = unpremultiply_sample(pixel[channel], pixel[3]);
                }
            }
            Ok(DynamicImage::ImageRgba8(image))
        }
        _ => Err(PilError::InternalError(
            "tall boxed Resize changed its premultiplied alpha mode".into(),
        )),
    }
}

fn unpremultiply_sample(sample: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        sample
    } else {
        (u16::from(sample) * 255 / u16::from(alpha)).min(255) as u8
    }
}

fn pillow_mode_uses_tall_pass_order(mode: Mode) -> bool {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => true,
        Mode::ImageF32
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => false,
    }
}
