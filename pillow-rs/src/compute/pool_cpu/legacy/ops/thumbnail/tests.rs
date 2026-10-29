#[cfg(not(feature = "parallel"))]
use super::resize_i;
use super::{
    execute_rgb_putpixel_thumbnail_fusion, execute_thumbnail,
    rgb_putpixel_thumbnail_fusion_supported,
};
use super::{reduce_f_thumbnail, reduce_i_thumbnail};
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::{DynamicImage, RgbImage, RgbaImage};

#[cfg(target_endian = "little")]
#[test]
fn f_thumbnail_reduce_matches_pillow_32bpc_grouping() {
    let source_words = [
        0x0e65_a54au32,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
        0x0e65_a54a,
    ];
    let source_bytes = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 4, source_bytes).expect("source shape must be valid"),
    );
    let output = reduce_f_thumbnail(&source, 1, 1, 4, 4)
        .expect("finite F-mode thumbnail reduction must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode thumbnail reduction must retain packed float storage");
    };
    assert_eq!(output.as_raw(), &0x0e65_a54au32.to_le_bytes());
}
#[cfg(target_endian = "little")]
#[test]
fn f_thumbnail_reduce_matches_pillow_float_group_order() {
    let source_words = [
        0xc42a_2b37,
        0xc3a2_c889,
        0xc3a6_341f,
        0xc432_e997,
        0x4411_0932,
        0xc46f_61cc,
    ];
    let source_bytes = source_words
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, source_bytes).expect("source shape must be valid"),
    );
    let output = reduce_f_thumbnail(&source, 1, 1, 3, 2)
        .expect("finite F-mode thumbnail reduction must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("F-mode thumbnail reduction must retain packed float storage");
    };
    assert_eq!(output.as_raw(), &0xc3ca_a3eau32.to_le_bytes());
}
#[cfg(target_endian = "little")]
#[test]
fn f_thumbnail_2x2_row_output_keeps_scalar_rounding_and_bits() {
    let (width, height) = (16u32, 10u32);
    let mut source_words = (0..width as usize * height as usize)
        .map(|index| (((index * 37 % 257) as i32 - 128) as f32) / 31.0)
        .map(f32::to_bits)
        .collect::<Vec<_>>();
    source_words[0] = (-0.0f32).to_bits();
    source_words[1] = (-0.0f32).to_bits();
    source_words[width as usize] = (-0.0f32).to_bits();
    source_words[width as usize + 1] = (-0.0f32).to_bits();
    source_words[6] = 0x8000_0001; // Negative minimum subnormal scales to -0.
    source_words[7] = 0;
    source_words[width as usize + 6] = 0;
    source_words[width as usize + 7] = 0;
    source_words[8] = 1; // Three minimum subnormals scale to one minimum subnormal.
    source_words[9] = 1;
    source_words[width as usize + 8] = 1;
    source_words[width as usize + 9] = 0;
    source_words[4] = f32::from_bits(0x7fc1_2345).to_bits();
    source_words[width as usize * 2 + 7] = f32::INFINITY.to_bits();
    source_words[width as usize * 4 + 13] = f32::NEG_INFINITY.to_bits();
    let source_bytes = source_words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect::<Vec<_>>();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(width, height, source_bytes).expect("F source shape must be valid"),
    );

    let actual = reduce_f_thumbnail(&source, width / 2, height / 2, 2, 2)
        .expect("even 2x2 F thumbnail reduction must succeed");
    let DynamicImage::ImageRgba8(actual) = actual else {
        panic!("F-mode thumbnail reduction must retain packed float storage");
    };
    let mut expected = Vec::with_capacity(width as usize * height as usize);
    for y in 0..height as usize / 2 {
        let top_row = y * 2 * width as usize;
        let bottom_row = top_row + width as usize;
        for x in 0..width as usize / 2 {
            let source_x = x * 2;
            let top_left = f32::from_bits(source_words[top_row + source_x]);
            let top_right = f32::from_bits(source_words[top_row + source_x + 1]);
            let bottom_left = f32::from_bits(source_words[bottom_row + source_x]);
            let bottom_right = f32::from_bits(source_words[bottom_row + source_x + 1]);
            let quartet = ((top_left + top_right) + bottom_left) + bottom_right;
            let mut sum = 0.0f64;
            sum += f64::from(quartet);
            expected.extend_from_slice(&((sum * 0.25) as f32).to_le_bytes());
        }
    }
    assert_eq!(actual.as_raw(), expected.as_slice());
}
#[cfg(target_endian = "little")]
#[test]
fn i_thumbnail_reduce_matches_pillow_int32_grouping() {
    let source_words = [i32::MAX; 4];
    let source_bytes: Vec<u8> = source_words
        .into_iter()
        .flat_map(i32::to_le_bytes)
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(2, 2, source_bytes).expect("source shape must be valid"),
    );
    let output =
        reduce_i_thumbnail(&source, 1, 1, 2, 2).expect("I-mode thumbnail reduction must succeed");
    let DynamicImage::ImageRgba8(output) = output else {
        panic!("I-mode thumbnail reduction must retain packed integer storage");
    };
    // Reduce.c forms the interior quartet while still INT32: four
    // INT32_MAX values wrap to -4 before promotion and averaging.
    assert_eq!(output.as_raw(), &(-1i32).to_le_bytes());
}
#[cfg(target_endian = "little")]
#[test]
fn i_thumbnail_2x2_row_path_matches_multiple_wrapping_quartets() {
    let source_words = [
        i32::MAX,
        1,
        i32::MIN,
        -1,
        -3,
        i32::MAX,
        9,
        11,
        4,
        -4,
        5,
        -5,
        6,
        8,
        i32::MIN,
        i32::MAX,
    ];
    let source_bytes = source_words
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 4, source_bytes).expect("source shape must be valid"),
    );
    let output =
        reduce_i_thumbnail(&source, 2, 2, 2, 2).expect("I-mode 2x2 reduction must succeed");

    let mut expected = Vec::with_capacity(2 * 2 * 4);
    for y in 0..2usize {
        for x in 0..2usize {
            let top = y * 2 * 4 + x * 2;
            let bottom = top + 4;
            let quartet = source_words[top]
                .wrapping_add(source_words[top + 1])
                .wrapping_add(source_words[bottom])
                .wrapping_add(source_words[bottom + 1]);
            let value = super::round_up(f64::from(quartet) / 4.0) as i32;
            expected.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(output.as_bytes(), expected);
}
#[cfg(not(feature = "parallel"))]
#[test]
fn i_thumbnail_streaming_2x2_matches_materialized_reduce_and_resize() {
    let samples: Vec<i32> = (0..32 * 24)
        .map(|index| match index % 8 {
            0 => i32::MAX,
            1 => i32::MIN,
            2 => -1,
            3 => 1,
            4 => index * 7919,
            5 => -(index * 3571),
            6 => 0,
            _ => i32::MAX / 2,
        })
        .collect();
    let source_bytes: Vec<u8> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(32, 24, source_bytes).expect("source shape must be valid"),
    );
    let filters = [
        ResampleFilter::Box,
        ResampleFilter::Bilinear,
        ResampleFilter::Hamming,
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
    ];

    for filter in filters {
        let reduced =
            reduce_i_thumbnail(&source, 16, 12, 2, 2).expect("I-mode 2x2 reduction must succeed");
        for (destination_width, destination_height) in [(8, 6), (7, 5)] {
            let expected = resize_i(&reduced, destination_width, destination_height, &filter)
                .expect("materialized I-mode resize must succeed");
            let actual = execute_thumbnail(
                &source,
                destination_width,
                destination_height,
                &filter,
                Some("I"),
            )
            .expect("I-mode thumbnail must succeed");
            let (DynamicImage::ImageRgba8(actual), DynamicImage::ImageRgba8(expected)) =
                (actual, expected)
            else {
                panic!("I-mode thumbnail must retain packed integer storage");
            };
            assert_eq!(
                actual.as_raw(),
                expected.as_raw(),
                "filter={filter:?}, size={destination_width}x{destination_height}"
            );
        }
    }
}
#[cfg(target_endian = "little")]
#[test]
fn i_thumbnail_odd_2x2_edges_keep_partial_double_accumulation() {
    let source_words = [
        i32::MAX,
        1,
        i32::MIN,
        -1,
        17,
        -3,
        i32::MAX,
        9,
        11,
        -29,
        4,
        -4,
        5,
        -5,
        i32::MIN,
    ];
    let source_bytes = source_words
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(5, 3, source_bytes).expect("source shape must be valid"),
    );
    let output =
        reduce_i_thumbnail(&source, 3, 2, 2, 2).expect("I-mode odd-edge reduction must succeed");

    let mut expected = Vec::with_capacity(3 * 2 * 4);
    let main_width = 5 / 2;
    let main_height = 3 / 2;
    for y in 0..2usize {
        for x in 0..3usize {
            let source_x = x * 2;
            let source_y = y * 2;
            let block_width = 2.min(5 - source_x);
            let block_height = 2.min(3 - source_y);
            let mut sum = 0.0f64;
            if x < main_width && y < main_height {
                let top = source_y * 5 + source_x;
                let bottom = top + 5;
                let quartet = source_words[top]
                    .wrapping_add(source_words[top + 1])
                    .wrapping_add(source_words[bottom])
                    .wrapping_add(source_words[bottom + 1]);
                sum = f64::from(quartet);
            } else {
                for dy in 0..block_height {
                    for dx in 0..block_width {
                        sum += f64::from(source_words[(source_y + dy) * 5 + source_x + dx]);
                    }
                }
            }
            let divisor =
                u32::try_from(block_width * block_height).expect("test block size fits u32");
            let value = super::round_up(sum / f64::from(divisor)) as i32;
            expected.extend_from_slice(&value.to_le_bytes());
        }
    }
    assert_eq!(output.as_bytes(), expected);
}
#[test]
fn rgb_putpixel_thumbnail_fusion_matches_materialized_putpixel_and_thumbnail() {
    let (source_width, source_height) = (1024, 768);
    let putpixel_ops = rgb_putpixel_thumbnail_ops(source_width, source_height);

    for dense in [false, true] {
        let source = rgb_putpixel_thumbnail_source(dense, source_width, source_height);
        assert_rgb_putpixel_thumbnail_supported_cases(dense, &source, &putpixel_ops);
    }
}

fn rgb_putpixel_thumbnail_ops(source_width: u32, source_height: u32) -> Vec<PipelineOp> {
    vec![
        PipelineOp::PutPixel {
            x: 2,
            y: 3,
            color: (180, 120, 60, 255),
            palette_index: false,
        },
        PipelineOp::PutPixel {
            x: 2,
            y: 3,
            color: (10, 20, 30, 255),
            palette_index: false,
        },
        PipelineOp::PutPixel {
            x: source_width - 1,
            y: source_height - 1,
            color: (7, 129, 251, 255),
            palette_index: false,
        },
    ]
}

fn rgb_putpixel_thumbnail_source(dense: bool, width: u32, height: u32) -> DynamicImage {
    let bytes = if dense {
        (0..width as usize * height as usize * 3)
            .map(|index| (index.wrapping_mul(73).wrapping_add(index / 17 * 29)) as u8)
            .collect()
    } else {
        vec![0; width as usize * height as usize * 3]
    };
    DynamicImage::ImageRgb8(
        RgbImage::from_raw(width, height, bytes)
            .expect("test RGB source dimensions must match its bytes"),
    )
}

fn assert_rgb_putpixel_thumbnail_supported_cases(
    dense: bool,
    source: &DynamicImage,
    putpixel_ops: &[PipelineOp],
) {
    let (output_width, output_height) = (256, 192);
    let nearest_thumbnail = PipelineOp::Thumbnail {
        w: output_width,
        h: output_height,
        filter: ResampleFilter::Nearest,
    };
    assert!(!rgb_putpixel_thumbnail_fusion_supported(
        source,
        putpixel_ops,
        &nearest_thumbnail,
        Some("RGB"),
    ));
    let materialized_source =
        crate::compute::pool_cpu::ops::effects::op_put_pixel_batch(source, putpixel_ops)
            .expect("validated RGB pixel writes must succeed");
    for filter in [
        ResampleFilter::Bilinear,
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
        ResampleFilter::Box,
        ResampleFilter::Hamming,
    ] {
        let thumbnail = PipelineOp::Thumbnail {
            w: output_width,
            h: output_height,
            filter,
        };
        assert!(rgb_putpixel_thumbnail_fusion_supported(
            source,
            putpixel_ops,
            &thumbnail,
            Some("RGB"),
        ));
        let expected = execute_thumbnail(
            &materialized_source,
            output_width,
            output_height,
            &filter,
            Some("RGB"),
        )
        .expect("reference RGB thumbnail must succeed");
        let actual =
            execute_rgb_putpixel_thumbnail_fusion(source, putpixel_ops, &thumbnail, Some("RGB"))
                .expect("fused RGB thumbnail must succeed");
        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "dense={dense} filter={filter:?}"
        );
    }
}
