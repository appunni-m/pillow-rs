use super::super::super::{
    BatchInputLayoutCandidates, BatchPackedDispatchPreflight, GpuInner,
    GpuPackedDispatchCandidates, GpuPackedDispatchPlan, GpuPool, NativeCoverPackedOutputDispatch,
    NativeGeometryDispatchOutcome, NativeMaskedBytePasteDispatch, gpu_batch_capacity,
    gpu_buffer_capacity_exceeds_limits, gpu_dispatch_dimensions_require_cpu,
    gpu_native_cover_packed_output_dispatch, gpu_native_rgb_luma_merge_bands,
    plan_gpu_native_masked_byte_paste_for_op, plan_gpu_native_rgb_crop, plan_gpu_native_rgb_merge,
    plan_gpu_packed_dispatches, plan_native_hsv_pad_dispatch, plan_native_la_transform_output,
    plan_native_luma_transform_output, plan_native_rgb_point_output,
    plan_native_rgb_transform_output,
};
use super::{BatchRouteStage, PreparedGpuBatchRoute};
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::pipeline::{PipelineOp, PixelMode};
use crate::raster::{DynamicImage, GenericImageView};

mod standard;

impl PreparedGpuBatchRoute<'_> {
    pub(super) fn execute(self) -> Result<DynamicImage, PilError> {
        let gpu = GpuPool::ensure_init()?;
        gpu.ensure_healthy("GPU batch start")?;
        let max_workgroups_per_dimension = gpu.device.limits().max_compute_workgroups_per_dimension;
        let candidates = self.packed_dispatch_candidates();
        let packed_plan = plan_gpu_packed_dispatches(
            self.ops(),
            self.img,
            self.mode,
            max_workgroups_per_dimension,
            candidates,
        );
        let packed_plan = match self.pool.preflight_packed_dispatches(
            self.ops(),
            self.img,
            self.mode,
            self.allow_cpu_fallback,
            max_workgroups_per_dimension,
            packed_plan,
        )? {
            BatchPackedDispatchPreflight::Continue(plan) => plan,
            BatchPackedDispatchPreflight::Return(image) => return Ok(image),
        };
        let image_pixels = CheckedDims::new(self.img.width(), self.img.height(), 1)?.total_pixels();
        let full_luma_putdata = packed_plan.packed_luma_putdata
            && matches!(self.ops(), [PipelineOp::PutData { data, mode: PixelMode::L }]
                if data.len() == image_pixels);
        GpuBatchExecutionRoute {
            prepared: self,
            gpu,
            packed_plan,
            full_luma_putdata,
        }
        .execute()
    }

    fn ops(&self) -> &[PipelineOp] {
        &self.normalized.dispatch_ops
    }

    fn packed_dispatch_candidates(&self) -> GpuPackedDispatchCandidates {
        let input = self.normalized.input_layouts;
        GpuPackedDispatchCandidates {
            packed_luma_point: input.packed_luma_point,
            packed_luma_putdata: input.packed_luma_putdata,
            packed_luma_order_statistic: input.packed_luma_order_statistic,
            packed_native_byte_filter: input.packed_native_byte_filter,
            packed_luma_pad: input.packed_luma_pad,
            packed_la_pad: input.packed_la_pad,
        }
    }
}

struct GpuBatchExecutionRoute<'a> {
    prepared: PreparedGpuBatchRoute<'a>,
    gpu: &'static GpuInner,
    packed_plan: GpuPackedDispatchPlan,
    full_luma_putdata: bool,
}

impl GpuBatchExecutionRoute<'_> {
    fn execute(&self) -> Result<DynamicImage, PilError> {
        let limits = self.gpu.device.limits();
        if let Some(result) = self.try_native_rgb_routes(&limits)? {
            return Ok(result);
        }
        if let Some(result) = self.try_native_compact_routes(&limits)? {
            return Ok(result);
        }
        let standard = match self.plan_standard_route(&limits)? {
            BatchRouteStage::Continue(plan) => plan,
            BatchRouteStage::Return(image) => return Ok(image),
        };
        if let Some(result) = self.try_native_geometry_routes(&limits, standard.capacity)? {
            return Ok(result);
        }
        let buffers = self.gpu.acquire_buffers(standard.capacity)?;
        self.log_operation_keys();
        let upload = self.build_upload_plan(&standard);
        self.log_upload_start(&upload);
        self.upload_batch_input(&standard, &upload, buffers)
    }

    fn ops(&self) -> &[PipelineOp] {
        self.prepared.ops()
    }

    fn img(&self) -> &DynamicImage {
        self.prepared.img
    }

    fn mode(&self) -> Option<&str> {
        self.prepared.mode
    }

    fn input_layouts(&self) -> BatchInputLayoutCandidates {
        self.prepared.normalized.input_layouts
    }

    fn try_native_rgb_routes(
        &self,
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        #[cfg(target_endian = "little")]
        {
            let ops = self.ops();
            let img = self.img();
            let mode = self.mode();
            if matches!(mode, None | Some("RGB"))
                && matches!(img, DynamicImage::ImageRgb8(_))
                && let [
                    PipelineOp::Crop {
                        left,
                        top,
                        right,
                        bottom,
                    },
                ] = ops
                && let Some(dispatch) = plan_gpu_native_rgb_crop(
                    img.width(),
                    img.height(),
                    *left,
                    *top,
                    *right,
                    *bottom,
                    limits.max_compute_workgroups_per_dimension,
                    limits.max_storage_buffer_binding_size,
                    limits.max_buffer_size,
                )
            {
                let mut buffers = self.gpu.acquire_buffers(dispatch.buffer_words)?;
                let result =
                    self.gpu
                        .execute_native_rgb_crop(img, *left, *top, dispatch, &mut buffers)?;
                self.gpu.recycle_buffers(buffers);
                return Ok(Some(result));
            }
            if let Some(bands) = gpu_native_rgb_luma_merge_bands(ops, img, mode)
                && let Some(dispatch) = plan_gpu_native_rgb_merge(
                    img.width(),
                    img.height(),
                    limits.max_compute_workgroups_per_dimension,
                    limits.max_storage_buffer_binding_size,
                    limits.max_buffer_size,
                )
            {
                let mut buffers = self.gpu.acquire_buffers(dispatch.buffer_words)?;
                let result =
                    self.gpu
                        .execute_native_rgb_luma_merge(img, bands, dispatch, &mut buffers)?;
                self.gpu.recycle_buffers(buffers);
                return Ok(Some(result));
            }
        }
        #[cfg(not(target_endian = "little"))]
        let _ = limits;
        Ok(None)
    }

    fn plan_native_masked_byte_paste(
        &self,
        limits: &wgpu::Limits,
    ) -> Option<NativeMaskedBytePastePlan<'_>> {
        let [op] = self.ops() else {
            return None;
        };
        let [auxiliary] = self.prepared.auxiliary_images.as_slice() else {
            return None;
        };
        let (Some(source_pixels), Some(mask_pixels)) =
            (auxiliary.second.as_deref(), auxiliary.third.as_deref())
        else {
            return None;
        };
        let (offset, logical_mode, dispatch) = plan_gpu_native_masked_byte_paste_for_op(
            op,
            self.img(),
            source_pixels,
            mask_pixels,
            self.mode(),
            limits,
        )?;
        Some(NativeMaskedBytePastePlan {
            source_pixels,
            mask_pixels,
            offset,
            logical_mode,
            dispatch,
        })
    }

    fn try_native_compact_routes(
        &self,
        limits: &wgpu::Limits,
    ) -> Result<Option<DynamicImage>, PilError> {
        let ops = self.ops();
        let img = self.img();
        let mode = self.mode();
        #[cfg(target_endian = "little")]
        let native_masked_byte_paste = self.plan_native_masked_byte_paste(limits);
        #[cfg(not(target_endian = "little"))]
        let native_masked_byte_paste: Option<NativeMaskedBytePastePlan<'_>> = None;
        #[cfg(target_endian = "little")]
        let native_hsv_pad = plan_native_hsv_pad_dispatch(
            ops,
            img,
            mode,
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        );
        #[cfg(not(target_endian = "little"))]
        let native_hsv_pad: Option<super::super::super::NativeHsvPadDispatch> = None;

        if native_masked_byte_paste.is_none()
            && native_hsv_pad.is_none()
            && !self.packed_plan.packed_luma_order_statistic
            && gpu_dispatch_dimensions_require_cpu(
                ops,
                img.dimensions(),
                limits.max_compute_workgroups_per_dimension,
                mode,
                self.packed_plan.packed_luma_pad,
            )
        {
            gpu_log!("[GPU] dispatch preflight routed batch to CPU: adapter workgroup limit");
            return self
                .prepared
                .pool
                .preflight_failure(
                    ops,
                    img,
                    mode,
                    self.prepared.allow_cpu_fallback,
                    "adapter workgroup limit",
                )
                .map(Some);
        }
        if let Some(plan) = native_masked_byte_paste {
            let mut buffers = self.gpu.acquire_buffers(plan.dispatch.word_count)?;
            let result = self.gpu.execute_native_masked_byte_paste(
                img,
                plan.source_pixels,
                plan.mask_pixels,
                plan.logical_mode,
                plan.offset,
                plan.dispatch,
                &mut buffers,
            )?;
            self.gpu.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        #[cfg(target_endian = "little")]
        if let Some(dispatch) = native_hsv_pad {
            let mut buffers = self
                .gpu
                .acquire_buffers(dispatch.output.output_word_count)?;
            let result = self
                .gpu
                .execute_native_hsv_pad(img, dispatch, &mut buffers)?;
            self.gpu.recycle_buffers(buffers);
            return Ok(Some(result));
        }
        Ok(None)
    }

    fn plan_standard_route(
        &self,
        limits: &wgpu::Limits,
    ) -> Result<BatchRouteStage<StandardGpuRoutePlan>, PilError> {
        let ops = self.ops();
        let img = self.img();
        let mode = self.mode();
        let auxiliary_images = &self.prepared.auxiliary_images;
        let input = self.input_layouts();
        let capacity = gpu_batch_capacity(ops, img, auxiliary_images, mode)?;
        let packed_luma_transform_output = input.packed_luma_transform_input
            && plan_native_luma_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
            .is_some();
        let native_rgb_output_plan = if matches!(
            ops,
            [PipelineOp::Eval { .. }
                | PipelineOp::Equalize
                | PipelineOp::MedianFilter { size: 3 }
                | PipelineOp::MinFilter { size: 3 }
                | PipelineOp::MaxFilter { size: 3 }
                | PipelineOp::BoxBlur { radius: 1 }]
        ) {
            plan_native_rgb_point_output(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
        } else {
            plan_native_rgb_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
        };
        let native_rgb_packed_output =
            input.native_rgb_compact_input && native_rgb_output_plan.is_some();
        // Keep compact input and output paired for point, equalize, and 3x3
        // order filters; other operations may retain compact input alone.
        let native_rgb_compact_input = input.native_rgb_compact_input
            && (!matches!(
                ops,
                [PipelineOp::Eval { .. }
                    | PipelineOp::Equalize
                    | PipelineOp::MedianFilter { size: 3 }
                    | PipelineOp::MinFilter { size: 3 }
                    | PipelineOp::MaxFilter { size: 3 }
                    | PipelineOp::BoxBlur { radius: 1 }]
            ) || native_rgb_packed_output);
        let native_la_transform_output = input.native_la_transform_input
            && plan_native_la_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
            .is_some();
        let native_cover_packed_output = gpu_native_cover_packed_output_dispatch(
            ops,
            self.prepared.normalized.native_cover_input_channels,
            mode,
            img.height(),
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
            capacity,
        );
        if gpu_buffer_capacity_exceeds_limits(
            capacity,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        ) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: image buffer exceeds adapter limits"
            );
            return self
                .prepared
                .pool
                .preflight_failure(
                    ops,
                    img,
                    mode,
                    self.prepared.allow_cpu_fallback,
                    "image buffer exceeds adapter limits",
                )
                .map(BatchRouteStage::Return);
        }
        Ok(BatchRouteStage::Continue(StandardGpuRoutePlan {
            capacity,
            packed_luma_transform_output,
            native_rgb_compact_input,
            native_rgb_packed_output,
            native_la_transform_output,
            native_cover_packed_output,
        }))
    }

    fn try_native_geometry_routes(
        &self,
        limits: &wgpu::Limits,
        capacity: u32,
    ) -> Result<Option<DynamicImage>, PilError> {
        #[cfg(target_endian = "little")]
        {
            if let Some(result) = self.gpu.try_execute_native_batch_routes(
                self.ops(),
                self.img(),
                self.mode(),
                &self.prepared.auxiliary_images,
                limits,
            )? {
                return Ok(Some(result));
            }
            match self.gpu.try_execute_native_flip(
                self.ops(),
                self.img(),
                self.mode(),
                limits,
                capacity,
            )? {
                NativeGeometryDispatchOutcome::NotApplicable => {}
                NativeGeometryDispatchOutcome::ExceedsLimits(reason) => {
                    return self
                        .prepared
                        .pool
                        .preflight_failure(
                            self.ops(),
                            self.img(),
                            self.mode(),
                            self.prepared.allow_cpu_fallback,
                            reason,
                        )
                        .map(Some);
                }
                NativeGeometryDispatchOutcome::Executed(result) => return Ok(Some(result)),
            }
            match self.gpu.try_execute_native_transpose_paths(
                self.ops(),
                self.img(),
                self.mode(),
                limits,
                capacity,
            )? {
                NativeGeometryDispatchOutcome::NotApplicable => {}
                NativeGeometryDispatchOutcome::ExceedsLimits(reason) => {
                    return self
                        .prepared
                        .pool
                        .preflight_failure(
                            self.ops(),
                            self.img(),
                            self.mode(),
                            self.prepared.allow_cpu_fallback,
                            reason,
                        )
                        .map(Some);
                }
                NativeGeometryDispatchOutcome::Executed(result) => return Ok(Some(result)),
            }
        }
        #[cfg(not(target_endian = "little"))]
        let _ = (limits, capacity);
        Ok(None)
    }
}

#[derive(Clone, Copy)]
struct StandardGpuRoutePlan {
    capacity: u32,
    packed_luma_transform_output: bool,
    native_rgb_compact_input: bool,
    native_rgb_packed_output: bool,
    native_la_transform_output: bool,
    native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
}

struct NativeMaskedBytePastePlan<'a> {
    source_pixels: &'a DynamicImage,
    mask_pixels: &'a DynamicImage,
    offset: (i32, i32),
    logical_mode: &'static str,
    dispatch: NativeMaskedBytePasteDispatch,
}
