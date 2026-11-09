//! Pillow-compatible contain sizing and centering for ImageOps.pad.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::PadColorInput;
use crate::raster::Mode;

/// Canvas facts shared by each exact-mode Pad strategy.
#[derive(Clone, Copy)]
pub(crate) struct PadFrame {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) content_width: u32,
    pub(crate) content_height: u32,
    pub(crate) offset_x: i64,
    pub(crate) offset_y: i64,
    /// Byte-normalized fill used by the existing direct Pad route.
    pub(crate) color: Option<(u8, u8, u8, u8)>,
    /// Original typed input for native-width mode owners.
    pub(crate) native_color: Option<PadColorInput>,
}

/// Row counts for a full-width Pad whose content is already in final shape.
#[derive(Clone, Copy)]
pub(crate) struct FullWidthVerticalRows {
    pub(crate) top: usize,
    pub(crate) bottom: usize,
}

/// A clipped source/destination interval for Pillow Paste's signed placement.
#[derive(Clone, Copy)]
pub(crate) struct ClippedAxis {
    pub(crate) source_start: usize,
    pub(crate) destination_start: usize,
    pub(crate) length: usize,
}

pub(crate) fn clipped_axis(
    offset: i64,
    source_extent: usize,
    destination_extent: usize,
) -> ClippedAxis {
    let (source_start, destination_start) = if offset < 0 {
        (
            usize::try_from(offset.unsigned_abs())
                .unwrap_or(usize::MAX)
                .min(source_extent),
            0,
        )
    } else {
        (
            0,
            usize::try_from(offset)
                .unwrap_or(usize::MAX)
                .min(destination_extent),
        )
    };
    let length = source_extent
        .saturating_sub(source_start)
        .min(destination_extent.saturating_sub(destination_start));
    ClippedAxis {
        source_start,
        destination_start,
        length,
    }
}

/// Returns the append-only path bounds when the content occupies the full
/// canvas width and has vertical borders only.
pub(crate) fn full_width_vertical_rows(
    source: (u32, u32),
    frame: PadFrame,
) -> Option<FullWidthVerticalRows> {
    if frame.width == 0
        || frame.height == 0
        || source.0 == 0
        || source.1 == 0
        || frame.offset_x != 0
        || frame.content_width != frame.width
        || source != (frame.content_width, frame.content_height)
    {
        return None;
    }
    let top = usize::try_from(frame.offset_y).ok()?;
    let source_height = usize::try_from(source.1).ok()?;
    let height = usize::try_from(frame.height).ok()?;
    let content_end = top.checked_add(source_height)?;
    if content_end > height {
        return None;
    }
    let bottom = height - content_end;
    (top != 0 || bottom != 0).then_some(FullWidthVerticalRows { top, bottom })
}

/// Builds Pillow-compatible contain dimensions and centering for Pad.
pub(crate) fn build(
    source: (u32, u32),
    target: (u32, u32),
    centering: (f64, f64),
    color: Option<(u8, u8, u8, u8)>,
) -> Result<PadFrame, PilError> {
    let (width, height) = target;
    // Pillow Pad delegates containment to ImageOps.contain's exact comparison.
    let (content_width, content_height) =
        crate::ops::imageops::contain_dimensions_from_sizes(source, target)?;
    crate::ops::imageops::validate_aspect_resize_dimensions(
        source,
        (content_width, content_height),
    )?;
    let (offset_x, offset_y) = if content_width != width {
        (
            crate::ops::imageops::round_positive_ties_even(
                f64::from(width - content_width) * centering.0.clamp(0.0, 1.0),
            ),
            0,
        )
    } else {
        (
            0,
            crate::ops::imageops::round_positive_ties_even(
                f64::from(height - content_height) * centering.1.clamp(0.0, 1.0),
            ),
        )
    };
    Ok(PadFrame {
        width,
        height,
        content_width,
        content_height,
        offset_x: i64::from(offset_x),
        offset_y: i64::from(offset_y),
        color,
        native_color: None,
    })
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
