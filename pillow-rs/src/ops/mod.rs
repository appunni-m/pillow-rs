//! Pillow-compatible image operation implementations.
//!
//! Submodules contain reusable core logic for `Image` methods and module-level
//! Pillow functions. Binding crates should call the high-level APIs rather than
//! reaching into these modules directly.

/// Analysis and statistics operations.
pub(crate) mod analysis;
/// Array-interface descriptor resolution.
pub(crate) mod array;
/// Pillow `ImageChops`-style channel arithmetic operations.
pub(crate) mod chops;
/// Pillow mode conversion operations.
pub(crate) mod convert;
/// Cropping and rectangular extraction operations.
pub(crate) mod crop;
/// Brightness, contrast, color, and sharpness enhancement operations.
pub(crate) mod enhance;
/// Image filter operations and convolution helpers.
pub(crate) mod filter;
/// Pillow `ImageOps`-style convenience operations.
pub(crate) mod imageops;
/// Module-level Pillow function implementations.
pub(crate) mod module_fns;
/// Parameterized convolution filter helpers.
pub(crate) mod param_filters;
pub(crate) mod paste;
pub(crate) mod pil_resize;
pub(crate) mod quantize;
/// Resize operations and resampling helpers.
pub(crate) mod resize;
/// Rotation operations.
pub(crate) mod rotate;
/// Band splitting and channel extraction operations.
pub(crate) mod split;
/// Geometric transform operations.
pub(crate) mod transform;
/// Transpose and flip operations.
pub(crate) mod transpose;
/// Shared operation helper functions.
pub(crate) mod utils;
