use crate::raster::RgbImage;

pub(crate) type Input = RgbImage;
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod expand;
