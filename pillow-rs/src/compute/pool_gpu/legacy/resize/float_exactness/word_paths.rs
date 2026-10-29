use super::{
    CheckedDims, DynamicImage, FilterCoeffs, FilterCoeffsF64, GPU_BUFFER_CAPACITY, PipelineOp,
    PixelMode, ResampleFilter, gpu_f_resize_f64_is_exact, gpu_f_resize_f64_sample_bits,
    gpu_pad_geometry, transpose_forward, transpose_output_dimensions,
};
use crate::raster::GenericImageView;
pub(in crate::compute::pool_gpu) fn gpu_f_resize_f64_pass_bits(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffsF64,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        usize::try_from(source_dimensions.1).ok()?
    } else {
        usize::try_from(source_dimensions.0).ok()?
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    if horizontal {
        for line in 0..line_count {
            for output_index in 0..output_count {
                result.push(gpu_f_resize_f64_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    true,
                    line,
                )?);
            }
        }
    } else {
        for output_index in 0..output_count {
            for line in 0..line_count {
                result.push(gpu_f_resize_f64_sample_bits(
                    bytes,
                    source_dimensions,
                    coeffs,
                    output_index,
                    false,
                    line,
                )?);
            }
        }
    }
    Some(result)
}

/// Relocate complete four-byte F words through an operation that does not
/// interpret the scalar as color or perform arithmetic.  Marker 9 uses this
/// between filtered resize stages so the next f64 reducer sees the exact
/// words Pillow materialized at the preceding geometry boundary.
#[inline(always)]
fn copy_f_resize_word(
    bytes: &[u8],
    output: &mut [u8],
    destination: usize,
    source: usize,
) -> Option<()> {
    let source_offset = source.checked_mul(4)?;
    let destination_offset = destination.checked_mul(4)?;
    let source_word = bytes.get(source_offset..source_offset.checked_add(4)?)?;
    output
        .get_mut(destination_offset..destination_offset.checked_add(4)?)?
        .copy_from_slice(source_word);
    Some(())
}

#[inline(always)]
fn gpu_f_resize_mirror_or_flip_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    expected: usize,
    flip_horizontal: bool,
    flip_vertical: bool,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    if width == 0 || height == 0 {
        return None;
    }
    let mut output = vec![0u8; expected];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let source_x = if flip_horizontal {
                width as usize - 1 - x
            } else {
                x
            };
            let source_y = if flip_vertical {
                height as usize - 1 - y
            } else {
                y
            };
            let destination = y.checked_mul(width as usize)?.checked_add(x)?;
            let source = source_y
                .checked_mul(width as usize)?
                .checked_add(source_x)?;
            copy_f_resize_word(bytes, &mut output, destination, source)?;
        }
    }
    Some((output, dimensions))
}

#[inline(always)]
fn gpu_f_resize_transpose_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    method: &crate::pipeline::TransposeMethod,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    if width == 0 || height == 0 {
        return None;
    }
    let output_dimensions = transpose_output_dimensions(method, width, height);
    let output_bytes = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
        .ok()?
        .total_bytes();
    let mut output = vec![0u8; output_bytes];
    for y in 0..height as usize {
        for x in 0..width as usize {
            let (output_x, output_y) = transpose_forward(method, width, height, x as u32, y as u32);
            let destination = usize::try_from(output_y)
                .ok()?
                .checked_mul(usize::try_from(output_dimensions.0).ok()?)?
                .checked_add(usize::try_from(output_x).ok()?)?;
            let source = y.checked_mul(width as usize)?.checked_add(x)?;
            copy_f_resize_word(bytes, &mut output, destination, source)?;
        }
    }
    Some((output, output_dimensions))
}

#[inline(always)]
fn gpu_f_resize_crop_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    if width == 0
        || height == 0
        || left >= right
        || top >= bottom
        || right > width
        || bottom > height
    {
        return None;
    }
    let output_dimensions = (right - left, bottom - top);
    let output_bytes = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
        .ok()?
        .total_bytes();
    let mut output = vec![0u8; output_bytes];
    for y in 0..output_dimensions.1 as usize {
        for x in 0..output_dimensions.0 as usize {
            let source = usize::try_from(top)
                .ok()?
                .checked_add(y)?
                .checked_mul(width as usize)?
                .checked_add(usize::try_from(left).ok()?.checked_add(x)?)?;
            let destination = y
                .checked_mul(output_dimensions.0 as usize)?
                .checked_add(x)?;
            copy_f_resize_word(bytes, &mut output, destination, source)?;
        }
    }
    Some((output, output_dimensions))
}

#[inline(always)]
fn gpu_f_resize_crop_border_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    border: u32,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    let border = border.checked_mul(2)?;
    if width == 0 || height == 0 || border >= width || border >= height {
        return None;
    }
    let output_dimensions = (width - border, height - border);
    let output_bytes = CheckedDims::new(output_dimensions.0, output_dimensions.1, 4)
        .ok()?
        .total_bytes();
    let mut output = vec![0u8; output_bytes];
    let border = usize::try_from(border).ok()?;
    for y in 0..output_dimensions.1 as usize {
        for x in 0..output_dimensions.0 as usize {
            let source = (y + border)
                .checked_mul(width as usize)?
                .checked_add(x + border)?;
            let destination = y
                .checked_mul(output_dimensions.0 as usize)?
                .checked_add(x)?;
            copy_f_resize_word(bytes, &mut output, destination, source)?;
        }
    }
    Some((output, output_dimensions))
}

#[inline(always)]
fn gpu_f_resize_offset_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    x: i32,
    y: i32,
    expected: usize,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    if width == 0 || height == 0 {
        return None;
    }
    let source_x = (-(i64::from(x))).rem_euclid(i64::from(width)) as usize;
    let source_y = (-(i64::from(y))).rem_euclid(i64::from(height)) as usize;
    let mut output = vec![0u8; expected];
    for destination_y in 0..height as usize {
        for destination_x in 0..width as usize {
            let source = ((destination_y + source_y) % height as usize)
                .checked_mul(width as usize)?
                .checked_add((destination_x + source_x) % width as usize)?;
            let destination = destination_y
                .checked_mul(width as usize)?
                .checked_add(destination_x)?;
            copy_f_resize_word(bytes, &mut output, destination, source)?;
        }
    }
    Some((output, dimensions))
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_relocate_words(
    bytes: &[u8],
    dimensions: (u32, u32),
    op: &PipelineOp,
) -> Option<(Vec<u8>, (u32, u32))> {
    let (width, height) = dimensions;
    let checked = CheckedDims::new(width, height, 4).ok()?;
    if checked.total_pixels() > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let expected = checked.total_bytes();
    if bytes.len() != expected {
        return None;
    }

    match op {
        PipelineOp::Duplicate => Some((bytes.to_vec(), dimensions)),
        PipelineOp::Mirror => {
            gpu_f_resize_mirror_or_flip_words(bytes, dimensions, expected, true, false)
        }
        PipelineOp::Flip => {
            gpu_f_resize_mirror_or_flip_words(bytes, dimensions, expected, false, true)
        }
        PipelineOp::Transpose { method } => gpu_f_resize_transpose_words(bytes, dimensions, method),
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => gpu_f_resize_crop_words(bytes, dimensions, *left, *top, *right, *bottom),
        PipelineOp::CropBorder { border } => {
            gpu_f_resize_crop_border_words(bytes, dimensions, *border)
        }
        PipelineOp::Offset { x, y } => {
            gpu_f_resize_offset_words(bytes, dimensions, *x, *y, expected)
        }
        _ => None,
    }
}

/// Materialize one exact nearest F resize pass for marker 9's subsequent
/// filtered stage.  The nearest table contains one unit-weight source word
/// per output coordinate; no scalar arithmetic is performed.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_nearest_pass_words(
    bytes: &[u8],
    source_dimensions: (u32, u32),
    coeffs: &FilterCoeffs,
    horizontal: bool,
) -> Option<Vec<u32>> {
    let source_width = usize::try_from(source_dimensions.0).ok()?;
    let source_height = usize::try_from(source_dimensions.1).ok()?;
    let output_count = coeffs.xmin.len();
    let line_count = if horizontal {
        source_height
    } else {
        source_width
    };
    let word_count = output_count.checked_mul(line_count)?;
    if word_count > GPU_BUFFER_CAPACITY as usize {
        return None;
    }
    let mut result = Vec::new();
    result.try_reserve(word_count).ok()?;
    for line in 0..line_count {
        for output_index in 0..output_count {
            let source_start = usize::try_from(*coeffs.xmin.get(output_index)?).ok()?;
            if *coeffs.count.get(output_index)? != 1
                || *coeffs.weights.get(*coeffs.offsets.get(output_index)?)? != 1 << 22
            {
                return None;
            }
            let coordinate = source_start;
            let source_pixel = if horizontal {
                if coordinate >= source_width {
                    return None;
                }
                line.checked_mul(source_width)?.checked_add(coordinate)?
            } else {
                if coordinate >= source_height {
                    return None;
                }
                coordinate.checked_mul(source_width)?.checked_add(line)?
            };
            let offset = source_pixel.checked_mul(4)?;
            let word = bytes.get(offset..offset.checked_add(4)?)?;
            result.push(u32::from_le_bytes([word[0], word[1], word[2], word[3]]));
        }
    }
    Some(result)
}

/// Admit a heterogeneous F-mode Pad whose contain resize is already covered
/// by marker 9.  Pad adds no sample arithmetic after that resize: its final
/// pass only copies complete four-byte words into the host-chosen canvas and
/// writes the resolved scalar fill word elsewhere.  Keep this proof narrow
/// to one non-nearest Pad with a changed contain axis.  A prefix made only of
/// PutData(F) replacements is allowed because marker 9 already proves those
/// raw word uploads; other prefixes retain their existing host paths.
pub(in crate::compute::pool_gpu) fn gpu_f_pad_f64_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let (op, prefix) = ops.split_last().expect("non-empty Pad proof ops");
    let PipelineOp::Pad { w, h, filter, .. } = op else {
        return false;
    };
    if prefix.iter().any(|op| {
        !matches!(
            op,
            PipelineOp::PutData {
                mode: PixelMode::F,
                ..
            }
        )
    }) {
        return false;
    }
    if matches!(filter, ResampleFilter::Nearest) {
        return false;
    }
    let source_dimensions = image.dimensions();
    let Some(((resize_w, resize_h), _)) =
        gpu_pad_geometry(op, source_dimensions.0, source_dimensions.1)
    else {
        return false;
    };
    if (resize_w, resize_h) == source_dimensions
        || CheckedDims::new(resize_w, resize_h, 1)
            .ok()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
        || CheckedDims::new(*w, *h, 1)
            .ok()
            .is_none_or(|dims| dims.total_pixels() > GPU_BUFFER_CAPACITY as usize)
    {
        return false;
    }
    // Reuse the complete marker-9 proof for the contain resize.  The Pad
    // placement shader is word-copy/fill only, so no second arithmetic proof
    // is necessary here.
    let mut resize_ops = prefix.to_vec();
    resize_ops.push(PipelineOp::Resize {
        w: resize_w,
        h: resize_h,
        filter: *filter,
    });
    gpu_f_resize_f64_is_exact(&resize_ops, image, logical_mode)
}
