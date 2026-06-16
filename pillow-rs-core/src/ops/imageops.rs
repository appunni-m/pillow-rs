//! ImageOps — high-level image operations (module-level functions).
//! Mirroring PIL.ImageOps: autocontrast, equalize, invert, flip, mirror,
//! posterize, solarize, expand, scale, contain, cover, fit, pad, grayscale.

use crate::error::PilError;
use crate::image::Image;
use crate::ops::resize::parse_resample;
use crate::pipeline::PipelineOp;

/// Normalize image contrast. Clips the darkest and lightest `cutoff` percent.
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Autocontrast { cutoff }))
}

/// Equalize the image histogram.
pub fn equalize(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Equalize))
}

/// Invert all pixel values (negative).
pub fn invert(image: &Image) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Invert))
}

/// Flip image vertically.
pub fn flip(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Flip))
}

/// Mirror image horizontally (same as FLIP_LEFT_RIGHT).
pub fn mirror(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Mirror))
}

/// Reduce number of bits per color channel.
pub fn posterize(image: &Image, bits: u8) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(
        image,
        PipelineOp::Posterize {
            bits: bits.clamp(1, 8),
        },
    ))
}

/// Invert all pixel values above threshold.
pub fn solarize(image: &Image, threshold: u8) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode == "LA" || mode == "RGBA" {
        return Err(PilError::OsError(format!("not supported for mode {mode}")));
    }
    Ok(Image::push_op(image, PipelineOp::Solarize { threshold }))
}

/// Convert to grayscale using PIL-compatible BT.601 formula.
pub fn grayscale(image: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Grayscale))
}

/// Colorize grayscale image using black/white color mapping.
pub fn colorize(
    image: &Image,
    black: (u8, u8, u8),
    white: (u8, u8, u8),
) -> Result<Image, PilError> {
    let mode = image.mode()?;
    if mode != "L" {
        // PIL raises AssertionError for non-L modes
        return Err(PilError::AssertionError(String::new()));
    }
    Ok(Image::push_op(image, PipelineOp::Colorize { black, white }))
}

/// Add a border around the image.
pub fn expand(image: &Image, border: u32, fill: (u8, u8, u8, u8)) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Expand { border, fill }))
}

/// Resize image to fit within (w, h) while preserving aspect ratio.
pub fn contain(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Contain { w, h, filter }))
}

/// Resize image to completely cover (w, h), cropping overflow.
pub fn cover(image: &Image, w: u32, h: u32, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Cover { w, h, filter }))
}

/// Resize and crop to fit within (w, h) with centering.
pub fn fit(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    bleed: f64,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Fit {
            w,
            h,
            filter,
            bleed,
            centering,
        },
    ))
}

/// Resize and pad to exactly (w, h) with optional color fill.
pub fn pad(
    image: &Image,
    w: u32,
    h: u32,
    filter: Option<&str>,
    color: Option<(u8, u8, u8, u8)>,
    centering: (f64, f64),
) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(
        image,
        PipelineOp::Pad {
            w,
            h,
            filter,
            color,
            centering,
        },
    ))
}

/// Scale image by a factor.
pub fn scale(image: &Image, factor: f64, filter: Option<&str>) -> Result<Image, PilError> {
    let filter = parse_resample(filter)?;
    Ok(Image::push_op(image, PipelineOp::Scale { factor, filter }))
}

/// Crop border pixels from the image.
pub fn crop(image: &Image, border: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::CropBorder { border }))
}
