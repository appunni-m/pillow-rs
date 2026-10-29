use crate::raster::{ImageBuffer, LumaA};

pub(crate) type Input = ImageBuffer<LumaA<u16>, Vec<u16>>;
pub(crate) mod default;
pub(crate) mod ops;
