//! Native-byte architecture declarations and MaxFilter/MinFilter kernels.

use crate::checked_dims::CheckedDims;
use crate::compute::{Architecture, Backend};
use crate::compute::registry::support::{
    ImplementationSupport, OperationId, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use wide::u8x16;

const LANES: usize = 16;

pub(crate) const fn native_byte_filter_architectures(
    operation: OperationId,
    mode: Mode,
    guard: StrategyGuard,
) -> [ImplementationSupport; 5] {
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx2"
            )),
            &[Architecture::Avx2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Avx,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                not(target_feature = "avx2")
            )),
            &[Architecture::Avx],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2",
                not(target_feature = "avx")
            )),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

pub(crate) const fn native_byte_blur_architectures(
    operation: OperationId,
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 5] {
    native_byte_filter_architectures(
        operation,
        mode,
        StrategyGuard::FilterNativeByteBlur { channels },
    )
}

/// Dispatches one admitted byte extrema request to its selected architecture.
// TODO(pillow-rs#3ptb): freeze native-byte MaxFilter/MinFilter output and
// architecture/fallback receipts across exact byte modes; source review is
// complete, but CPU parity remains pending.
pub(crate) fn execute_native_byte_extrema(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    mode: Mode,
    channels: u8,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let (size, select_max, operation) = match request {
        super::super::ops::FilterRequest::MaxFilter { size } => {
            (size, true, OperationId::MaxFilter)
        }
        super::super::ops::FilterRequest::MinFilter { size } => {
            (size, false, OperationId::MinFilter)
        }
        _ => {
            return Err(PilError::InternalError(
                "byte extrema strategy received a different filter request".into(),
            ));
        }
    };
    let implementation = selection.implementation;
    if implementation.operation != operation
        || implementation.mode != mode
        || implementation.backend != Backend::Cpu
        || selection.requested_backend != Backend::Cpu
    {
        return Err(PilError::InternalError(
            "byte extrema strategy received a mismatched CPU selection".into(),
        ));
    }
    match implementation.architecture {
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => (),
        _ => {
            return Err(PilError::InternalError(
                "byte extrema strategy received an unsupported architecture".into(),
            ));
        }
    }
    if image.mode() != mode {
        return Err(PilError::InternalError(
            "byte extrema strategy received a different raster mode".into(),
        ));
    }
    let output = rank_filter_native_bytes(
        image.as_bytes(),
        image.width(),
        image.height(),
        channels,
        size,
        select_max,
    )?;
    crate::compute::pool_cpu::helpers::dynamic_image::from_raw_bytes(
        mode,
        image.width(),
        image.height(),
        output,
    )
    .ok_or_else(|| PilError::InternalError("byte extrema output mode shape mismatch".into()))
}

fn rank_filter_native_bytes(
    raw: &[u8],
    width: u32,
    height: u32,
    channels: u8,
    size: u32,
    select_max: bool,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(width, height, channels)?;
    if size == 0 || size.is_multiple_of(2) || raw.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "byte extrema request bypassed its prepared shape guard".into(),
        ));
    }
    if super::native_small_uniform_bytes(
        raw,
        width,
        height,
        usize::from(channels),
        super::SMALL_UNIFORM_BYTE_MAX_PIXELS,
    ) || super::native_zero_bytes(raw, width, height, usize::from(channels))
    {
        let mut output = vec![0; raw.len()];
        let (blocks, scalar_tail) =
            crate::compute::pool_cpu::helpers::bytes::vector_copy::copy_native_bytes(
                raw, &mut output,
            )
            .ok_or_else(|| PilError::InternalError("byte extrema identity copy shape mismatch".into()))?;
        crate::compute::record_pipeline_operation_path("native-copy");
        crate::compute::record_pipeline_operation_vector_blocks(blocks);
        crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
        return Ok(output);
    }
    let width = width as usize;
    let height = height as usize;
    let channels = usize::from(channels);
    let half = (size / 2) as usize;
    let mut horizontal = vec![0u8; dimensions.total_bytes()];
    let mut output = vec![0u8; dimensions.total_bytes()];
    for y in 0..height {
        let row_start = y * dimensions.row_stride();
        rank_filter_horizontal_row(
            raw,
            &mut horizontal[row_start..row_start + dimensions.row_stride()],
            row_start,
            width,
            channels,
            half,
            select_max,
        );
    }
    for y in 0..height {
        let row_start = y * dimensions.row_stride();
        rank_filter_vertical_row(
            &horizontal,
            &mut output[row_start..row_start + dimensions.row_stride()],
            dimensions.row_stride(),
            width,
            height,
            channels,
            half,
            select_max,
            y,
        );
    }
    let blocks = width.div_ceil(LANES) as u64 * channels as u64 * height as u64 * 2;
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    Ok(output)
}

#[inline]
fn rank_filter_identity(select_max: bool) -> u8 {
    if select_max { 0 } else { u8::MAX }
}

#[inline]
fn rank_filter_horizontal_window(
    raw: &[u8],
    row_start: usize,
    width: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    half: usize,
    window_offset: usize,
    select_max: bool,
) -> u8x16 {
    let mut values = [rank_filter_identity(select_max); LANES];
    let last = width - 1;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start
            .saturating_add(lane)
            .saturating_add(window_offset)
            .saturating_sub(half)
            .min(last);
        *value = raw[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

#[inline]
fn rank_filter_vertical_window(
    horizontal: &[u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    y: usize,
    half: usize,
    window_offset: usize,
    select_max: bool,
) -> u8x16 {
    let mut values = [rank_filter_identity(select_max); LANES];
    let source_y = y
        .saturating_add(window_offset)
        .saturating_sub(half)
        .min(height - 1);
    let row_start = source_y * row_stride;
    for (lane, value) in values.iter_mut().enumerate().take(active) {
        let source_x = x_start + lane;
        debug_assert!(source_x < width);
        *value = horizontal[row_start + source_x * channels + channel];
    }
    u8x16::new(values)
}

fn rank_filter_horizontal_row(
    raw: &[u8],
    output: &mut [u8],
    row_start: usize,
    width: usize,
    channels: usize,
    half: usize,
    select_max: bool,
) {
    let half = half.min(width - 1);
    let window_len = half.saturating_mul(2).saturating_add(1);
    for channel in 0..channels {
        let mut x = 0usize;
        while x < width {
            let active = (width - x).min(LANES);
            let mut selected = u8x16::splat(rank_filter_identity(select_max));
            for window_offset in 0..window_len {
                let values = rank_filter_horizontal_window(
                    raw,
                    row_start,
                    width,
                    channels,
                    channel,
                    x,
                    active,
                    half,
                    window_offset,
                    select_max,
                );
                selected = if select_max { selected.max(values) } else { selected.min(values) };
            }
            store_channel_vector(output, channels, channel, x, active, selected);
            x += active;
        }
    }
}

fn rank_filter_vertical_row(
    horizontal: &[u8],
    output: &mut [u8],
    row_stride: usize,
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    select_max: bool,
    y: usize,
) {
    let half = half.min(height - 1);
    let window_len = half.saturating_mul(2).saturating_add(1);
    for channel in 0..channels {
        let mut x = 0usize;
        while x < width {
            let active = (width - x).min(LANES);
            let mut selected = u8x16::splat(rank_filter_identity(select_max));
            for window_offset in 0..window_len {
                let values = rank_filter_vertical_window(
                    horizontal,
                    row_stride,
                    width,
                    height,
                    channels,
                    channel,
                    x,
                    active,
                    y,
                    half,
                    window_offset,
                    select_max,
                );
                selected = if select_max { selected.max(values) } else { selected.min(values) };
            }
            store_channel_vector(output, channels, channel, x, active, selected);
            x += active;
        }
    }
}

#[inline]
fn store_channel_vector(
    output: &mut [u8],
    channels: usize,
    channel: usize,
    x_start: usize,
    active: usize,
    values: u8x16,
) {
    for (lane, value) in values.to_array().into_iter().enumerate().take(active) {
        output[(x_start + lane) * channels + channel] = value;
    }
}
