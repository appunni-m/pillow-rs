use crate::raster::{ImageBuffer, Rgb};

pub(crate) type Input = ImageBuffer<Rgb<u16>, Vec<u16>>;
pub(crate) mod default;
pub(crate) mod ops;
