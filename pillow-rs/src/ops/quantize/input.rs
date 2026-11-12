//! Host-neutral inputs for image quantization.

use crate::image::Image;

/// Host-neutral palette argument for the public quantize entry point.
#[derive(Debug, Clone)]
pub enum QuantizePalette {
    /// No palette argument was supplied.
    None,
    /// An image object was supplied as the palette argument.
    Image(Box<Image>),
    /// A supplied P image has no attached palette table.
    ///
    /// This is distinct from an attached zero-entry table, which remains
    /// `Image` and is rejected. The operation selects its default palette from
    /// the actual source mode for this state.
    NoAttachedPalette,
    /// A non-image palette argument was supplied by the host adapter.
    /// Python adapters should resolve falsey values to `None` before crossing
    /// the Rust boundary. A truthy value without a supported image carrier is
    /// an error, never an implicit request for an automatically generated
    /// palette.
    Other,
}
