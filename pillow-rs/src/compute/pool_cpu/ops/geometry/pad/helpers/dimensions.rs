//! Pillow-compatible contain sizing and centering for ImageOps.pad.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::Mode;

/// Canvas facts shared by each exact-mode Pad strategy.
#[derive(Clone, Copy)]
pub(crate) struct PadFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) content_width: u32,
    pub(crate) content_height: u32,
    pub(crate) offset_x: u32,
    pub(crate) offset_y: u32,
    pub(crate) color: Option<(u8, u8, u8, u8)>,
}

/// Reproduces the archived CPU contain sizing and its validation order.
pub(crate) fn build(
    source: (u32, u32),
    target: (u32, u32),
    centering: (f64, f64),
    color: Option<(u8, u8, u8, u8)>,
) -> Result<PadFrame, PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    let image_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(width) / f64::from(height);
    let (content_width, content_height) = if (image_ratio - target_ratio).abs() < 1e-10 {
        (width, height)
    } else if image_ratio > target_ratio {
        (
            width,
            bankers_round(
                f64::from(source_height) / f64::from(source_width) * f64::from(width),
            ) as u32,
        )
    } else {
        (
            bankers_round(
                f64::from(source_width) / f64::from(source_height) * f64::from(height),
            ) as u32,
            height,
        )
    };
    validate_contained_size(source, (content_width, content_height))?;
    let (offset_x, offset_y) = if content_width != width {
        (
            bankers_round(
                f64::from(width - content_width) * centering.0.clamp(0.0, 1.0),
            ) as u32,
            0,
        )
    } else {
        (
            0,
            bankers_round(
                f64::from(height - content_height) * centering.1.clamp(0.0, 1.0),
            ) as u32,
        )
    };
    Ok(PadFrame {
        width,
        height,
        content_width,
        content_height,
        offset_x,
        offset_y,
        color,
    })
}

fn validate_contained_size(
    source: (u32, u32),
    content: (u32, u32),
) -> Result<(), PilError> {
    if source.0 != 0 && source.1 != 0 && (content.0 == 0 || content.1 == 0) {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    if source.0 == 0 && source.1 != 0 && content.1 != source.1 {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

fn bankers_round(value: f64) -> f64 {
    let floor = value.floor();
    let fraction = value - floor;
    if fraction == 0.5 {
        if floor % 2.0 == 0.0 { floor } else { floor + 1.0 }
    } else {
        (value + 0.5).floor()
    }
}

/// Returns the full-width content window used by native Pad resize kernels.
pub(crate) fn resize_window(
    canvas: CheckedDims,
    content: CheckedDims,
    offset_y: u32,
    mode: Mode,
) -> Result<std::ops::Range<usize>, PilError> {
    let mode_name = mode.pillow_name().unwrap_or("unknown");
    let offset_y = usize::try_from(offset_y)
        .map_err(|_| PilError::DimensionError(format!("{mode_name} pad offset overflow")))?;
    let start = offset_y
        .checked_mul(canvas.row_stride())
        .ok_or_else(|| PilError::DimensionError(format!("{mode_name} pad offset overflow")))?;
    start
        .checked_add(content.total_bytes())
        .filter(|end| *end <= canvas.total_bytes())
        .map(|end| start..end)
        .ok_or_else(|| PilError::DimensionError(format!("{mode_name} pad window overflow")))
}
