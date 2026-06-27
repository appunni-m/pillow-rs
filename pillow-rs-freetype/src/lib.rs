//! pillow-rs-freetype — Pure-Rust port of FreeType with optional system FreeType backend.
//!
//! Two bitmap backends, selected at font-construction time:
//!
//! | Backend          | Autohinter        | Rasterizer       |
//! |------------------|-------------------|------------------|
//! | `PureRust`       | Our port          | `grays.rs`       |
//! | `SystemFreeType` | System FreeType   | System FreeType  |
//!
//! `SystemFreeType` uses `FT_LOAD_RENDER` matching PIL's `_imagingft.c`.
//!
//! The vendored C source under `freetype/` is a **read-only reference**.

#![allow(missing_docs)]
// ft_backend.rs uses system FreeType FFI.
#![allow(unsafe_code)]
// sha2/serde/serde_json are dev-deps used by the coverage test.
#![cfg_attr(test, allow(unused_crate_dependencies))]
// Many internal helpers are exercised through the integration test rather than
// unit tests; keep them during the port.
#![allow(dead_code)]

pub mod autohint;
pub mod error;
pub mod fixed;
pub mod font;
pub mod ft_backend;
pub mod grays;
pub mod outline;
pub mod scaler;
pub mod tables;
pub mod tt;

pub use error::FontError;
pub use font::{BitmapBackend, Font, GlyphMask};
