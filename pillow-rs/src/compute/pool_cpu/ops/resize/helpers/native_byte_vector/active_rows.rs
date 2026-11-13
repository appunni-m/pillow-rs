//! Active-row guards shared by native-byte vector Resize kernels.

use crate::ops::pil_resize::FilterCoeffs;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn vertical_row_touches_source(
    coefficients: &FilterCoeffs,
    coefficient_source_height: usize,
    source_row_offset: usize,
    active_rows: &[bool],
    output_y: usize,
) -> Option<bool> {
    let first_coefficient_row = usize::try_from(*coefficients.xmin.get(output_y)?).ok()?;
    let tap_count = *coefficients.count.get(output_y)?;
    for tap in 0..tap_count {
        let coefficient_row = first_coefficient_row.checked_add(tap)?;
        if coefficient_row >= coefficient_source_height {
            return None;
        }
        let source_row = source_row_offset.checked_add(coefficient_row)?;
        if *active_rows.get(source_row)? {
            return Some(true);
        }
    }
    Some(false)
}
