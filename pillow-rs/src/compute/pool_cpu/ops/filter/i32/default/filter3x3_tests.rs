#[cfg(test)]
mod i32_filter3x3_uniform_tests {
    use super::super::{
        execute, uniform_i32_filter3x3_is_identity,
        I32_UNIFORM_FILTER3X3_MAX_PIXELS,
    };
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn uniform_cross_smoothing_filter_returns_the_exact_input() {
        let (width, height, sample) = (32, 24, 37i32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let kernel = [0.0, 1.0, 0.0, 1.0, 4.0, 1.0, 0.0, 1.0, 0.0];
        let normalized = std::array::from_fn(|index| kernel[index] / 8.0);
        assert!(uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &normalized,
            0.5,
        ));

        let image = DynamicImage::ImageI32(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );
        let result = execute(&image, &kernel, 8.0, 0.0).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniformity_proof_rejects_nonidentity_nonuniform_and_oversized_inputs() {
        let (width, height, sample) = (7, 6, 11i32);
        let raw = sample.to_le_bytes().repeat(width * height);
        let mut identity_kernel = [0.0; 9];
        identity_kernel[4] = 1.0;
        assert!(uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &identity_kernel,
            0.5,
        ));

        let mut nonidentity_kernel = identity_kernel;
        nonidentity_kernel[4] = 2.0;
        assert!(!uniform_i32_filter3x3_is_identity(
            &raw,
            width,
            height,
            &nonidentity_kernel,
            0.5,
        ));

        let mut varied = raw.clone();
        varied[4 * (width + 3)..4 * (width + 3) + 4].copy_from_slice(&12i32.to_le_bytes());
        assert!(!uniform_i32_filter3x3_is_identity(
            &varied,
            width,
            height,
            &identity_kernel,
            0.5,
        ));
        assert!(!uniform_i32_filter3x3_is_identity(
            &raw,
            I32_UNIFORM_FILTER3X3_MAX_PIXELS + 1,
            3,
            &identity_kernel,
            0.5,
        ));
    }
}

