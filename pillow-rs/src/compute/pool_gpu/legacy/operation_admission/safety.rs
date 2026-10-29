use super::super::{
    MAX_GPU_BLUR_RADIUS, MAX_GPU_FILTER_SIZE, MAX_GPU_REDUCE_FACTOR, MAX_GPU_SCALE_FIXED_POINT,
    PipelineOp, registry,
};
use super::{gpu_f32_finite, gpu_rotate_op_is_safe, gpu_transform_op_is_safe};

pub(super) fn gpu_operation_is_safe(op: &PipelineOp) -> bool {
    match op {
        PipelineOp::Filter3x3 { .. }
        | PipelineOp::Filter5x5 { .. }
        | PipelineOp::GaussianBlur { .. }
        | PipelineOp::BoxBlur { .. }
        | PipelineOp::BoxBlurXY { .. }
        | PipelineOp::MedianFilter { .. }
        | PipelineOp::MaxFilter { .. }
        | PipelineOp::MinFilter { .. }
        | PipelineOp::RankFilter { .. } => gpu_filter_operation_is_safe(op),
        PipelineOp::Reduce { .. }
        // The CPU implementation shifts by `8 - bits`; zero is not a valid
        // direct PipelineOp value and would otherwise underflow that shift.
        // Public constructors already clamp to 1..=8, but keep the GPU
        // safety gate correct for callers that build the pipeline directly.
        | PipelineOp::Posterize { .. }
        | PipelineOp::ExtractBand { .. }
        | PipelineOp::Brightness { .. }
        | PipelineOp::Contrast { .. }
        | PipelineOp::ColorSaturation { .. }
        | PipelineOp::Sharpness { .. }
        | PipelineOp::Add { .. }
        | PipelineOp::Subtract { .. }
        | PipelineOp::BlendModule { .. }
        | PipelineOp::Scale { .. } => gpu_numeric_operation_is_safe(op),
        _ => gpu_geometry_operation_is_safe(op),
    }
}

fn gpu_filter_operation_is_safe(op: &PipelineOp) -> bool {
    match op {
        PipelineOp::Filter3x3 {
            kernel,
            scale,
            offset,
        } => gpu_filter_kernel_parameters_are_safe(&kernel[..], *scale, *offset),
        PipelineOp::Filter5x5 {
            kernel,
            scale,
            offset,
        } => gpu_filter_kernel_parameters_are_safe(&kernel[..], *scale, *offset),
        PipelineOp::GaussianBlur { sigma } => {
            registry::separable_gaussian_blur_radius(*sigma).is_some()
        }
        PipelineOp::BoxBlur { radius } => *radius <= MAX_GPU_BLUR_RADIUS,
        PipelineOp::BoxBlurXY {
            radius_x,
            radius_y,
            passes,
        } => {
            *passes > 0
                && *passes <= 3
                && registry::separable_box_blur_params_f32(*radius_x).is_some()
                && registry::separable_box_blur_params_f32(*radius_y).is_some()
        }
        PipelineOp::MedianFilter { size }
        | PipelineOp::MaxFilter { size }
        | PipelineOp::MinFilter { size }
        | PipelineOp::RankFilter { size, .. } => {
            *size >= 1 && *size <= MAX_GPU_FILTER_SIZE && *size % 2 == 1
        }
        _ => true,
    }
}

fn gpu_filter_kernel_parameters_are_safe(kernel: &[f32], scale: f32, offset: f32) -> bool {
    registry::gpu_filter_kernel_is_exact(kernel, scale, offset)
        || registry::gpu_filter_rational_denominator(kernel, scale, offset).is_some()
}

fn gpu_numeric_operation_is_safe(op: &PipelineOp) -> bool {
    match op {
        PipelineOp::Reduce { x_factor, y_factor } => {
            *x_factor <= MAX_GPU_REDUCE_FACTOR
                && *y_factor <= MAX_GPU_REDUCE_FACTOR
                && *x_factor >= 1
                && *y_factor >= 1
        }
        // The CPU implementation shifts by `8 - bits`; zero is not a valid
        // direct PipelineOp value and would otherwise underflow that shift.
        // Public constructors already clamp to 1..=8, but keep the GPU
        // safety gate correct for callers that build the pipeline directly.
        PipelineOp::Posterize { bits } => (1..=8).contains(bits),
        PipelineOp::ExtractBand { index } => *index < 4,
        // Keep the scalar preflight in lockstep with the WGSL fixed-point
        // parameterization. The helper checks every byte result against
        // Pillow's f64 contract before a dispatch is admitted.
        PipelineOp::Brightness { factor } => registry::gpu_brightness_factor_int(*factor).is_some(),
        PipelineOp::Contrast { factor } => registry::gpu_blend_alpha_params(*factor).is_some(),
        PipelineOp::ColorSaturation { factor } => {
            registry::gpu_color_saturation_factor_params(*factor).is_some()
        }
        PipelineOp::Sharpness { factor } => registry::gpu_sharpness_factor_int(*factor).is_some(),
        PipelineOp::Add { scale, offset, .. } | PipelineOp::Subtract { scale, offset, .. } => {
            // The scalar preflight exhaustively compares all byte pairs with
            // the f32 values consumed by the real WGSL kernel.
            registry::gpu_chops_affine_supported(*scale, *offset)
        }
        PipelineOp::BlendModule { alpha, .. } => registry::gpu_blend_alpha_params(*alpha).is_some(),
        PipelineOp::Scale { factor, .. } => {
            factor.is_finite()
                && *factor > 0.0
                && (*factor * 65536.0).is_finite()
                && *factor * 65536.0 >= 1.0
                && *factor * 65536.0 <= MAX_GPU_SCALE_FIXED_POINT
        }
        _ => true,
    }
}

fn gpu_geometry_operation_is_safe(op: &PipelineOp) -> bool {
    let finite_f32 = gpu_f32_finite;
    match op {
        PipelineOp::ResizeBoxed {
            w, h, box_coords, ..
        } => {
            *w > 0
                && *h > 0
                && [box_coords.0, box_coords.1, box_coords.2, box_coords.3]
                    .into_iter()
                    .all(finite_f32)
        }
        PipelineOp::Contain { w, h, .. }
        | PipelineOp::Cover { w, h, .. }
        | PipelineOp::Pad { w, h, .. } => *w > 0 && *h > 0,
        PipelineOp::EffectSpread { distance } => *distance <= i32::MAX as u32,
        PipelineOp::EffectNoise { sigma } => finite_f32(*sigma),
        PipelineOp::Rotate {
            angle,
            center,
            translate,
            ..
        } => gpu_rotate_op_is_safe(*angle, *center, *translate),
        PipelineOp::Transform {
            w,
            h,
            method,
            data,
            filter,
            ..
        } => gpu_transform_op_is_safe(op, w, h, method, data, filter),
        PipelineOp::AlphaComposite { dest, src, .. } => *dest == (0, 0) && *src == (0, 0),
        PipelineOp::Autocontrast { cutoff, .. } => finite_f32(*cutoff),
        _ => true,
    }
}
