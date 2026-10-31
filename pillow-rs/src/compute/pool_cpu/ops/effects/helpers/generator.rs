//! Results shared by eager CPU image generators.

use crate::raster::DynamicImage;

pub(crate) struct GeneratedImage {
    pub(crate) image: DynamicImage,
    pub(crate) vector_blocks: u64,
    pub(crate) scalar_tail: u64,
}
