//! Pillow-compatible image operation implementations.
//!
//! Submodules contain reusable core logic for `Image` methods and module-level
//! Pillow functions. Binding crates should call the high-level APIs rather than
//! reaching into these modules directly.

/// Analysis and statistics operations.
pub mod analysis;
/// Array-interface descriptor resolution.
pub mod array;
/// Pillow `ImageChops`-style channel arithmetic operations.
pub mod chops;
/// Pillow mode conversion operations.
pub mod convert;
/// Cropping and rectangular extraction operations.
pub mod crop;
/// Brightness, contrast, color, and sharpness enhancement operations.
pub mod enhance;
/// Image filter operations and convolution helpers.
pub mod filter;
/// Pillow `ImageOps`-style convenience operations.
pub mod imageops;
/// Module-level Pillow function implementations.
pub mod module_fns;
/// Parameterized convolution filter helpers.
pub mod param_filters;
pub mod paste;
pub mod pil_resize;
pub mod quantize;
/// Resize operations and resampling helpers.
pub mod resize;
/// Rotation operations.
pub mod rotate;
/// Band splitting and channel extraction operations.
pub mod split;
/// Geometric transform operations.
pub mod transform;
/// Transpose and flip operations.
pub mod transpose;
/// Shared operation helper functions.
pub mod utils;
