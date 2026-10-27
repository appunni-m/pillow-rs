//! Pure Rust image processing with a Pillow-compatible core API.
//!
//! `pillow-rs` is the runtime-independent image crate used by the Python and
//! WebAssembly bindings in this workspace. It takes Rust primitives, image
//! buffers, mode strings, and font bytes as input; binding crates are
//! responsible for translating Python objects, JavaScript objects, file paths,
//! and host I/O before calling into this crate.
//!
//! # Design Boundary
//!
//! Core modules do not depend on Python or JavaScript runtime types. Public
//! functions model Pillow behavior where practical, but their contracts are
//! expressed in Rust terms: [`Image`] values, [`crate::raster::DynamicImage`]
//! buffers, [`FreeTypeFont`] values, and [`PilError`] failures.
//!
//! # Allocation Safety
//!
//! Image allocation paths validate dimensions before allocating. This prevents
//! zero-sized images, integer overflow, and accidental attacker-controlled large
//! allocations before a pixel buffer is created.
//!
//! # Modes And Layout
//!
//! Public image APIs use Pillow mode strings such as `"L"`, `"RGB"`, `"RGBA"`,
//! `"CMYK"`, `"P"`, and `"1"`. Core operations normalize those modes into Rust
//! image buffers before dispatching to CPU, SIMD, or GPU backends. Raw byte APIs
//! return tightly packed pixels in the image's current mode unless a method says
//! otherwise.
//!
//! # Feature Flags
//!
//! - `gpu` enables the wgpu/WebGPU backend where an operation has a shader.
//! - `parallel` opt-in enables Rayon-backed Parallel CPU row and pixel paths;
//!   SIMD adapters and GPU execution stay thread-scheduler independent.
//! - `image-codecs-all` enables all codec features exposed through
//!   `image-slash-star`; individual codec features can be selected instead.
//!
//! CPU execution remains the fallback path. Feature flags should not change the
//! public Pillow-style contract of an operation.
//!
//! # Public API Boundary
//!
//! `pillow-rs/src/lib.rs` is the single public API definition for this crate.
//! Implementation modules are private crate internals; downstream callers,
//! including the Python and WebAssembly bindings, must use the exact root
//! symbols re-exported below such as [`Image`], [`FreeTypeFont`], [`Draw`],
//! [`imageops_grayscale`], and [`imagefont_getbbox`].
//!
//! # Quick Start
//!
//! ```
//! use pillow_rs::Image;
//!
//! let image = Image::new(4, 4, "RGB", (0, 128, 255, 255)).unwrap();
//! assert_eq!(image.size().unwrap(), (4, 4));
//! assert_eq!(image.mode().unwrap(), "RGB");
//! ```
//!
//! # Converting Modes
//!
//! ```
//! use pillow_rs::Image;
//!
//! let rgb = Image::new(2, 2, "RGB", (255, 0, 0, 255)).unwrap();
//! let gray = rgb.convert("L", None, None, None, None).unwrap();
//! assert_eq!(gray.mode().unwrap(), "L");
//! assert_eq!(gray.tobytes().unwrap().len(), 4);
//! ```
//!
// AS PER DESIGN — DO NOT REMOVE:
//   These allows are explicitly deferred. pillow-rs must eventually:
//   1. Replace unwrap() with expect() explaining invariants
//   2. Prefer Result propagation for recoverable failures
//   3. Replace .map().unwrap_or() with .map_or()
//   4. Remove redundant clones
//   5. Replace manual loop indexing with iter()/enumerate()
//   Tracked at docs/ARCHITECTURE.md (existing Clippy allowances). Remove each when addressed.
// The `image` dev dependency is consumed by the comparison benchmark, not this library target.
#![allow(unused_crate_dependencies)]
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::needless_range_loop)]
// PIL API compatibility — many functions have 8+ parameters matching PIL signatures
#![allow(clippy::too_many_arguments)]

#[cfg(feature = "parallel")]
use rayon as _;
#[cfg(feature = "test-api")]
use std::collections::BTreeMap;

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE THESE MODULES:
//   Each module below is a foundational piece of the systemic fix architecture
//   (see docs/ARCHITECTURE.md). Removing any of them breaks the guarantees enforced
//   by CI and creates entire classes of bugs.
//
//   - checked_dims:  Only way to allocate image buffers (no overflow, no DoS)
//   - par:           Approved Rayon row helpers (opt-in through `parallel`)
//   - image_utils:   Canonical buffer conversion (no duplicate copies)
// ============================================================================

/// Validated image dimensions and allocation-size helpers.
mod checked_dims;
/// Pillow-compatible color parsing and color-space conversion.
mod color;
/// CPU/GPU/SIMD operation registration and dispatch infrastructure.
mod compute;
/// Pillow-compatible drawing primitives and text rendering.
mod draw;
/// Error types shared by core image, drawing, color, and format operations.
mod error;
/// Pillow-compatible font loading and `_imagingft` adapter behavior.
mod font;
/// Lightweight image format identifiers.
mod format;
/// High-level image object and Pillow-style image methods.
mod image;
/// Rust-owned state for Pillow's host-neutral image-sequence iterator.
mod image_sequence;
/// Shared helpers for converting between image buffers and modes.
mod image_utils;
/// Exact Pillow-compatible RGB-to-LAB byte conversion.
mod lab;
/// Reusable image operation implementations.
mod ops;
#[cfg(feature = "parallel")]
/// Approved parallel iteration helpers for pixel and row operations.
mod par;
/// Declarative operation pipeline types used by compute backends.
///
/// See [`pipeline::PipelineOp`] for the operation descriptor carried through
/// lazy image pipelines.
mod pipeline;
/// Pillow-owned materialized pixel storage and raster primitives.
mod raster;

/// Test-build-only controls for deterministic target fault contracts.
#[cfg(feature = "test-api")]
pub mod test_api {
    /// Arm the allow-listed deterministic GPU batch fault for the next executor.
    pub fn arm_fault(point: &str) -> Result<(), String> {
        match point {
            "gpu_batch.admission.device_failure" => {
                #[cfg(feature = "gpu")]
                {
                    crate::compute::arm_gpu_batch_admission_failure();
                    Ok(())
                }
                #[cfg(not(feature = "gpu"))]
                {
                    Err("GPU batch fault requires the gpu feature".into())
                }
            }
            _ => Err(format!("unknown migration parity fault point: {point}")),
        }
    }

    /// Clear an armed fault before the next fixture-driven workflow.
    pub fn clear_fault() {
        #[cfg(feature = "gpu")]
        crate::compute::clear_gpu_batch_test_fault();
    }
}

pub use crate::color::ColorValue;
pub use crate::color::PaletteColorInput;
pub use crate::color::color_has_explicit_alpha;
pub use crate::color::getcolor;
pub use crate::color::palette_getcolor;
pub use crate::color::palette_getcolor_append;
pub use crate::color::palette_getcolor_validate;
pub use crate::color::palette_getcolor_validate_input;
pub use crate::color::palette_to_text;
pub use crate::color::parse_color_str;
pub use crate::color::parse_color_str_unclamped;
pub use crate::color::resolve_new_color;
pub use crate::compute::Backend;
#[cfg(feature = "gpu")]
pub use crate::compute::BatchError;
#[cfg(feature = "gpu")]
pub use crate::compute::BatchErrorKind;
#[cfg(feature = "gpu")]
pub use crate::compute::BatchKey;
#[cfg(feature = "gpu")]
pub use crate::compute::BatchOutput;
#[cfg(feature = "gpu")]
pub use crate::compute::BatchResult;
#[cfg(feature = "gpu")]
pub use crate::compute::GpuBatchConfig;
#[cfg(feature = "gpu")]
pub use crate::compute::GpuBatchExecutor;
#[cfg(feature = "gpu")]
pub use crate::compute::GpuBatchStats;
#[cfg(feature = "gpu")]
pub use crate::compute::GpuImage;
pub use crate::compute::PipelineOperationTelemetry;
pub use crate::compute::PipelineResourceTelemetry;
#[doc(hidden)]
pub use crate::compute::PipelineTelemetryOrigin;
pub use crate::compute::PipelineTelemetrySample;
pub use crate::compute::active_backends;
pub use crate::compute::available_backends;
pub use crate::compute::backend_enabled;
pub use crate::compute::disable_backend;
pub use crate::compute::enable_backend;
pub use crate::draw::Draw;
pub use crate::draw::DrawBoxInput;
pub use crate::draw::DrawCircleCenterInput;
pub use crate::draw::DrawColorInput;
pub use crate::draw::DrawPointsInput;
pub use crate::draw::RegularPolygonCircle;
pub use crate::draw::RegularPolygonSides;
pub use crate::draw::normalize_draw_box;
pub use crate::draw::normalize_draw_circle_center;
pub use crate::draw::normalize_draw_width;
pub use crate::draw::normalize_rounded_rectangle_box;
pub use crate::draw::outline_curve_points;
pub use crate::error::PilError;
pub use crate::font::FontLengthAndBounds;
pub use crate::font::FontMaskAndBounds;
pub use crate::font::FontMaskAndOffset;
pub use crate::font::FontSizeAndOffset;
pub use crate::font::FontTextBounds;
pub use crate::font::FreeTypeFont;
pub use crate::font::ImageFontBBoxValue;
pub use crate::font::ImageFontFaceInfo;
pub use crate::font::ImageFontLoadOptions;
pub use crate::font::ImageFontSourceInput;
pub use crate::font::ImageFontTextInput;
pub use crate::font::ImageFontTextOptions;
pub use crate::font::ImageFontVariantOptions;
pub use crate::font::ImageFontVariationAxesInput;
pub use crate::font::ImageFontVariationAxis;
pub use crate::font::ImageFontVariationNameInput;
pub use crate::font::mask_image as imagefont_mask_image;
pub use crate::font::normalize_font_bbox;
pub use crate::font::normalize_layout_engine;
pub use crate::font::pilfont::PilFont;
pub use crate::font::pilfont::PilFontGlyphImage;
pub use crate::font::pilfont::PilFontMask;
pub use crate::font::pilfont::PilFontMode;
pub use crate::font::pilfont::PilFontTextInput;
pub use crate::font::transposed_bbox;
pub use crate::font::validate_transposed_length;
pub use crate::image::ChannelSelector;
pub use crate::image::ExifCompatFields;
pub use crate::image::FormattedExtrema;
pub use crate::image::FormattedImageData;
pub use crate::image::FormattedPixelValue;
pub use crate::image::Image;
pub use crate::image::ImageInfoValue;
pub use crate::image::ImagingCoreBytesInput;
pub use crate::image::NewColorInput;
pub use crate::image::OpenFormatsInput;
pub use crate::image::OpenModeInput;
pub use crate::image::PaletteTransparency;
pub use crate::image::PutAlphaInput;
pub use crate::image::PutDataInput;
pub use crate::image::PutDataValue;
pub use crate::image::PutDataValueKind;
pub use crate::image::PutPixelValue;
// Preserve the original public Rust paths after adopting host-neutral names.
// Remove these aliases only as part of an explicit breaking-release decision.
#[doc(hidden)]
pub use crate::image::NewColorInput as PythonNewColorInput;
#[doc(hidden)]
pub use crate::image::OpenFormatsInput as PythonOpenFormatsInput;
#[doc(hidden)]
pub use crate::image::OpenModeInput as PythonOpenModeInput;
pub use crate::image::StatResult;
pub use crate::image::StatValue;
pub use crate::image::imaging_core_to_bytes;
pub use crate::image::prepare_exif_compat;
pub use crate::image::stat_from_histogram;
pub use crate::image::stat_from_list;
pub use crate::image::validate_open_inputs;
#[doc(hidden)]
pub use crate::image::validate_open_inputs as validate_python_open_inputs;
pub use crate::image::validate_open_source_bytes;
#[doc(hidden)]
pub use crate::image::validate_open_source_bytes as validate_python_open_source_bytes;
pub use crate::image_sequence::ImageSequenceIterator;
pub use crate::ops::analysis::ImageAnalysisMask;
pub use crate::ops::analysis::validate_transparency_mask;
pub use crate::ops::array::ArrayLayout;
pub use crate::ops::array::from_resolved_array_interface;
pub use crate::ops::array::resolve_array_layout;
pub use crate::ops::chops::add as chops_add;
pub use crate::ops::chops::add_modulo as chops_add_modulo;
pub use crate::ops::chops::constant as chops_constant;
pub use crate::ops::chops::darker as chops_darker;
pub use crate::ops::chops::difference as chops_difference;
pub use crate::ops::chops::duplicate as chops_duplicate;
pub use crate::ops::chops::hard_light as chops_hard_light;
pub use crate::ops::chops::invert as chops_invert;
pub use crate::ops::chops::lighter as chops_lighter;
pub use crate::ops::chops::logical_and as chops_logical_and;
pub use crate::ops::chops::logical_or as chops_logical_or;
pub use crate::ops::chops::logical_xor as chops_logical_xor;
pub use crate::ops::chops::multiply as chops_multiply;
pub use crate::ops::chops::offset as chops_offset;
pub use crate::ops::chops::offset_with_default as chops_offset_with_default;
pub use crate::ops::chops::overlay as chops_overlay;
pub use crate::ops::chops::screen as chops_screen;
pub use crate::ops::chops::soft_light as chops_soft_light;
pub use crate::ops::chops::subtract as chops_subtract;
pub use crate::ops::chops::subtract_modulo as chops_subtract_modulo;
pub use crate::ops::convert::ConvertModeInput;
#[doc(hidden)]
pub use crate::ops::convert::ConvertModeInput as PythonConvertModeInput;
pub use crate::ops::convert::ConvertPaletteInput;
#[doc(hidden)]
pub use crate::ops::convert::ConvertPaletteInput as PythonConvertPaletteInput;
pub use crate::ops::convert::DitherInput;
#[doc(hidden)]
pub use crate::ops::convert::DitherInput as PythonDitherInput;
pub use crate::ops::convert::normalize_convert_dither;
#[doc(hidden)]
pub use crate::ops::convert::normalize_convert_dither as normalize_python_convert_dither;
pub use crate::ops::convert::validate_convert_dither;
#[doc(hidden)]
pub use crate::ops::convert::validate_convert_dither as validate_python_convert_dither;
pub use crate::ops::filter::prepare_kernel;
pub use crate::ops::filter::validate_kernel_coefficients;
pub use crate::ops::imageops::CenteringInput;
pub use crate::ops::imageops::ImageOpsColor;
pub use crate::ops::imageops::ImageOpsMask;
pub use crate::ops::imageops::SolarizeInput;
pub use crate::ops::imageops::autocontrast as imageops_autocontrast;
pub use crate::ops::imageops::autocontrast_with_mask as imageops_autocontrast_with_mask;
pub use crate::ops::imageops::colorize as imageops_colorize;
pub use crate::ops::imageops::contain as imageops_contain;
pub use crate::ops::imageops::contain_with_input as imageops_contain_with_input;
pub use crate::ops::imageops::cover as imageops_cover;
pub use crate::ops::imageops::cover_with_input as imageops_cover_with_input;
pub use crate::ops::imageops::crop as imageops_crop;
pub use crate::ops::imageops::equalize as imageops_equalize;
pub use crate::ops::imageops::equalize_with_mask as imageops_equalize_with_mask;
pub use crate::ops::imageops::exif_get_orientation;
pub use crate::ops::imageops::exif_remove_orientation;
pub use crate::ops::imageops::exif_transpose as imageops_exif_transpose;
pub use crate::ops::imageops::expand as imageops_expand;
pub use crate::ops::imageops::expand_with_input as imageops_expand_with_input;
pub use crate::ops::imageops::fit as imageops_fit;
pub use crate::ops::imageops::fit_with_input as imageops_fit_with_input;
pub use crate::ops::imageops::flip as imageops_flip;
pub use crate::ops::imageops::grayscale as imageops_grayscale;
pub use crate::ops::imageops::invert_ops as imageops_invert;
pub use crate::ops::imageops::mirror as imageops_mirror;
pub use crate::ops::imageops::pad as imageops_pad;
pub use crate::ops::imageops::pad_with_input as imageops_pad_with_input;
#[doc(hidden)]
pub use crate::ops::imageops::pad_with_input_and_containment as imageops_pad_with_input_and_containment;
pub use crate::ops::imageops::posterize as imageops_posterize;
pub use crate::ops::imageops::scale_with_input as imageops_scale_with_input;
pub use crate::ops::imageops::solarize as imageops_solarize;
pub use crate::ops::imageops::solarize_with_input as imageops_solarize_with_input;
pub use crate::ops::imageops::validate_colorize_mode as imageops_validate_colorize_mode;
pub use crate::ops::imageops::validate_deform_resample as imageops_validate_deform_resample;
pub use crate::ops::module_fns::EvalInputKind;
pub use crate::ops::module_fns::MergeInput;
pub use crate::ops::module_fns::alpha_composite as image_alpha_composite;
pub use crate::ops::module_fns::blend as image_blend;
pub use crate::ops::module_fns::composite as image_composite;
pub use crate::ops::module_fns::effect_mandelbrot as image_effect_mandelbrot;
pub use crate::ops::module_fns::effect_mandelbrot_with_extent as image_effect_mandelbrot_with_extent;
pub use crate::ops::module_fns::effect_noise as image_effect_noise;
pub use crate::ops::module_fns::effect_noise_from_size as image_effect_noise_from_size;
pub use crate::ops::module_fns::effect_spread as image_effect_spread;
pub use crate::ops::module_fns::eval as image_eval;
pub use crate::ops::module_fns::eval_callable as image_eval_callable;
pub use crate::ops::module_fns::eval_float as image_eval_float;
pub use crate::ops::module_fns::eval_point_transform as image_eval_point_transform;
pub use crate::ops::module_fns::eval_replicated_for_image as image_eval_replicated_for_image;
pub use crate::ops::module_fns::eval_validated as image_eval_validated;
pub use crate::ops::module_fns::frombytes as image_frombytes;
pub use crate::ops::module_fns::frombytes_owned as image_frombytes_owned;
pub use crate::ops::module_fns::linear_gradient as image_linear_gradient;
#[doc(hidden)]
pub use crate::ops::module_fns::make_float_lut as image_eval_float_lut;
pub use crate::ops::module_fns::make_lut;
pub use crate::ops::module_fns::merge as image_merge;
pub use crate::ops::module_fns::merge_inputs as image_merge_inputs;
pub use crate::ops::module_fns::radial_gradient as image_radial_gradient;
pub use crate::ops::module_fns::validate_eval_input;
pub use crate::ops::param_filters::Color3DLutTable;
pub use crate::ops::param_filters::PreparedColor3DLut;
pub use crate::ops::param_filters::color3dlut_check_size;
pub use crate::ops::param_filters::color3dlut_generate_table;
pub use crate::ops::param_filters::color3dlut_prepare_table;
pub use crate::ops::param_filters::color3dlut_repr;
pub use crate::ops::param_filters::color3dlut_transform_table;
pub use crate::ops::param_filters::prepare_color3dlut;
pub use crate::ops::paste::AlphaCompositeBox;
pub use crate::ops::paste::PasteBoxInput;
#[doc(hidden)]
pub use crate::ops::paste::PasteBoxInput as PythonPasteBox;
pub use crate::ops::paste::PasteMaskInput;
#[doc(hidden)]
pub use crate::ops::paste::PasteMaskInput as PythonPasteMask;
pub use crate::ops::paste::PasteSource;
pub use crate::ops::paste::PasteSourceInput;
#[doc(hidden)]
pub use crate::ops::paste::PasteSourceInput as PythonPasteSource;
pub use crate::ops::quantize::QuantizePalette;
pub use crate::ops::resize::ResampleInput;
pub use crate::ops::resize::parse_resample;
pub use crate::ops::resize::parse_resample_input;
pub use crate::ops::rotate::RotateExpandInput;
pub use crate::ops::rotate::RotatePointInput;
pub use crate::ops::rotate::RotateResampleInput;
pub use crate::ops::rotate::normalize_rotate;
#[doc(hidden)]
pub use crate::ops::rotate::normalize_rotate as normalize_python_rotate;
pub use crate::ops::transform::ReduceBox;
pub use crate::ops::transform::ReduceFactor;
pub use crate::ops::transform::TransformData;
pub use crate::ops::transform::TransformFill;
pub use crate::ops::transpose::TransposeInput;
pub use crate::ops::transpose::normalize_transpose_input;
pub use crate::ops::transpose::normalize_transposed_font_input;
pub use crate::ops::transpose::transpose_name_from_int;
pub use crate::ops::utils::align_row_to_32;
pub use crate::pipeline::ColorMode;
pub use crate::pipeline::DitherMethod;
pub use crate::pipeline::PipelineOp;
pub use crate::pipeline::PixelMode;
pub use crate::pipeline::ResampleFilter;
pub use crate::pipeline::TransformMethod;
pub use crate::pipeline::TransposeMethod;
pub use crate::raster::ConvertBuffer;
pub use crate::raster::DynamicImage;
pub use crate::raster::EncodableLayout;
pub use crate::raster::Enlargeable;
pub use crate::raster::EnumeratePixels;
pub use crate::raster::EnumeratePixelsMut;
pub use crate::raster::EnumerateRows;
pub use crate::raster::EnumerateRowsMut;
pub use crate::raster::ExtendedColorType;
pub use crate::raster::FromColor;
pub use crate::raster::FromPrimitive;
pub use crate::raster::GenericImage;
pub use crate::raster::GenericImageView;
pub use crate::raster::GrayAlphaImage;
pub use crate::raster::GrayImage;
pub use crate::raster::ImageBuffer;
pub use crate::raster::Luma;
pub use crate::raster::LumaA;
pub use crate::raster::Pixel;
pub use crate::raster::Pixels;
pub use crate::raster::PixelsMut;
pub use crate::raster::Primitive;
pub use crate::raster::Rect;
pub use crate::raster::Rgb;
pub use crate::raster::Rgb32FImage;
pub use crate::raster::RgbImage;
pub use crate::raster::Rgba;
pub use crate::raster::Rgba32FImage;
pub use crate::raster::RgbaImage;
pub use crate::raster::Rows;
pub use crate::raster::RowsMut;

pub use crate::font::public_api::imagefont_from_bytes;

pub use crate::font::public_api::imagefont_from_bytes_with_options;

pub use crate::font::public_api::imagefont_from_source;

pub use crate::font::public_api::imagefont_load_default;

pub use crate::font::public_api::imagefont_size;

pub use crate::font::public_api::imagefont_text_bbox;

pub use crate::font::public_api::imagefont_text_bbox_bytes;

pub use crate::font::public_api::imagefont_getname;

pub use crate::font::public_api::imagefont_getname_optional;

pub use crate::font::public_api::imagefont_getmetrics;

pub use crate::font::public_api::imagefont_getlength;

pub use crate::font::public_api::imagefont_native_getlength_26dot6;

pub use crate::font::public_api::imagefont_native_getsize;

pub use crate::font::public_api::imagefont_native_render;

pub use crate::font::public_api::imagefont_native_render_input;

pub use crate::font::public_api::imagefont_native_face_attrs;

pub use crate::font::public_api::imagefont_native_face_info;

pub use crate::font::public_api::imagefont_font_format;

pub use crate::font::public_api::imagefont_is_scalable;

pub use crate::font::public_api::imagefont_getlength_bytes;

pub use crate::font::public_api::imagefont_getlength_with_options;

pub use crate::font::public_api::imagefont_getlength_bytes_with_options;

pub use crate::font::public_api::imagefont_has_variations;

pub use crate::font::public_api::imagefont_variant;

pub use crate::font::public_api::imagefont_variant_with_options;

pub use crate::font::public_api::imagefont_get_variation_axes;

pub use crate::font::public_api::imagefont_get_variation_names;

pub use crate::font::public_api::imagefont_set_variation_by_name;

pub use crate::font::public_api::imagefont_set_variation_by_name_input;

pub use crate::font::public_api::imagefont_set_variation_by_axes;

pub use crate::font::public_api::imagefont_set_variation_by_axes_input;

pub use crate::font::public_api::imagefont_native_getvaraxes;

pub use crate::font::public_api::imagefont_native_getvarnames;

pub use crate::font::public_api::imagefont_native_setvarname;

pub use crate::font::public_api::imagefont_native_setvaraxes;

pub use crate::font::public_api::imagefont_getbbox;

pub use crate::font::public_api::imagefont_getbbox_bytes;

#[cfg(feature = "test-api")]
pub use crate::font::public_api::imagefont_getbbox_binary;

#[cfg(feature = "test-api")]
pub use crate::font::public_api::imagefont_getbbox_binary_bytes;

pub use crate::font::public_api::imagefont_getbbox_with_options;

pub use crate::font::public_api::imagefont_getbbox_with_optional_font;

pub use crate::font::public_api::imagefont_textbbox_at_with_optional_font;

pub use crate::font::public_api::imagefont_textbbox_at_with_optional_font_input;

pub use crate::font::public_api::imagefont_getlength_with_optional_font;

pub use crate::font::public_api::imagefont_getlength_with_optional_font_input;

pub use crate::font::public_api::imagefont_multiline_textbbox;

pub use crate::font::public_api::imagefont_multiline_textbbox_with_optional_font;

pub use crate::font::public_api::imagefont_multiline_textbbox_with_optional_font_input;

pub use crate::font::public_api::imagefont_getbbox_bytes_with_options;

pub use crate::font::public_api::imagefont_getbbox_input_with_options;

pub use crate::font::public_api::imagefont_getmask;

pub use crate::font::public_api::imagefont_getmask_bytes;

pub use crate::font::public_api::imagefont_getmask2;

pub use crate::font::public_api::imagefont_getmask2_bytes;

pub use crate::font::public_api::imagefont_getmask_with_options;

pub use crate::font::public_api::imagefont_getmask_bytes_with_options;

pub use crate::font::public_api::imagefont_getmask_input_with_options;

pub use crate::font::public_api::imagefont_getmask2_with_start;

pub use crate::font::public_api::imagefont_getmask2_bytes_with_start;

pub use crate::font::public_api::imagefont_getmask2_with_options;

pub use crate::font::public_api::imagefont_getmask2_bytes_with_options;

pub use crate::font::public_api::imagefont_getmask2_input_with_options;

pub use crate::font::public_api::imagefont_getlength_input_with_options;

pub use crate::font::public_api::imagefont_get_transposed_mask;

pub use crate::font::public_api::imagefont_get_transposed_mask_input;

#[cfg(feature = "test-api")]
pub use crate::font::public_api::imagefont_render_text_binary;

/// Public backend capability summary for one registered operation.
#[cfg(feature = "test-api")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackendSupportEntry {
    /// CPU implementation marker.
    pub cpu_fn: Option<()>,
    /// SIMD-pool implementation marker.
    pub simd_fn: Option<()>,
    /// GPU shader name when the operation has a shader.
    pub gpu_shader: Option<&'static str>,
    /// GPU shader source marker.
    pub gpu_source: Option<()>,
}

/// Returns the backend capability registry as explicit public summary data.
#[cfg(feature = "test-api")]
pub fn backend_support_registry() -> Result<BTreeMap<&'static str, BackendSupportEntry>, PilError> {
    Ok(crate::compute::registry::registry()?
        .iter()
        .map(|(name, entry)| {
            (
                *name,
                BackendSupportEntry {
                    cpu_fn: entry.cpu_fn.map(|_| ()),
                    simd_fn: entry.simd_fn.map(|_| ()),
                    gpu_shader: {
                        #[cfg(feature = "gpu")]
                        {
                            entry.gpu_shader
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            None
                        }
                    },
                    gpu_source: {
                        #[cfg(feature = "gpu")]
                        {
                            entry.gpu_source.map(|_| ())
                        }
                        #[cfg(not(feature = "gpu"))]
                        {
                            None
                        }
                    },
                },
            )
        })
        .collect())
}

/// Returns whether CPU declares support for a pipeline operation.
#[cfg(feature = "test-api")]
pub fn backend_cpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    crate::compute::registry::cpu_supports(op)
}

/// Returns whether SIMD declares support for a pipeline operation.
#[cfg(feature = "test-api")]
pub fn backend_simd_supports(op: &PipelineOp) -> Result<bool, PilError> {
    crate::compute::registry::simd_supports(op)
}

/// Returns whether GPU declares support for a pipeline operation.
#[cfg(feature = "test-api")]
pub fn backend_gpu_supports(op: &PipelineOp) -> Result<bool, PilError> {
    #[cfg(feature = "gpu")]
    {
        crate::compute::registry::gpu_supports(op)
    }
    #[cfg(not(feature = "gpu"))]
    {
        let _ = op;
        Ok(false)
    }
}

/// Returns the backend support matrix as deterministic pretty JSON.
#[cfg(feature = "test-api")]
pub fn backend_support_matrix_json() -> Result<String, PilError> {
    let registry = backend_support_registry()?;
    let operations = registry
        .iter()
        .map(|(name, entry)| {
            format!(
                "{{\"operation\":\"{name}\",\"cpu\":{},\"simd_pool\":{},\"gpu_shader\":{}}}",
                entry.cpu_fn.is_some(),
                entry.simd_fn.is_some(),
                entry.gpu_shader.is_some()
            )
        })
        .collect::<Vec<_>>();
    let counts = backend_support_matrix_counts(&registry);

    let cpu_without_simd_json = counts
        .cpu_without_simd
        .iter()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let operations_json = operations
        .iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>()
        .join(",\n    ");

    Ok(format!(
        concat!(
            "{{\n",
            "  \"totals\": {{\n",
            "    \"operations\": {operations},\n",
            "    \"cpu\": {cpu},\n",
            "    \"simd_pool\": {simd_pool},\n",
            "    \"gpu_shader\": {gpu_shader},\n",
            "    \"cpu_without_simd\": {cpu_without_simd_len}\n",
            "  }},\n",
            "  \"cpu_without_simd\": [{cpu_without_simd_json}],\n",
            "  \"operations\": [\n",
            "    {operations_json}\n",
            "  ]\n",
            "}}"
        ),
        operations = registry.len(),
        cpu = counts.cpu,
        simd_pool = counts.simd_pool,
        gpu_shader = counts.gpu_shader,
        cpu_without_simd_len = counts.cpu_without_simd.len(),
        cpu_without_simd_json = cpu_without_simd_json,
        operations_json = operations_json
    ))
}

#[cfg(feature = "test-api")]
struct BackendSupportMatrixCounts {
    cpu: usize,
    simd_pool: usize,
    gpu_shader: usize,
    cpu_without_simd: Vec<&'static str>,
}

#[cfg(feature = "test-api")]
fn backend_support_matrix_counts(
    registry: &BTreeMap<&'static str, BackendSupportEntry>,
) -> BackendSupportMatrixCounts {
    let cpu = registry
        .values()
        .filter(|entry| entry.cpu_fn.is_some())
        .count();
    let simd_pool = registry
        .values()
        .filter(|entry| entry.simd_fn.is_some())
        .count();
    let gpu_shader = registry
        .values()
        .filter(|entry| entry.gpu_shader.is_some())
        .count();
    let cpu_without_simd = registry
        .iter()
        .filter_map(|(name, entry)| {
            (entry.cpu_fn.is_some() && entry.simd_fn.is_none()).then_some(*name)
        })
        .collect();

    BackendSupportMatrixCounts {
        cpu,
        simd_pool,
        gpu_shader,
        cpu_without_simd,
    }
}
