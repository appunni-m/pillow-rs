use super::{filter_5x5_byte_row, uniform_box};

const NORMALIZED_BOX: [f32; 25] = [1.0f32 / 25.0; 25];

fn scalar_reference(raw: &[u8], width: usize, height: usize, channels: usize) -> Vec<u8> {
    let row_stride = width * channels;
    let mut expected = raw.to_vec();
    for y in 0..height {
        let start = y * row_stride;
        filter_5x5_byte_row(
            raw,
            &mut expected[start..start + row_stride],
            y as i32,
            width as i32,
            height as i32,
            channels,
            &NORMALIZED_BOX,
            0.5,
        );
    }
    expected
}

fn assert_matches_reference(raw: &[u8], width: usize, height: usize, channels: usize) {
    let expected = scalar_reference(raw, width, height, channels);
    let mut actual = raw.to_vec();
    assert!(uniform_box::apply_if_uniform(
        raw,
        &mut actual,
        width,
        height,
        channels,
        &NORMALIZED_BOX,
        0.5,
    ));
    assert_eq!(
        actual, expected,
        "{width}x{height} with {channels} channels"
    );

    for y in 0..height {
        for x in 0..width {
            if x < 2 || x >= width - 2 || y < 2 || y >= height - 2 {
                let start = (y * width + x) * channels;
                assert_eq!(
                    &actual[start..start + channels],
                    &raw[start..start + channels],
                    "copied border at ({x}, {y}), {channels} channels"
                );
            }
        }
    }
}

#[test]
fn uniform_color_box_matches_ordered_filter_for_la_rgb_rgba_and_tails() {
    for channels in [2usize, 3, 4] {
        for width in [5usize, 6, 7, 17] {
            for height in [5usize, 6, 9] {
                let row_stride = width * channels;
                let raw = (0..row_stride * height)
                    .map(|index| ((index * 73 + (index / row_stride) * 29 + 17) % 256) as u8)
                    .collect::<Vec<_>>();
                assert_matches_reference(&raw, width, height, channels);
            }
        }
    }
}

#[test]
fn uniform_color_box_matches_zero_and_full_scale_and_keeps_borders() {
    for channels in [2usize, 3, 4] {
        for width in [5usize, 17] {
            for height in [5usize, 9] {
                for value in [0u8, 255] {
                    let raw = vec![value; width * height * channels];
                    assert_matches_reference(&raw, width, height, channels);
                }
            }
        }
    }
}

#[test]
fn uniform_color_box_matches_every_possible_25_byte_sum() {
    for channels in [2usize, 3, 4] {
        for total in 0..=25 * 255 {
            let mut remaining = total;
            let mut raw = vec![0u8; 25 * channels];
            for pixel in raw.chunks_exact_mut(channels) {
                let sample = remaining.min(255) as u8;
                pixel.fill(sample);
                remaining -= usize::from(sample);
            }
            assert_eq!(remaining, 0);
            assert_matches_reference(&raw, 5, 5, channels);
        }
    }
}

#[test]
fn uniform_color_box_public_cpu_kernel_preserves_native_layout_and_source() {
    use crate::compute::Backend;
    use crate::image::Image;

    for (mode, channels) in [("LA", 2usize), ("RGB", 3), ("RGBA", 4)] {
        let width = 17;
        let height = 9;
        let raw = (0..width * height * channels)
            .map(|index| ((index * 73 + 19) % 256) as u8)
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, (width as u32, height as u32), &raw)
            .expect("valid focused native image");
        let result = source
            .kernel_filter(Some(vec![1.0; 25]), Some(25.0), 0.0, (5, 5))
            .expect("valid 5x5 kernel")
            .use_backend(Backend::Cpu);
        assert_eq!(result.mode().expect("result mode"), mode);
        assert_eq!(result.size().expect("result dimensions"), (17, 9));
        assert_eq!(
            result.tobytes().expect("CPU kernel materialization"),
            scalar_reference(&raw, width, height, channels),
        );
        assert_eq!(source.tobytes().expect("source bytes"), raw);
    }
}

fn assert_declines_without_mutation(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    kernel: &[f32; 25],
    rounding_bias: f32,
) {
    let before = out.to_vec();
    assert!(!uniform_box::apply_if_uniform(
        raw,
        out,
        width,
        height,
        channels,
        kernel,
        rounding_bias,
    ));
    assert_eq!(out, before.as_slice());
}

#[test]
fn uniform_color_box_rejects_wrong_lengths_layouts_and_parameters_without_mutation() {
    let valid_raw = vec![37u8; 5 * 5 * 3];
    let mut output = vec![0xa5; valid_raw.len()];

    assert_declines_without_mutation(
        &valid_raw[..valid_raw.len() - 1],
        &mut output,
        5,
        5,
        3,
        &NORMALIZED_BOX,
        0.5,
    );
    let mut overlong = valid_raw.clone();
    overlong.push(0x5a);
    assert_declines_without_mutation(&overlong, &mut output, 5, 5, 3, &NORMALIZED_BOX, 0.5);
    let short_output_len = output.len() - 1;
    assert_declines_without_mutation(
        &valid_raw,
        &mut output[..short_output_len],
        5,
        5,
        3,
        &NORMALIZED_BOX,
        0.5,
    );
    assert_declines_without_mutation(&valid_raw, &mut output, 4, 5, 3, &NORMALIZED_BOX, 0.5);
    assert_declines_without_mutation(&valid_raw, &mut output, 5, 4, 3, &NORMALIZED_BOX, 0.5);

    let one_channel = vec![19u8; 5 * 5];
    let mut one_channel_out = one_channel.clone();
    assert_declines_without_mutation(
        &one_channel,
        &mut one_channel_out,
        5,
        5,
        1,
        &NORMALIZED_BOX,
        0.5,
    );
    let five_channels = vec![19u8; 5 * 5 * 5];
    let mut five_channels_out = five_channels.clone();
    assert_declines_without_mutation(
        &five_channels,
        &mut five_channels_out,
        5,
        5,
        5,
        &NORMALIZED_BOX,
        0.5,
    );

    let mut changed_coefficient = NORMALIZED_BOX;
    changed_coefficient[12] = f32::from_bits(changed_coefficient[12].to_bits() + 1);
    assert_declines_without_mutation(&valid_raw, &mut output, 5, 5, 3, &changed_coefficient, 0.5);
    let unnormalized = [1.0f32; 25];
    assert_declines_without_mutation(&valid_raw, &mut output, 5, 5, 3, &unnormalized, 0.5);
    assert_declines_without_mutation(&valid_raw, &mut output, 5, 5, 3, &NORMALIZED_BOX, 0.0);

    assert_declines_without_mutation(&[], &mut [], usize::MAX, 5, 4, &NORMALIZED_BOX, 0.5);
    assert_declines_without_mutation(&[], &mut [], usize::MAX / 5, 5, 4, &NORMALIZED_BOX, 0.5);
}
