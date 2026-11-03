//! Exact ImageF32 mode dispatch for ordinary and boxed Resize.
//!
//! The operation-level prepared selection chooses the AVX FMA strategy for
//! eligible filtered work. Default uses the portable ordered f64 multiply-add
//! path; nearest, empty, and unboxed identity requests remain Default.

use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, RgbaImage};

pub(crate) fn resize(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageF32(input) = image else {
        return Err(PilError::InternalError(
            "ImageF32 Resize strategy received another raster mode".into(),
        ));
    };

    let output = match bounds {
        Some((left, top, right, bottom))
            if u64::from(image.height()) > u64::from(image.width()) * 100
                && height < image.height() =>
        {
            let vertical = resize_selected(
                context,
                architecture,
                input,
                image.width(),
                height,
                filter,
                Some((0.0, top, f64::from(image.width()), bottom)),
            )?;
            resize_selected(
                context,
                architecture,
                &vertical,
                width,
                height,
                filter,
                Some((left, 0.0, right, f64::from(height))),
            )?
        }
        _ => resize_selected(context, architecture, input, width, height, filter, bounds)?,
    };
    Ok(DynamicImage::ImageF32(output))
}

fn resize_selected(
    _context: &ExecutionContext,
    architecture: Architecture,
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<RgbaImage, PilError> {
    match architecture {
        Architecture::Default => {
            super::default::resize_impl::execute(input, width, height, filter, bounds)
        }
        #[cfg(target_arch = "x86_64")]
        Architecture::AvxFma => super::avx::resize_impl::execute(
            _context, input, width, height, filter, bounds, Architecture::AvxFma,
        ),
        _ => Err(PilError::InternalError(format!(
            "ImageF32 Resize received non-F32 implementation selection {architecture:?}"
        ))),
    }
}
