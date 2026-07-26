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
//! buffers, [`Font`] values, [`PixelFormat`] modes, and [`PilError`] failures.
//!
//! # Allocation Safety
//!
//! Image allocation paths must validate dimensions through [`CheckedDims`].
//! This prevents zero-sized images, integer overflow, and accidental
//! attacker-controlled large allocations before a pixel buffer is created.
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
//! # Module Map
//!
//! - [`image`] contains the high-level image type and Pillow-style image
//!   operations.
//! - [`ops`] contains reusable image operation implementations.
//! - [`draw`] contains drawing and text rendering helpers.
//! - [`font`] contains Pillow-compatible font surfaces backed by
//!   `freetype`.
//! - [`color`] contains mode-aware color parsing and color-space conversions.
//! - [`formats`] contains codec-facing image format handlers.
//! - [`compute`] contains CPU/GPU/SIMD operation dispatch infrastructure.
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
//! # Allocating Buffers Safely
//!
//! ```
//! use pillow_rs::{CheckedDims, PixelFormat};
//!
//! let dims = CheckedDims::new(16, 16, PixelFormat::RGBA.channels()).unwrap();
//! let pixels = dims.alloc_buffer();
//! assert_eq!(pixels.len(), 16 * 16 * 4);
//! ```

// AS PER DESIGN — DO NOT REMOVE:
//   These allows are explicitly deferred. pillow-rs must eventually:
//   1. Replace unwrap() with expect() explaining invariants
//   2. Either use InfallibleExt::because() or Result propagation
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

// ============================================================================
// AS PER DESIGN — DO NOT REMOVE THESE MODULES:
//   Each module below is a foundational piece of the systemic fix architecture
//   (see SYSTEMIC_FIXES.md). Removing any of them breaks the guarantees enforced
//   by CI and creates entire classes of bugs.
//
//   - checked_dims:  Only way to allocate image buffers (no overflow, no DoS)
//   - pixel_format:  Named enum replacing bare 0/1/2/3 mode codes
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
/// Helpers for documenting and handling operations that are logically infallible.
mod infallible;
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
/// Named pixel-format metadata used instead of bare mode integers.
mod pixel_format;

pub use crate::checked_dims::CheckedDims;
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
pub use crate::font::Font;
pub use crate::font::imagingft::get_transposed_mask as font_get_transposed_mask;
pub use crate::font::imagingft::getbbox as font_getbbox;
pub use crate::font::imagingft::getlength as font_getlength;
pub use crate::font::imagingft::getmask as font_getmask;
pub use crate::font::imagingft::getmask2_with_start as font_getmask2_with_start;
pub use crate::font::imagingft::getmetrics as font_getmetrics;
pub use crate::font::imagingft::getname as font_getname;
pub use crate::font::imagingft::has_variations as font_has_variations;
pub use crate::font::pilfont::PilFont;
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
pub use crate::infallible::InfallibleExt;
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
pub use crate::pipeline::ResampleFilter;
pub use crate::pixel_format::PixelFormat;
