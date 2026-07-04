//! Pure-Rust TrueType font rendering: auto-hinter, rasterizer, table parsers.
//!
//! ```rust
//! use pillow_rs_freetype::{BitmapBackend, Font};
//! let data = std::fs::read("font.ttf")?;
//! let font = Font::truetype(&data, 12.0, BitmapBackend::FreeType)?;
//! let mask = font.getmask("A")?;
//! # Ok::<(), pillow_rs_freetype::FontError>(())
//! ```
//!
//! The vendored C source under `freetype/` is a **read-only reference**;
//! this crate contains no FFI and links nothing.
//!
//! # Architecture
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`autohint`] | Latin auto-hinter (reload, segments, edges, 4-phase snapping, IUP) |
//! | [`tt`] | TrueType table parsers (glyf, cmap, hmtx, head, hhea, maxp, name, OS/2, loca) |
//! | [`scaler`] | Glyph scaling (FU→26.6), pp1.x shift, cbox computation |
//! | [`grays`] | Smooth anti-aliased rasterizer (FT_INT64 DDA) |
//! | [`font`] | High-level API: `Font::truetype`, `getmask`, `getbbox` |
//! | [`fixed`] | Fixed-point math: `ft_mul_fix`, `ft_div_fix`, `ft_ceil_fix`, etc. |

#![forbid(unsafe_code)]
#![allow(missing_docs)]
// 26.6 fixed-point math uses portable wrapping helpers (wrapping.rs) and
// module-level allows for verified C ports (autohint, grays, scaler, tt, font).
// sha2/serde/serde_json are dev-deps used by the coverage test.
#![cfg_attr(test, allow(unused_crate_dependencies))]

pub mod autohint;
pub mod casts;
pub mod error;
pub mod fixed;
pub mod font;
pub mod grays;
pub mod outline;
pub mod scaler;
pub mod tables;
pub mod tt;
pub mod wrapping;

pub use error::FontError;
pub use font::{BitmapBackend, Font, GlyphMask};
