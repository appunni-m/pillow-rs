//! Operation-level dispatch for native interleaved byte filter kernels.

use super::super::ops::FilterRequest;
use crate::error::PilError;
use crate::raster::Mode;

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    raw: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    mode: Mode,
    request: FilterRequest,
) -> Result<Option<Vec<u8>>, PilError> {
    let fuse_three_passes = super::super::ops::default_blur_uses_three_pass_fusion(mode);
    match request {
        FilterRequest::GaussianBlur { sigma } => super::blur::execute_gaussian_blur_native_bytes(
            raw,
            width as u32,
            height as u32,
            channels,
            sigma,
            fuse_three_passes,
        ),
        FilterRequest::BoxBlur { radius } => super::blur::execute_box_blur_native_bytes(
            raw,
            width as u32,
            height as u32,
            channels,
            radius,
        ),
        FilterRequest::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => super::blur::execute_box_blur_xy_native_bytes(
            raw,
            width as u32,
            height as u32,
            channels,
            radius_x,
            radius_y,
            passes,
            fuse_three_passes,
        ),
        FilterRequest::MedianFilter { size } => super::rank::execute_native_bytes(
            raw,
            width as u32,
            height as u32,
            channels,
            size,
            size * size / 2,
        ),
        FilterRequest::MaxFilter { size } => super::rank::execute_native_bytes(
            raw,
            width as u32,
            height as u32,
            channels,
            size,
            size * size - 1,
        ),
        FilterRequest::MinFilter { size } => {
            super::rank::execute_native_bytes(raw, width as u32, height as u32, channels, size, 0)
        }
        FilterRequest::RankFilter { size, rank } => super::rank::execute_native_bytes(
            raw,
            width as u32,
            height as u32,
            channels,
            size,
            rank,
        ),
        FilterRequest::Filter3x3 { .. } | FilterRequest::Filter5x5 { .. } => {
            Err(PilError::InternalError(
                "convolution request reached byte neighborhood dispatch".into(),
            ))
        }
    }
}
