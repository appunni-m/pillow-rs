//! Native typed pixel writes; source order and defensive batch errors are unchanged.

pub(crate) use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::update_hidden_lanes;
use crate::compute::registry::support::{
    PaletteKnowledge, PixelWriteRunFacts, RequestFacts, StrategyInputs,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{GenericImageView, ImageBuffer, Pixel};

/// Color normalization and single-write coordinate validation belong to Image's caller.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PixelWrite {
    pub(crate) x: u32,
    pub(crate) y: u32,
    pub(crate) color: (u8, u8, u8, u8),
}

pub(crate) struct PixelInput<'a, P: Pixel<Subpixel = u8>> {
    pub(crate) image: &'a ImageBuffer<P, Vec<u8>>,
    pub(crate) write: PixelWrite,
}

pub(crate) struct BatchInput<'a, P: Pixel<Subpixel = u8>> {
    pub(crate) image: &'a ImageBuffer<P, Vec<u8>>,
    pub(crate) ops: &'a [PipelineOp],
}

pub(crate) fn write_native_bytes(
    samples: &mut [u8],
    pixel_index: usize,
    bytes: &[u8],
) -> Result<(), PilError> {
    let start = pixel_index
        .checked_mul(bytes.len())
        .ok_or_else(|| PilError::DimensionError("putdata byte offset overflow".into()))?;
    let end = start
        .checked_add(bytes.len())
        .ok_or_else(|| PilError::DimensionError("putdata byte offset overflow".into()))?;
    let destination = samples
        .get_mut(start..end)
        .ok_or_else(|| PilError::InternalError("putdata storage offset out of bounds".into()))?;
    destination.copy_from_slice(bytes);
    Ok(())
}

pub(crate) fn write_native_luma16(
    samples: &mut [u16],
    pixel_index: usize,
    sample: u16,
) -> Result<(), PilError> {
    let pixel = samples
        .get_mut(pixel_index)
        .ok_or_else(|| PilError::InternalError("putpixel I;16 offset out of bounds".into()))?;
    *pixel = sample;
    Ok(())
}

pub(crate) fn facts<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<P, Vec<u8>>,
    write: PixelWrite,
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_raw().len()),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::PutPixel {
            x: write.x,
            y: write.y,
        },
    }
}

pub(crate) fn batch_facts<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<P, Vec<u8>>,
    ops: &[PipelineOp],
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(),
        height: image.height(),
        byte_len: Some(image.as_raw().len()),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::PutPixelBatch {
            count: ops.len(),
            writes: PixelWriteRunFacts::Unknown,
        },
    }
}

/// Only the RGBA specialization needs this inspection; serial preparation does not scan.
#[cfg(any(
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse2"
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) fn rgba_run_facts(
    image: &crate::raster::RgbaImage,
    ops: &[PipelineOp],
) -> PixelWriteRunFacts {
    let Some(expected_bytes) = (image.width() as usize)
        .checked_mul(image.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4))
    else {
        return PixelWriteRunFacts::Ineligible;
    };
    if ops.len() < 2 || expected_bytes < 16 || image.as_raw().len() != expected_bytes {
        return PixelWriteRunFacts::Ineligible;
    }
    if ops.iter().all(|op| {
        matches!(op,
            PipelineOp::PutPixel { x, y, palette_index: false, .. }
                if *x < image.width() && *y < image.height()
        )
    }) {
        PixelWriteRunFacts::NativeRgba
    } else {
        PixelWriteRunFacts::Ineligible
    }
}

#[inline]
pub(crate) fn write<P: Pixel<Subpixel = u8>>(
    input: PixelInput<'_, P>,
    pixel: P,
) -> ImageBuffer<P, Vec<u8>> {
    // This logical-only writer is for modes without hidden physical lanes;
    // compact 2/3-channel modes use write_with_hidden_lanes below.
    let mut output = input.image.clone();
    output.put_pixel(input.write.x, input.write.y, pixel);
    output
}

pub(crate) fn write_with_hidden_lanes<P, F>(
    input: PixelInput<'_, P>,
    pixel: P,
    lane_count: usize,
    lane_values: [u8; 2],
    fallback: F,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
{
    let x = input.write.x;
    let y = input.write.y;
    let index = pixel_index(input.image.width(), x, y)?;
    let default = fallback(&pixel);
    let mut output = input.image.clone();
    let mut hidden_lanes = output.take_hidden_pixel_lanes();
    output.put_pixel(x, y, pixel);
    update_hidden_lanes(
        &output,
        &mut hidden_lanes,
        index,
        lane_count,
        lane_values,
        default,
        &fallback,
    )?;
    if !output.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "PutPixel hidden lane extent did not match its image".into(),
        ));
    }
    Ok(output)
}

/// Attach the mode-owned physical lanes after an architecture kernel rebuilt the logical buffer.
pub(crate) fn preserve_hidden_lanes_after_write<P, F>(
    source: &ImageBuffer<P, Vec<u8>>,
    mut output: ImageBuffer<P, Vec<u8>>,
    write: PixelWrite,
    lane_count: usize,
    lane_values: [u8; 2],
    default: [u8; 2],
    fallback: F,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
    F: Fn(&P) -> [u8; 2],
{
    let index = pixel_index(source.width(), write.x, write.y)?;
    let mut hidden_lanes = source.hidden_pixel_lanes().cloned();
    update_hidden_lanes(
        source,
        &mut hidden_lanes,
        index,
        lane_count,
        lane_values,
        default,
        &fallback,
    )?;
    if !output.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "PutPixel hidden lane extent did not match its image".into(),
        ));
    }
    Ok(output)
}

#[inline]
pub(crate) fn write_batch<P: Pixel<Subpixel = u8>>(
    input: BatchInput<'_, P>,
    mut pixel_from_color: impl FnMut((u8, u8, u8, u8)) -> P,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
    // Compact 2/3-channel mode owners use write_batch_with_hidden_lanes below.
    let mut output = input.image.clone();
    for op in input.ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return Err(PilError::ValueError("expected PutPixel op".into()));
        };
        if !output.in_bounds(*x, *y) {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        output.put_pixel(*x, *y, pixel_from_color(*color));
    }
    Ok(output)
}

pub(crate) fn write_batch_with_hidden_lanes<P, F, G>(
    input: BatchInput<'_, P>,
    mut pixel_from_color: F,
    mut lanes_from_color: G,
    lane_count: usize,
    fallback: impl Fn(&P) -> [u8; 2],
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
    F: FnMut((u8, u8, u8, u8)) -> P,
    G: FnMut((u8, u8, u8, u8)) -> [u8; 2],
{
    let mut output = input.image.clone();
    let mut hidden_lanes = output.take_hidden_pixel_lanes();
    for op in input.ops {
        let PipelineOp::PutPixel { x, y, color, .. } = op else {
            return Err(PilError::ValueError("expected PutPixel op".into()));
        };
        if !output.in_bounds(*x, *y) {
            return Err(PilError::IndexError("image index out of range".into()));
        }
        let color = *color;
        let pixel = pixel_from_color(color);
        let lane_values = lanes_from_color(color);
        let default = fallback(&pixel);
        let index = pixel_index(output.width(), *x, *y)?;
        output.put_pixel(*x, *y, pixel);
        update_hidden_lanes(
            &output,
            &mut hidden_lanes,
            index,
            lane_count,
            lane_values,
            default,
            &fallback,
        )?;
    }
    if !output.replace_hidden_pixel_lanes(hidden_lanes) {
        return Err(PilError::InternalError(
            "PutPixel batch hidden lane extent did not match its image".into(),
        ));
    }
    Ok(output)
}

fn pixel_index(width: u32, x: u32, y: u32) -> Result<usize, PilError> {
    usize::try_from(y)
        .ok()
        .and_then(|y| {
            usize::try_from(width)
                .ok()
                .and_then(|width| y.checked_mul(width))
        })
        .and_then(|row| usize::try_from(x).ok().and_then(|x| row.checked_add(x)))
        .ok_or_else(|| PilError::DimensionError("PutPixel pixel index overflow".into()))
}
