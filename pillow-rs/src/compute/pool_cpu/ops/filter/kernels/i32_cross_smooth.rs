//! Sparse I-mode 3x3 cross-kernel loop for serial CPU execution.

const KERNEL: [f32; 9] = [0.0, 0.125, 0.0, 0.125, 0.5, 0.125, 0.0, 0.125, 0.0];

pub(super) fn matches(kernel: &[f32; 9], offset: f32) -> bool {
    offset.to_bits() == 0.0f32.to_bits()
        && kernel
            .iter()
            .zip(KERNEL)
            .all(|(actual, expected)| actual.to_bits() == expected.to_bits())
}

#[inline(always)]
fn read_pixel(raw: &[u8], row_start: usize, x: usize) -> i32 {
    let index = row_start + x * 4;
    i32::from_le_bytes([raw[index], raw[index + 1], raw[index + 2], raw[index + 3]])
}

#[inline(always)]
fn filtered_pixel(top_center: i32, left: i32, center: i32, right: i32, bottom_center: i32) -> i32 {
    // The active weights are powers of two, so every product is exact for an
    // i32 converted to f32. The ordered additions therefore match Pillow's
    // fused multiply-add expression without dispatching a runtime FMA.
    let bottom = bottom_center as f32 * KERNEL[1];
    let middle = center as f32 * KERNEL[4];
    let middle = middle + left as f32 * KERNEL[3];
    let middle = middle + right as f32 * KERNEL[5];
    let top = top_center as f32 * KERNEL[7];
    let mut value = 0.5f32;
    value += bottom;
    value += middle;
    value += top;
    if value >= 0.0 { value as i32 } else { 0 }
}

/// Apply the cross kernel. The checked typed view removes four byte-index
/// bounds checks per sample on aligned little-endian images; unaligned or
/// big-endian inputs use the exact byte decoding path.
pub(super) fn apply(raw: &[u8], output: &mut [u8], width: usize, height: usize) {
    #[cfg(target_endian = "little")]
    if let (Ok(samples), Ok(output_samples)) = (
        bytemuck::try_cast_slice::<u8, i32>(raw),
        bytemuck::try_cast_slice_mut::<u8, i32>(output),
    ) {
        apply_samples(samples, output_samples, width, height);
        return;
    }

    apply_bytes(raw, output, width, height);
}

#[cfg(target_endian = "little")]
fn apply_samples(raw: &[i32], output: &mut [i32], width: usize, height: usize) {
    if width < 3 || height < 3 {
        return;
    }

    for y in 1..height - 1 {
        let top_start = (y - 1) * width;
        let middle_start = y * width;
        let bottom_start = (y + 1) * width;
        let (mut top_center, mut left, mut center, mut right, mut bottom_center) = (
            raw[top_start + 1],
            raw[middle_start],
            raw[middle_start + 1],
            raw[middle_start + 2],
            raw[bottom_start + 1],
        );

        for x in 1..width - 1 {
            output[middle_start + x] =
                filtered_pixel(top_center, left, center, right, bottom_center);

            if x + 1 < width - 1 {
                top_center = raw[top_start + x + 1];
                left = center;
                center = right;
                right = raw[middle_start + x + 2];
                bottom_center = raw[bottom_start + x + 1];
            }
        }
    }
}

fn apply_bytes(raw: &[u8], output: &mut [u8], width: usize, height: usize) {
    if width < 3 || height < 3 {
        return;
    }

    let row_stride = width * 4;
    for y in 1..height - 1 {
        let top_start = (y - 1) * row_stride;
        let middle_start = y * row_stride;
        let bottom_start = (y + 1) * row_stride;
        let (mut top_center, mut left, mut center, mut right, mut bottom_center) = (
            read_pixel(raw, top_start, 1),
            read_pixel(raw, middle_start, 0),
            read_pixel(raw, middle_start, 1),
            read_pixel(raw, middle_start, 2),
            read_pixel(raw, bottom_start, 1),
        );

        for x in 1..width - 1 {
            let value = filtered_pixel(top_center, left, center, right, bottom_center);
            let output_index = (y * width + x) * 4;
            output[output_index..output_index + 4].copy_from_slice(&value.to_le_bytes());

            if x + 1 < width - 1 {
                top_center = read_pixel(raw, top_start, x + 1);
                left = center;
                center = right;
                right = read_pixel(raw, middle_start, x + 2);
                bottom_center = read_pixel(raw, bottom_start, x + 1);
            }
        }
    }
}
