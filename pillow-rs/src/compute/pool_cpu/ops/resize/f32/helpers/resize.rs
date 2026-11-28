//! Shared F32 Resize orchestration over the native four-byte sample carrier.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::F64MulAdd;
use crate::ops::pil_resize::f32_samples_from_le_bytes;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

pub(crate) type FilterResolver = fn(ResampleFilter) -> (fn(f64) -> f64, f64);

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(crate) fn execute_with_fma<const PRODUCT_BLOCK_WIDTH: usize, F: F64MulAdd>(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
    fma: &F,
) -> Result<RgbaImage, PilError> {
    execute_with_fma_and_filter::<PRODUCT_BLOCK_WIDTH, F>(
        input,
        width,
        height,
        filter,
        bounds,
        fma,
        crate::ops::pil_resize::filter_from_resample,
    )
}

// VERIFY: source=reviewed@2026-11-28; parity=pending; benchmark=pending
pub(crate) fn execute_with_fma_and_filter<const PRODUCT_BLOCK_WIDTH: usize, F: F64MulAdd>(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::super::helpers::typed_nearest::BoxBounds,
    fma: &F,
    resolve_filter: FilterResolver,
) -> Result<RgbaImage, PilError> {
    let (source_width, source_height) = input.dimensions();
    if width == 0 || height == 0 || source_width == 0 || source_height == 0 {
        return empty(width, height);
    }
    if bounds.is_none() && (width, height) == (source_width, source_height) {
        return Ok(input.clone());
    }
    if matches!(filter, ResampleFilter::Nearest) {
        return super::nearest::execute(input, width, height, bounds);
    }

    let sample_count = CheckedDims::new_allow_empty(source_width, source_height, 1)?.total_pixels();
    let samples = f32_samples_from_le_bytes(input.as_raw(), sample_count);
    if samples.len() != sample_count {
        return Err(PilError::ValueError(
            "resize_f source storage does not contain one float per pixel".into(),
        ));
    }
    if bounds.is_none()
        && let Some(&constant) = samples.first()
        && constant.is_finite()
        && constant.to_bits() != (-0.0f32).to_bits()
        && samples
            .iter()
            .all(|sample| sample.to_bits() == constant.to_bits())
    {
        return repeated(width, height, constant);
    }

    let vertical_first = source_width != width
        && source_height != height
        && !super::super::super::helpers::coefficients::horizontal_first(
            (source_width, source_height),
            (width, height),
        );
    let (kernel, support) = resolve_filter(filter);
    super::filtered::execute::<PRODUCT_BLOCK_WIDTH, _>(
        &samples,
        (source_width, source_height),
        (width, height),
        kernel,
        support,
        bounds,
        vertical_first,
        fma,
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn empty(width: u32, height: u32) -> Result<RgbaImage, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    RgbaImage::from_raw(width, height, dims.alloc_buffer()).ok_or_else(|| {
        PilError::ValueError("resize_f output shape must match its dimensions".into())
    })
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn repeated(width: u32, height: u32, sample: f32) -> Result<RgbaImage, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    let bytes = sample.to_le_bytes().repeat(dims.total_pixels());
    RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::ValueError("resize_f output shape must match its dimensions".into())
    })
}
