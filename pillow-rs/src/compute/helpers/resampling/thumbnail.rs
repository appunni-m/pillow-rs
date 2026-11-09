//! Reducing-gap facts shared by Thumbnail preparation and support guards.

use crate::error::PilError;
use crate::pipeline::ResampleFilter;

pub(crate) fn reduce_factors(source: (u32, u32), output: (u32, u32), enabled: bool) -> (u32, u32) {
    if !enabled || source.0 == 0 || source.1 == 0 || output.0 == 0 || output.1 == 0 {
        return (1, 1);
    }
    let factor_x = ((f64::from(source.0) / f64::from(output.0) / 2.0) as u32).max(1);
    let factor_y = ((f64::from(source.1) / f64::from(output.1) / 2.0) as u32).max(1);
    (factor_x, factor_y)
}

pub(crate) fn reduce_factors_with_gap(
    source: (u32, u32),
    output: (u32, u32),
    reducing_gap: Option<f64>,
) -> Result<(u32, u32), PilError> {
    let Some(gap) = reducing_gap else {
        return Ok((1, 1));
    };
    if source.0 == 0 || source.1 == 0 || output.0 == 0 || output.1 == 0 {
        return Ok((1, 1));
    }
    let factor = |extent: u32, requested: u32| -> Result<u32, PilError> {
        let value = f64::from(extent) / f64::from(requested) / gap;
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
    Ok((factor(source.0, output.0)?, factor(source.1, output.1)?))
}

pub(crate) fn uses_even_two_by_two_reduction(
    source: (u32, u32),
    output: (u32, u32),
    filter: ResampleFilter,
) -> bool {
    !matches!(filter, ResampleFilter::Nearest)
        && source.0 > 0
        && source.1 > 0
        && source.0 % 2 == 0
        && source.1 % 2 == 0
        && reduce_factors(source, output, true) == (2, 2)
}
