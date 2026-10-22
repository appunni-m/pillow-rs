use super::*;

pub(super) fn transpose_output_dimensions(
    method: &TransposeMethod,
    width: u32,
    height: u32,
) -> (u32, u32) {
    match method {
        TransposeMethod::Rotate90
        | TransposeMethod::Rotate270
        | TransposeMethod::Transpose
        | TransposeMethod::Transverse => (height, width),
        _ => (width, height),
    }
}

pub(super) fn transpose_forward(
    method: &TransposeMethod,
    width: u32,
    height: u32,
    x: u32,
    y: u32,
) -> (u32, u32) {
    match method {
        TransposeMethod::FlipLeftRight => (width - 1 - x, y),
        TransposeMethod::FlipTopBottom => (x, height - 1 - y),
        TransposeMethod::Rotate90 => (y, width - 1 - x),
        TransposeMethod::Rotate180 => (width - 1 - x, height - 1 - y),
        TransposeMethod::Rotate270 => (height - 1 - y, x),
        TransposeMethod::Transpose => (y, x),
        TransposeMethod::Transverse => (height - 1 - y, width - 1 - x),
    }
}

/// Compose adjacent transposes before allocating GPU resources. The seven
/// Pillow methods plus identity form the closed D4 transform group. Use a
/// canonical non-degenerate rectangle to distinguish all eight elements:
/// composing against the initial image shape can alias methods on a 1-pixel
/// axis even though an earlier resize gives the transpose run a larger input.
/// `None` is therefore exactly identity, including on empty input images.
pub(super) fn fuse_gpu_transpose_ops(ops: &[PipelineOp]) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let PipelineOp::Transpose { method } = &ops[index] else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut combined = Some(method.clone());
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let PipelineOp::Transpose { method: next } = &ops[index + consumed] else {
                break;
            };
            combined = match combined {
                Some(first) => {
                    crate::compute::pool_simd::compose_transpose_methods(&first, next, 2, 3)
                }
                None => Some(next.clone()),
            };
            consumed += 1;
        }
        if let Some(method) = combined {
            fused.push(PipelineOp::Transpose { method });
        }
        index += consumed;
    }
    fused
}

/// Collapse adjacent CropBorder operations to one source-relative crop.
/// Removing the intermediate raster avoids a dispatch and full-frame write;
/// the checked sum preserves the original sequence if malformed internal
/// descriptors would overflow.
pub(super) fn fuse_gpu_crop_border_ops(ops: &[PipelineOp]) -> Vec<PipelineOp> {
    let mut fused = Vec::with_capacity(ops.len());
    let mut index = 0usize;
    while index < ops.len() {
        let PipelineOp::CropBorder { border } = &ops[index] else {
            fused.push(ops[index].clone());
            index += 1;
            continue;
        };
        let mut combined = *border;
        let mut consumed = 1usize;
        while index + consumed < ops.len() {
            let PipelineOp::CropBorder { border: next } = &ops[index + consumed] else {
                break;
            };
            let Some(sum) = combined.checked_add(*next) else {
                break;
            };
            combined = sum;
            consumed += 1;
        }
        if consumed == 1 {
            fused.push(ops[index].clone());
        } else {
            fused.push(PipelineOp::CropBorder { border: combined });
        }
        index += consumed;
    }
    fused
}

/// Return whether two adjacent public Chops operations can share one exact
/// dual-input GPU traversal.  The source identity guard is important: the
/// fused shader consumes the same secondary image for both formulas, so two
/// equal-looking but independently constructed images must retain the normal
/// two-dispatch path.
pub(super) fn can_fuse_gpu_multiply_screen(ops: &[PipelineOp], index: usize) -> bool {
    if index + 1 >= ops.len() {
        return false;
    }
    match (&ops[index], &ops[index + 1]) {
        (PipelineOp::Multiply { other: first }, PipelineOp::Screen { other: second }) => {
            first.shares_execution_source(second)
        }
        _ => false,
    }
}

/// Return whether a full-frame RGBA alpha composite immediately followed by
/// Mirror can be evaluated directly into the mirrored output. This shader
/// reads both original inputs at the mirrored coordinate, so it is valid only
/// for Pillow's zero-offset, equal-sized RGBA operation.
pub(super) fn can_fuse_gpu_alpha_composite_mirror(
    ops: &[PipelineOp],
    index: usize,
    logical_mode: Option<&str>,
    input_dimensions: (u32, u32),
) -> bool {
    if logical_mode != Some("RGBA") || index + 1 >= ops.len() {
        return false;
    }
    match (&ops[index], &ops[index + 1]) {
        (
            PipelineOp::AlphaComposite {
                source,
                dest: (0, 0),
                src: (0, 0),
            },
            PipelineOp::Mirror,
        ) => source.size().ok() == Some(input_dimensions),
        _ => false,
    }
}

/// Count public operations lowered into private fused GPU dispatches for one
/// submission. Keep the count chunk-local so the receipt never claims a pair
/// was fused when the resource/work scheduler split it across submissions.
pub(super) fn gpu_fused_operation_count(
    ops: &[PipelineOp],
    logical_mode: Option<&str>,
    image_dimensions: (u32, u32),
) -> u64 {
    let mut count = 0u64;
    let mut index = 0usize;
    let (mut cur_w, mut cur_h) = image_dimensions;
    while index < ops.len() {
        if can_fuse_gpu_multiply_screen(ops, index)
            || can_fuse_gpu_alpha_composite_mirror(ops, index, logical_mode, (cur_w, cur_h))
        {
            count = count.saturating_add(2);
            if let Some(next) = op_output_dims(&ops[index + 1], cur_w, cur_h) {
                (cur_w, cur_h) = next;
            }
            index += 2;
        } else {
            (cur_w, cur_h) = op_output_dims(&ops[index], cur_w, cur_h).unwrap_or((cur_w, cur_h));
            index += 1;
        }
    }
    count
}

pub(super) fn gpu_dispatch_count(
    ops: &[PipelineOp],
    logical_mode: Option<&str>,
    image_dimensions: (u32, u32),
) -> u64 {
    let mut count = 0u64;
    let mut index = 0usize;
    let (mut cur_w, mut cur_h) = image_dimensions;
    while index < ops.len() {
        if can_fuse_gpu_multiply_screen(ops, index)
            || can_fuse_gpu_alpha_composite_mirror(ops, index, logical_mode, (cur_w, cur_h))
        {
            count += 1;
            if let Some(next) = op_output_dims(&ops[index + 1], cur_w, cur_h) {
                (cur_w, cur_h) = next;
            }
            index += 2;
            continue;
        }
        let next = op_output_dims(&ops[index], cur_w, cur_h).unwrap_or((cur_w, cur_h));
        count += if ops.len() == 1
            && gpu_f_resize_compact_box_vertical_only_geometry(
                &ops[index],
                (cur_w, cur_h),
                next,
                logical_mode,
            ) {
            // The unchanged horizontal axis is an identity copy.  The
            // vertical compact reducer can consume the original source
            // directly, so this proof emits one native dispatch.
            1
        } else if matches!(
            &ops[index],
            PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }
        ) {
            // Equalize uses a command-buffer clear followed by gather, LUT
            // derivation, and remap compute dispatches.
            3
        } else if matches!(&ops[index], PipelineOp::Autocontrast { .. }) {
            // Autocontrast still clears its histogram with a compute dispatch.
            4
        } else if matches!(
            &ops[index],
            PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. }
        ) || matches!(&ops[index], PipelineOp::Resize { filter, .. }
            if !matches!(filter, ResampleFilter::Nearest)
                || gpu_resize_nearest_uses_coefficients(logical_mode))
        {
            2
        } else if matches!(&ops[index], PipelineOp::Pad { .. }) {
            // Omit identity resize axes and the placement pass when the
            // contain result already fills the canvas. RGBX can fuse the
            // vertical resize and placement into one pass.
            match gpu_pad_geometry(&ops[index], cur_w, cur_h) {
                Some((_, _))
                    if gpu_pad_rgbx_fused_vertical_geometry(
                        &ops[index],
                        (cur_w, cur_h),
                        logical_mode,
                    )
                    .is_some() =>
                {
                    2
                }
                Some((resize_dims, _)) => {
                    u64::from(resize_dims != (cur_w, cur_h)) * 2 + u64::from(resize_dims != next)
                }
                None => 3,
            }
        } else {
            GpuInner::blur_pass_count(&ops[index]).map_or(1usize, |passes| passes.saturating_mul(2))
                as u64
        };
        (cur_w, cur_h) = next;
        index += 1;
    }
    count
}
