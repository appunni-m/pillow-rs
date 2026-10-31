//! Prepared exact-mode adapter for native-byte order-statistic kernels.

use crate::checked_dims::CheckedDims;
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const LANES: usize = 16;
const MAX_SIZE: u32 = 15;
const MAX_AREA: usize = 225;

/// Executes a declared byte rank strategy for one exact mode and architecture.
pub(crate) fn execute_native_byte_rank(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    mode: Mode,
    channels: u8,
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    let (size, rank) = match request {
        super::super::ops::FilterRequest::MedianFilter { size } => {
            (size, size.saturating_mul(size) / 2)
        }
        super::super::ops::FilterRequest::RankFilter { size, rank } => (size, rank),
        _ => {
            return Err(PilError::InternalError(
                "byte rank strategy received a different filter request".into(),
            ));
        }
    };
    match architecture {
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => (),
        _ => {
            return Err(PilError::InternalError(
                "byte rank strategy received an unsupported architecture".into(),
            ));
        }
    }
    if image.mode() != mode {
        return Err(PilError::InternalError(
            "byte rank strategy received a different raster mode".into(),
        ));
    }
    let output = rank_filter_native_bytes(
        image.as_bytes(),
        image.width(),
        image.height(),
        channels,
        mode,
        size,
        rank,
    )?;
    crate::compute::pool_cpu::helpers::dynamic_image::from_raw_bytes(
        mode,
        image.width(),
        image.height(),
        output,
    )
    .ok_or_else(|| PilError::InternalError("byte rank output mode shape mismatch".into()))
}

fn rank_filter_native_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: u8,
    mode: Mode,
    size: u32,
    rank: u32,
) -> Result<Vec<u8>, PilError> {
    if !(1..=4).contains(&channels) || size == 0 || size.is_multiple_of(2) || size > MAX_SIZE {
        return Err(PilError::InternalError(
            "byte rank request bypassed its prepared window guard".into(),
        ));
    }
    let dimensions = CheckedDims::new(width, height, channels)?;
    let size_usize = size as usize;
    let area = size_usize * size_usize;
    if area > MAX_AREA || rank as usize >= area || raw.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "byte rank request bypassed its prepared shape guard".into(),
        ));
    }
    if super::native_small_uniform_bytes(
        raw,
        width,
        height,
        usize::from(channels),
        super::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) {
        let mut output = vec![0; raw.len()];
        let (blocks, scalar_tail) =
            crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
                raw, &mut output,
            )
            .ok_or_else(|| PilError::InternalError("byte rank identity copy shape mismatch".into()))?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return Ok(output);
    }
    let width = width as usize;
    let height = height as usize;
    let channels = usize::from(channels);
    let half = size_usize / 2;
    let row_stride = dimensions.row_stride();
    let mut output = vec![0u8; dimensions.total_bytes()];
    for y in 0..height {
        let row_start = y * row_stride;
        super::rank_vector_kernel::execute_row(
            raw,
            &mut output[row_start..row_start + row_stride],
            row_stride,
            width,
            height,
            channels,
            mode,
            size_usize,
            half,
            area,
            rank as usize,
            y,
        );
    }
    let vector_blocks = width.div_ceil(LANES) as u64 * channels as u64 * height as u64;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(output)
}
