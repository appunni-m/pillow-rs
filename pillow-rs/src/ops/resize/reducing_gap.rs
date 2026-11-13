use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::ResampleFilter;

/// Floating source bounds used by the final resize stage.
#[doc(hidden)]
pub type ResizeBounds = (f64, f64, f64, f64);
type ReducingGapResult = Result<(Image, ResizeBounds), PilError>;

/// Integer pre-reduction and translated bounds selected for a resize.
#[derive(Clone, Copy)]
#[doc(hidden)]
pub struct ReducingGapPlan {
    /// Horizontal integer reduction factor.
    pub x_factor: i32,
    /// Vertical integer reduction factor.
    pub y_factor: i32,
    /// Source rectangle used by the reduction stage.
    pub safe_box: (i32, i32, i32, i32),
    /// Resize rectangle translated into the reduced image's coordinates.
    pub resized_bounds: ResizeBounds,
}

/// Reduction arguments retained for the Python `Image.reduce` callback.
#[derive(Clone, Copy)]
#[doc(hidden)]
pub struct ReducingGapCallbackPlan {
    /// Horizontal integer reduction factor, without fixed-width narrowing.
    pub x_factor: f64,
    /// Vertical integer reduction factor, without fixed-width narrowing.
    pub y_factor: f64,
    /// Integer safe bounds represented without fixed-width narrowing.
    pub safe_box: (f64, f64, f64, f64),
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn factor_for_requested_size(
    extent: f64,
    output: i64,
    gap: f64,
) -> Result<f64, PilError> {
    if output == 0 {
        return Err(PilError::ZeroDivisionError("division by zero".into()));
    }
    let value = extent / output as f64 / gap;
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
    Ok(value.trunc())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn factor(extent: f64, output: u32, gap: f64) -> Result<f64, PilError> {
    callback_factor(extent, i64::from(output), gap)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn integer_as_c_int(value: f64) -> Result<i32, PilError> {
    if value < f64::from(i32::MIN) {
        return Err(PilError::OverflowError(
            "signed integer is less than minimum".into(),
        ));
    }
    if value > f64::from(i32::MAX) {
        return Err(PilError::OverflowError(
            "signed integer is greater than maximum".into(),
        ));
    }
    Ok(value as i32)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare(
    bounds: ResizeBounds,
    gap: f64,
    filter: ResampleFilter,
    output_size: (u32, u32),
    source_size: (u32, u32),
) -> Result<Option<ReducingGapPlan>, PilError> {
    let (x_factor, y_factor) = factors(bounds, output_size, gap)?;
    if x_factor <= 1.0 && y_factor <= 1.0 {
        return Ok(None);
    }
    // Pillow evaluates `_get_safe_box` before its reducer parses either the
    // factors or the resulting box as C integers. Preserve its float-to-int
    // faults and ordering before checking the reducer's argument widths.
    let safe_box = calculate_safe_box(
        bounds,
        filter,
        (i64::from(output_size.0), i64::from(output_size.1)),
        source_size,
    )?;
    // Pillow's public reducer checks each C-int factor before it validates the
    // safe box. Parse both factors and all coordinates before checking scale
    // positivity, as the native reducer does after argument conversion.
    let x_factor = integer_as_c_int(x_factor)?;
    let y_factor = integer_as_c_int(y_factor)?;
    let safe_box = (
        integer_as_c_int(safe_box.0)?,
        integer_as_c_int(safe_box.1)?,
        integer_as_c_int(safe_box.2)?,
        integer_as_c_int(safe_box.3)?,
    );
    if x_factor <= 0 || y_factor <= 0 {
        return Err(PilError::ValueError("scale must be > 0".into()));
    }
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn validate_reduction_before_invalid_output_size(
    source: &Image,
    bounds: ResizeBounds,
    gap: f64,
    filter: ResampleFilter,
    output_size: (i64, i64),
    source_size: (u32, u32),
) -> Result<(), PilError> {
    let Some(plan) = prepare_callback(bounds, gap, filter, output_size, source_size)? else {
        return Ok(());
    };
    let x_factor = integer_as_c_int(plan.x_factor)?;
    let y_factor = integer_as_c_int(plan.y_factor)?;
    let safe_box = (
        i64::from(integer_as_c_int(plan.safe_box.0)?),
        i64::from(integer_as_c_int(plan.safe_box.1)?),
        i64::from(integer_as_c_int(plan.safe_box.2)?),
        i64::from(integer_as_c_int(plan.safe_box.3)?),
    );
    // A reducing route runs before Pillow's final positive-size validation.
    // Admission errors therefore take precedence even though its result is
    // discarded when the requested resize dimensions are invalid.
    let _ = source.reduce_for_resize((i64::from(x_factor), i64::from(y_factor)), safe_box)?;
    Ok(())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn prepare_callback(
    bounds: ResizeBounds,
    gap: f64,
    filter: ResampleFilter,
    output_size: (i64, i64),
    source_size: (u32, u32),
) -> Result<Option<ReducingGapCallbackPlan>, PilError> {
    let x_factor = callback_factor(bounds.2 - bounds.0, output_size.0, gap)?;
    let y_factor = callback_factor(bounds.3 - bounds.1, output_size.1, gap)?;
    if x_factor <= 1.0 && y_factor <= 1.0 {
        return Ok(None);
    }
    // Pillow computes its safe box before the reducer validates that box.
    Ok(Some(ReducingGapCallbackPlan {
        x_factor,
        y_factor,
        safe_box: calculate_safe_box(bounds, filter, output_size, source_size)?,
    }))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn callback_factor(extent: f64, output: i64, gap: f64) -> Result<f64, PilError> {
    let value = factor_for_requested_size(extent, output, gap)?;
    Ok(if value == 0.0 { 1.0 } else { value })
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn factors(
    bounds: ResizeBounds,
    output_size: (u32, u32),
    gap: f64,
) -> Result<(f64, f64), PilError> {
    Ok((
        factor(bounds.2 - bounds.0, output_size.0, gap)?,
        factor(bounds.3 - bounds.1, output_size.1, gap)?,
    ))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn calculate_safe_box(
    bounds: ResizeBounds,
    filter: ResampleFilter,
    output_size: (i64, i64),
    source_size: (u32, u32),
) -> Result<(f64, f64, f64, f64), PilError> {
    let (_, support) = crate::ops::pil_resize::filter_from_resample(filter);
    // Match Pillow's `_get_safe_box` evaluation order: divide the source span
    // by the output extent before multiplying by the filter support.
    let scale_x = (bounds.2 - bounds.0) / output_size.0 as f64;
    let scale_y = (bounds.3 - bounds.1) / output_size.1 as f64;
    let support_x = (support - 0.5) * scale_x;
    let support_y = (support - 0.5) * scale_y;
    // Python evaluates int()/ceil() before max()/min(). Rust's float min/max
    // can absorb NaN or clamp infinities, which would hide those public faults.
    let left = python_truncated_integer(bounds.0 - support_x)?.max(0.0);
    let top = python_truncated_integer(bounds.1 - support_y)?.max(0.0);
    let right = python_ceiling_integer(bounds.2 + support_x)?.min(f64::from(source_size.0));
    let bottom = python_ceiling_integer(bounds.3 + support_y)?.min(f64::from(source_size.1));
    Ok((left, top, right, bottom))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn python_truncated_integer(value: f64) -> Result<f64, PilError> {
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
    Ok(value.trunc())
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn python_ceiling_integer(value: f64) -> Result<f64, PilError> {
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
    Ok(value.ceil())
}

// TODO(pillow-rs#3ptb): parity-check signed factors, zero/negative output sizes,
// typed I;16 reduction, non-finite safe-box intermediates, C-int bounds, and
// final-bound error precedence.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn apply_reducing_gap(
    source: Image,
    bounds: ResizeBounds,
    gap: f64,
    filter: ResampleFilter,
    output_size: (u32, u32),
    source_size: (u32, u32),
) -> ReducingGapResult {
    let Some(plan) = prepare(bounds, gap, filter, output_size, source_size)? else {
        return Ok((source, bounds));
    };
    let safe = plan.safe_box;
    // NO_PILLOW_SUPPORT: Pillow rejects I;16 Reduce; this path uses native u16.
    let source = source.reduce_for_resize(
        (i64::from(plan.x_factor), i64::from(plan.y_factor)),
        (
            i64::from(safe.0),
            i64::from(safe.1),
            i64::from(safe.2),
            i64::from(safe.3),
        ),
    )?;
    Ok((source, plan.resized_bounds))
}
