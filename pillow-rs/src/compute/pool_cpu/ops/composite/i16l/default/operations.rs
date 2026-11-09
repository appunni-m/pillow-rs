//! Serial native-sample implementations for ImageI16L.

use crate::compute::pool_cpu::ops::composite::ops::CompositeLayout;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

const MODE: Mode = Mode::ImageI16L;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    if !remaining.is_empty() {
        return Err(PilError::InternalError(
            "identity Merge received extra bands".into(),
        ));
    }
    Ok(source.clone())
}

pub(crate) fn composite(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::composite::helpers::composite::execute(
        source,
        destination,
        mask,
        mask_alpha,
        MODE,
        CompositeLayout::U16 { channels: 1 },
    )
}
