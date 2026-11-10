use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::ResampleFilter;
use crate::raster::Mode;

pub(crate) type ResizeBounds = (f64, f64, f64, f64);
type ReducingGapResult = Result<(Image, ResizeBounds), PilError>;

#[derive(Clone, Copy)]
pub(crate) struct ReducingGapPlan {
    pub(crate) x_factor: u32,
    pub(crate) y_factor: u32,
    pub(crate) safe_box: (u32, u32, u32, u32),
    pub(crate) resized_bounds: ResizeBounds,
}

pub(crate) fn factor(extent: f64, output: u32, gap: f64) -> Result<u32, PilError> {
    let value = extent / f64::from(output) / gap;
    if value.is_nan() {
        return Err(PilError::ValueError(
            "cannot convert float NaN to integer".into(),
        ));
    }
    if value.is_infinite() {
        return Err(PilError::OverflowError(
            "cannot convert float infinity to integer".into(),
        ));
    }
    Ok((value as u32).max(1))
}

pub(crate) fn prepare(
    bounds: ResizeBounds,
    gap: f64,
    filter: ResampleFilter,
    output_size: (u32, u32),
    source_size: (u32, u32),
) -> Result<Option<ReducingGapPlan>, PilError> {
    let x_factor = factor(bounds.2 - bounds.0, output_size.0, gap)?;
    let y_factor = factor(bounds.3 - bounds.1, output_size.1, gap)?;
    if x_factor <= 1 && y_factor <= 1 {
        return Ok(None);
    }
    super::validate_resize_box(bounds, source_size)?;
    let (_, support) = crate::ops::pil_resize::filter_from_resample(filter);
    let support_x = (support - 0.5) * (bounds.2 - bounds.0) / f64::from(output_size.0);
    let support_y = (support - 0.5) * (bounds.3 - bounds.1) / f64::from(output_size.1);
    let safe_box = (
        (bounds.0 - support_x).trunc().max(0.0) as u32,
        (bounds.1 - support_y).trunc().max(0.0) as u32,
        (bounds.2 + support_x).ceil().min(f64::from(source_size.0)) as u32,
        (bounds.3 + support_y).ceil().min(f64::from(source_size.1)) as u32,
    );
    let resized_bounds = (
        (bounds.0 - f64::from(safe_box.0)) / f64::from(x_factor),
        (bounds.1 - f64::from(safe_box.1)) / f64::from(y_factor),
        (bounds.2 - f64::from(safe_box.0)) / f64::from(x_factor),
        (bounds.3 - f64::from(safe_box.1)) / f64::from(y_factor),
    );
    Ok(Some(ReducingGapPlan {
        x_factor,
        y_factor,
        safe_box,
        resized_bounds,
    }))
}

pub(super) fn apply_reducing_gap(
    mut source: Image,
    bounds: ResizeBounds,
    gap: f64,
    filter: ResampleFilter,
    mode: Mode,
    output_size: (u32, u32),
    source_size: (u32, u32),
) -> ReducingGapResult {
    let factor_x = factor(bounds.2 - bounds.0, output_size.0, gap)?;
    let factor_y = factor(bounds.3 - bounds.1, output_size.1, gap)?;
    if factor_x <= 1 && factor_y <= 1 {
        return Ok((source, bounds));
    }
    // Pillow's integer reducer does not accept 16-bit modes, although its
    // ordinary filtered resampler does.
    if matches!(
        mode,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
    ) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let Some(plan) = prepare(bounds, gap, filter, output_size, source_size)? else {
        return Ok((source, bounds));
    };
    let safe = plan.safe_box;
    if safe != (0, 0, source_size.0, source_size.1) {
        source = source.crop(Some((
            safe.0 as i32,
            safe.1 as i32,
            safe.2 as i32,
            safe.3 as i32,
        )))?;
    }
    source = source.reduce(plan.x_factor, plan.y_factor)?;
    Ok((source, plan.resized_bounds))
}
