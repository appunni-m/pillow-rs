use crate::error::PilError;

// TODO(pillow-rs#3ptb): verify tie rounding, zero-sized inputs, and error
// ordering for Contain/Cover/Fit/Scale in the frozen CPU parity matrix.

pub(in crate::compute::pool_cpu::ops::geometry) fn contain_dimensions(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let size = crate::ops::imageops::contain_dimensions_from_sizes(source, target)?;
    crate::ops::imageops::validate_aspect_resize_dimensions(source, size)?;
    Ok(size)
}

pub(in crate::compute::pool_cpu::ops::geometry) fn cover_dimensions(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let size = crate::ops::imageops::cover_dimensions_from_sizes(source, target)?;
    crate::ops::imageops::validate_aspect_resize_dimensions(source, size)?;
    Ok(size)
}

pub(in crate::compute::pool_cpu::ops::geometry) fn fit_box(
    source: (u32, u32),
    target: (u32, u32),
    bleed: f64,
    centering: (f64, f64),
) -> Result<Option<(f64, f64, f64, f64)>, PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    if source_height == 0 {
        return Err(PilError::ZeroDivisionError("float division by zero".into()));
    }
    if height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    if source_width == 0 {
        if width == 0 && height == source_height && bleed == 0.0 {
            return Ok(None);
        }
        if width == 0 || height < source_height {
            return Err(PilError::ValueError("height and width must be > 0".into()));
        }
    } else if width == 0 {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(Some(crate::ops::imageops::fit_box_coordinates(
        source, target, bleed, centering,
    )))
}
