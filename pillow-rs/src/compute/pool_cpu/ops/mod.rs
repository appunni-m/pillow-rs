//! Exact-mode operation families for serial CPU execution.
// TODO(pillow-rs#3ptb): finish the architecture-module declaration audit for
// the remaining families. Direct-instruction paths were traced across all
// families; Transpose's per-mode declarations were checked on 2026-11-04.
// TODO(pillow-rs#3ptb): finish the family-by-family audit that every execution
// boundary consumes the prepared selection derived from ExecutionContext.
// Resize, Chops, Pad, Crop, Transpose, and supported Draw routes are
// source-reviewed for prepared-selection flow. Thumbnail's I32 stream
// consumes its selection; ImageOps Quantize threads selection through all 25
// modes. Continue the family audit and finish Draw's semantic/fault review.

pub(crate) mod analysis;
pub(crate) mod chops;
pub(crate) mod color;
pub(crate) mod color3dlut;
pub(crate) mod composite;
pub(crate) mod crop;
pub(crate) mod draw;
pub(crate) mod effects;
pub(crate) mod enhance;
pub(crate) mod eval;
pub(crate) mod filter;
pub(crate) mod geometry;
pub(crate) mod grayscale;
pub(crate) mod imageops;
pub(crate) mod paste;
pub(crate) mod put;
pub(crate) mod reduce;
pub(crate) mod resize;
pub(crate) mod rotate;
pub(crate) mod thumbnail;
pub(crate) mod transform;
pub(crate) mod transpose;
