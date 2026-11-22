use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

// TODO(pillow-rs#3ptb): verify x86_64 baseline output, selection receipts, and
// public-call latency against portable Default and AVX2.
// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &super::super::Input,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    let output = super::rotate270_tiled::try_execute(image, method)?.ok_or_else(|| {
        PilError::InternalError("selected x86_64 RGBA transpose guard was not satisfied".into())
    })?;
    crate::compute::record_pipeline_operation_path("native-copy");
    Ok(DynamicImage::ImageRgba8(output))
}
