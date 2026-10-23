//! Host-neutral inputs for image quantization.

use crate::image::Image;

/// Host-neutral palette argument for the public quantize entry point.
#[derive(Debug, Clone)]
pub enum QuantizePalette {
    /// No palette argument was supplied.
    None,
    /// An image object was supplied as the palette argument.
    Image(Box<Image>),
    /// A non-image palette argument was supplied and is ignored by Pillow's
    /// compatibility path until a real palette image is provided.
    Other,
}
