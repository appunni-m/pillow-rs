use crate::error::PilError;

// TODO(pillow-rs#3ptb): verify tie rounding, zero-sized inputs, and error
// ordering for Contain/Cover/Fit/Scale in the frozen CPU parity matrix.

pub(super) fn bankers_round(value: f64) -> f64 {
    let floor = value.floor();
    let fraction = value - floor;
    if fraction == 0.5 {
        if floor % 2.0 == 0.0 { floor } else { floor + 1.0 }
    } else {
        (value + 0.5).floor()
    }
}

pub(super) fn contain_dimensions(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    if source_height == 0 || height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(width) / f64::from(height);
    let size = if (image_ratio - target_ratio).abs() < 1e-10 {
        (width, height)
    } else if image_ratio > target_ratio {
        (
            width,
            bankers_round(f64::from(source_height) / f64::from(source_width)
                * f64::from(width)) as u32,
        )
    } else {
        (
            bankers_round(f64::from(source_width) / f64::from(source_height)
                * f64::from(height)) as u32,
            height,
        )
    };
    validate_contain_cover_size(source, size)?;
    Ok(size)
}

pub(super) fn cover_dimensions(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(u32, u32), PilError> {
    let (source_width, source_height) = source;
    let (width, height) = target;
    if source_height == 0 || height == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let image_ratio = f64::from(source_width) / f64::from(source_height);
    let target_ratio = f64::from(width) / f64::from(height);
    let size = if (image_ratio - target_ratio).abs() < 1e-10 {
        (width, height)
    } else if image_ratio < target_ratio {
        if source_width == 0 {
            return Err(PilError::ZeroDivisionError("division by zero".into()));
        }
        (
            width,
            bankers_round(f64::from(source_height) / f64::from(source_width)
                * f64::from(width)) as u32,
        )
    } else {
        (
            bankers_round(f64::from(source_width) / f64::from(source_height)
                * f64::from(height)) as u32,
            height,
        )
    };
    validate_contain_cover_size(source, size)?;
    Ok(size)
}

fn validate_contain_cover_size(
    source: (u32, u32),
    target: (u32, u32),
) -> Result<(), PilError> {
    let empty_width_copy = source.0 == 0 && target.0 == 0 && target.1 == source.1;
    if (target.0 == 0 || target.1 == 0) && !empty_width_copy {
        return Err(PilError::ValueError("height and width must be > 0".into()));
    }
    Ok(())
}

pub(super) fn fit_box(
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
