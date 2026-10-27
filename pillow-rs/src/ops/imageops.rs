//! Pillow `ImageOps`-style module functions.
//!
//! Functions take [`crate::Image`] handles and return lazy result images where the
//! operation can be represented in the compute pipeline.

use crate::error::PilError;
use crate::image::Image;
use crate::ops::resize::{ResampleInput, parse_resample, parse_resample_input};
use crate::pipeline::PipelineOp;
use std::sync::Arc;

mod color;
mod exif;
mod geometry;
mod point;

pub(crate) use color::resolve_imageops_color;
pub use exif::{exif_get_orientation, exif_remove_orientation, exif_transpose};
pub use geometry::{
    contain, contain_with_input, cover, cover_with_input, crop, expand, expand_with_input, fit,
    fit_with_input, pad, pad_with_input, pad_with_input_and_containment, scale_with_input,
    validate_deform_resample,
};
pub use point::*;

/// Host-neutral centering input for `ImageOps.fit` and `ImageOps.pad`.
#[derive(Debug, Clone)]
pub enum CenteringInput {
    /// Use Pillow's default `(0.5, 0.5)`.
    Default,
    /// A scalar supplied where a pair was expected.
    Scalar(f64),
    /// A sequence supplied by the caller.
    Values(Vec<f64>),
    /// A value that could not be represented as a numeric sequence.
    Invalid,
}

/// Host-neutral mask input for ImageOps functions.
#[derive(Debug, Clone)]
pub enum ImageOpsMask {
    /// No mask was supplied.
    None,
    /// A mask image extracted by a binding.
    Image(Box<Image>),
    /// A non-image value was supplied, preserving its host type name for the
    /// same attribute error Pillow raises when it calls ``mask.load()``.
    Invalid(String),
}

/// Host-neutral color input for `ImageOps.pad` and `ImageOps.expand`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageOpsColor {
    /// No explicit color was supplied; use the operation default.
    None,
    /// A named or CSS-style color extracted from the host object.
    Name(String),
    /// A scalar color value extracted from the host object.
    Scalar(i64),
    /// A color component sequence extracted from the host object.
    Components(Vec<i64>),
    /// A value that was not a supported color representation.
    Invalid,
}

/// Host-neutral threshold input for [`solarize_with_input`].
pub enum SolarizeInput {
    /// A built-in numeric threshold, including NaN and infinities.
    Threshold(f64),
    /// Results of `sample < threshold` for samples 0 through 255.
    /// Bindings retain host comparison order and exceptions for custom objects.
    Comparisons(Box<[bool; 256]>),
}

pub fn validate_imageops_mask(image: &Image, mask: ImageOpsMask) -> Result<(), PilError> {
    match mask {
        ImageOpsMask::None => Ok(()),
        ImageOpsMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
            "'{type_name}' object has no attribute 'load'"
        ))),
        ImageOpsMask::Image(mask) => crate::ops::analysis::validate_transparency_mask(image, &mask),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CenteringInput, ImageOpsColor, contain_with_input, cover_with_input, expand_with_input,
        fit, fit_with_input, pad, pad_with_input, resolve_imageops_color, scale_with_input,
    };
    use crate::error::PilError;
    use crate::image::Image;
    use crate::ops::resize::ResampleInput;
    use crate::raster::GenericImageView;

    fn empty_image(size: (u32, u32)) -> Image {
        Image::new(size.0, size.1, "L", (0, 0, 0, 0)).expect("empty image dimensions are valid")
    }

    #[test]
    fn pad_same_size_returns_copy_without_adding_operation() {
        let image = empty_image((2, 3));
        let result = pad(&image, 2, 3, None, None, (0.5, 0.5))
            .expect("same-size Pad returns an independent copy");
        assert_eq!(result.size().expect("copy dimensions are known"), (2, 3));
        assert!(!matches!(result, Image::Pipeline { .. }));
    }

    #[test]
    fn scale_factor_one_copies_empty_shapes_before_filter_validation() {
        for size in [(0, 0), (0, 1), (1, 0), (2, 3)] {
            let image = empty_image(size);
            let result = scale_with_input(&image, 1.0, Some(ResampleInput::Code(99)))
                .expect("factor one must ignore the resample value");
            assert_eq!(result.size().expect("copy dimensions are known"), size);
        }
    }

    #[test]
    fn scale_preserves_all_zero_resize_result() {
        let image = empty_image((0, 0));
        let result = scale_with_input(&image, 0.5, None).expect("empty resize remains empty");
        assert_eq!(result.size().expect("copy dimensions are known"), (0, 0));
    }

    #[test]
    fn scale_infinity_observes_first_zero_dimension() {
        let both_zero = scale_with_input(&empty_image((0, 0)), f64::INFINITY, None)
            .expect_err("inf times zero must fail as NaN");
        assert!(matches!(
            both_zero,
            PilError::ValueError(message) if message == "cannot convert float NaN to integer"
        ));

        let zero_width = scale_with_input(&empty_image((0, 1)), f64::INFINITY, None)
            .expect_err("inf times zero width must fail as NaN");
        assert!(matches!(
            zero_width,
            PilError::ValueError(message) if message == "cannot convert float NaN to integer"
        ));

        let zero_height = scale_with_input(&empty_image((1, 0)), f64::INFINITY, None)
            .expect_err("nonzero width must observe infinity first");
        assert!(matches!(
            zero_height,
            PilError::OverflowError(message) if message == "cannot convert float infinity to integer"
        ));
    }

    #[test]
    fn scale_rejects_rounded_zero_for_nonempty_source() {
        let image = empty_image((2, 3));
        let error = scale_with_input(&image, 0.1, None)
            .expect_err("a nonempty source cannot resize to a zero dimension");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
    }

    #[test]
    fn pad_rejects_rounded_zero_contain_dimensions() {
        for size in [(2, 100), (100, 2)] {
            let image = empty_image(size);
            let error = pad_with_input(
                &image,
                1,
                1,
                None,
                ImageOpsColor::None,
                CenteringInput::Default,
            )
            .expect_err("contain must reject a rounded zero resize axis");
            assert!(matches!(
                error,
                PilError::ValueError(message) if message == "height and width must be > 0"
            ));
        }
    }

    #[test]
    fn pad_cmyk_color_keeps_black_ink_component() {
        assert_eq!(
            resolve_imageops_color(ImageOpsColor::Components(vec![17, 83, 149, 31]), "CMYK")
                .expect("four-component CMYK fill is valid"),
            Some((17, 83, 149, 31))
        );
    }

    #[test]
    fn pad_rgbx_color_resolution_preserves_pillow_fourth_byte_rules() {
        assert_eq!(
            resolve_imageops_color(ImageOpsColor::Components(vec![17, 83, 149, 31]), "RGBX")
                .expect("four-component RGBX fill is valid"),
            Some((17, 83, 149, 31))
        );
        assert_eq!(
            resolve_imageops_color(ImageOpsColor::Components(vec![17, 83, 149]), "RGBX")
                .expect("three-component RGBX fill is valid"),
            Some((17, 83, 149, u8::MAX))
        );
        assert_eq!(
            resolve_imageops_color(ImageOpsColor::Scalar(17), "RGBX")
                .expect("scalar RGBX fill is valid"),
            Some((17, 0, 0, 0))
        );
        assert_eq!(
            resolve_imageops_color(ImageOpsColor::None, "RGBX")
                .expect("default RGBX fill is valid"),
            None
        );
    }

    #[test]
    fn expand_rgbx_and_cmyk_resolve_three_components_in_native_order() {
        let fill = [17, 83, 149, u8::MAX];
        for mode in ["CMYK", "RGBX"] {
            let source = Image::new(1, 1, mode, (23, 47, 89, 131))
                .expect("native four-byte source is valid");
            let expanded =
                expand_with_input(&source, 1, ImageOpsColor::Components(vec![17, 83, 149]))
                    .expect("three-component native fill is valid");
            assert_eq!(expanded.mode().expect("mode is preserved"), mode);

            let mut expected = fill.repeat(9);
            expected[16..20].copy_from_slice(&[23, 47, 89, 131]);
            assert_eq!(expanded.tobytes().expect("expanded bytes"), expected);
        }
    }

    #[test]
    fn expand_rejects_two_component_fill_for_cmyk_and_rgbx() {
        for mode in ["CMYK", "RGBX"] {
            let source = Image::new(1, 1, mode, (23, 47, 89, 131))
                .expect("native four-byte source is valid");
            let error = expand_with_input(&source, 1, ImageOpsColor::Components(vec![17, 83]))
                .expect_err("CMYK and RGBX reject two-component colors");
            assert!(matches!(
                error,
                PilError::TypeError(message)
                    if message == "color must be int, or tuple of one, three or four elements"
            ));
        }
    }

    #[test]
    fn pad_preserves_pillow_zero_width_source_rules() {
        let source = empty_image((0, 2));
        let padded = pad_with_input(
            &source,
            2,
            2,
            None,
            ImageOpsColor::None,
            CenteringInput::Default,
        )
        .expect("matching source height keeps an empty-width contain result")
        .materialize()
        .expect("empty-width padded image materializes");
        assert_eq!(padded.dimensions(), (2, 2));
        assert_eq!(padded.as_bytes(), &[0, 0, 0, 0]);

        let empty = pad_with_input(
            &source,
            0,
            2,
            None,
            ImageOpsColor::None,
            CenteringInput::Default,
        )
        .expect("zero-width target with matching source height is valid")
        .materialize()
        .expect("zero-width target materializes");
        assert_eq!(empty.dimensions(), (0, 2));
        assert!(empty.as_bytes().is_empty());

        let error = pad_with_input(
            &source,
            1,
            1,
            None,
            ImageOpsColor::None,
            CenteringInput::Default,
        )
        .expect_err("changed contain height must fail for an empty-width source");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
    }

    #[test]
    fn contain_preserves_zero_dimension_and_filter_error_ordering() {
        let nonempty = empty_image((2, 100));
        let error = contain_with_input(&nonempty, 1, 1, None)
            .expect_err("rounded-zero contain output must fail");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));

        let invalid_filter = contain_with_input(&nonempty, 1, 1, Some(ResampleInput::Code(99)))
            .expect_err("resize filter validation precedes rounded-zero validation");
        assert!(
            matches!(invalid_filter, PilError::ValueError(message) if message.starts_with(
                "Unknown resampling filter (99)."
            ))
        );

        let zero_height =
            contain_with_input(&empty_image((1, 0)), 1, 1, Some(ResampleInput::Code(99)))
                .expect_err("zero source height must raise before filter validation");
        assert!(matches!(
            zero_height,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
    }

    #[test]
    fn contain_preserves_empty_width_source_rules() {
        let source = empty_image((0, 2));
        for target in [(0, 2), (1, 2), (2, 2)] {
            let result = contain_with_input(&source, target.0, target.1, None)
                .expect("unchanged contain height keeps an empty-width copy")
                .materialize()
                .expect("empty-width contain materializes");
            assert_eq!(result.dimensions(), (0, 2));
            assert!(result.as_bytes().is_empty());
        }

        let error = contain_with_input(&source, 1, 1, None)
            .expect_err("changed contain height must reject an empty-width result");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));
    }

    #[test]
    fn cover_preserves_empty_width_source_rules() {
        let source = empty_image((0, 2));
        let result = cover_with_input(&source, 0, 2, None)
            .expect("same-size empty-width cover returns a copy")
            .materialize()
            .expect("empty-width cover materializes");
        assert_eq!(result.dimensions(), (0, 2));
        assert!(result.as_bytes().is_empty());

        let error = cover_with_input(&source, 0, 1, None)
            .expect_err("changed empty-width cover size must fail resize validation");
        assert!(matches!(
            error,
            PilError::ValueError(message) if message == "height and width must be > 0"
        ));

        let division = cover_with_input(&source, 1, 2, None)
            .expect_err("positive destination width divides by zero source width");
        assert!(matches!(
            division,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));

        let source_height =
            cover_with_input(&empty_image((1, 0)), 1, 1, Some(ResampleInput::Code(99)))
                .expect_err("zero source height precedes filter validation");
        assert!(matches!(
            source_height,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
    }

    #[test]
    fn fit_preserves_zero_dimension_source_rules() {
        let source = empty_image((0, 2));
        assert_fit_zero_dimension_outputs(&source);
        assert_fit_zero_dimension_errors(&source);
        assert_fit_filter_ordering_and_centering(&source);
    }

    fn assert_fit_zero_dimension_outputs(source: &Image) {
        let empty = fit_with_input(source, 0, 2, None, 0.0, CenteringInput::Default)
            .expect("same-size empty-width fit is a valid copy")
            .materialize()
            .expect("empty-width fit materializes");
        assert_eq!(empty.dimensions(), (0, 2));
        assert!(empty.as_bytes().is_empty());

        let black = fit_with_input(source, 2, 2, None, 0.0, CenteringInput::Default)
            .expect("a positive target at the source height is valid")
            .materialize()
            .expect("zero-width source fit materializes");
        assert_eq!(black.dimensions(), (2, 2));
        assert_eq!(black.as_bytes(), &[0, 0, 0, 0]);
    }

    fn assert_fit_zero_dimension_errors(source: &Image) {
        for target in [(0, 1), (1, 1), (1, 0), (0, 3)] {
            let error = fit_with_input(
                source,
                target.0,
                target.1,
                None,
                0.0,
                CenteringInput::Default,
            )
            .expect_err("fit must preserve Pillow's zero-dimension error");
            match (target, error) {
                ((1, 0), PilError::ZeroDivisionError(message)) => {
                    assert_eq!(message, "division by zero")
                }
                (_, PilError::ValueError(message)) => {
                    assert_eq!(message, "height and width must be > 0")
                }
                (target, error) => panic!("unexpected fit result for {target:?}: {error:?}"),
            }
        }
    }

    fn assert_fit_filter_ordering_and_centering(source: &Image) {
        let invalid_filter = fit_with_input(
            source,
            2,
            2,
            Some(ResampleInput::Code(99)),
            0.0,
            CenteringInput::Default,
        )
        .expect_err("filter validation follows fit geometry");
        assert!(matches!(
            invalid_filter,
            PilError::ValueError(message) if message.starts_with("Unknown resampling filter (99).")
        ));

        let invalid_filter_after_zero_width = fit_with_input(
            source,
            0,
            3,
            Some(ResampleInput::Code(99)),
            0.0,
            CenteringInput::Default,
        )
        .expect_err("Image.resize parses the filter before rejecting a zero width");
        assert!(matches!(
            invalid_filter_after_zero_width,
            PilError::ValueError(message) if message.starts_with("Unknown resampling filter (99).")
        ));

        let normalized_centering = fit_with_input(
            source,
            2,
            2,
            None,
            0.0,
            CenteringInput::Values(vec![-1.0, 2.0]),
        )
        .expect("out-of-range centering falls back to the midpoint")
        .materialize()
        .expect("normalized fit materializes");
        assert_eq!(normalized_centering.dimensions(), (2, 2));
        assert_eq!(normalized_centering.as_bytes(), &[0, 0, 0, 0]);
    }

    #[test]
    fn fit_preserves_zero_height_and_filter_error_ordering() {
        let zero_height = fit_with_input(
            &empty_image((1, 0)),
            1,
            1,
            Some(ResampleInput::Code(99)),
            0.0,
            CenteringInput::Default,
        )
        .expect_err("source height division precedes filter parsing");
        assert!(matches!(
            zero_height,
            PilError::ZeroDivisionError(message) if message == "float division by zero"
        ));

        let zero_target_height = fit(
            &empty_image((2, 3)),
            1,
            0,
            Some("not-a-filter"),
            0.0,
            (0.5, 0.5),
        )
        .expect_err("target height division precedes filter parsing");
        assert!(matches!(
            zero_target_height,
            PilError::ZeroDivisionError(message) if message == "division by zero"
        ));
    }

    #[test]
    fn fit_integer_f_crop_precedes_filter_tails_for_special_values() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&f32::INFINITY.to_le_bytes());
        bytes.extend_from_slice(&(-2.25f32).to_le_bytes());
        let source = Image::frombytes("F", (1, 2), &bytes).expect("F source");
        let fitted = fit(&source, 1, 1, Some("LANCZOS"), 0.0, (1.0, 1.0))
            .expect("Fit operation")
            .materialize()
            .expect("Fit materializes");
        assert_eq!(fitted.as_bytes(), &(-2.25f32).to_le_bytes());
    }
}
