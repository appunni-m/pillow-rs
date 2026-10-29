use super::*;

pub(super) struct GpuBatchOutputLayout {
    pub(super) put_alpha_mode: Option<PixelMode>,
    pub(super) output_color_type: Option<crate::raster::ColorType>,
    pub(super) native_rgb: bool,
    pub(super) native_luma8_extract: bool,
    pub(super) native_luma8_grayscale: bool,
    pub(super) native_luma8_constant: bool,
}

impl GpuBatchOutputLayout {
    pub(super) fn from_operations(
        ops: &[PipelineOp],
        input_color_type: crate::raster::ColorType,
    ) -> Self {
        // Mode-preserving operations after a conversion do not undo the
        // promotion established by the last mode-changing operation.
        let mut put_alpha_mode = None;
        let mut output_color_type = None;
        for op in ops {
            match op {
                PipelineOp::Grayscale | PipelineOp::ExtractBand { .. } => {
                    put_alpha_mode = None;
                    output_color_type = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::Constant { .. } => {
                    put_alpha_mode = None;
                    output_color_type = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::Convert { mode, .. } => {
                    put_alpha_mode = None;
                    output_color_type = gpu_output_color_type(mode);
                }
                PipelineOp::ConvertLab { .. } | PipelineOp::Colorize { .. } => {
                    put_alpha_mode = None;
                    output_color_type = Some(crate::raster::ColorType::Rgb8);
                }
                PipelineOp::Merge { mode, .. } => {
                    put_alpha_mode = None;
                    output_color_type = gpu_output_color_type(mode);
                }
                PipelineOp::Color3DLut { target_mode, .. } => {
                    put_alpha_mode = None;
                    output_color_type = gpu_pixel_mode_color_type(*target_mode);
                }
                PipelineOp::EffectNoise { .. } => {
                    put_alpha_mode = None;
                    output_color_type = Some(crate::raster::ColorType::L8);
                }
                PipelineOp::PutAlpha { mode, .. } | PipelineOp::PutAlphaData { mode, .. } => {
                    put_alpha_mode = Some(*mode);
                    output_color_type = None;
                }
                _ => {}
            }
        }

        let last_op = ops.last();
        let native_luma8 = output_color_type == Some(crate::raster::ColorType::L8);
        Self {
            native_rgb: put_alpha_mode.is_none()
                && output_color_type.unwrap_or(input_color_type) == crate::raster::ColorType::Rgb8,
            native_luma8_extract: native_luma8
                && matches!(last_op, Some(PipelineOp::ExtractBand { .. })),
            native_luma8_grayscale: native_luma8 && matches!(last_op, Some(PipelineOp::Grayscale)),
            native_luma8_constant: native_luma8
                && matches!(last_op, Some(PipelineOp::Constant { .. })),
            put_alpha_mode,
            output_color_type,
        }
    }
}
