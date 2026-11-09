use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn execute(image: &DynamicImage, size: u32) -> Result<DynamicImage, PilError> {
    let DynamicImage::One(input) = image else {
        return Err(PilError::InternalError(
            "ModeFilter mode strategy received a mismatched raster variant".into(),
        ));
    };
    crate::compute::pool_cpu::ops::filter::helpers::mode_filter::execute::<_, 1>(input, size)
        .map(DynamicImage::One)
}
