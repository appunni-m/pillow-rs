//! `pillow-rs-font` — façade re-exporting [`pillow_rs_freetype`].
//!
//! The implementation now lives in `pillow-rs-freetype` (a byte-perfect
//! pure-Rust port of FreeType 2.14.1). This crate preserves the historical
//! `pillow_rs_font` name so existing callers (`pillow-rs`) keep working.

#![forbid(unsafe_code)]
#![allow(missing_docs)]

pub use pillow_rs_freetype::{Font, FontError, GlyphMask};
