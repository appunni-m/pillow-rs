//! ImageChops — channel operations (arithmetic, logical, blending).
//! All functions take images and return a new combined image via PipelineOp.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Add two images. Result = image1 + image2, scaled and offset.
pub fn add(image1: &Image, image2: &Image, scale: f64, offset: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Add {
            other: Arc::new(image2.clone()),
            scale,
            offset,
        },
    ))
}

/// Subtract image2 from image1.
pub fn subtract(
    image1: &Image,
    image2: &Image,
    scale: f64,
    offset: f64,
) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Subtract {
            other: Arc::new(image2.clone()),
            scale,
            offset,
        },
    ))
}

/// Multiply two images.
pub fn multiply(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Multiply {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Screen blend mode (PIL uses integer division).
pub fn screen(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Screen {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Return the darker pixel at each position.
pub fn darker(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Darker {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Return the lighter pixel at each position.
pub fn lighter(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Lighter {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Absolute difference between two images.
pub fn difference(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Difference {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Overlay blend mode.
pub fn overlay(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::Overlay {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Soft light blend mode.
pub fn soft_light(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::SoftLight {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Hard light blend mode.
pub fn hard_light(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::HardLight {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Bitwise AND.
pub fn logical_and(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalAnd {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Bitwise OR.
pub fn logical_or(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalOr {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Bitwise XOR.
pub fn logical_xor(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::LogicalXor {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Duplicate an image.
pub fn duplicate(image: &Image) -> Image {
    image.copy()
}

/// Invert an image (same as ImageOps.invert).
pub fn invert(image: &Image) -> Result<Image, PilError> {
    crate::ops::imageops::invert(image)
}

/// Offset image contents.
pub fn offset(image: &Image, xoffset: i32, yoffset: i32) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::Offset {
            x: xoffset,
            y: yoffset,
        },
    ))
}

/// Modulo addition (wrap-around).
pub fn add_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::AddModulo {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Modulo subtraction.
pub fn subtract_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image1,
        PipelineOp::SubtractModulo {
            other: Arc::new(image2.clone()),
        },
    ))
}

/// Fill with constant value (single-channel fill).
pub fn constant(image: &Image, value: u8) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::Constant { value }))
}
