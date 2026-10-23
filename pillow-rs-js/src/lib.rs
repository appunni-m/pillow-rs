// AS PER DESIGN — DO NOT REMOVE: Deferred lint cleanup. See CODEBASE_AUDIT.md Fix 2.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]
#![allow(clippy::map_unwrap_or)]
#![allow(clippy::redundant_clone)]
// WASM binding conventions differ from standard Rust naming
#![allow(non_snake_case)]
#![allow(dead_code)]
#![allow(unused_variables)]

//! pillow-rs WASM — full Pillow API for the browser. Thin delegation to pillow-rs.
mod inputs;
mod values;
pub(crate) use inputs::*;
pub(crate) use values::*;

mod image;
pub use image::Image;
mod image_io;
pub use image_io::*;
mod draw;
pub use draw::*;
mod font;
pub use font::*;
mod image_chops;
pub use image_chops::*;
mod image_ops;
pub use image_ops::*;
mod palette;
pub use palette::*;
mod statistics;
pub use statistics::*;
mod effects;
pub use effects::*;
mod color;
pub use color::*;
mod backend;
pub use backend::*;
mod pipeline_telemetry;
pub use pipeline_telemetry::*;
