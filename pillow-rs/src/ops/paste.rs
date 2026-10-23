//! Paste operations — image overlay, color fill, and mask-based alpha blending.

mod color;
mod input;
mod operations;
mod types;

pub use types::{
    AlphaCompositeBox, PasteSource, PythonPasteBox, PythonPasteMask, PythonPasteSource,
};
