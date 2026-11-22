//! Native signed-32 sample Resize, preserving Pillow's pass-rounding boundary.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::{
    F64MulAdd, PortableFma, PortableSeparateMulAdd, i32_samples_from_le_bytes,
};
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

use super::super::super::helpers::typed_nearest::BoxBounds;

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
) -> Result<RgbaImage, PilError> {
    execute_with_pass_fma(
        input,
        width,
        height,
        filter,
        bounds,
        // Pillow-compatible I32 rounding uses distinct arithmetic per pass.
        &PortableSeparateMulAdd,
        &PortableFma,
    )
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(crate) fn execute_with_pass_fma<H: F64MulAdd, V: F64MulAdd>(
    input: &RgbaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: BoxBounds,
    horizontal_fma: &H,
    vertical_fma: &V,
) -> Result<RgbaImage, PilError> {
    let (source_width, source_height) = input.dimensions();
    if width == 0 || height == 0 || source_width == 0 || source_height == 0 {
        return empty(width, height);
    }
    if bounds.is_none() && (width, height) == (source_width, source_height) {
        return Ok(input.clone());
    }
    let source_dims = CheckedDims::new_allow_empty(source_width, source_height, 4)?;
    let source = i32_samples_from_le_bytes(input.as_raw(), source_dims.total_pixels());
    if matches!(filter, ResampleFilter::Nearest) {
        return super::nearest::execute(&source, (source_width, source_height), (width, height));
    }
    let (horizontal, vertical) = super::super::super::helpers::coefficients::for_resize(
        (source_width, source_height),
        (width, height),
        filter,
        bounds,
    );
    let (need_horizontal, need_vertical) =
        super::super::super::helpers::coefficients::resample_pass_requirements(
            (source_width, source_height),
            (width, height),
            bounds,
        );
    // Pillow skips unchanged axes; a negative-lobe filter is not an identity
    // operation when evaluated at the same source and destination size.
    match (need_horizontal, need_vertical) {
        (false, false) => return Ok(input.clone()),
        (true, false) => {
            return super::super::helpers::streaming::execute_horizontal_only(
                &source,
                (source_width, source_height),
                (width, height),
                &horizontal,
                horizontal_fma,
            );
        }
        (false, true) => {
            return super::super::helpers::streaming::execute_vertical_only(
                &source,
                (source_width, source_height),
                (width, height),
                &vertical,
                vertical_fma,
            );
        }
        (true, true) => {}
    }
    if super::super::super::helpers::coefficients::horizontal_first(
        (source_width, source_height),
        (width, height),
    ) {
        super::super::helpers::streaming::execute(
            &source,
            source_width,
            width,
            height,
            &horizontal,
            &vertical,
            horizontal_fma,
            vertical_fma,
        )
    } else {
        super::super::helpers::streaming::execute_vertical_first(
            &source,
            (source_width, source_height),
            (width, height),
            &horizontal,
            &vertical,
            horizontal_fma,
            vertical_fma,
        )
    }
}

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
fn empty(width: u32, height: u32) -> Result<RgbaImage, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, 4)?;
    RgbaImage::from_raw(width, height, dims.alloc_buffer())
        .ok_or_else(|| PilError::ValueError("resize_i: failed to create output buffer".into()))
}
