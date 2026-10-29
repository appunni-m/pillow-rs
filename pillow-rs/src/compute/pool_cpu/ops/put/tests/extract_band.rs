//! Existing CPU band regressions with the execution policy injected explicitly.

use super::super::ops::execute_extract_band;
use crate::compute::{Architecture, ExecutionContext};
use crate::compute::Backend;
use crate::image::Image;
use crate::raster::{DynamicImage, GenericImageView, RgbImage};
use std::sync::Arc;

fn rgb_with_trailing_storage(width: u32, height: u32) -> DynamicImage {
    let logical_bytes = usize::try_from(width * height * 3).expect("small RGB shape");
    let samples = (0..logical_bytes + 18)
        .map(|index| u8::try_from((index * 73 + 19) % 256).expect("sample byte fits"))
        .collect();
    DynamicImage::ImageRgb8(
        RgbImage::from_raw(width, height, samples).expect("valid overlong RGB storage"),
    )
}

#[test]
fn rgb_extract_band_ignores_trailing_storage_for_every_tail_and_band() {
    let ctx = ExecutionContext::new(Backend::Cpu, Architecture::Default);
    let shapes = [
        (0, 7),
        (7, 0),
        (1, 1),
        (2, 1),
        (3, 1),
        (5, 1),
        (6, 1),
        (7, 1),
    ];
    for (width, height) in shapes {
        let source = rgb_with_trailing_storage(width, height);
        let pixel_count = usize::try_from(width * height).expect("small RGB shape");
        for channel in [0, 1, 2, 3, u8::MAX] {
            let output = execute_extract_band(&source, channel, &ctx).expect("CPU RGB extraction");
            let expected: Vec<u8> = if channel < 3 {
                source.as_bytes()[..pixel_count * 3]
                    .chunks_exact(3)
                    .map(|pixel| pixel[usize::from(channel)])
                    .collect()
            } else {
                vec![u8::MAX; pixel_count]
            };
            assert_eq!(output.dimensions(), (width, height));
            assert_eq!(output.color(), crate::raster::ColorType::L8);
            assert_eq!(
                output.as_bytes(),
                expected,
                "shape={width}x{height}, band={channel}"
            );
        }
    }
}

#[test]
fn public_cpu_getchannel_ignores_overlong_loaded_rgb_storage() {
    // Public Rust callers can replace LoadedData.image with a valid
    // overlong ImageBuffer; Python frombytes truncates this storage first.
    // Counts above the eager threshold exercise the deferred CPU path.
    for width in [257, 258, 259, 260] {
        let source = rgb_with_trailing_storage(width, 1);
        let mut image = Image::new(width, 1, "RGB", (0, 0, 0, 255)).expect("RGB image");
        let Image::Loaded(data) = &mut image else {
            panic!("new RGB image must have loaded storage");
        };
        data.image = Arc::new(source.clone());
        for channel in 0..3 {
            let result = image
                .getchannel(channel)
                .expect("public valid RGB band")
                .use_backend(Backend::Cpu);
            let actual = result.tobytes().expect("CPU band materialization");
            let expected: Vec<u8> = source.as_bytes()
                [..usize::try_from(width * 3).expect("small RGB shape")]
                .chunks_exact(3)
                .map(|pixel| pixel[usize::try_from(channel).expect("valid band")])
                .collect();
            assert_eq!(actual, expected, "width={width}, band={channel}");
            assert_eq!(result.mode().expect("output mode"), "L");
        }
        assert!(image.getchannel(-1).is_err());
        assert!(image.getchannel(3).is_err());
    }
}
