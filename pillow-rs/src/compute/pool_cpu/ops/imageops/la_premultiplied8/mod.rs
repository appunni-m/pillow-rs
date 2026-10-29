use crate::raster::GrayAlphaImage;

pub(crate) type Input = GrayAlphaImage;
pub(crate) mod default;
pub(crate) mod ops;
pub(crate) mod expand;
