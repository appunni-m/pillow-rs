//! Serial native-sample implementations for ImageF32.

use crate::error::PilError;
use crate::raster::DynamicImage;
use std::sync::Arc;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    if !remaining.is_empty() {
        return Err(PilError::InternalError("identity Merge received extra bands".into()));
    }
    Ok(source.clone())
}
