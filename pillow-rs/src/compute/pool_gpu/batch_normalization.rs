use super::*;

pub(super) struct NormalizedGpuBatch {
    pub(super) source_is_luma_point_run: bool,
    pub(super) native_cover_input_channels: Option<u8>,
    pub(super) host_autocontrast_luma_lut: bool,
    pub(super) host_autocontrast_rgb_lut: bool,
    pub(super) native_sharpness_la_patch: Option<(usize, [u8; 2])>,
    pub(super) dispatch_ops: Vec<PipelineOp>,
}

pub(super) enum BatchNormalization {
    Continue(NormalizedGpuBatch),
    Return(DynamicImage),
}

impl GpuPool {
    pub(super) fn normalize_batch_operations(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
    ) -> Result<BatchNormalization, PilError> {
        let source_is_luma_point_run = ops.iter().all(|op| matches!(op, PipelineOp::Eval { .. }))
            && matches!(image, DynamicImage::ImageLuma8(_))
            && matches!(mode, None | Some("L"));
        let native_cover_input_channels = gpu_native_cover_input_channels(ops, image, mode);
        // For unmasked native L/RGB inputs, histogram selection is small
        // control-plane work compared with the packed full-frame GPU path.
        // Derive Pillow's exact LUT on the host, then remap pixels on GPU.
        // Masked and other layouts keep the four-pass GPU histogram path.
        let host_autocontrast_lut = host_autocontrast_dispatch_op(ops, image, mode)?;
        let host_autocontrast_luma_lut = host_autocontrast_lut.is_some()
            && matches!(image, DynamicImage::ImageLuma8(_))
            && matches!(mode, None | Some("L"));
        let host_autocontrast_rgb_lut = host_autocontrast_lut.is_some()
            && matches!(image, DynamicImage::ImageRgb8(_))
            && matches!(mode, None | Some("RGB"));
        let mut dispatch_ops = if let Some(op) = host_autocontrast_lut {
            vec![op]
        } else {
            ops.iter()
                .filter(|op| !gpu_reduce_is_identity(op) && !gpu_sharpness_is_identity(op))
                .cloned()
                .collect()
        };
        if dispatch_ops.is_empty() {
            // Exact identities have no pixel invocations. Keep the selected
            // GPU backend and report a zero-dispatch independent result.
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(BatchNormalization::Return(image.clone()));
        }
        dispatch_ops = if gpu_byte_point_mode_allowed(image, mode) {
            fuse_gpu_point_ops(&dispatch_ops, mode_code(image))
        } else {
            dispatch_ops
        };
        if gpu_image_layout_is_supported(image) {
            // Transpose only relocates native samples; logical-mode preflight
            // remains authoritative for palette indices and typed words.
            dispatch_ops = fuse_gpu_transpose_ops(&dispatch_ops);
            if dispatch_ops.is_empty() {
                crate::compute::record_pipeline_operation_path("native-copy");
                crate::compute::record_pipeline_dispatch_count(0);
                return Ok(BatchNormalization::Return(image.clone()));
            }
        }
        dispatch_ops = fuse_gpu_crop_border_ops(&dispatch_ops);
        // Thumbnail may expand to Reduce + Resize, so normalize geometry
        // wrappers before the later operation-aligned preflight vectors.
        dispatch_ops = expand_gpu_geometry_ops(&dispatch_ops, image, image.dimensions(), mode);

        if gpu_uniform_blur_can_copy(&dispatch_ops, image) {
            // A normalized blur preserves every channel of a constant image.
            gpu_log!("[GPU] lowering constant blur to one identity dispatch");
            dispatch_ops = vec![PipelineOp::Duplicate];
        }
        let native_sharpness_la_patch =
            gpu_native_sharpness_la_putpixel(&dispatch_ops, image, mode);
        if native_sharpness_la_patch.is_some() {
            dispatch_ops.remove(0);
        }

        Ok(BatchNormalization::Continue(NormalizedGpuBatch {
            source_is_luma_point_run,
            native_cover_input_channels,
            host_autocontrast_luma_lut,
            host_autocontrast_rgb_lut,
            native_sharpness_la_patch,
            dispatch_ops,
        }))
    }
}

fn host_autocontrast_dispatch_op(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> Result<Option<PipelineOp>, PilError> {
    let [PipelineOp::Autocontrast { cutoff, mask }] = ops else {
        return Ok(None);
    };
    let native_layout = matches!(
        image,
        DynamicImage::ImageLuma8(_) | DynamicImage::ImageRgb8(_)
    ) && (mode.is_none()
        || (matches!(image, DynamicImage::ImageLuma8(_)) && mode == Some("L"))
        || (matches!(image, DynamicImage::ImageRgb8(_)) && mode == Some("RGB")));
    if mask.is_none()
        && cutoff.is_finite()
        && native_layout
        && image.width() != 0
        && image.height() != 0
    {
        Ok(Some(PipelineOp::Eval {
            lut: crate::compute::pool_cpu::ops::imageops::autocontrast_lut(image, *cutoff, None)?
                .into(),
        }))
    } else {
        Ok(None)
    }
}
