//! General column traversal and native radius-one row-ring blur.

use super::line::blur_one_row;

use super::{BOX_BLUR_BIAS, CheckedDims, PilError};

/// Apply a single native-byte BoxBlur(1) pass with a three-row horizontal
/// scratch ring. This keeps the horizontal intermediate in cache and writes
/// each final row directly, avoiding a full-frame source clone and vertical
/// scratch buffer for the common one-pass case.
#[inline]
fn blur_box_radius_one_horizontal_row(
    source: &[u8],
    ring: &mut [u8],
    row: usize,
    row_bytes: usize,
    width: usize,
    channels: usize,
    horizontal_weight: u32,
) {
    if channels == 1 {
        let source_start = row * row_bytes;
        blur_box_radius_one_horizontal_l_row(
            &source[source_start..source_start + row_bytes],
            ring,
            width,
            horizontal_weight,
        );
        return;
    }
    if channels == 3 {
        let source_start = row * row_bytes;
        blur_box_radius_one_horizontal_rgb_row(
            &source[source_start..source_start + row_bytes],
            ring,
            width,
            horizontal_weight,
        );
        return;
    }
    blur_one_row(
        source,
        ring,
        row * row_bytes,
        width,
        channels,
        1,
        horizontal_weight,
        0,
    );
}

#[inline]
fn blur_box_radius_one_horizontal_l_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert_eq!(source.len(), width);
    debug_assert_eq!(destination.len(), width);

    let last = width - 1;
    let mut sum = u32::from(source[0]) * 2 + u32::from(source[usize::from(width > 1)]);
    destination[0] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    if width == 1 {
        return;
    }

    sum = sum
        .wrapping_sub(u32::from(source[0]))
        .wrapping_add(u32::from(source[2.min(last)]));
    destination[1] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    if width == 2 {
        return;
    }

    for center in 2..last {
        sum = sum
            .wrapping_sub(u32::from(source[center - 2]))
            .wrapping_add(u32::from(source[center + 1]));
        destination[center] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    }

    sum = sum
        .wrapping_sub(u32::from(source[width - 3]))
        .wrapping_add(u32::from(source[last]));
    destination[last] = (sum.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
}

#[inline]
fn blur_box_radius_one_horizontal_rgb_row(
    source: &[u8],
    destination: &mut [u8],
    width: usize,
    weight: u32,
) {
    debug_assert!(width > 0);
    debug_assert_eq!(source.len(), width * 3);
    debug_assert_eq!(destination.len(), width * 3);

    let last = width - 1;
    let last_base = last * 3;
    let first_right = usize::from(width > 1) * 3;
    let mut red = u32::from(source[0]) * 2 + u32::from(source[first_right]);
    let mut green = u32::from(source[1]) * 2 + u32::from(source[first_right + 1]);
    let mut blue = u32::from(source[2]) * 2 + u32::from(source[first_right + 2]);

    destination[0] = (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[1] = (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[2] = (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    if width == 1 {
        return;
    }

    let first_entering = (2.min(last)) * 3;
    red = red
        .wrapping_sub(u32::from(source[0]))
        .wrapping_add(u32::from(source[first_entering]));
    green = green
        .wrapping_sub(u32::from(source[1]))
        .wrapping_add(u32::from(source[first_entering + 1]));
    blue = blue
        .wrapping_sub(u32::from(source[2]))
        .wrapping_add(u32::from(source[first_entering + 2]));

    if width > 2 {
        for center in 1..width - 2 {
            let output_base = center * 3;
            destination[output_base] =
                (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
            destination[output_base + 1] =
                (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
            destination[output_base + 2] =
                (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;

            let leaving_base = (center - 1) * 3;
            let entering_base = (center + 2) * 3;
            red = red
                .wrapping_sub(u32::from(source[leaving_base]))
                .wrapping_add(u32::from(source[entering_base]));
            green = green
                .wrapping_sub(u32::from(source[leaving_base + 1]))
                .wrapping_add(u32::from(source[entering_base + 1]));
            blue = blue
                .wrapping_sub(u32::from(source[leaving_base + 2]))
                .wrapping_add(u32::from(source[entering_base + 2]));
        }

        let penultimate = width - 2;
        let output_base = penultimate * 3;
        destination[output_base] =
            (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        destination[output_base + 1] =
            (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
        destination[output_base + 2] =
            (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;

        let leaving_base = (width - 3) * 3;
        red = red
            .wrapping_sub(u32::from(source[leaving_base]))
            .wrapping_add(u32::from(source[last_base]));
        green = green
            .wrapping_sub(u32::from(source[leaving_base + 1]))
            .wrapping_add(u32::from(source[last_base + 1]));
        blue = blue
            .wrapping_sub(u32::from(source[leaving_base + 2]))
            .wrapping_add(u32::from(source[last_base + 2]));
    }

    destination[last_base] = (red.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[last_base + 1] =
        (green.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
    destination[last_base + 2] =
        (blue.wrapping_mul(weight).wrapping_add(BOX_BLUR_BIAS) >> 24) as u8;
}

pub(super) fn blur_box_radius_one_row_ring(
    source: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    horizontal_weight: u32,
    vertical_weight: u32,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(width, height, channels as u8)?;
    if source.len() != dimensions.total_bytes() {
        return Err(PilError::DimensionError(
            "BoxBlur source byte length does not match its dimensions".into(),
        ));
    }
    let ring_height = dimensions.height.min(3);
    let ring_dimensions = CheckedDims::new(dimensions.width, ring_height, channels as u8)?;
    let row_bytes = dimensions.row_stride();
    let width = dimensions.width as usize;
    let height = dimensions.height as usize;
    let ring_slots = ring_height as usize;
    let mut output = dimensions.alloc_buffer();
    let mut ring = ring_dimensions.alloc_buffer();

    for row in 0..height.min(2) {
        let start = (row % ring_slots) * row_bytes;
        blur_box_radius_one_horizontal_row(
            source,
            &mut ring[start..start + row_bytes],
            row,
            row_bytes,
            width,
            channels,
            horizontal_weight,
        );
    }

    for y in 0..height {
        let center_slot = y % ring_slots;
        let bottom_y = (y + 1).min(height - 1);
        let bottom_slot = bottom_y % ring_slots;
        let center_start = center_slot * row_bytes;
        let top_y = y.saturating_sub(1);
        let top_start = (top_y % ring_slots) * row_bytes;
        let output_start = y * row_bytes;
        let output_row = &mut output[output_start..output_start + row_bytes];
        let top_row = &ring[top_start..top_start + row_bytes];
        let center_row = &ring[center_start..center_start + row_bytes];
        let bottom_start = bottom_slot * row_bytes;
        let bottom_row = &ring[bottom_start..bottom_start + row_bytes];
        for ((output, &top), (&center, &bottom)) in output_row
            .iter_mut()
            .zip(top_row)
            .zip(center_row.iter().zip(bottom_row))
        {
            let sum = u32::from(top) + u32::from(center) + u32::from(bottom);
            *output = (sum
                .wrapping_mul(vertical_weight)
                .wrapping_add(BOX_BLUR_BIAS)
                >> 24) as u8;
        }

        let next_row = y + 2;
        if next_row < height {
            let next_start = (next_row % ring_slots) * row_bytes;
            blur_box_radius_one_horizontal_row(
                source,
                &mut ring[next_start..next_start + row_bytes],
                next_row,
                row_bytes,
                width,
                channels,
                horizontal_weight,
            );
        }
    }

    Ok(output)
}

#[cfg(test)]
mod serial_box_blur_radius_one_row_ring_tests {
    use super::super::{
        CheckedDims, blur_parameters,
        line::{blur_columns, blur_rows},
    };
    use super::blur_box_radius_one_row_ring;

    #[test]
    fn row_ring_matches_fixed_point_two_passes_at_edges_and_tails() {
        let (radius, weight, fractional_weight) = blur_parameters(1.0);
        assert_eq!((radius, fractional_weight), (1, 0));

        for (width, height) in [
            (1, 1),
            (1, 7),
            (2, 3),
            (3, 2),
            (4, 5),
            (5, 9),
            (17, 9),
            (65, 33),
        ] {
            for channels in [1, 2, 3, 4] {
                let dimensions = CheckedDims::new(width, height, channels).unwrap();
                let source = (0..dimensions.total_bytes())
                    .map(|index| ((index * 73 + index / 11 * 29 + 37) % 256) as u8)
                    .collect::<Vec<_>>();
                let actual = blur_box_radius_one_row_ring(
                    &source,
                    width,
                    height,
                    channels as usize,
                    weight,
                    weight,
                )
                .unwrap();
                let mut horizontal = dimensions.alloc_buffer();
                let mut expected = dimensions.alloc_buffer();
                blur_rows(
                    &source,
                    &mut horizontal,
                    width as usize,
                    height as usize,
                    channels as usize,
                    radius,
                    weight,
                    fractional_weight,
                );
                blur_columns(
                    &horizontal,
                    &mut expected,
                    width as usize,
                    height as usize,
                    channels as usize,
                    radius,
                    weight,
                    fractional_weight,
                    &mut vec![0; dimensions.row_stride()],
                );

                assert_eq!(actual, expected, "{width}x{height}, channels={channels}");
            }
        }
    }
}
