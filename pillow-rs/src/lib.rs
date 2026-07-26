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
pub mod checked_dims;
/// Pillow-compatible color parsing and color-space conversion.
pub mod color;
/// CPU/GPU/SIMD operation registration and dispatch infrastructure.
pub mod compute;
/// Pillow-compatible drawing primitives and text rendering.
pub mod draw;
/// Error types shared by core image, drawing, color, and format operations.
pub mod error;
/// Pillow-compatible font loading and `_imagingft` adapter behavior.
pub mod font;
/// Lightweight image format identifiers.
pub mod format;
/// High-level image object and Pillow-style image methods.
pub mod image;
/// Shared helpers for converting between image buffers and modes.
pub mod image_utils;
/// Helpers for documenting and handling operations that are logically infallible.
pub mod infallible;
/// Reusable image operation implementations.
pub mod ops;
#[cfg(feature = "parallel")]
/// Approved parallel iteration helpers for pixel and row operations.
pub mod par;
/// Declarative operation pipeline types used by compute backends.
///
/// See [`pipeline::PipelineOp`] for the operation descriptor carried through
/// lazy image pipelines.
pub mod pipeline;
/// Named pixel-format metadata used instead of bare mode integers.
pub mod pixel_format;

pub use crate::checked_dims::CheckedDims;
pub use crate::image::Image;
pub use draw::Draw;
pub use error::PilError;
pub use font::Font;
pub use infallible::InfallibleExt;
pub use pixel_format::PixelFormat;
