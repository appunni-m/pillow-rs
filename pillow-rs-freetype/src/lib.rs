//! pillow-rs-freetype — pure-Rust byte-perfect port of FreeType 2.14.1.
//!
//! Ports the subset of FreeType needed for PIL-style TrueType rendering:
//!   - `ftcalc.c`  → [`fixed`] (FT_MulFix / FT_DivFix / FT_*Fix, FT_INT64 path)
//!   - `sfnt/tt*.c`, `ttgload.c` → [`tt`] (table loaders + glyph outlines)
//!   - `ftgrays.c` → [`grays`] (smooth anti-aliased rasterizer, FT_INT64 path)
//!   - `ftoutln.c`, `ftglyph.c` → [`scaler`] (scaling + pixel CBox)
//!   - PIL `ImageFont` surface → [`font`]
//!
//! The vendored C source under `freetype/` is a **read-only reference**; this
//! crate contains no FFI and links nothing.

#![forbid(unsafe_code)]
#![allow(missing_docs)]
// sha2/serde/serde_json are dev-deps used by the coverage test.
#![cfg_attr(test, allow(unused_crate_dependencies))]
// Many internal helpers are exercised through the integration test rather than
// unit tests; keep them during the port.
#![allow(dead_code)]

pub mod autohint;
pub mod error;
pub mod fixed;
pub mod font;
pub mod grays;
pub mod outline;
pub mod scaler;
pub mod tables;
pub mod tt;

pub use error::FontError;
pub use font::{BitmapBackend, Font, GlyphMask};
