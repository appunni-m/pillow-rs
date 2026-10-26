use super::{PortableFma, pil_resize_f_boxed_with_fma};
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, RgbaImage};

#[test]
fn eight_tap_boxed_resize_matches_ordered_generic_reference() {
    for (source_width, source_height, dst_w, dst_h, bounds, filter) in [
        (
            512u32,
            384u32,
            256u32,
            192u32,
            (0.0, 0.0, 512.0, 384.0),
            ResampleFilter::Bicubic,
        ),
        (
            37,
            23,
            19,
            11,
            (0.25, 0.5, 36.625, 22.75),
            ResampleFilter::Bicubic,
        ),
        (
            96,
            48,
            17,
            9,
            (0.125, 0.25, 95.625, 47.5),
            ResampleFilter::Lanczos,
        ),
    ] {
        let samples = (0..source_width as usize * source_height as usize)
            .map(|index| (((index * 37 % 257) as i32 - 128) as f32) / 31.0)
            .collect::<Vec<_>>();
        let bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, bytes).expect("packed F samples"),
        );
        let optimized = pil_resize_f_boxed_with_fma::<_, true>(
            &image,
            dst_w,
            dst_h,
            bounds.0,
            bounds.1,
            bounds.2,
            bounds.3,
            filter,
            &PortableFma,
        );
        let generic = pil_resize_f_boxed_with_fma::<_, false>(
            &image,
            dst_w,
            dst_h,
            bounds.0,
            bounds.1,
            bounds.2,
            bounds.3,
            filter,
            &PortableFma,
        );
        assert_eq!(optimized.as_bytes(), generic.as_bytes());
    }
}
