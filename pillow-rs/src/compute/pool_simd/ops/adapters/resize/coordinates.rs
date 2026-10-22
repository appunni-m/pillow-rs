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
