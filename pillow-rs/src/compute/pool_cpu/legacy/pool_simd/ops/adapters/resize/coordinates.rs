//! Shared nearest-neighbor coordinates and capability checks for Resize kernels.

pub(super) fn resize_nearest_indices(source_size: u32, output_size: u32) -> Option<Vec<usize>> {
    if source_size == 0 || output_size == 0 {
        return None;
    }
    let scale = f64::from(source_size) / f64::from(output_size);
    let mut position = scale * 0.5;
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    for _ in 0..output_size {
        let index = position as u32;
        indices.push(if index >= source_size {
            usize::try_from(source_size - 1).ok()?
        } else {
            usize::try_from(index).ok()?
        });
        position += scale;
    }
    Some(indices)
}

pub(in crate::compute::pool_simd::ops::adapters) fn resize_nearest_vectorizable(
    source_width: u32,
    source_height: u32,
    output_width: u32,
    output_height: u32,
    channels: usize,
) -> bool {
    if channels == 0
        || !(1..=4).contains(&channels)
        || source_width == 0
        || source_height == 0
        || output_width == 0
        || output_height == 0
    {
        return false;
    }
    // Nearest-neighbour coordinates are inherently a scalar gather on
    // architectures without a portable byte-gather instruction. That does
    // not make the operation a CPU fallback: the gathered samples are packed
    // into native vector blocks and the output data plane is written by the
    // SIMD kernel. Admission therefore depends on valid non-empty dimensions,
    // not on the source span of a particular gather block.
    resize_nearest_indices(source_width, output_width).is_some()
        && resize_nearest_indices(source_height, output_height).is_some()
}

/// Build the affine nearest-neighbour map used by boxed P/PA resampling.
/// Coordinates are evaluated in f64 lanes after Pillow's float box boundary
/// conversion; individual source loads remain scalar gathers because portable
/// byte gather is not available on every SIMD target.
pub(super) fn boxed_nearest_indices(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> Option<Vec<usize>> {
    if source_size == 0 || output_size == 0 || !box_start.is_finite() || !box_end.is_finite() {
        return None;
    }
    let box_start = box_start as f32 as f64;
    let box_end = box_end as f32 as f64;
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    let last = f64::from(source_size - 1);
    let mut indices = Vec::with_capacity(usize::try_from(output_size).ok()?);
    // Pillow's nearest boxed resize enters `ImagingScaleAffine`, which
    // initializes one coordinate and advances it with `xo += a[0]` for each
    // output pixel. Recomputing `(index + 0.5) * scale` in SIMD lanes rounds
    // differently at exact upsample boundaries, so keep the scalar coordinate
    // recurrence for selection and leave SIMD to the subsequent byte gather.
    let mut coordinate = box_start + scale * 0.5;
    for _ in 0..output_size as usize {
        indices.push(coordinate.floor().clamp(0.0, last) as usize);
        coordinate += scale;
    }
    Some(indices)
}

/// Return whether boxed nearest coordinates select each same-position sample.
/// The recurrence mirrors Pillow's affine scaler; checking one axis is linear
/// in its extent and avoids constructing a per-output-pixel gather map.
pub(super) fn boxed_nearest_axis_maps_identity(
    source_size: u32,
    output_size: u32,
    box_start: f64,
    box_end: f64,
) -> bool {
    if source_size == 0
        || source_size != output_size
        || !box_start.is_finite()
        || !box_end.is_finite()
    {
        return false;
    }
    let box_start = box_start as f32 as f64;
    let box_end = box_end as f32 as f64;
    let scale = (box_end as f32 - box_start as f32) as f64 / f64::from(output_size);
    if !scale.is_finite() {
        return false;
    }
    let last = f64::from(source_size - 1);
    let mut coordinate = box_start + scale * 0.5;
    for expected in 0..output_size as usize {
        if coordinate.floor().clamp(0.0, last) as usize != expected {
            return false;
        }
        coordinate += scale;
    }
    true
}
