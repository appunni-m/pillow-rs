// PIL API compatibility — many functions have 8+ parameters matching PIL signatures
#![allow(clippy::too_many_arguments)]

pub mod bitmap_font;
pub mod color;
pub mod draw;
pub mod error;
pub mod font;
pub mod format;
pub mod formats;
pub mod gpu;
pub mod image;
pub mod ops;
pub mod pipeline;

pub use draw::Draw;
pub use error::PilError;
pub use font::Font;
pub use image::Image;
