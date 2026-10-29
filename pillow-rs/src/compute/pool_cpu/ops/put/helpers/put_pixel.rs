//! Native typed pixel writes; source order and defensive batch errors are unchanged.

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

pub(crate) fn facts<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<P, Vec<u8>>, write: PixelWrite,
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(), height: image.height(), byte_len: Some(image.as_raw().len()),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::PutPixel { x: write.x, y: write.y },
    }
}

pub(crate) fn batch_facts<P: Pixel<Subpixel = u8>>(
    image: &ImageBuffer<P, Vec<u8>>, ops: &[PipelineOp],
) -> StrategyInputs {
    StrategyInputs {
        width: image.width(), height: image.height(), byte_len: Some(image.as_raw().len()),
        palette: PaletteKnowledge::Unknown,
        request: RequestFacts::PutPixelBatch { count: ops.len(), writes: PixelWriteRunFacts::Unknown },
    }
}

/// Only the RGBA specialization needs this inspection; serial preparation does not scan.
#[cfg(any(
    all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon"),
))]
pub(crate) fn rgba_run_facts(
    image: &crate::raster::RgbaImage, ops: &[PipelineOp],
) -> PixelWriteRunFacts {
    let Some(expected_bytes) = (image.width() as usize).checked_mul(image.height() as usize)
        .and_then(|pixels| pixels.checked_mul(4)) else {
        return PixelWriteRunFacts::Ineligible;
    };
    if ops.len() < 2 || expected_bytes < 16 || image.as_raw().len() != expected_bytes {
        return PixelWriteRunFacts::Ineligible;
    }
    if ops.iter().all(|op| matches!(op,
        PipelineOp::PutPixel { x, y, palette_index: false, .. }
            if *x < image.width() && *y < image.height()
    )) {
        PixelWriteRunFacts::NativeRgba
    } else {
        PixelWriteRunFacts::Ineligible
    }
}

#[inline]
pub(crate) fn write<P: Pixel<Subpixel = u8>>(
    input: PixelInput<'_, P>, pixel: P,
) -> ImageBuffer<P, Vec<u8>> {
    let mut output = input.image.clone();
    output.put_pixel(input.write.x, input.write.y, pixel);
    output
}

#[inline]
pub(crate) fn write_batch<P: Pixel<Subpixel = u8>>(
    input: BatchInput<'_, P>, mut pixel_from_color: impl FnMut((u8, u8, u8, u8)) -> P,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError> {
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
