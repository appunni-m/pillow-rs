//! float exactness for GPU resize exactness.

use super::super::*;

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
    let copy_word = |output: &mut [u8], destination: usize, source: usize| -> Option<()> {
        let source_offset = source.checked_mul(4)?;
        let destination_offset = destination.checked_mul(4)?;
        let source_word = bytes.get(source_offset..source_offset.checked_add(4)?)?;
        output
            .get_mut(destination_offset..destination_offset.checked_add(4)?)?
            .copy_from_slice(source_word);
        Some(())
    };

    match op {
        PipelineOp::Duplicate => Some((bytes.to_vec(), dimensions)),
        PipelineOp::Mirror | PipelineOp::Flip => {
            if width == 0 || height == 0 {
                return None;
            }
            let mut output = vec![0u8; expected];
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let source_x = if matches!(op, PipelineOp::Mirror) {
                        width as usize - 1 - x
                    } else {
                        x
                    };
                    let source_y = if matches!(op, PipelineOp::Flip) {
                        height as usize - 1 - y
                    } else {
                        y
                    };
                    let destination = y.checked_mul(width as usize)?.checked_add(x)?;
                    let source = source_y
                        .checked_mul(width as usize)?
                        .checked_add(source_x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, dimensions))
        }
        PipelineOp::Transpose { method } => {
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
                    let (output_x, output_y) =
                        transpose_forward(method, width, height, x as u32, y as u32);
                    let destination = usize::try_from(output_y)
                        .ok()?
                        .checked_mul(usize::try_from(output_dimensions.0).ok()?)?
                        .checked_add(usize::try_from(output_x).ok()?)?;
                    let source = y.checked_mul(width as usize)?.checked_add(x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, output_dimensions))
        }
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            if width == 0
                || height == 0
                || *left >= *right
                || *top >= *bottom
                || *right > width
                || *bottom > height
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
                    let source = usize::try_from(*top)
                        .ok()?
                        .checked_add(y)?
                        .checked_mul(width as usize)?
                        .checked_add(usize::try_from(*left).ok()?.checked_add(x)?)?;
                    let destination = y
                        .checked_mul(output_dimensions.0 as usize)?
                        .checked_add(x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, output_dimensions))
        }
        PipelineOp::CropBorder { border } => {
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
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, output_dimensions))
        }
        PipelineOp::Offset { x, y } => {
            if width == 0 || height == 0 {
                return None;
            }
            let source_x = (-(i64::from(*x))).rem_euclid(i64::from(width)) as usize;
            let source_y = (-(i64::from(*y))).rem_euclid(i64::from(height)) as usize;
            let mut output = vec![0u8; expected];
            for destination_y in 0..height as usize {
                for destination_x in 0..width as usize {
                    let source = ((destination_y + source_y) % height as usize)
                        .checked_mul(width as usize)?
                        .checked_add((destination_x + source_x) % width as usize)?;
                    let destination = destination_y
                        .checked_mul(width as usize)?
                        .checked_add(destination_x)?;
                    copy_word(&mut output, destination, source)?;
                }
            }
            Some((output, dimensions))
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

/// Return whether a filtered F resize can use the integer-only marker-6
/// shader. This is intentionally stricter than an empirical GPU comparison:
/// the fixed table must equal Pillow's f64 table, every source word must be a
/// finite normal f32, and every sequential signed row sum must fit a 53-bit
/// aligned integer. For two changed axes, the exact rounded horizontal words
/// are passed through the vertical proof so Pillow's intermediate f32 store
/// remains part of the admission contract.
#[inline]
fn gpu_f_resize_integer_coefficients_match(fixed: &FilterCoeffs, exact: &FilterCoeffsF64) -> bool {
    fixed.xmin == exact.xmin
        && fixed.count == exact.count
        && fixed.count.iter().enumerate().all(|(row, &count)| {
            let start = fixed.offsets[row];
            let (Some(weights), Some(exact_weights)) = (
                fixed.weights.get(start..start.saturating_add(count)),
                exact.weights.get(row),
            ) else {
                return false;
            };
            exact_weights.len() == count
                && weights
                    .iter()
                    .zip(exact_weights)
                    .all(|(&fixed, &exact)| exact == fixed as f64 / 4_194_304.0)
        })
}

pub(in crate::compute::pool_gpu) fn gpu_f_resize_integer_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.len() != 1 || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }
    let PipelineOp::Resize { w, h, filter } = &ops[0] else {
        return false;
    };
    if matches!(filter, ResampleFilter::Nearest) || *w == 0 || *h == 0 {
        return false;
    }
    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let source_dimensions = image.dimensions();
    let expected = CheckedDims::new(source_dimensions.0, source_dimensions.1, 4)
        .ok()
        .map(|dims| dims.total_bytes());
    let bytes = pixels.as_raw();
    if expected != Some(bytes.len()) {
        return false;
    }
    for sample in bytes.chunks_exact(4) {
        let bits = u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
        let Some(_) = gpu_f32_integer_parts(bits) else {
            return false;
        };
    }

    let horizontal_changed = *w != source_dimensions.0;
    let vertical_changed = *h != source_dimensions.1;
    if !horizontal_changed && !vertical_changed {
        return false;
    }
    if horizontal_changed
        && vertical_changed
        && *w > source_dimensions.0
        && *h < source_dimensions.1
    {
        return false;
    }

    let (kernel, support) = filter_from_resample(*filter);
    let horizontal = gpu_resize_coefficients(*w, source_dimensions.0, *filter);
    let vertical = gpu_resize_coefficients(*h, source_dimensions.1, *filter);
    let horizontal_f64 = precompute_coeffs_f64(*w, source_dimensions.0, kernel, support);
    let vertical_f64 = precompute_coeffs_f64(*h, source_dimensions.1, kernel, support);
    if !gpu_f_resize_integer_coefficients_match(&horizontal, &horizontal_f64)
        || !gpu_f_resize_integer_coefficients_match(&vertical, &vertical_f64)
    {
        return false;
    }

    if horizontal_changed && vertical_changed {
        // The horizontal shader writes one f32 word per output pixel. Prove
        // that exact integer reduction and feed those rounded words to the
        // vertical proof; independently proving both axes against the raw
        // source would skip this observable Pillow boundary.
        let horizontal_words =
            gpu_f_resize_integer_pass_bits(bytes, source_dimensions, &horizontal, true);
        let Some(horizontal_words) = horizontal_words else {
            return false;
        };
        let mut horizontal_bytes = Vec::new();
        if horizontal_bytes
            .try_reserve(horizontal_words.len().saturating_mul(4))
            .is_err()
        {
            return false;
        }
        for word in horizontal_words {
            horizontal_bytes.extend_from_slice(&word.to_le_bytes());
        }
        return gpu_f_resize_integer_pass_bits(
            &horizontal_bytes,
            (*w, source_dimensions.1),
            &vertical,
            false,
        )
        .is_some();
    }

    if horizontal_changed {
        horizontal.count.iter().enumerate().all(|(row, _)| {
            gpu_f_resize_integer_row_is_exact(bytes, source_dimensions, &horizontal, row, true)
        })
    } else {
        vertical.count.iter().enumerate().all(|(row, _)| {
            gpu_f_resize_integer_row_is_exact(bytes, source_dimensions, &vertical, row, false)
        })
    }
}

/// Return whether a filtered F-mode resize is safe for marker-6's exact
/// device reduction.
///
/// The integer-emulation proof above admits finite normal f32 significands,
/// signed samples, and two changed axes when the fixed table is bit-for-bit
/// equal to Pillow's f64 table. This historical dyadic proof remains for
/// chained and other geometry whose intermediate f32 boundary is not covered
/// by that one-pass integer lane. It requires the fixed table to be bit-for-
/// bit equal to Pillow's f64 table, then proves that every f32 product and
/// sequential reduction is exact on the native adapter.
/// Bilinear rows have at most two taps, while Box downscales use at most 64
/// equal power-of-two taps on each changed axis. The exponent-span bound
/// keeps every multi-tap Box partial sum representable in f32, making its
/// sequential shader reduction equal to the exact f64 sum followed by the
/// F-mode f32 store. Multiple all-Box resizes are admitted with the same
/// bound accumulated across their passes; scanned `PutData` words can replace
/// an intermediate without widening the proven value domain.
//
// This mirrors Pillow's `Resample.c::precompute_coeffs` and
// `ImagingResample` f64-accumulate/f32-store boundary. It intentionally does
// not claim arbitrary arithmetic-filter parity; those inputs remain under
// exact host semantic control.
pub(in crate::compute::pool_gpu) fn gpu_f_resize_dyadic_is_exact(
    ops: &[PipelineOp],
    image: &DynamicImage,
    logical_mode: Option<&str>,
) -> bool {
    if logical_mode != Some("F") || ops.is_empty() || !matches!(image, DynamicImage::ImageRgba8(_))
    {
        return false;
    }

    // Marker 6 also has an exact integer-emulation lane for arbitrary normal
    // f32 significands and signed reductions. Keep the historical dyadic
    // proof below for chained and otherwise unproven geometry.
    if gpu_f_resize_integer_is_exact(ops, image, logical_mode) {
        return true;
    }

    let DynamicImage::ImageRgba8(pixels) = image else {
        return false;
    };
    let expected = CheckedDims::new(image.width(), image.height(), 4)
        .ok()
        .map(|dims| dims.total_bytes());
    if expected != Some(pixels.as_raw().len()) {
        return false;
    }
    let Some(mut summary) = GpuFDyadicExponentSummary::scan_words(pixels.as_raw()) else {
        return false;
    };
    let Some(plan) = gpu_f_resize_dyadic_collect_plan(ops, image.dimensions(), &mut summary) else {
        return false;
    };
    let Some((min_exponent, max_exponent)) = summary.supported_exponent_range() else {
        return false;
    };
    gpu_f_resize_dyadic_plan_is_exact(image.dimensions(), &plan, min_exponent, max_exponent)
}

#[derive(Clone, Copy)]
struct GpuFDyadicExponentSummary {
    sign: Option<bool>,
    min_exponent: Option<i32>,
    max_exponent: Option<i32>,
}

impl GpuFDyadicExponentSummary {
    fn scan_words(bytes: &[u8]) -> Option<Self> {
        if !bytes.chunks_exact(4).remainder().is_empty() {
            return None;
        }
        let mut summary = Self {
            sign: None,
            min_exponent: None,
            max_exponent: None,
        };
        for sample in bytes.chunks_exact(4) {
            let bits = u32::from_le_bytes([sample[0], sample[1], sample[2], sample[3]]);
            if bits == (-0.0f32).to_bits() {
                return None;
            }
            if f32::from_bits(bits) == 0.0 {
                continue;
            }
            let exponent_bits = ((bits >> 23) & 0xff) as i32;
            let significand = bits & 0x7f_ff_ff;
            if exponent_bits == 0 || exponent_bits == 0xff || significand != 0 {
                return None;
            }
            let positive = bits & (1 << 31) == 0;
            if summary.sign.is_some_and(|expected| expected != positive) {
                return None;
            }
            summary.sign = Some(positive);
            let exponent = exponent_bits - 127;
            summary.min_exponent = Some(
                summary
                    .min_exponent
                    .map_or(exponent, |min| min.min(exponent)),
            );
            summary.max_exponent = Some(
                summary
                    .max_exponent
                    .map_or(exponent, |max| max.max(exponent)),
            );
        }
        Some(summary)
    }

    fn merge(&mut self, next: Self) -> bool {
        if self
            .sign
            .is_some_and(|expected| next.sign.is_some_and(|actual| expected != actual))
        {
            return false;
        }
        self.sign = self.sign.or(next.sign);
        self.min_exponent = match (self.min_exponent, next.min_exponent) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (left, right) => left.or(right),
        };
        self.max_exponent = match (self.max_exponent, next.max_exponent) {
            (Some(left), Some(right)) => Some(left.max(right)),
            (left, right) => left.or(right),
        };
        true
    }

    fn supported_exponent_range(self) -> Option<(i32, i32)> {
        const MIN_NORMAL_EXPONENT: i32 = -80;
        const MAX_EXPONENT_SPAN: i32 = 16;

        let min_exponent = self.min_exponent?;
        let max_exponent = self.max_exponent.expect("minimum exponent implies maximum");
        if min_exponent < MIN_NORMAL_EXPONENT
            || max_exponent.saturating_sub(min_exponent) > MAX_EXPONENT_SPAN
        {
            return None;
        }
        Some((min_exponent, max_exponent))
    }
}

type GpuFDyadicResizeGeometry = ((u32, u32), (u32, u32), ResampleFilter);

struct GpuFDyadicResizePlan {
    output_dimensions: (u32, u32),
    resize_count: usize,
    changed_axis_count: usize,
    filter: ResampleFilter,
    geometries: Vec<GpuFDyadicResizeGeometry>,
}

fn gpu_f_resize_dyadic_collect_plan(
    ops: &[PipelineOp],
    source_dimensions: (u32, u32),
    summary: &mut GpuFDyadicExponentSummary,
) -> Option<GpuFDyadicResizePlan> {
    if source_dimensions.0 == 0 || source_dimensions.1 == 0 {
        return None;
    }
    let mut dimensions = source_dimensions;
    let mut resize_count = 0usize;
    let mut changed_axis = false;
    let mut changed_axis_count = 0usize;
    let mut filter_kind = None;
    let mut geometries = Vec::new();
    for op in ops {
        match op {
            PipelineOp::PutData {
                data,
                mode: PixelMode::F,
            } => {
                let expected = CheckedDims::new(dimensions.0, dimensions.1, 4)
                    .ok()
                    .map(|dims| dims.total_bytes());
                if expected != Some(data.len()) {
                    return None;
                }
                let next = GpuFDyadicExponentSummary::scan_words(data)?;
                if !summary.merge(next) {
                    return None;
                }
            }
            PipelineOp::Resize { w, h, filter } => {
                resize_count = resize_count.saturating_add(1);
                if *w == 0 || *h == 0 || (*w > dimensions.0 && *h < dimensions.1) {
                    return None;
                }
                changed_axis_count += usize::from(*w != dimensions.0);
                changed_axis_count += usize::from(*h != dimensions.1);
                changed_axis |= (*w, *h) != dimensions;
                filter_kind = Some(*filter);
                geometries.push((dimensions, (*w, *h), *filter));
                dimensions = (*w, *h);
            }
            _ => return None,
        }
    }
    let filter = filter_kind?;
    if resize_count == 0 || !changed_axis || changed_axis_count == 0 {
        return None;
    }
    Some(GpuFDyadicResizePlan {
        output_dimensions: dimensions,
        resize_count,
        changed_axis_count,
        filter,
        geometries,
    })
}

fn gpu_f_resize_dyadic_plan_is_exact(
    source_dimensions: (u32, u32),
    plan: &GpuFDyadicResizePlan,
    min_exponent: i32,
    max_exponent: i32,
) -> bool {
    if plan.resize_count > 1 {
        return gpu_f_resize_dyadic_box_chain_is_exact(
            &plan.geometries,
            min_exponent,
            max_exponent,
        );
    }
    gpu_f_resize_dyadic_single_is_exact(
        source_dimensions,
        plan.output_dimensions,
        plan.filter,
        plan.changed_axis_count,
        min_exponent,
        max_exponent,
    )
}

fn gpu_f_resize_dyadic_box_chain_is_exact(
    geometries: &[GpuFDyadicResizeGeometry],
    min_exponent: i32,
    max_exponent: i32,
) -> bool {
    let mut total_shift = 0i32;
    for &(source_dimensions, output_dimensions, filter) in geometries {
        if !matches!(filter, ResampleFilter::Box) {
            return false;
        }
        let (kernel, support) = filter_from_resample(filter);
        let horizontal = gpu_resize_coefficients(output_dimensions.0, source_dimensions.0, filter);
        let vertical = gpu_resize_coefficients(output_dimensions.1, source_dimensions.1, filter);
        let horizontal_f64 =
            precompute_coeffs_f64(output_dimensions.0, source_dimensions.0, kernel, support);
        let vertical_f64 =
            precompute_coeffs_f64(output_dimensions.1, source_dimensions.1, kernel, support);
        if !gpu_f_resize_dyadic_rows_match(&horizontal, &horizontal_f64)
            || !gpu_f_resize_dyadic_rows_match(&vertical, &vertical_f64)
        {
            return false;
        }
        let Some(horizontal_shift) = gpu_f_resize_dyadic_box_axis_shift(&horizontal) else {
            return false;
        };
        let Some(vertical_shift) = gpu_f_resize_dyadic_box_axis_shift(&vertical) else {
            return false;
        };
        total_shift = total_shift
            .checked_add(horizontal_shift)
            .and_then(|shift| shift.checked_add(vertical_shift))
            .unwrap_or(i32::MAX);
    }
    max_exponent
        .saturating_sub(min_exponent)
        .saturating_add(total_shift)
        <= 23
}

fn gpu_f_resize_dyadic_single_is_exact(
    source_dimensions: (u32, u32),
    output_dimensions: (u32, u32),
    filter: ResampleFilter,
    changed_axis_count: usize,
    min_exponent: i32,
    max_exponent: i32,
) -> bool {
    let (kernel, support) = filter_from_resample(filter);
    let horizontal = gpu_resize_coefficients(output_dimensions.0, source_dimensions.0, filter);
    let vertical = gpu_resize_coefficients(output_dimensions.1, source_dimensions.1, filter);
    let horizontal_f64 =
        precompute_coeffs_f64(output_dimensions.0, source_dimensions.0, kernel, support);
    let vertical_f64 =
        precompute_coeffs_f64(output_dimensions.1, source_dimensions.1, kernel, support);
    if !gpu_f_resize_dyadic_rows_match(&horizontal, &horizontal_f64)
        || !gpu_f_resize_dyadic_rows_match(&vertical, &vertical_f64)
    {
        return false;
    }
    match filter {
        ResampleFilter::Bilinear
        | ResampleFilter::Bicubic
        | ResampleFilter::Lanczos
        | ResampleFilter::Hamming => {
            gpu_f_resize_dyadic_two_tap_nonnegative(&horizontal)
                && gpu_f_resize_dyadic_two_tap_nonnegative(&vertical)
        }
        ResampleFilter::Box => {
            let Some(horizontal_shift) = gpu_f_resize_dyadic_box_axis_shift(&horizontal) else {
                return false;
            };
            let Some(vertical_shift) = gpu_f_resize_dyadic_box_axis_shift(&vertical) else {
                return false;
            };
            if max_exponent.saturating_sub(min_exponent) + horizontal_shift + vertical_shift > 23 {
                return false;
            }
            changed_axis_count > 0
        }
        _ => false,
    }
}

fn gpu_f_resize_dyadic_rows_match(fixed: &FilterCoeffs, exact: &FilterCoeffsF64) -> bool {
    if fixed.xmin != exact.xmin || fixed.count != exact.count {
        return false;
    }
    for (row, &count) in fixed.count.iter().enumerate() {
        let start = fixed.offsets[row];
        let Some(weights) = fixed.weights.get(start..start.saturating_add(count)) else {
            return false;
        };
        let Some(exact_weights) = exact.weights.get(row) else {
            return false;
        };
        if exact_weights.len() != count
            || weights
                .iter()
                .zip(exact_weights)
                .any(|(&fixed, &exact)| exact != fixed as f64 / 4_194_304.0)
        {
            return false;
        }
    }
    true
}

fn gpu_f_resize_dyadic_two_tap_nonnegative(coeffs: &FilterCoeffs) -> bool {
    coeffs.count.iter().enumerate().all(|(row, &count)| {
        if !(1..=2).contains(&count) {
            return false;
        }
        let start = coeffs.offsets[row];
        coeffs
            .weights
            .get(start..start.saturating_add(count))
            .is_some_and(|weights| {
                weights
                    .iter()
                    .all(|&weight| (0..=4_194_304).contains(&weight))
            })
    })
}

fn gpu_f_resize_dyadic_box_axis_shift(coeffs: &FilterCoeffs) -> Option<i32> {
    let mut max_shift = 0i32;
    for (row, &count) in coeffs.count.iter().enumerate() {
        let count = u32::try_from(count).ok()?;
        if count == 0 || count > MAX_GPU_REDUCE_FACTOR || !count.is_power_of_two() {
            return None;
        }
        let shift = count.trailing_zeros() as i32;
        max_shift = max_shift.max(shift);
        let expected_weight = (1i64 << 22) / i64::from(count);
        let start = coeffs.offsets[row];
        let weights = coeffs
            .weights
            .get(start..start.saturating_add(count as usize))?;
        if weights.iter().any(|&weight| weight != expected_weight) {
            return None;
        }
    }
    Some(max_shift)
}
