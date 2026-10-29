use crate::raster::RgbaImage;

pub(crate) type Input = RgbaImage;
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod sse2;
pub(crate) mod neon;
pub(crate) mod wasm_simd128;
pub(crate) mod expand;
