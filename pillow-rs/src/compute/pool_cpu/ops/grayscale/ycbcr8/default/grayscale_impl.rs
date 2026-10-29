//! Native Y extraction with the preserved portable packed-word loop.

use crate::checked_dims::CheckedDims;
use crate::raster::{GrayImage, RgbImage, Luma};

pub(crate) type Input = RgbImage;
pub(crate) type Kernel<'a> = &'a Input;
pub(crate) fn prepare(input: &Input) -> Kernel<'_> { input }
pub(crate) fn execute(input: Kernel<'_>) -> GrayImage {
    native_ycbcr_luma(input).unwrap_or_else(|| {
        let source = input.clone();
        GrayImage::from_fn(source.width(), source.height(), |x, y| {
            Luma([source.get_pixel(x, y)[0]])
        })
    })
}

fn native_ycbcr_luma(source: &RgbImage) -> Option<crate::raster::GrayImage> {
    let dims = CheckedDims::new(source.width(), source.height(), 3).ok()?;
    let pixels = dims.total_pixels();
    if pixels == 0 || source.as_raw().len() != dims.total_bytes() {
        return None;
    }
    // Default execution uses the original bounded portable word path. The
    // architecture strategies own optional vector kernels; complete groups
    // and this scalar tail preserve source ownership and materialization.
    let output = native_ycbcr_luma_packed_blocks(source.as_raw(), pixels);
    crate::compute::record_pipeline_allocation(pixels);
    crate::raster::GrayImage::from_raw(source.width(), source.height(), output)
}

/// Extract four Y samples per block using bounded word loads and stores. Each
/// complete block is exactly twelve bytes, so the loads stay within the source
/// allocation. The remainder contains complete three-byte pixels only.
fn native_ycbcr_luma_packed_blocks(source: &[u8], pixels: usize) -> Vec<u8> {
    let mut packed = Vec::<[u8; 4]>::with_capacity(pixels.div_ceil(4));
    let mut blocks = source.chunks_exact(12);
    packed.extend(blocks.by_ref().map(|block| {
        let first = u32::from_le_bytes(
            block[0..4]
                .try_into()
                .expect("a complete YCbCr block has four bytes"),
        );
        let second = u32::from_le_bytes(
            block[4..8]
                .try_into()
                .expect("a complete YCbCr block has four bytes"),
        );
        let third = u32::from_le_bytes(
            block[8..12]
                .try_into()
                .expect("a complete YCbCr block has four bytes"),
        );
        [
            first as u8,
            (first >> 24) as u8,
            (second >> 16) as u8,
            (third >> 8) as u8,
        ]
    }));
    let remainder = blocks.remainder();
    if !remainder.is_empty() {
        let mut tail = [0; 4];
        for (pixel, luma) in remainder.chunks_exact(3).zip(&mut tail) {
            *luma = pixel[0];
        }
        packed.push(tail);
    }
    // `[u8; 4]` has byte alignment, so flattening reuses this allocation.
    let mut output = packed.into_flattened();
    output.truncate(pixels);
    debug_assert_eq!(output.len(), pixels);
    output
}

