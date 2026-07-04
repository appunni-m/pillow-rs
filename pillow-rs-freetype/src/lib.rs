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
//! The Rust renderer owns the force-autohint path.  The native TrueType
//! default path uses a narrow FreeType bridge for bytecode-compatible glyph
//! loading.
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

#![deny(unsafe_code)]
#![allow(missing_docs)]
// 26.6 fixed-point arithmetic uses infallible cast wrappers from casts.rs.
// The single remaining allow (arithmetic_side_effects) covers 579 sites
// of i32 +/×/- operations inherent to the 26.6 domain. See casts.rs for why
// wrapping_add/saturating_add are incorrect alternatives.
#![allow(clippy::arithmetic_side_effects, clippy::if_same_then_else)]
// sha2/serde/serde_json are dev-deps used by the coverage test.
#![cfg_attr(test, allow(unused_crate_dependencies))]
// Internal helpers exercised by integration tests (coverage_matrix_tests.rs)
// trigger dead_code. Remove once they have dedicated unit tests.
#![allow(dead_code)]

pub mod autohint;
pub mod casts;
pub mod error;
pub mod fixed;
pub mod font;
pub mod grays;
#[allow(unsafe_code)]
mod native_ft;
pub mod outline;
pub mod scaler;
pub mod tables;
pub mod tt;

pub use error::FontError;
pub use font::{BitmapBackend, CharmapInfo, FaceInfo, Font, GlyphMask, SfntTableInfo, SizeMetrics};
