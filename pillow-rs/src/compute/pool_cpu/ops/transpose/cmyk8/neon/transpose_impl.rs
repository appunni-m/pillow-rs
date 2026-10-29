use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::DynamicImage;

pub(crate) fn execute(
    image: &super::Input,
    method: &TransposeMethod,
) -> Result<DynamicImage, PilError> {
    let (output, vector_blocks, scalar_tail) =
        crate::compute::pool_cpu::ops::transpose::helpers::native_transpose::transpose_u8::<_, 4, 4>(
            image, method,
        )?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(DynamicImage::ImageCmyk8(output))
}
