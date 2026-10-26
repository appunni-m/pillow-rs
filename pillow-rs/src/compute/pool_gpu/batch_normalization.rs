use super::*;

/// Pillow's `ImagingLineBoxBlur{8,32}` in `src/libImaging/BoxBlur.c` uses a
/// normalized replicated-edge average, so a constant image remains constant
/// through every box pass (and therefore Pillow's three-pass Gaussian
/// lowering). Scanning larger frames can cost more than the six GPU passes it
/// replaces, so keep this exact lowering bounded to the benchmark-sized
/// working set where the launch savings are measurable.
const GPU_UNIFORM_BLUR_MAX_PIXELS: usize = 1024 * 1024;

fn gpu_uniform_blur_can_copy(ops: &[PipelineOp], image: &DynamicImage) -> bool {
    if ops.len() != 1
        || !matches!(
            ops[0],
            PipelineOp::BoxBlur { .. }
                | PipelineOp::BoxBlurXY { .. }
                | PipelineOp::GaussianBlur { .. }
        )
        || !gpu_operation_is_safe(&ops[0])
        || !gpu_image_layout_is_supported(image)
    {
        return false;
    }

    let Ok(dims) = CheckedDims::new(image.width(), image.height(), 1) else {
        return false;
    };
    let pixel_count = dims.total_pixels();
    if pixel_count == 0 || pixel_count > GPU_UNIFORM_BLUR_MAX_PIXELS {
        return false;
    }

    let channels = match image {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return false,
    };
    let bytes = image.as_bytes();
    let Some(first) = bytes.get(..channels) else {
        return false;
    };
    bytes
        .chunks_exact(channels)
        .take(pixel_count)
        .all(|pixel| pixel == first)
}

pub(super) struct NormalizedGpuBatch {
    pub(super) native_cover_input_channels: Option<u8>,
    pub(super) native_sharpness_la_patch: Option<(usize, [u8; 2])>,
    pub(super) input_layouts: BatchInputLayoutCandidates,
    pub(super) dispatch_ops: Vec<PipelineOp>,
}

#[derive(Clone, Copy)]
pub(super) struct BatchInputLayoutCandidates {
    pub(super) packed_luma_colorize: bool,
    pub(super) packed_luma_convert: bool,
    pub(super) native_alpha_convert_input: bool,
    pub(super) packed_luma_transform_input: bool,
    pub(super) native_rgb_compact_input: bool,
    pub(super) native_la_transform_input: bool,
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_putdata: bool,
    pub(super) packed_luma_order_statistic: bool,
    pub(super) packed_native_byte_filter: bool,
    pub(super) packed_luma_pad: bool,
    pub(super) packed_la_pad: bool,
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
        let input_layouts = batch_input_layout_candidates(
            &dispatch_ops,
            image,
            mode,
            source_is_luma_point_run,
            host_autocontrast_luma_lut,
            host_autocontrast_rgb_lut,
        );

        Ok(BatchNormalization::Continue(NormalizedGpuBatch {
            native_cover_input_channels,
            native_sharpness_la_patch,
            input_layouts,
            dispatch_ops,
        }))
    }
}

fn batch_input_layout_candidates(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
    source_is_luma_point_run: bool,
    host_autocontrast_luma_lut: bool,
    host_autocontrast_rgb_lut: bool,
) -> BatchInputLayoutCandidates {
    // Only a standalone L -> RGB Colorize can use the packed four-sample
    // upload; a preceding GPU operation leaves the ordinary RGBA transport.
    let packed_luma_colorize = matches!(ops, [PipelineOp::Colorize { .. }])
        && matches!(image, DynamicImage::ImageLuma8(_));
    // A standalone L -> RGBA conversion can read native L bytes as a packed
    // four-samples-per-word source. Other targets and prefixes keep standard
    // RGBA transport and the existing shader contract.
    let packed_luma_convert = gpu_packed_luma_convert_input(ops, image, mode);
    let native_alpha_convert_input = gpu_native_alpha_convert_input(ops, image, mode);
    let packed_luma_transform_input = gpu_packed_luma_transform_input(ops, image, mode);
    let native_rgb_compact_input =
        gpu_native_rgb_compact_input(ops, image, mode, host_autocontrast_rgb_lut);
    let native_la_transform_input = gpu_native_la_transform_input(ops, image, mode);
    let packed_luma_point = gpu_packed_luma_point_input(
        source_is_luma_point_run,
        host_autocontrast_luma_lut,
        ops,
        image,
        mode,
    );
    // Native L PutData keeps source and replacement bytes compact. P/1 and
    // mode-converted images retain their separate semantics.
    let packed_luma_putdata = cfg!(target_endian = "little")
        && matches!(mode, None | Some("L"))
        && image.width() != 0
        && image.height() != 0
        && matches!(
            ops,
            [PipelineOp::PutData {
                data,
                mode: PixelMode::L,
            }] if matches!(image, DynamicImage::ImageLuma8(source)
                if data.len() <= source.as_raw().len())
        );
    BatchInputLayoutCandidates {
        packed_luma_colorize,
        packed_luma_convert,
        native_alpha_convert_input,
        packed_luma_transform_input,
        native_rgb_compact_input,
        native_la_transform_input,
        packed_luma_point,
        packed_luma_putdata,
        packed_luma_order_statistic: gpu_packed_luma_order_statistic_input(ops, image, mode),
        packed_native_byte_filter: gpu_packed_native_byte_filter_input(ops, image, mode),
        packed_luma_pad: gpu_packed_luma_pad_input(ops, image, mode),
        packed_la_pad: gpu_packed_la_pad_input(ops, image, mode),
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
