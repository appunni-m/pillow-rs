//! SIMD Reduce family admission, vector kernels, and result materialization.

use super::{
    SimdImageShape, SimdLayout, native_copy_image_bytes, native_copy_layout,
    native_zero_byte_image, shape_native_copy_channels, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
#[cfg(feature = "parallel")]
use std::sync::atomic::{AtomicBool, Ordering};
use wide::u32x8;

/// Return the native byte layout and whether the final channel is alpha for
/// `Image.reduce`. CMYK is four stored color samples, not RGBA; HSV and YCbCr
/// are three independent native samples. Keeping this distinction in the
/// admission contract prevents the reduction kernel from premultiplying a
/// color channel that only happens to occupy the fourth byte.
pub(super) fn native_reduce_layout(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, false)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, true)),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some((3, false))
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, true)),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, false)),
        _ => None,
    }
}

pub(super) fn shape_native_reduce_layout(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, bool)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, false)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, true)),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some((3, false))
        }
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, true)),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, false)),
        _ => None,
    }
}

const SIMD_REDUCE_LANES: usize = 8;
#[cfg(feature = "parallel")]
const SIMD_REDUCE_PARALLEL_PIXEL_THRESHOLD: usize = 32 * 32;

#[derive(Clone, Copy)]
struct NativeReduceAverage {
    multiplier: u32,
    amend: u32,
}

/// Geometry and fixed-point divisors shared by every output pixel in one
/// native byte reduction. Only the four edge classes can use different block
/// dimensions; their divisors do not need to be recomputed per SIMD lane.
#[derive(Clone, Copy)]
pub(in crate::compute::pool_simd::ops::adapters) struct NativeReduceGeometry {
    width: usize,
    x_factor: usize,
    y_factor: usize,
    main_width: usize,
    main_height: usize,
    right_width: usize,
    bottom_height: usize,
    tail_x: usize,
    tail_y: usize,
    full: Option<NativeReduceAverage>,
    right: Option<NativeReduceAverage>,
    bottom: Option<NativeReduceAverage>,
    corner: Option<NativeReduceAverage>,
}

impl NativeReduceGeometry {
    fn new(width: usize, height: usize, x_factor: usize, y_factor: usize) -> Option<Self> {
        if x_factor == 0 || y_factor == 0 {
            return None;
        }
        let main_width = width / x_factor;
        let main_height = height / y_factor;
        let right_width = width % x_factor;
        let bottom_height = height % y_factor;
        let tail_x = main_width.checked_mul(x_factor)?;
        let tail_y = main_height.checked_mul(y_factor)?;
        let average = |block_width: usize, block_height: usize| {
            let count = block_width.checked_mul(block_height)?;
            // The u32 vector arithmetic below is exact while sum+amend fits
            // in u32. Larger public factors remain valid CPU work but stay
            // outside this SIMD contract.
            if count == 0 || count > (u32::MAX / 256) as usize {
                return None;
            }
            Some(NativeReduceAverage {
                multiplier: ((1u128 << 32) / (count as u128 * 256)) as u32,
                amend: (count / 2) as u32,
            })
        };
        let full = if main_width != 0 && main_height != 0 {
            Some(average(x_factor, y_factor)?)
        } else {
            None
        };
        let right = if right_width != 0 && main_height != 0 {
            Some(average(right_width, y_factor)?)
        } else {
            None
        };
        let bottom = if main_width != 0 && bottom_height != 0 {
            Some(average(x_factor, bottom_height)?)
        } else {
            None
        };
        let corner = if right_width != 0 && bottom_height != 0 {
            Some(average(right_width, bottom_height)?)
        } else {
            None
        };
        Some(Self {
            width,
            x_factor,
            y_factor,
            main_width,
            main_height,
            right_width,
            bottom_height,
            tail_x,
            tail_y,
            full,
            right,
            bottom,
            corner,
        })
    }

    #[inline]
    fn block_parameters(
        &self,
        x: usize,
        y: usize,
    ) -> Option<(usize, usize, usize, usize, u32, u32)> {
        let is_right = x >= self.main_width;
        let is_bottom = y >= self.main_height;
        let (block_width, source_x) = if is_right {
            (self.right_width, self.tail_x)
        } else {
            (self.x_factor, x.checked_mul(self.x_factor)?)
        };
        let (block_height, source_y) = if is_bottom {
            (self.bottom_height, self.tail_y)
        } else {
            (self.y_factor, y.checked_mul(self.y_factor)?)
        };
        if block_width == 0 || block_height == 0 {
            return None;
        }
        let average = match (is_right, is_bottom) {
            (false, false) => self.full,
            (true, false) => self.right,
            (false, true) => self.bottom,
            (true, true) => self.corner,
        }?;
        Some((
            source_x,
            source_y,
            block_width,
            block_height,
            average.multiplier,
            average.amend,
        ))
    }
}

#[inline]
fn native_reduce_supported_buffer(img: &DynamicImage, channels: usize, mode: Option<&str>) -> bool {
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    native_reduce_layout(img, mode).is_some_and(|(layout, _)| layout == channels)
        && img.as_bytes().len() == expected_bytes
        && expected_bytes != 0
}

pub(super) fn native_reduce_supported_for_image(
    img: &DynamicImage,
    x_factor: u32,
    y_factor: u32,
    mode: Option<&str>,
) -> bool {
    if x_factor == 0 || y_factor == 0 {
        return false;
    }
    let Some((channels, _premultiplied_alpha)) = native_reduce_layout(img, mode) else {
        // A 1×1 reduction is an exact native copy even for indexed/raw-byte
        // logical modes. Any actual averaging needs a standard byte mode so
        // alpha and channel semantics remain explicit.
        return x_factor == 1
            && y_factor == 1
            && native_copy_layout(img, mode).is_some_and(|channels| {
                let expected = (img.width() as usize)
                    .checked_mul(img.height() as usize)
                    .and_then(|pixels| pixels.checked_mul(channels));
                expected == Some(img.as_bytes().len()) && expected != Some(0)
            });
    };
    if !native_reduce_supported_buffer(img, channels, mode) {
        return false;
    }
    if x_factor == 1 && y_factor == 1 {
        return true;
    }
    let width = img.width() as usize;
    let height = img.height() as usize;
    let output_width = width.div_ceil(x_factor as usize);
    let output_height = height.div_ceil(y_factor as usize);
    output_width
        .checked_mul(output_height)
        .is_some_and(|pixels| pixels != 0)
        && output_height != 0
        && NativeReduceGeometry::new(width, height, x_factor as usize, y_factor as usize).is_some()
}

pub(super) fn native_reduce_supported_for_shape(
    shape: SimdImageShape,
    x_factor: u32,
    y_factor: u32,
    mode: Option<&str>,
) -> bool {
    if x_factor == 0 || y_factor == 0 {
        return false;
    }
    let Some((channels, _premultiplied_alpha)) = shape_native_reduce_layout(shape, mode) else {
        return x_factor == 1
            && y_factor == 1
            && shape_native_copy_channels(shape, mode).is_some_and(|channels| {
                shape
                    .width
                    .checked_mul(shape.height)
                    .and_then(|pixels| pixels.checked_mul(channels as u32))
                    .is_some_and(|bytes| bytes != 0)
            });
    };
    let Some(expected_bytes) = shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
    else {
        return false;
    };
    if expected_bytes == 0 {
        return false;
    }
    if x_factor == 1 && y_factor == 1 {
        return true;
    }
    let output_width = shape.width.div_ceil(x_factor);
    let output_height = shape.height.div_ceil(y_factor);
    output_width
        .checked_mul(output_height)
        .is_some_and(|pixels| pixels != 0)
        && output_height != 0
        && NativeReduceGeometry::new(
            shape.width as usize,
            shape.height as usize,
            x_factor as usize,
            y_factor as usize,
        )
        .is_some()
}

mod kernels;
#[cfg(test)]
use kernels::{
    native_reduce_3byte_2x2_vector_block, native_reduce_average, native_reduce_pixel_sums,
    native_reduce_rgb_3x5_vector_block,
};
pub(super) use kernels::{native_reduce_scalar_pixel, native_reduce_vector_block};

#[inline]

/// Reduce a sufficiently sparse, even-sized RGB image by touching only the
/// nonzero 2x2 blocks. The public thumbnail path uses this for its common
/// reducing-gap factor; dense inputs and partial edge blocks retain the
/// general native reducer below.
pub(super) fn native_reduce_sparse_rgb_2x2(img: &DynamicImage) -> Option<(Vec<u8>, u32, u32)> {
    let (width, height) = (img.width() as usize, img.height() as usize);
    let source = img.as_bytes();
    let source_pixels = width.checked_mul(height)?;
    if width == 0
        || height == 0
        || width % 2 != 0
        || height % 2 != 0
        || source_pixels < 512 * 512
        || source.len() != source_pixels.checked_mul(3)?
    {
        return None;
    }

    let output_width = width / 2;
    let output_height = height / 2;
    let maximum_nonzero_pixels = source_pixels / 1024;
    let mut nonzero_pixels = 0usize;
    let mut block_sums = std::collections::HashMap::<usize, [u16; 3]>::new();
    let mut accumulate_pixel = |source_index: usize, pixel: &[u8]| {
        if pixel[0] == 0 && pixel[1] == 0 && pixel[2] == 0 {
            return true;
        }
        nonzero_pixels += 1;
        if nonzero_pixels > maximum_nonzero_pixels {
            return false;
        }
        let source_x = source_index % width;
        let source_y = source_index / width;
        let block_index = (source_y / 2) * output_width + source_x / 2;
        let sums = block_sums.entry(block_index).or_insert([0; 3]);
        sums[0] += u16::from(pixel[0]);
        sums[1] += u16::from(pixel[1]);
        sums[2] += u16::from(pixel[2]);
        true
    };

    // Three native words cover eight RGB pixels. Skip entirely black groups
    // with one bounded check, then visit the pixels in nonzero groups so each
    // source contribution is assigned to its exact Pillow reduction block.
    let grouped_pixels = source.len() / 24 * 8;
    for group in 0..source.len() / 24 {
        let byte_start = group * 24;
        let bytes = &source[byte_start..byte_start + 24];
        let is_zero = [0usize, 8, 16].into_iter().all(|offset| {
            u64::from_ne_bytes(
                bytes[offset..offset + 8]
                    .try_into()
                    .expect("each sparse RGB scan word has eight bytes"),
            ) == 0
        });
        if is_zero {
            continue;
        }
        for (pixel_offset, pixel) in bytes.chunks_exact(3).enumerate() {
            if !accumulate_pixel(group * 8 + pixel_offset, pixel) {
                return None;
            }
        }
    }
    for (pixel_offset, pixel) in source[grouped_pixels * 3..].chunks_exact(3).enumerate() {
        if !accumulate_pixel(grouped_pixels + pixel_offset, pixel) {
            return None;
        }
    }

    let output_len = output_width.checked_mul(output_height)?.checked_mul(3)?;
    let mut output = vec![0u8; output_len];
    for (block_index, sums) in block_sums {
        let output_index = block_index.checked_mul(3)?;
        output[output_index] = ((sums[0] + 2) >> 2) as u8;
        output[output_index + 1] = ((sums[1] + 2) >> 2) as u8;
        output[output_index + 2] = ((sums[2] + 2) >> 2) as u8;
    }
    Some((output, output_width as u32, output_height as u32))
}

#[inline]
fn native_reduce_process_row(
    source: &[u8],
    geometry: &NativeReduceGeometry,
    output_width: usize,
    channels: usize,
    premultiplied_alpha: bool,
    rgb_3x5_fast_path: bool,
    y: usize,
    row: &mut [u8],
) -> bool {
    let vector_limit = if output_width < SIMD_REDUCE_LANES {
        output_width
    } else {
        output_width / SIMD_REDUCE_LANES * SIMD_REDUCE_LANES
    };
    for local in (0..vector_limit).step_by(SIMD_REDUCE_LANES) {
        let valid_pixels = (output_width - local).min(SIMD_REDUCE_LANES);
        let Some(block) = native_reduce_vector_block(
            source,
            geometry,
            local,
            y,
            valid_pixels,
            channels,
            premultiplied_alpha,
            rgb_3x5_fast_path,
        ) else {
            return false;
        };
        let dst_start = local * channels;
        let dst_end = dst_start + valid_pixels * channels;
        let Some(dst) = row.get_mut(dst_start..dst_end) else {
            return false;
        };
        dst.copy_from_slice(&block[..valid_pixels * channels]);
    }
    for local in vector_limit..output_width {
        let Some(pixel) =
            native_reduce_scalar_pixel(source, geometry, channels, premultiplied_alpha, local, y)
        else {
            return false;
        };
        let dst_start = local * channels;
        let dst_end = dst_start + channels;
        let Some(dst) = row.get_mut(dst_start..dst_end) else {
            return false;
        };
        dst.copy_from_slice(&pixel[..channels]);
    }
    true
}

fn native_reduce_zero_output(
    output: Vec<u8>,
    output_width: usize,
    output_height: usize,
    output_pixels: usize,
) -> (Vec<u8>, u32, u32, u64, u64) {
    (
        output,
        output_width as u32,
        output_height as u32,
        output_pixels.div_ceil(SIMD_REDUCE_LANES) as u64,
        0,
    )
}

fn native_reduce_row_telemetry(output_width: usize, output_height: usize) -> (u64, u64) {
    let vector_blocks_per_row = if output_width < SIMD_REDUCE_LANES {
        1
    } else {
        output_width / SIMD_REDUCE_LANES
    };
    let scalar_tail_per_row = if output_width < SIMD_REDUCE_LANES {
        0
    } else {
        output_width % SIMD_REDUCE_LANES
    };
    (
        (vector_blocks_per_row * output_height) as u64,
        (scalar_tail_per_row * output_height) as u64,
    )
}

pub(super) fn native_reduce_bytes(
    img: &DynamicImage,
    channels: usize,
    premultiplied_alpha: bool,
    x_factor: u32,
    y_factor: u32,
    rgb_3x5_fast_path: bool,
) -> Option<(Vec<u8>, u32, u32, u64, u64)> {
    let width = img.width() as usize;
    let height = img.height() as usize;
    let x_factor = usize::try_from(x_factor).ok()?;
    let y_factor = usize::try_from(y_factor).ok()?;
    let output_width = width.div_ceil(x_factor);
    let output_height = height.div_ceil(y_factor);
    let output_len = output_width
        .checked_mul(output_height)?
        .checked_mul(channels)?;
    let source_len = width.checked_mul(height)?.checked_mul(channels)?;
    let source = img.as_bytes();
    if source.len() != source_len || output_height == 0 {
        return None;
    }
    let output_pixels = output_width.checked_mul(output_height)?;
    if native_zero_byte_image(img, channels) {
        return Some(native_reduce_zero_output(
            vec![0u8; output_len],
            output_width,
            output_height,
            output_pixels,
        ));
    }
    let geometry = NativeReduceGeometry::new(width, height, x_factor, y_factor)?;
    let mut output = vec![0u8; output_len];
    let (vector_blocks, scalar_tail) = native_reduce_row_telemetry(output_width, output_height);

    // Reduction output rows are independent. Keep their SIMD arithmetic and
    // exact source gathers while traversing rows serially; Parallel CPU is a
    // distinct backend and does not change the SIMD scheduler.
    #[cfg(feature = "parallel")]
    if output_pixels >= SIMD_REDUCE_PARALLEL_PIXEL_THRESHOLD {
        let failed = AtomicBool::new(false);
        super::simd_rows_mut_serial!(
            &mut output,
            output_width * channels,
            output_height,
            |_row_start, _row_end, y, row| {
                if !native_reduce_process_row(
                    source,
                    &geometry,
                    output_width,
                    channels,
                    premultiplied_alpha,
                    rgb_3x5_fast_path,
                    y as usize,
                    row,
                ) {
                    failed.store(true, Ordering::Relaxed);
                }
            }
        );
        if failed.load(Ordering::Relaxed) {
            return None;
        }
    } else {
        for y in 0..output_height {
            let row_start = y * output_width * channels;
            if !native_reduce_process_row(
                source,
                &geometry,
                output_width,
                channels,
                premultiplied_alpha,
                rgb_3x5_fast_path,
                y,
                &mut output[row_start..row_start + output_width * channels],
            ) {
                return None;
            }
        }
    }

    #[cfg(not(feature = "parallel"))]
    for y in 0..output_height {
        let row_start = y * output_width * channels;
        if !native_reduce_process_row(
            source,
            &geometry,
            output_width,
            channels,
            premultiplied_alpha,
            rgb_3x5_fast_path,
            y,
            &mut output[row_start..row_start + output_width * channels],
        ) {
            return None;
        }
    }
    Some((
        output,
        output_width as u32,
        output_height as u32,
        vector_blocks,
        scalar_tail,
    ))
}

pub fn simd_reduce(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Reduce { x_factor, y_factor } = op else {
        return Err(PilError::ValueError("expected Reduce op".into()));
    };
    if *x_factor == 0 || *y_factor == 0 {
        return Err(simd_unsupported("Reduce"));
    }
    if *x_factor == 1 && *y_factor == 1 {
        return native_copy_image_bytes(img, mode)?.ok_or_else(|| simd_unsupported("Reduce"));
    }
    let (channels, premultiplied_alpha) =
        native_reduce_layout(img, mode).ok_or_else(|| simd_unsupported("Reduce"))?;
    let Some((output, width, height, vector_blocks, scalar_tail)) = native_reduce_bytes(
        img,
        channels,
        premultiplied_alpha,
        *x_factor,
        *y_factor,
        matches!(mode, None | Some("RGB")) && matches!(img, DynamicImage::ImageRgb8(_)),
    ) else {
        return Err(simd_unsupported("Reduce"));
    };
    if vector_blocks == 0 {
        return Err(simd_unsupported("Reduce"));
    }
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(width, height, output, channels)
}

#[cfg(test)]
mod tests;
