use crate::raster::RgbaImage;

pub(crate) type Input = RgbaImage;
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod expand;
