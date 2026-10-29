//! Portable native-byte YCbCr luma extraction for the CPU backend.

use crate::checked_dims::CheckedDims;
use crate::raster::{DynamicImage, GrayImage};

/// Copy Y from exact native YCbCr byte triples. Mismatched storage returns
/// `None`; the family must not convert another mode to reach this kernel.
pub(super) fn luma(image: &DynamicImage) -> Option<GrayImage> {
    let DynamicImage::ImageRgb8(source) = image else {
        return None;
    };
    let dims = CheckedDims::new(image.width(), image.height(), 3).ok()?;
    let pixels = dims.total_pixels();
    if pixels == 0 {
        return Some(GrayImage::new(image.width(), image.height()));
    }
    if source.as_raw().len() != dims.total_bytes() {
        return None;
    }
    // Keep CPU execution serial and portable. SSSE3 belongs to the explicit
    // SIMD family kernel; this bounded word path handles complete groups and
    // its scalar tail without changing source ownership or materialization.
    let output = packed_luma_blocks(source.as_raw(), pixels);
    crate::compute::record_pipeline_allocation(pixels);
    GrayImage::from_raw(image.width(), image.height(), output)
}

/// Extract four Y samples per block using bounded word loads and stores. Each
/// complete block is exactly twelve bytes, so the loads stay within the source
/// allocation. The remainder contains complete three-byte pixels only.
pub(super) fn packed_luma_blocks(source: &[u8], pixels: usize) -> Vec<u8> {
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
