//! Host-neutral inputs for image quantization.

use crate::image::Image;

/// Host-neutral palette argument for the public quantize entry point.
#[derive(Debug, Clone)]
pub enum QuantizePalette {
    /// No palette argument was supplied.
    None,
    /// An image object was supplied as the palette argument.
    Image(Box<Image>),
    /// A non-image palette argument was supplied by the host adapter.
    /// Python adapters should resolve falsey values to `None` before crossing
    /// the Rust boundary. A truthy value without a supported image carrier is
    /// an error, never an implicit request for an automatically generated
    /// palette.
    Other,
}
