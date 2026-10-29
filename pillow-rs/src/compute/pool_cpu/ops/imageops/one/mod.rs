use crate::raster::GrayImage;

pub(crate) type Input = GrayImage;
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod sse2;
pub(crate) mod neon;
pub(crate) mod wasm_simd128;
pub(crate) mod expand;
