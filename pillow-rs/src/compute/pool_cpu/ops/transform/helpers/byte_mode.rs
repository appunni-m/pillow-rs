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
    };
}

pub(crate) use byte_mode_default;

macro_rules! byte_mode_nearest {
    ($variant:ident, $image:ty, $channels:literal, $mode_name:literal, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            use $crate::raster::GenericImageView;

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
    };
}

pub(crate) use byte_mode_nearest;

macro_rules! byte_mode_vector {
    ($variant:ident, $image:ty, $channels:literal, $alpha:literal, $mode_name:literal, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            use $crate::raster::GenericImageView;

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
    };
}

pub(crate) use byte_mode_vector;

macro_rules! luma16_mode_vector {
    ($variant:ident, $order:ident, $mode_name:literal, $shape_error:literal) => {
        pub(crate) fn execute(
            input: &$crate::raster::DynamicImage,
            request: $crate::compute::pool_cpu::ops::transform::ops::TransformRequest<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            use $crate::raster::GenericImageView;

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
    };
}

pub(crate) use luma16_mode_vector;
