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
mod colorize;
mod equalize;
mod exif;
mod geometry;
mod point;

pub(crate) use color::{
    resolve_imageops_color, resolve_pad_palette_fill, resolve_palette_tuple_bytes,
};
pub(crate) use geometry::fit_box_coordinates;
pub use exif::{
    exif_get_orientation, exif_remove_orientation, exif_remove_xmp_orientation,
    exif_transpose, exif_transpose_with_orientation, ImageOpsExifXmpValue,
    EXIF_XMP_ATTRIBUTE_PREFIX, EXIF_XMP_ATTRIBUTE_SUFFIX, EXIF_XMP_ELEMENT_PREFIX,
    EXIF_XMP_ELEMENT_SUFFIX,
};
pub use colorize::*;
pub use equalize::*;
pub use geometry::{
    ImageOpsAspectArithmetic, ImageOpsAspectCallbacks, ImageOpsAspectComparison,
    ImageOpsAspectOperation, ImageOpsAspectSize, ImageOpsFitCallbacks,
    ImageOpsFitRouteCallbacks, ImageOpsFitConstant,
    ImageOpsBorderCallbacks, ImageOpsCropCallbacks, ImageOpsCropRouteCallbacks,
    ImageOpsExpandGeometryCallbacks,
    ImageOpsExpandCallbacks, ImageOpsPadAxis, ImageOpsPadCallbacks, ImageOpsPadRouteCallbacks,
    ImageOpsScaleCallbacks,
    ImageOpsScaleRouteCallbacks,
    ImageOpsAspectRouteCallbacks, ImageOpsNativeSizeCallbacks, ImageOpsNativeSizeProperty,
    aspect_size_with_route_callbacks,
    crop_box_with_callbacks, crop_box_with_route_callbacks,
    fit_box_with_callbacks, fit_box_with_route_callbacks,
    scale_size_with_callbacks, scale_with_route_callbacks,
    contain, contain_geometry_preflight, contain_with_input, cover, cover_geometry_preflight,
    cover_with_input, crop, crop_box_geometry_preflight, expand,
    expand_canvas_with_prepared_fill, expand_with_input,
    expand_with_margins, expand_with_margins_and_input, fit, fit_with_input, pad,
    fit_geometry_preflight, pad_fill_preflight, pad_filter_preflight,
    pad_geometry_preflight, pad_preflight_with_input, pad_with_input,
    pad_with_input_and_containment, pad_with_preflight, pad_with_preflight_and_fill,
    parse_imageops_filter_input, native_size_pair_with_callbacks,
    pad_fallback_with_callbacks, pad_with_route_callbacks,
    prepare_expand_fill_for_mode,
    expand_with_callbacks, finish_expand_fill_palette_from_host, scale_with_input,
    scale_geometry_preflight, validate_deform_resample, validate_expand_output_size_for_mode,
    validate_prepared_expand_output_size, PadFillPreflight,
    PadGeometryPreflight, PadPreflight, PreparedExpandFill,
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
    /// Integer coordinates resolved by the Python ImageOps.pad wrapper.
    ///
    /// Pillow passes these values through its native Paste coordinate parser,
    /// including signed clipping when custom numeric protocols return an
    /// out-of-range rounded value.
    #[doc(hidden)]
    PadPosition(i32, i32),
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

/// Host-neutral fill input for ImageOps geometry and Image.rotate.
#[derive(Debug, Clone)]
pub enum ImageOpsColor {
    /// No explicit color was supplied; use the operation default.
    None,
    /// A named or CSS-style color extracted from the host object.
    Name(String),
    /// A scalar color value extracted from the host object.
    Scalar(i64),
    /// A color component sequence extracted from the host object.
    Components(Vec<i64>),
    /// Integer components from a Python tuple, whose identity affects Pillow
    /// color coercion for single-band and palette images.
    TupleComponents(Vec<i64>),
    /// Mixed or arbitrary-width numeric components from a Python tuple.
    ///
    /// ImageOps.expand retains these kinds for P palette lookup and ordered
    /// `bytes(color)` validation instead of narrowing them through f64.
    TupleNumericComponents(Vec<crate::image::PaletteColorComponent>),
    /// Numeric tuple components retained for operation-specific palette paths.
    TupleComponentsWithFallback {
        /// Original numeric kinds needed by the palette path.
        components: Vec<crate::image::PaletteColorComponent>,
        /// Existing conversion behavior for fallback operation/mode paths.
        fallback: Box<Self>,
    },
    /// A Python pad tuple containing an arbitrary-width integer, with the
    /// ordinary host conversion retained for non-P mode behavior.
    WideTupleComponents {
        /// Original numeric kinds needed by Image.new's P palette path.
        components: Vec<crate::image::PaletteColorComponent>,
        /// Existing per-mode conversion for all non-P pad modes.
        fallback: Box<Self>,
    },
    /// Integer components from a Python list. Pillow accepts RGB(A) lists for
    /// mode P through `Image.new`, while other raster modes reject them.
    ListComponents(Vec<i64>),
    /// Integer components from a Python list containing an arbitrary-width
    /// value. Pillow normalizes valid P-mode lists to tuples before lookup.
    ListNumericComponents(Vec<crate::image::PaletteColorComponent>),
    /// A P-mode RGB(A) fill resolved by the host ImagePalette callback path.
    /// Carries the resulting palette index and temporary palette bytes into
    /// the native canvas strategy without losing host conversion behavior.
    #[doc(hidden)]
    PaletteFill { index: u8, palette: Vec<u8> },
    /// A scalar floating-point fill, accepted by Pillow's `F` mode.
    FloatingScalar(f64),
    /// A floating-point tuple retained for the destination mode to validate.
    FloatingComponents(Vec<f64>),
    /// A Python list whose values are not an accepted integer-list form.
    InvalidList,
    /// A value that was not a supported color representation.
    Invalid,
}

/// Host-neutral threshold input for [`solarize_with_input`].
pub enum SolarizeInput {
    /// A built-in numeric threshold, including NaN and infinities.
    Threshold(f64),
    /// Results of `sample < threshold` for samples 0 through 255.
    /// Bindings provide the host comparison callback; core fixes its sample order.
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
