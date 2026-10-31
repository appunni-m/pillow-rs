//! Maps native P indices into the reordered palette index plane.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage};

pub(crate) fn execute(
    source: &GrayImage,
    dest_map: &[u8],
) -> Result<DynamicImage, PilError> {
    let bytes = super::super::super::helpers::remap_palette::scalar(
        source.as_raw(),
        dest_map,
    );
    super::super::super::helpers::remap_palette::palette_image(
        source.width(),
        source.height(),
        bytes,
    )
}
