//! F-mode orchestration over four-byte little-endian float samples.

use crate::compute::{Backend, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::DynamicImage;

#[cfg(target_arch = "x86_64")]
use crate::ops::pil_resize::X86FmaToken;

pub(crate) use super::kernels::default::resize_f_with_fma;

pub(crate) fn resize_f(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_f_in_context(
        &ExecutionContext::for_backend(Backend::Cpu),
        image,
        width,
        height,
        filter,
    )
}

pub(crate) fn resize_f_in_context(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    #[cfg(target_arch = "x86_64")]
    if let Some(fma) = X86FmaToken::from_context(context) {
        return super::kernels::fma::resize(image, width, height, filter, &fma);
    }
    let _ = context;
    super::kernels::default::resize_f_default(image, width, height, filter)
}

pub(super) fn resize(
    context: &ExecutionContext,
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: &ResampleFilter,
) -> Result<DynamicImage, PilError> {
    resize_f_in_context(context, image, width, height, filter)
}
