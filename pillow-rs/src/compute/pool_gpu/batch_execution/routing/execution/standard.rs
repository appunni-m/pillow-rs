use super::super::super::super::{
    BatchInputLayoutCandidates, BufferPool, GpuBatchOutputLayout, GpuBatchResourceTelemetryOptions,
    GpuBatchUploadOptions, GpuPackedDispatchPlan, PipelineResourceTelemetry, ReadbackTarget,
    execution_mode_code, gpu_batch_resource_telemetry, gpu_fused_operation_count,
    gpu_luma16_convert_is_supported, gpu_luma16_geometry_is_supported,
    gpu_luma16_paste_is_supported, gpu_native_expand_channels, gpu_native_extract_band_channels,
    gpu_native_grayscale_cmyk_input, gpu_native_grayscale_rgb_input, gpu_native_reduce_rgb_input,
    gpu_native_sharpness_l_input, gpu_native_sharpness_la_input, gpu_native_sharpness_rgb_input,
    gpu_result_as_color_type, put_alpha_output,
};
use super::{GpuBatchExecutionRoute, StandardGpuRoutePlan};
use crate::compute::registry;
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{ColorType, DynamicImage, GenericImageView};

impl GpuBatchExecutionRoute<'_> {
    pub(super) fn log_operation_keys(&self) {
        let (w, h) = self.img().dimensions();
        let mode_code = execution_mode_code(self.img(), self.mode());
        let op_keys: Vec<&str> = self.ops().iter().map(registry::variant_key).collect();
        log::debug!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            self.ops().len(),
            w,
            h,
            mode_code,
            op_keys
        );
    }

    pub(super) fn build_upload_plan(&self, standard: &StandardGpuRoutePlan) -> GpuBatchUploadPlan {
        let ops = self.ops();
        let img = self.img();
        let mode = self.mode();
        let input = self.input_layouts();
        let packed = self.packed_plan;
        let (w, h) = img.dimensions();
        let native_luma16 = gpu_luma16_geometry_is_supported(ops, img, mode);
        let native_luma16_convert = gpu_luma16_convert_is_supported(ops, img);
        let native_luma16_paste = gpu_luma16_paste_is_supported(ops, img);
        let native_extract_band_channels = gpu_native_extract_band_channels(ops, img, mode);
        let output_only_constant = matches!(ops, [PipelineOp::Constant { .. }]);
        let native_grayscale_rgb_input = gpu_native_grayscale_rgb_input(ops, img, mode);
        let native_grayscale_cmyk_input = gpu_native_grayscale_cmyk_input(ops, img, mode);
        let native_sharpness_l_input = gpu_native_sharpness_l_input(ops, img, mode);
        let native_sharpness_la_input = gpu_native_sharpness_la_input(ops, img, mode);
        let native_sharpness_rgb_input = gpu_native_sharpness_rgb_input(ops, img, mode);
        let native_reduce_rgb_input = gpu_native_reduce_rgb_input(ops, img, mode);
        let native_expand_channels = gpu_native_expand_channels(ops, img, mode);
        #[cfg(target_endian = "little")]
        let mapped_native_box_blur_input = self.gpu.direct_primary_readback
            && matches!(ops, [PipelineOp::BoxBlur { radius: 1 }])
            && ((packed.packed_native_byte_filter
                && matches!(mode, None | Some("L"))
                && matches!(img, DynamicImage::ImageLuma8(image)
                    if image.as_raw().len() == (w as usize).saturating_mul(h as usize)))
                || (standard.native_rgb_compact_input
                    && matches!(mode, None | Some("RGB"))
                    && matches!(img, DynamicImage::ImageRgb8(image)
                        if image.as_raw().len() == (w as usize).saturating_mul(h as usize).saturating_mul(3))));
        #[cfg(not(target_endian = "little"))]
        let mapped_native_box_blur_input = false;
        #[cfg(target_endian = "little")]
        let mapped_native_grayscale_input = self.gpu.direct_primary_readback
            && (native_grayscale_rgb_input || native_grayscale_cmyk_input);
        #[cfg(not(target_endian = "little"))]
        let mapped_native_grayscale_input = false;
        GpuBatchUploadPlan {
            dimensions: (w, h),
            mode_code: execution_mode_code(img, mode),
            full_luma_putdata: self.full_luma_putdata,
            native_luma16,
            native_luma16_convert,
            native_luma16_paste,
            native_extract_band_channels,
            output_only_constant,
            native_grayscale_rgb_input,
            native_grayscale_cmyk_input,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
            native_reduce_rgb_input,
            native_expand_channels,
            mapped_native_box_blur_input,
            mapped_native_grayscale_input,
            native_cover_input_channels: self.prepared.normalized.native_cover_input_channels,
            native_sharpness_la_patch: self.prepared.normalized.native_sharpness_la_patch,
            input,
            packed,
        }
    }

    pub(super) fn log_upload_start(&self, upload: &GpuBatchUploadPlan) {
        gpu_log!(
            "[GPU] step=upload start output_only_constant={} native_luma16={} native_luma16_convert={} native_luma16_paste={}",
            upload.output_only_constant,
            upload.native_luma16,
            upload.native_luma16_convert,
            upload.native_luma16_paste
        );
    }

    pub(super) fn upload_batch_input(
        &self,
        standard: &StandardGpuRoutePlan,
        upload: &GpuBatchUploadPlan,
        mut buffers: BufferPool,
    ) -> Result<DynamicImage, PilError> {
        let input = upload.input;
        let packed = upload.packed;
        self.gpu.upload_batch_input(
            self.img(),
            &mut buffers,
            GpuBatchUploadOptions {
                mode: self.mode(),
                native_luma16: upload.native_luma16,
                native_luma16_convert: upload.native_luma16_convert,
                native_luma16_paste: upload.native_luma16_paste,
                full_luma_putdata: upload.full_luma_putdata,
                output_only_constant: upload.output_only_constant,
                packed_la_pad: packed.packed_la_pad,
                packed_native_byte_filter: packed.packed_native_byte_filter,
                packed_luma_colorize: input.packed_luma_colorize,
                packed_luma_convert: input.packed_luma_convert,
                native_alpha_convert_input: input.native_alpha_convert_input,
                packed_luma_transform_input: input.packed_luma_transform_input,
                packed_luma_point: packed.packed_luma_point,
                packed_luma_putdata: packed.packed_luma_putdata,
                packed_luma_order_statistic: packed.packed_luma_order_statistic,
                packed_luma_pad: packed.packed_luma_pad,
                native_la_transform_output: standard.native_la_transform_output,
                native_rgb_compact_input: standard.native_rgb_compact_input,
                native_extract_band_channels: upload.native_extract_band_channels,
                native_grayscale_rgb_input: upload.native_grayscale_rgb_input,
                native_grayscale_cmyk_input: upload.native_grayscale_cmyk_input,
                native_sharpness_l_input: upload.native_sharpness_l_input,
                native_sharpness_la_input: upload.native_sharpness_la_input,
                native_sharpness_la_patch: upload.native_sharpness_la_patch,
                native_sharpness_rgb_input: upload.native_sharpness_rgb_input,
                native_reduce_rgb_input: upload.native_reduce_rgb_input,
                native_expand_channels: upload.native_expand_channels,
                native_cover_input_channels: upload.native_cover_input_channels,
                mapped_native_box_blur_input: upload.mapped_native_box_blur_input,
                mapped_native_grayscale_input: upload.mapped_native_grayscale_input,
            },
        )?;
        gpu_log!(
            "[GPU] step=upload done output_only_constant={} native_luma16={}",
            upload.output_only_constant,
            upload.native_luma16
        );
        gpu_log!("[GPU] step=execute_batch_impl start");
        let submission = self.execute_batch_impl(standard, upload, buffers)?;
        self.readback_and_finalize(standard, upload, submission)
    }

    fn execute_batch_impl(
        &self,
        standard: &StandardGpuRoutePlan,
        upload: &GpuBatchUploadPlan,
        mut buffers: BufferPool,
    ) -> Result<GpuBatchSubmission, PilError> {
        let mapped_native_input =
            if upload.mapped_native_box_blur_input || upload.mapped_native_grayscale_input {
                Some(buffers.mapped_native_input.take().ok_or_else(|| {
                    PilError::InternalError(
                        "mapped native upload completed without retaining its input buffer".into(),
                    )
                })?)
            } else {
                None
            };
        let initial_input = mapped_native_input.as_ref().map(|input| &input.buffer);
        let input = upload.input;
        let packed = upload.packed;
        let proofs = &self.prepared.resize_proofs;
        let (
            final_is_a,
            final_w,
            final_h,
            readback,
            mut resource_telemetry,
            dispatch_count,
            native_expand_output,
        ) = self.gpu.execute_batch_impl(
            self.ops(),
            &self.prepared.auxiliary_images,
            upload.dimensions.0,
            upload.dimensions.1,
            upload.mode_code,
            input.packed_luma_colorize,
            input.packed_luma_convert,
            input.packed_luma_transform_input,
            standard.packed_luma_transform_output,
            standard.native_rgb_compact_input,
            standard.native_rgb_packed_output,
            standard.native_la_transform_output,
            packed.packed_luma_point,
            packed.packed_luma_putdata,
            packed.packed_luma_order_statistic,
            packed.packed_native_byte_filter,
            packed.packed_luma_pad,
            packed.packed_la_pad,
            upload.native_extract_band_channels.is_some(),
            upload.native_grayscale_rgb_input,
            upload.native_sharpness_l_input,
            upload.native_sharpness_la_input,
            upload.native_sharpness_rgb_input,
            upload.native_reduce_rgb_input,
            upload.native_expand_channels,
            upload.native_cover_input_channels,
            standard.native_cover_packed_output,
            self.mode(),
            self.prepared.contrast_mean,
            proofs.constant_bits,
            proofs.box_copy_is_exact,
            proofs.identity_is_exact,
            proofs.box_average_is_exact,
            proofs.dyadic_is_exact,
            proofs.f64_is_exact,
            proofs.f64_ordered_is_exact,
            initial_input,
            &mut buffers,
        )?;
        resource_telemetry.fused_operation_count =
            resource_telemetry
                .fused_operation_count
                .max(gpu_fused_operation_count(
                    self.ops(),
                    self.mode(),
                    upload.dimensions,
                ));
        if let Some(input) = mapped_native_input {
            buffers.mapped_native_input = Some(input);
        }
        gpu_log!(
            "[GPU] step=execute_batch_impl done final=({},{}) is_a={}",
            final_w,
            final_h,
            final_is_a
        );
        Ok(GpuBatchSubmission {
            buffers,
            final_is_a,
            final_w,
            final_h,
            readback,
            resource_telemetry,
            dispatch_count,
            native_expand_output,
        })
    }

    fn readback_and_finalize(
        &self,
        standard: &StandardGpuRoutePlan,
        upload: &GpuBatchUploadPlan,
        mut submission: GpuBatchSubmission,
    ) -> Result<DynamicImage, PilError> {
        let output_layout = GpuBatchOutputLayout::from_operations(self.ops(), self.img().color());
        let native_luma16_output = output_layout.output_color_type == Some(ColorType::L16);
        gpu_log!("[GPU] step=readback start");
        let readback_buffer = submission
            .readback
            .buffer(&submission.buffers, submission.final_is_a);
        let result = self.decode_readback(
            &submission,
            standard,
            upload,
            &output_layout,
            native_luma16_output,
            readback_buffer,
        )?;
        gpu_log!("[GPU] step=readback done");
        self.record_readback_telemetry(standard, upload, &output_layout, &mut submission)?;
        crate::compute::record_pipeline_resource_telemetry(submission.resource_telemetry);
        crate::compute::record_pipeline_dispatch_count(submission.dispatch_count);
        if let ReadbackTarget::Staging(staging) = submission.readback {
            self.gpu.recycle_staging(staging);
        }
        self.gpu.recycle_buffers(submission.buffers);
        self.restore_output_mode(
            result,
            standard,
            upload,
            &output_layout,
            native_luma16_output,
        )
    }

    fn decode_readback(
        &self,
        submission: &GpuBatchSubmission,
        standard: &StandardGpuRoutePlan,
        upload: &GpuBatchUploadPlan,
        output_layout: &GpuBatchOutputLayout,
        native_luma16_output: bool,
        readback_buffer: &wgpu::Buffer,
    ) -> Result<DynamicImage, PilError> {
        let packed = upload.packed;
        let mode = self.mode();
        if let Some(dispatch) = standard.native_cover_packed_output {
            self.gpu.readback_to_cover_native_bytes(
                submission.final_w,
                submission.final_h,
                dispatch,
                readback_buffer,
            )
        } else if standard.native_rgb_packed_output {
            self.gpu.readback_to_native_channels(
                submission.final_w,
                submission.final_h,
                3,
                readback_buffer,
            )
        } else if packed.packed_la_pad
            || standard.native_la_transform_output
            || (packed.packed_native_byte_filter && matches!(mode, Some("LA")))
        {
            self.gpu.readback_to_native_channels(
                submission.final_w,
                submission.final_h,
                2,
                readback_buffer,
            )
        } else if upload.native_luma16 {
            self.gpu.readback_to_luma16(
                submission.final_w,
                submission.final_h,
                readback_buffer,
                mode,
            )
        } else if upload.native_luma16_paste || native_luma16_output {
            self.gpu.readback_to_luma16_numeric(
                submission.final_w,
                submission.final_h,
                readback_buffer,
            )
        } else if output_layout.native_luma8_extract
            || output_layout.native_luma8_grayscale
            || output_layout.native_luma8_constant
            || standard.packed_luma_transform_output
            || packed.packed_luma_point
            || packed.packed_luma_putdata
            || packed.packed_luma_order_statistic
            || packed.packed_native_byte_filter
            || packed.packed_luma_pad
            || upload.native_sharpness_l_input
        {
            self.gpu
                .readback_to_luma8(submission.final_w, submission.final_h, readback_buffer)
        } else if upload.native_sharpness_la_input {
            self.gpu.readback_to_luma_alpha_from_rgba_transport(
                submission.final_w,
                submission.final_h,
                readback_buffer,
            )
        } else if submission.native_expand_output {
            let channels = upload.native_expand_channels.ok_or_else(|| {
                PilError::InternalError(
                    "GPU native Expand output is missing its channel count".into(),
                )
            })?;
            self.gpu.readback_to_native_channels(
                submission.final_w,
                submission.final_h,
                channels,
                readback_buffer,
            )
        } else {
            self.gpu.readback_to_image(
                submission.final_w,
                submission.final_h,
                readback_buffer,
                output_layout.native_rgb,
            )
        }
    }

    fn record_readback_telemetry(
        &self,
        standard: &StandardGpuRoutePlan,
        upload: &GpuBatchUploadPlan,
        output_layout: &GpuBatchOutputLayout,
        submission: &mut GpuBatchSubmission,
    ) -> Result<(), PilError> {
        submission.resource_telemetry = gpu_batch_resource_telemetry(
            submission.resource_telemetry,
            &submission.buffers,
            &submission.readback,
            self.img(),
            GpuBatchResourceTelemetryOptions {
                input_dimensions: upload.dimensions,
                output_dimensions: (submission.final_w, submission.final_h),
                mode: self.mode(),
                output_layout,
                output_only_constant: upload.output_only_constant,
                full_luma_putdata: upload.full_luma_putdata,
                native_extract_band_channels: upload.native_extract_band_channels,
                native_grayscale_rgb_input: upload.native_grayscale_rgb_input,
                native_rgb_compact_input: standard.native_rgb_compact_input,
                native_sharpness_l_input: upload.native_sharpness_l_input,
                packed_native_byte_filter: upload.packed.packed_native_byte_filter,
                packed_la_pad: upload.packed.packed_la_pad,
                packed_luma_pad: upload.packed.packed_luma_pad,
                native_sharpness_la_input: upload.native_sharpness_la_input,
                native_la_transform_output: standard.native_la_transform_output,
                native_sharpness_rgb_input: upload.native_sharpness_rgb_input,
                native_reduce_rgb_input: upload.native_reduce_rgb_input,
                native_expand_channels: upload.native_expand_channels,
                native_cover_input_channels: upload.native_cover_input_channels,
                packed_luma_colorize: upload.input.packed_luma_colorize,
                packed_luma_convert: upload.input.packed_luma_convert,
                native_alpha_convert_input: upload.input.native_alpha_convert_input,
                packed_luma_transform_input: upload.input.packed_luma_transform_input,
                packed_luma_point: upload.packed.packed_luma_point,
                packed_luma_putdata: upload.packed.packed_luma_putdata,
                packed_luma_order_statistic: upload.packed.packed_luma_order_statistic,
                native_luma16_convert: upload.native_luma16_convert,
                native_luma16_paste: upload.native_luma16_paste,
                native_cover_packed_output: standard.native_cover_packed_output,
                native_rgb_packed_output: standard.native_rgb_packed_output,
                packed_luma_transform_output: standard.packed_luma_transform_output,
                native_expand_output: submission.native_expand_output,
            },
        )?;
        Ok(())
    }

    fn restore_output_mode(
        &self,
        result: DynamicImage,
        standard: &StandardGpuRoutePlan,
        upload: &GpuBatchUploadPlan,
        output_layout: &GpuBatchOutputLayout,
        native_luma16_output: bool,
    ) -> Result<DynamicImage, PilError> {
        let packed = upload.packed;
        if output_layout.native_rgb
            || standard.native_la_transform_output
            || upload.native_sharpness_l_input
            || upload.native_sharpness_la_input
            || standard.native_cover_packed_output.is_some()
            || native_luma16_output
            || packed.packed_luma_pad
            || packed.packed_la_pad
        {
            return Ok(result);
        }
        if output_layout.native_luma8_extract
            || output_layout.native_luma8_grayscale
            || output_layout.native_luma8_constant
            || standard.packed_luma_transform_output
            || packed.packed_luma_point
            || packed.packed_luma_putdata
            || packed.packed_luma_order_statistic
            || packed.packed_native_byte_filter
            || packed.packed_luma_pad
            || packed.packed_la_pad
        {
            return Ok(result);
        }
        if let Some(mode) = output_layout.put_alpha_mode {
            return put_alpha_output(result, mode);
        }
        if let Some(color_type) = output_layout.output_color_type {
            gpu_result_as_color_type(result, color_type)
        } else {
            Ok(crate::image::preserve_mode(self.img(), result))
        }
    }
}

pub(super) struct GpuBatchUploadPlan {
    dimensions: (u32, u32),
    mode_code: u32,
    full_luma_putdata: bool,
    native_luma16: bool,
    native_luma16_convert: bool,
    native_luma16_paste: bool,
    native_extract_band_channels: Option<u8>,
    output_only_constant: bool,
    native_grayscale_rgb_input: bool,
    native_grayscale_cmyk_input: bool,
    native_sharpness_l_input: bool,
    native_sharpness_la_input: bool,
    native_sharpness_rgb_input: bool,
    native_reduce_rgb_input: bool,
    native_expand_channels: Option<u8>,
    mapped_native_box_blur_input: bool,
    mapped_native_grayscale_input: bool,
    native_cover_input_channels: Option<u8>,
    native_sharpness_la_patch: Option<(usize, [u8; 2])>,
    input: BatchInputLayoutCandidates,
    packed: GpuPackedDispatchPlan,
}

struct GpuBatchSubmission {
    buffers: BufferPool,
    final_is_a: bool,
    final_w: u32,
    final_h: u32,
    readback: ReadbackTarget,
    resource_telemetry: PipelineResourceTelemetry,
    dispatch_count: u64,
    native_expand_output: bool,
}
