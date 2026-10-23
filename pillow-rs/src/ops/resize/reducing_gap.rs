use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::ResampleFilter;

pub(super) fn apply_reducing_gap(
    mut source: Image,
    mut bounds: (f64, f64, f64, f64),
    gap: f64,
    filter: ResampleFilter,
    mode: &str,
    output_size: (u32, u32),
    source_size: (u32, u32),
) -> Result<(Image, (f64, f64, f64, f64)), PilError> {
    let factor = |extent: f64, output: u32| -> Result<u32, PilError> {
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
    };
    let factor_x = factor(bounds.2 - bounds.0, output_size.0)?;
    let factor_y = factor(bounds.3 - bounds.1, output_size.1)?;
    if factor_x <= 1 && factor_y <= 1 {
        return Ok((source, bounds));
    }
    // Pillow's integer reducer does not accept 16-bit modes, although its
    // ordinary filtered resampler does.
    if mode.starts_with("I;16") {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    super::validate_resize_box(bounds, source_size)?;
    let (_, support) = crate::ops::pil_resize::filter_from_resample(filter);
    let support_x = (support - 0.5) * (bounds.2 - bounds.0) / f64::from(output_size.0);
    let support_y = (support - 0.5) * (bounds.3 - bounds.1) / f64::from(output_size.1);
    // Keep the filter halo when reducing an interior box. Pillow truncates
    // left/top and rounds right/bottom upward here.
    let safe = (
        (bounds.0 - support_x).trunc().max(0.0) as u32,
        (bounds.1 - support_y).trunc().max(0.0) as u32,
        (bounds.2 + support_x).ceil().min(f64::from(source_size.0)) as u32,
        (bounds.3 + support_y).ceil().min(f64::from(source_size.1)) as u32,
    );
    if safe != (0, 0, source_size.0, source_size.1) {
        source = source.crop(Some((
            safe.0 as i32,
            safe.1 as i32,
            safe.2 as i32,
            safe.3 as i32,
        )))?;
    }
    source = source.reduce(factor_x, factor_y)?;
    bounds = (
        (bounds.0 - f64::from(safe.0)) / f64::from(factor_x),
        (bounds.1 - f64::from(safe.1)) / f64::from(factor_y),
        (bounds.2 - f64::from(safe.0)) / f64::from(factor_x),
        (bounds.3 - f64::from(safe.1)) / f64::from(factor_y),
    );
    Ok((source, bounds))
}
