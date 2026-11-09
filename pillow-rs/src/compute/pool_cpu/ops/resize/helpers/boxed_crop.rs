//! Exact integer crop and identity shortcuts for boxed Resize.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

#[derive(Clone, Copy)]
struct ExactCrop {
    left: u32,
    top: u32,
    full_source: bool,
}

pub(crate) fn narrow_valid_bounds(
    source_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> Option<(f64, f64, f64, f64)> {
    let bounds = validate_bounds(bounds, source_size).ok()?;
    if bounds.0.is_finite() && bounds.1.is_finite() && bounds.2.is_finite() && bounds.3.is_finite()
    {
        Some(bounds)
    } else {
        None
    }
}

/// Narrows and validates source-box coordinates at Pillow's float32 boundary.
pub(crate) fn validate_bounds(
    bounds: (f64, f64, f64, f64),
    source_size: (u32, u32),
) -> Result<(f64, f64, f64, f64), PilError> {
    let bounds = (
        f64::from(bounds.0 as f32),
        f64::from(bounds.1 as f32),
        f64::from(bounds.2 as f32),
        f64::from(bounds.3 as f32),
    );
    if bounds.0 < 0.0 || bounds.1 < 0.0 {
        return Err(PilError::ValueError("box offset can't be negative".into()));
    }
    if bounds.2 > f64::from(source_size.0 as f32) || bounds.3 > f64::from(source_size.1 as f32) {
        return Err(PilError::ValueError(
            "box can't exceed original image size".into(),
        ));
    }
    if bounds.2 < bounds.0 || bounds.3 < bounds.1 {
        return Err(PilError::ValueError("box can't be empty".into()));
    }
    Ok(bounds)
}

fn exact_crop(
    source_size: (u32, u32),
    destination_size: (u32, u32),
    bounds: (f64, f64, f64, f64),
) -> Option<ExactCrop> {
    // Pillow narrows box bounds to f32 before deciding whether `_resize` can
    // return a copy or crop. Preserve that boundary; filtering an exact F32
    // crop would let NaN/Inf samples leak across taps Pillow never evaluates.
    let (left, top, right, bottom) = narrow_valid_bounds(source_size, bounds)?;
    let (left, top, right, bottom) = (left as f32, top as f32, right as f32, bottom as f32);
    if left.fract() != 0.0
        || top.fract() != 0.0
        || right - left != destination_size.0 as f32
        || bottom - top != destination_size.1 as f32
    {
        return None;
    }
    Some(ExactCrop {
        left: left as u32,
        top: top as u32,
        full_source: source_size == destination_size
            && left == 0.0
            && top == 0.0
            && right == source_size.0 as f32
            && bottom == source_size.1 as f32,
    })
}

pub(crate) fn uses_default_shortcut(
    source_size: (u32, u32),
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
    straight_alpha_mode: bool,
) -> bool {
    let Some(crop) = exact_crop(source_size, destination_size, bounds) else {
        return false;
    };
    crop.full_source || !(straight_alpha_mode && !matches!(filter, ResampleFilter::Nearest))
}

pub(crate) fn execute_default(
    image: &DynamicImage,
    destination_size: (u32, u32),
    filter: ResampleFilter,
    bounds: (f64, f64, f64, f64),
) -> Option<DynamicImage> {
    let crop = exact_crop((image.width(), image.height()), destination_size, bounds)?;
    if crop.full_source {
        return Some(image.clone());
    }
    // Pillow's recursive alpha path still filters straight LA/RGBA crops so
    // premultiplication and unpremultiplication retain their rounding order.
    if !matches!(filter, ResampleFilter::Nearest)
        && matches!(image.mode(), Mode::ImageLumaA8 | Mode::ImageRgba8)
    {
        return None;
    }
    Some(image.crop_imm(crop.left, crop.top, destination_size.0, destination_size.1))
}
