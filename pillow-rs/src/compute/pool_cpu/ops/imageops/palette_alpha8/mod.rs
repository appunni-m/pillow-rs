use crate::raster::GrayAlphaImage;

pub(crate) type Input = GrayAlphaImage;
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod sse2;
pub(crate) mod neon;
pub(crate) mod wasm_simd128;
pub(crate) mod expand;
