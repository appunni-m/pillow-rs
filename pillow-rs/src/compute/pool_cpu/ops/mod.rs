//! Exact-mode operation families for serial CPU execution.
// TODO(pillow-rs#3ptb): audit every architecture module declaration against
// its mode support and dispatch cfg. Keep architecture implementations until
// target-specific reachability is verified.
// TODO(pillow-rs#3ptb): finish the family-by-family audit that every execution
// boundary consumes the prepared selection derived from ExecutionContext.
// Resize and Chops are the only families covered by this continuation so far.

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
