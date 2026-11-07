//! Selects the exact native-byte vector kernel after strategy admission.

use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::error::PilError;
use crate::pipeline::TransposeMethod;
use crate::raster::{ImageBuffer, Pixel};

pub(crate) fn execute<P, const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: RotateRequest,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    if !(1..=4).contains(&CHANNELS) {
        return Err(PilError::InternalError(
            "Rotate vector strategy requires one to four native byte channels".into(),
        ));
    }
    if image.width() == 0 || image.height() == 0 {
        return super::vector_nearest::execute::<P, CHANNELS>(image, request);
    }
    if super::geometry::identity_contract(request) {
        return execute_identity::<P, CHANNELS>(image);
    }
    if let Some(degree) =
        super::geometry::discrete_fast_angle(request, image.width(), image.height())
    {
        return execute_discrete::<P, CHANNELS>(image, degree);
    }
    if FORCE_NEAREST || request.nearest {
        super::vector_nearest::execute::<P, CHANNELS>(image, request)
    } else {
        super::vector_bilinear::execute::<P, CHANNELS, ALPHA>(image, request)
    }
}

fn execute_identity<P, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let dimensions =
        crate::checked_dims::CheckedDims::new(image.width(), image.height(), CHANNELS as u8)?;
    let mut output = dimensions.alloc_buffer();
    let (vector_blocks, scalar_tail) =
        crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
            image.as_raw(),
            &mut output,
        )
        .ok_or_else(|| PilError::InternalError("Rotate identity copy shape mismatch".into()))?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::image_utils::typed_bytes_to_buffer::<P>(dimensions, output)
}

fn execute_discrete<P, const CHANNELS: usize>(
    image: &ImageBuffer<P, Vec<u8>>,
    degree: u32,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let method = match degree {
        90 => TransposeMethod::Rotate90,
        180 => TransposeMethod::Rotate180,
        270 => TransposeMethod::Rotate270,
        _ => return Err(PilError::InternalError("invalid discrete Rotate angle".into())),
    };
    let (output, vector_blocks, scalar_tail) = match CHANNELS {
        1 => crate::compute::pool_cpu::ops::transpose::helpers::native_transpose::transpose_u8::<
            P, 1, 16,
        >(image, &method)?,
        2 => crate::compute::pool_cpu::ops::transpose::helpers::native_transpose::transpose_u8::<
            P, 2, 8,
        >(image, &method)?,
        3 => crate::compute::pool_cpu::ops::transpose::helpers::native_transpose::transpose_u8::<
            P, 3, 4,
        >(image, &method)?,
        4 => crate::compute::pool_cpu::ops::transpose::helpers::native_transpose::transpose_u8::<
            P, 4, 4,
        >(image, &method)?,
        _ => return Err(PilError::InternalError("invalid Rotate channel count".into())),
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(output)
}
