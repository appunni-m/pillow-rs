//! Typed preparation and output assembly shared by grayscale mode strategies.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{FromPrimitive, GrayImage, ImageBuffer, Pixel};

pub(crate) struct Checked<'a, P: Pixel> {
    pub(crate) input: &'a ImageBuffer<P, Vec<P::Subpixel>>,
    pub(crate) dimensions: CheckedDims,
}

impl<'a, P: Pixel> Checked<'a, P> {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) fn new(input: &'a ImageBuffer<P, Vec<P::Subpixel>>) -> Result<Self, PilError> {
        let dimensions = CheckedDims::new_allow_empty(input.width(), input.height(), 1)?;
        Ok(Self { input, dimensions })
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn finish(width: u32, height: u32, bytes: Vec<u8>) -> Result<GrayImage, PilError> {
    if bytes.capacity() != 0 {
        crate::compute::record_pipeline_allocation(bytes.capacity());
    }
    GrayImage::from_raw(width, height, bytes)
        .ok_or_else(|| PilError::InternalError("pil_grayscale buffer mismatch".into()))
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Converts native high-depth RGB samples directly into the operation's L
/// output, without allocating or retagging through an intermediate RGB8 image.
pub(crate) fn native_rgb<S: Copy + ToGrayByte, const CHANNELS: usize>(
    source: &[S],
    width: u32,
    height: u32,
    dimensions: CheckedDims,
) -> Result<GrayImage, PilError> {
    let mut output = Vec::with_capacity(dimensions.total_pixels());
    for pixel in source
        .chunks_exact(CHANNELS)
        .take(dimensions.total_pixels())
    {
        let rgb = [
            pixel[0].to_gray_byte(),
            pixel[1].to_gray_byte(),
            pixel[2].to_gray_byte(),
        ];
        output.push(crate::color::grayscale_rgb_pixel(&rgb, 32768));
    }
    finish(width, height, output)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Converts native LA16 luma samples directly into the operation's L output.
pub(crate) fn native_luma_a16(
    source: &[u16],
    width: u32,
    height: u32,
    dimensions: CheckedDims,
) -> Result<GrayImage, PilError> {
    let mut output = Vec::with_capacity(dimensions.total_pixels());
    output.extend(
        source
            .chunks_exact(2)
            .take(dimensions.total_pixels())
            .map(|pixel| u8::from_primitive(pixel[0])),
    );
    finish(width, height, output)
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
/// Preserves Pillow's clipped I;16 grayscale behavior without an RGB8 raster.
pub(crate) fn native_luma16_clipped(
    source: &[u16],
    width: u32,
    height: u32,
    dimensions: CheckedDims,
) -> Result<GrayImage, PilError> {
    let mut output = Vec::with_capacity(dimensions.total_pixels());
    output.extend(
        source
            .iter()
            .take(dimensions.total_pixels())
            .map(|sample| (*sample).min(u16::from(u8::MAX)) as u8),
    );
    finish(width, height, output)
}

pub(crate) trait ToGrayByte: Copy {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    fn to_gray_byte(self) -> u8;
}

impl ToGrayByte for u16 {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[inline]
    fn to_gray_byte(self) -> u8 {
        u8::from_primitive(self)
    }
}

impl ToGrayByte for f32 {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    #[inline]
    fn to_gray_byte(self) -> u8 {
        u8::from_primitive(self)
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
#[cfg(any(
    target_arch = "x86_64",
    all(
        any(target_arch = "x86", target_arch = "x86_64"),
        any(
            target_feature = "sse2",
            target_feature = "ssse3",
            target_feature = "avx2"
        )
    ),
    all(target_arch = "wasm32", target_feature = "simd128"),
    all(target_arch = "aarch64", target_feature = "neon")
))]
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn vector_output(
    width: u32,
    height: u32,
    output: (Vec<u8>, u64, u64),
    path: &'static str,
) -> Result<GrayImage, PilError> {
    let (bytes, blocks, tail) = output;
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(tail);
    crate::compute::record_pipeline_operation_path(if blocks == 0 {
        "scalar-control"
    } else {
        path
    });
    finish(width, height, bytes)
}
