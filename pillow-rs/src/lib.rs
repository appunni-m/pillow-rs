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
//! expressed in Rust terms: [`Image`] values, [`image_slash_star::DynamicImage`]
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
//! - `parallel` enables approved row and pixel parallelism.
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
//   Tracked at CODEBASE_AUDIT.md Fix 2. Remove each when addressed.
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
//   (see SYSTEMIC_FIXES.md). Removing any of them breaks the guarantees enforced
//   by CI and creates entire classes of bugs.
//
//   - checked_dims:  Only way to allocate image buffers (no overflow, no DoS)
//   - par:           Approved parallelization macros (no raw rayon; native default)
//   - image_utils:   Canonical buffer conversion (no duplicate copies)
//   - compute::op_def: Declarative op registration (no parallel match arms)
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
/// Shared helpers for converting between image buffers and modes.
mod image_utils;
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
pub use crate::color::getcolor;
pub use crate::color::palette_getcolor;
pub use crate::color::palette_getcolor_append;
pub use crate::color::palette_getcolor_validate;
pub use crate::color::palette_to_text;
pub use crate::color::parse_color_str;
pub use crate::color::resolve_new_color;
pub use crate::compute::Backend;
pub use crate::compute::active_backends;
pub use crate::compute::available_backends;
pub use crate::compute::backend_enabled;
pub use crate::compute::disable_backend;
pub use crate::compute::enable_backend;
pub use crate::draw::Draw;
pub use crate::draw::outline_curve_points;
pub use crate::error::PilError;
pub use crate::font::FreeTypeFont;
pub use crate::font::ImageFont;
pub use crate::font::ImageFontLoadOptions;
pub use crate::font::ImageFontTextOptions;
pub use crate::font::ImageFontVariantOptions;
pub use crate::font::ImageFontVariationAxis;
pub use crate::font::pilfont::PilFont;
pub use crate::font::pilfont::PilFontGlyphImage;
pub use crate::font::pilfont::PilFontMask;
pub use crate::font::pilfont::PilFontMode;
pub use crate::font::transposed_bbox;
pub use crate::font::validate_transposed_length;
pub use crate::image::Image;
pub use crate::image::PaletteTransparency;
pub use crate::image::PutDataValue;
pub use crate::image::StatResult;
pub use crate::image::StatValue;
pub use crate::image::stat_from_list;
pub use crate::ops::array::ArrayLayout;
pub use crate::ops::array::resolve_array_layout;
pub use crate::ops::chops::add as chops_add;
pub use crate::ops::chops::add_modulo as chops_add_modulo;
pub use crate::ops::chops::constant as chops_constant;
pub use crate::ops::chops::darker as chops_darker;
pub use crate::ops::chops::difference as chops_difference;
pub use crate::ops::chops::hard_light as chops_hard_light;
pub use crate::ops::chops::invert as chops_invert;
pub use crate::ops::chops::lighter as chops_lighter;
pub use crate::ops::chops::logical_and as chops_logical_and;
pub use crate::ops::chops::logical_or as chops_logical_or;
pub use crate::ops::chops::logical_xor as chops_logical_xor;
pub use crate::ops::chops::multiply as chops_multiply;
pub use crate::ops::chops::offset as chops_offset;
pub use crate::ops::chops::overlay as chops_overlay;
pub use crate::ops::chops::screen as chops_screen;
pub use crate::ops::chops::soft_light as chops_soft_light;
pub use crate::ops::chops::subtract as chops_subtract;
pub use crate::ops::chops::subtract_modulo as chops_subtract_modulo;
pub use crate::ops::imageops::autocontrast as imageops_autocontrast;
pub use crate::ops::imageops::colorize as imageops_colorize;
pub use crate::ops::imageops::contain as imageops_contain;
pub use crate::ops::imageops::cover as imageops_cover;
pub use crate::ops::imageops::crop as imageops_crop;
pub use crate::ops::imageops::equalize as imageops_equalize;
pub use crate::ops::imageops::exif_get_orientation;
pub use crate::ops::imageops::exif_remove_orientation;
pub use crate::ops::imageops::expand as imageops_expand;
pub use crate::ops::imageops::fit as imageops_fit;
pub use crate::ops::imageops::flip as imageops_flip;
pub use crate::ops::imageops::grayscale as imageops_grayscale;
pub use crate::ops::imageops::invert_ops as imageops_invert;
pub use crate::ops::imageops::mirror as imageops_mirror;
pub use crate::ops::imageops::pad as imageops_pad;
pub use crate::ops::imageops::posterize as imageops_posterize;
pub use crate::ops::imageops::scale as imageops_scale;
pub use crate::ops::imageops::solarize as imageops_solarize;
pub use crate::ops::module_fns::blend as image_blend;
pub use crate::ops::module_fns::composite as image_composite;
pub use crate::ops::module_fns::effect_mandelbrot as image_effect_mandelbrot;
pub use crate::ops::module_fns::effect_noise as image_effect_noise;
pub use crate::ops::module_fns::effect_spread as image_effect_spread;
pub use crate::ops::module_fns::eval as image_eval;
pub use crate::ops::module_fns::eval_replicated_for_image as image_eval_replicated_for_image;
pub use crate::ops::module_fns::eval_validated as image_eval_validated;
pub use crate::ops::module_fns::linear_gradient as image_linear_gradient;
pub use crate::ops::module_fns::merge as image_merge;
pub use crate::ops::module_fns::radial_gradient as image_radial_gradient;
pub use crate::ops::param_filters::color3dlut_repr;
pub use crate::ops::paste::PasteSource;
pub use crate::ops::resize::parse_resample;
pub use crate::ops::utils::align_row_to_32;
pub use crate::ops::utils::flatten_pixel_list;
#[cfg(feature = "test-api")]
pub use crate::pipeline::PipelineOp;
pub use crate::pipeline::ResampleFilter;

/// Load a TrueType/OpenType face from bytes at the requested Pillow point size.
pub fn imagefont_from_bytes(data: Vec<u8>, size: f32) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::from_bytes(data, size)
}

/// Load a TrueType/OpenType face from bytes with Pillow constructor options.
pub fn imagefont_from_bytes_with_options(
    data: Vec<u8>,
    size: f32,
    options: &ImageFontLoadOptions,
) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::from_bytes_with_options(data, size, options)
}

/// Loads the same embedded default font subset as Pillow.
pub fn imagefont_load_default(size: f32) -> Result<FreeTypeFont, PilError> {
    FreeTypeFont::load_default(size)
}

/// Return the requested Pillow point size for this FreeType font.
pub fn imagefont_size(font: &FreeTypeFont) -> f32 {
    font.font_size()
}

/// Return the non-negative text mask extent for Pillow-style text layout.
pub fn imagefont_text_bbox(font: &FreeTypeFont, text: &str) -> Result<(u32, u32), PilError> {
    font.text_bbox(text)
}

/// Return the non-negative text mask extent for Pillow byte text.
pub fn imagefont_text_bbox_bytes(font: &FreeTypeFont, text: &[u8]) -> Result<(u32, u32), PilError> {
    font.text_bbox_bytes(text)
}

/// Return Pillow's public `(family, style)` font name tuple.
pub fn imagefont_getname(font: &FreeTypeFont) -> (&str, &str) {
    font.getname()
}

/// Return Pillow's raw public name tuple, preserving missing face names.
#[cfg(feature = "test-api")]
pub fn imagefont_getname_optional(font: &FreeTypeFont) -> (Option<&str>, Option<&str>) {
    font.getname_optional()
}

/// Return Pillow's public ascent/descent metrics.
pub fn imagefont_getmetrics(font: &FreeTypeFont) -> (u32, u32) {
    font.getmetrics()
}

/// Return Pillow's public text length in pixels.
pub fn imagefont_getlength(font: &FreeTypeFont, text: &str) -> Result<f32, PilError> {
    font.getlength(text)
}

/// Return Pillow native `_imagingft.Font.getlength()` 26.6 advance.
pub fn imagefont_native_getlength_26dot6(font: &FreeTypeFont, text: &str) -> Result<i32, PilError> {
    font.native_getlength_26dot6(text)
}

/// Return Pillow native `_imagingft.Font.getsize()` size and offset tuple.
pub fn imagefont_native_getsize(
    font: &FreeTypeFont,
    text: &str,
) -> Result<((i32, i32), (i32, i32)), PilError> {
    font.native_getsize(text)
}

/// Return Pillow native `_imagingft.Font.render()` mask and offset.
pub fn imagefont_native_render(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.native_render(text, options)
}

/// Return Pillow native `_imagingft.Font` public face attributes.
pub fn imagefont_native_face_attrs(
    font: &FreeTypeFont,
) -> (Option<&str>, Option<&str>, u32, u32, u32, u32, u32, i64) {
    font.native_face_attrs()
}

/// Return Pillow's public text length for byte text.
pub fn imagefont_getlength_bytes(font: &FreeTypeFont, text: &[u8]) -> Result<f32, PilError> {
    font.getlength_bytes(text)
}

/// Return Pillow's public text length with optional layout arguments.
pub fn imagefont_getlength_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    font.getlength_with_options(text, options)
}

/// Return Pillow's public text length for byte text with optional layout arguments.
pub fn imagefont_getlength_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<f32, PilError> {
    font.getlength_bytes_with_options(text, options)
}

/// Return whether the font exposes variation axes.
pub fn imagefont_has_variations(font: &FreeTypeFont) -> bool {
    font.has_variations()
}

/// Create a Pillow-compatible variant font.
pub fn imagefont_variant(font: &FreeTypeFont, size: Option<f32>) -> Result<FreeTypeFont, PilError> {
    font.font_variant(size)
}

/// Create a Pillow-compatible variant font with public override arguments.
pub fn imagefont_variant_with_options(
    font: &FreeTypeFont,
    options: &ImageFontVariantOptions,
) -> Result<FreeTypeFont, PilError> {
    font.font_variant_with_options(options)
}

/// Return Pillow's public variation-axis records.
pub fn imagefont_get_variation_axes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    font.get_variation_axes()
}

/// Return Pillow's public named-variation style names.
pub fn imagefont_get_variation_names(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    font.get_variation_names()
}

/// Set a named variation instance by Pillow-style name bytes.
pub fn imagefont_set_variation_by_name(
    font: &mut FreeTypeFont,
    name: &[u8],
) -> Result<(), PilError> {
    font.set_variation_by_name(name)
}

/// Set variation design coordinates from Pillow-style user coordinates.
pub fn imagefont_set_variation_by_axes(
    font: &mut FreeTypeFont,
    axes: &[f32],
) -> Result<(), PilError> {
    font.set_variation_by_axes(axes)
}

/// Return Pillow native `_imagingft.Font.getvaraxes()` records.
pub fn imagefont_native_getvaraxes(
    font: &FreeTypeFont,
) -> Result<Vec<ImageFontVariationAxis>, PilError> {
    font.native_getvaraxes()
}

/// Return Pillow native `_imagingft.Font.getvarnames()` records.
pub fn imagefont_native_getvarnames(font: &FreeTypeFont) -> Result<Vec<Vec<u8>>, PilError> {
    font.native_getvarnames()
}

/// Set Pillow native `_imagingft.Font` named instance index.
pub fn imagefont_native_setvarname(
    font: &mut FreeTypeFont,
    instance_index: i64,
) -> Result<(), PilError> {
    font.native_setvarname(instance_index)
}

/// Set Pillow native `_imagingft.Font` variation coordinates.
pub fn imagefont_native_setvaraxes(font: &mut FreeTypeFont, axes: &[f32]) -> Result<(), PilError> {
    font.native_setvaraxes(axes)
}

/// Return Pillow's public text bounding box.
pub fn imagefont_getbbox(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox(text)
}

/// Return Pillow's public text bounding box for byte text.
pub fn imagefont_getbbox_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox_bytes(text)
}

/// Return Pillow's public binary-mode text bounding box.
#[cfg(feature = "test-api")]
pub fn imagefont_getbbox_binary(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox_binary(text)
}

/// Return Pillow's public binary-mode text bounding box for byte text.
#[cfg(feature = "test-api")]
pub fn imagefont_getbbox_binary_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(i32, i32, i32, i32), PilError> {
    font.getbbox_binary_bytes(text)
}

/// Return Pillow's public text bounding box with optional layout arguments.
pub fn imagefont_getbbox_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    font.getbbox_with_options(text, options)
}

/// Return Pillow's public text bounding box for byte text with optional layout arguments.
pub fn imagefont_getbbox_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<(f32, f32, f32, f32), PilError> {
    font.getbbox_bytes_with_options(text, options)
}

/// Return the Pillow-compatible grayscale text mask.
pub fn imagefont_getmask(font: &FreeTypeFont, text: &str) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask(text)
}

/// Return the Pillow-compatible grayscale text mask for byte text.
pub fn imagefont_getmask_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_bytes(text)
}

/// Render a Pillow-compatible mask with offset.
pub fn imagefont_getmask2(
    font: &FreeTypeFont,
    text: &str,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.getmask2(text)
}

/// Render a Pillow-compatible mask with offset for byte text.
pub fn imagefont_getmask2_bytes(
    font: &FreeTypeFont,
    text: &[u8],
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.getmask2_bytes(text)
}

/// Return the Pillow-compatible grayscale text mask with optional render arguments.
pub fn imagefont_getmask_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_with_options(text, options)
}

/// Return the Pillow-compatible grayscale text mask for byte text with optional render arguments.
pub fn imagefont_getmask_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.getmask_bytes_with_options(text, options)
}

/// Render a Pillow-compatible mask with a fractional raster start.
pub fn imagefont_getmask2_with_start(
    font: &FreeTypeFont,
    text: &str,
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.getmask2_with_start(text, start)
}

/// Render a Pillow-compatible byte-text mask with a fractional raster start.
pub fn imagefont_getmask2_bytes_with_start(
    font: &FreeTypeFont,
    text: &[u8],
    start: (f64, f64),
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.getmask2_bytes_with_start(text, start)
}

/// Render a Pillow-compatible mask with optional render arguments.
pub fn imagefont_getmask2_with_options(
    font: &FreeTypeFont,
    text: &str,
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.getmask2_with_options(text, options)
}

/// Render a Pillow-compatible byte-text mask with optional render arguments.
pub fn imagefont_getmask2_bytes_with_options(
    font: &FreeTypeFont,
    text: &[u8],
    options: &ImageFontTextOptions,
) -> Result<(u32, u32, Vec<u8>, (i32, i32)), PilError> {
    font.getmask2_bytes_with_options(text, options)
}

/// Render a font mask and apply Pillow's optional transpose operation.
pub fn imagefont_get_transposed_mask(
    font: &FreeTypeFont,
    text: &str,
    orientation: Option<&str>,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.get_transposed_mask(text, orientation)
}

/// Return Pillow-compatible binary-mode RGBA text rendering.
#[cfg(feature = "test-api")]
pub fn imagefont_render_text_binary(
    font: &FreeTypeFont,
    text: &str,
    fill: (u8, u8, u8, u8),
    spacing: f32,
) -> Result<(u32, u32, Vec<u8>), PilError> {
    font.render_text_binary(text, fill, spacing)
}

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
                    gpu_shader: entry.gpu_shader,
                    gpu_source: entry.gpu_source.map(|_| ()),
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
    crate::compute::registry::gpu_supports(op)
}

/// Returns whether a pipeline operation maps to a GPU operation descriptor.
#[cfg(feature = "test-api")]
pub fn backend_map_op_to_gpu(op: &PipelineOp) -> Result<bool, PilError> {
    Ok(crate::compute::registry::map_op_to_gpu(op)?.is_some())
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
        .collect::<Vec<_>>();

    let cpu_without_simd_json = cpu_without_simd
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
        cpu = cpu,
        simd_pool = simd_pool,
        gpu_shader = gpu_shader,
        cpu_without_simd_len = cpu_without_simd.len(),
        cpu_without_simd_json = cpu_without_simd_json,
        operations_json = operations_json
    ))
}
