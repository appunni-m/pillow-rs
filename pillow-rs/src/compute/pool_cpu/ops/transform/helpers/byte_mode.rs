//! Small generator for exact byte-mode Default Transform wrappers.

macro_rules! byte_mode_default {
    ($variant:ident, $image:ty, $channels:literal, $force_nearest:literal, $alpha:literal, $kernel:ident, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            let $crate::raster::DynamicImage::$variant(_) = input else {
                return Err($crate::error::PilError::InternalError(
                    "Transform mode strategy received a different raster identity".into(),
                ));
            };
            let bytes = $crate::compute::pool_cpu::ops::transform::helpers::byte::execute::<
                $channels,
                $force_nearest,
                $alpha,
            >(
                input,
                request,
                $crate::compute::pool_cpu::ops::transform::helpers::byte::AffineNearestKernel::$kernel,
            )?;
            let output = <$image>::from_raw(request.width, request.height, bytes).ok_or_else(|| {
                $crate::error::PilError::InternalError($shape_error.into())
            })?;
            Ok($crate::raster::DynamicImage::$variant(output))
        }

        pub(crate) fn execute_into(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
            output: &mut $crate::raster::DynamicImage,
        ) -> Result<(), $crate::error::PilError> {
            let (
                $crate::raster::DynamicImage::$variant(_),
                $crate::raster::DynamicImage::$variant(_),
            ) = (input, &*output)
            else {
                return Err($crate::error::PilError::InternalError(
                    "Transform mode strategy received a different destination raster identity".into(),
                ));
            };
            let destination = output.as_bytes_mut().ok_or_else(|| {
                $crate::error::PilError::InternalError(
                    "Transform destination does not expose byte storage".into(),
                )
            })?;
            $crate::compute::pool_cpu::ops::transform::helpers::byte::execute_into::<
                $channels,
                $force_nearest,
                $alpha,
            >(
                input,
                request,
                $crate::compute::pool_cpu::ops::transform::helpers::byte::AffineNearestKernel::$kernel,
                destination,
            )
        }

    };
}

pub(crate) use byte_mode_default;

macro_rules! byte_mode_mesh_record {
    ($variant:ident, $channels:literal, $alpha:literal, $mode_name:literal) => {
        pub(crate) fn apply_mesh_record(
            input: &$crate::raster::DynamicImage,
            output: &mut $crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
            box_coordinates: (i64, i64, i64, i64),
            coefficients: [f64; 8],
            source_scratch: &mut Vec<u8>,
        ) -> Result<(), $crate::error::PilError> {
            let (
                $crate::raster::DynamicImage::$variant(source),
                $crate::raster::DynamicImage::$variant(destination),
            ) = (input, &mut *output)
            else {
                return Err($crate::error::PilError::InternalError(concat!(
                    "Transform MESH expected exact ",
                    $mode_name,
                    " source and destination modes"
                )
                .into()));
            };
            $crate::compute::pool_cpu::ops::transform::helpers::byte::apply_mesh_record::<
                $channels,
                $alpha,
            >(
                source.as_raw(),
                source.dimensions(),
                destination.as_mut(),
                request,
                box_coordinates,
                coefficients,
                source_scratch,
            )?;
            if $alpha && !matches!(request.filter, $crate::pipeline::ResampleFilter::Nearest) {
                let $crate::raster::DynamicImage::$variant(image) = output else {
                    return Err($crate::error::PilError::InternalError(concat!(
                        "Transform MESH alpha output expected exact ",
                        $mode_name,
                        " mode"
                    )
                    .into()));
                };
                $crate::compute::pool_cpu::ops::transform::helpers::alpha::unpremultiply_region(
                    image.as_mut(),
                    $channels,
                    (request.width, request.height),
                    box_coordinates,
                );
            }
            Ok(())
        }
    };
}

pub(crate) use byte_mode_mesh_record;

macro_rules! byte_mode_nearest {
    ($variant:ident, $image:ty, $channels:literal, $mode_name:literal, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            let $crate::raster::DynamicImage::$variant(source) = input else {
                return Err($crate::error::PilError::InternalError(
                    concat!("Transform mode strategy expected ", $mode_name).into(),
                ));
            };
            let bytes = $crate::compute::pool_cpu::ops::transform::helpers::byte_nearest::execute::<
                $channels,
            >(source.as_raw(), source.dimensions(), request)?;
            let output = <$image>::from_raw(request.width, request.height, bytes).ok_or_else(|| {
                $crate::error::PilError::InternalError($shape_error.into())
            })?;
            Ok($crate::raster::DynamicImage::$variant(output))
        }

        pub(crate) fn execute_into(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
            output: &mut $crate::raster::DynamicImage,
        ) -> Result<(), $crate::error::PilError> {
            let (
                $crate::raster::DynamicImage::$variant(source),
                $crate::raster::DynamicImage::$variant(_),
            ) = (input, &*output)
            else {
                return Err($crate::error::PilError::InternalError(
                    concat!("Transform mode strategy expected exact ", $mode_name, " buffers").into(),
                ));
            };
            let destination = output.as_bytes_mut().ok_or_else(|| {
                $crate::error::PilError::InternalError(
                    "Transform destination does not expose byte storage".into(),
                )
            })?;
            $crate::compute::pool_cpu::ops::transform::helpers::byte_nearest::execute_into::<
                $channels,
            >(source.as_raw(), source.dimensions(), request, destination)
        }

    };
}

pub(crate) use byte_mode_nearest;

macro_rules! byte_mode_vector {
    ($variant:ident, $image:ty, $channels:literal, $alpha:literal, $mode_name:literal, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            let $crate::raster::DynamicImage::$variant(source) = input else {
                return Err($crate::error::PilError::InternalError(
                    concat!("Transform mode strategy expected ", $mode_name).into(),
                ));
            };
            let bytes = if matches!(request.method, $crate::pipeline::TransformMethod::Affine)
                && matches!(request.filter, $crate::pipeline::ResampleFilter::Bilinear)
            {
                $crate::compute::pool_cpu::ops::transform::helpers::vector_affine_bilinear::execute::<
                    $channels,
                    $alpha,
                >(source.as_raw(), source.dimensions(), request)?
            } else {
                $crate::compute::pool_cpu::ops::transform::helpers::byte_nearest::execute::<
                    $channels,
                >(source.as_raw(), source.dimensions(), request)?
            };
            let output = <$image>::from_raw(request.width, request.height, bytes).ok_or_else(|| {
                $crate::error::PilError::InternalError($shape_error.into())
            })?;
            Ok($crate::raster::DynamicImage::$variant(output))
        }

        pub(crate) fn execute_into(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
            output: &mut $crate::raster::DynamicImage,
        ) -> Result<(), $crate::error::PilError> {
            let (
                $crate::raster::DynamicImage::$variant(source),
                $crate::raster::DynamicImage::$variant(_),
            ) = (input, &*output)
            else {
                return Err($crate::error::PilError::InternalError(
                    concat!("Transform mode strategy expected exact ", $mode_name, " buffers").into(),
                ));
            };
            let destination = output.as_bytes_mut().ok_or_else(|| {
                $crate::error::PilError::InternalError(
                    "Transform destination does not expose byte storage".into(),
                )
            })?;
            if matches!(request.method, $crate::pipeline::TransformMethod::Affine)
                && matches!(request.filter, $crate::pipeline::ResampleFilter::Bilinear)
            {
                $crate::compute::pool_cpu::ops::transform::helpers::vector_affine_bilinear::execute_into::<
                    $channels,
                    $alpha,
                >(source.as_raw(), source.dimensions(), request, destination)
            } else {
                $crate::compute::pool_cpu::ops::transform::helpers::byte_nearest::execute_into::<
                    $channels,
                >(source.as_raw(), source.dimensions(), request, destination)
            }
        }

    };
}

pub(crate) use byte_mode_vector;

macro_rules! luma16_mode_default_into {
    ($variant:ident, $order:ident, $mode_name:literal) => {
        pub(crate) fn execute_into(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
            output: &mut $crate::raster::DynamicImage,
        ) -> Result<(), $crate::error::PilError> {
            let (
                $crate::raster::DynamicImage::$variant(source),
                $crate::raster::DynamicImage::$variant(destination),
            ) = (input, output)
            else {
                return Err($crate::error::PilError::InternalError(concat!(
                    "Transform mode strategy expected exact ",
                    $mode_name,
                    " buffers"
                ).into()));
            };
            $crate::compute::pool_cpu::ops::transform::helpers::luma16::execute_into(
                source.as_raw(),
                source.dimensions(),
                request,
                $crate::compute::pool_cpu::ops::transform::helpers::luma16::ByteOrder::$order,
                destination.as_mut(),
            )
        }
    };
}

pub(crate) use luma16_mode_default_into;

macro_rules! luma16_mode_vector {
    ($variant:ident, $order:ident, $mode_name:literal, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            let $crate::raster::DynamicImage::$variant(source) = input else {
                return Err($crate::error::PilError::InternalError(
                    concat!("Transform mode strategy expected ", $mode_name).into(),
                ));
            };
            let samples = $crate::compute::pool_cpu::ops::transform::helpers::vector_luma16::execute(
                source.as_raw(),
                source.dimensions(),
                request,
                $crate::compute::pool_cpu::ops::transform::helpers::luma16::ByteOrder::$order,
            )?;
            let output = $crate::raster::ImageBuffer::from_raw(request.width, request.height, samples)
                .ok_or_else(|| $crate::error::PilError::InternalError($shape_error.into()))?;
            Ok($crate::raster::DynamicImage::$variant(output))
        }

        pub(crate) fn execute_into(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
            output: &mut $crate::raster::DynamicImage,
        ) -> Result<(), $crate::error::PilError> {
            let (
                $crate::raster::DynamicImage::$variant(source),
                $crate::raster::DynamicImage::$variant(destination),
            ) = (input, &mut *output)
            else {
                return Err($crate::error::PilError::InternalError(
                    concat!("Transform mode strategy expected exact ", $mode_name, " buffers").into(),
                ));
            };
            $crate::compute::pool_cpu::ops::transform::helpers::vector_luma16::execute_into(
                source.as_raw(),
                source.dimensions(),
                request,
                $crate::compute::pool_cpu::ops::transform::helpers::luma16::ByteOrder::$order,
                destination.as_mut(),
            )
        }

    };
}

pub(crate) use luma16_mode_vector;
