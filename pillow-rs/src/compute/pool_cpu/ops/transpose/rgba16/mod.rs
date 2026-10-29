use crate::raster::{ImageBuffer, Rgba};

pub(crate) type Input = ImageBuffer<Rgba<u16>, Vec<u16>>;
pub(crate) mod default;
pub(crate) mod ops;
