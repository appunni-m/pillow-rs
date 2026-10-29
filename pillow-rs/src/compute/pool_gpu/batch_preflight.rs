use super::logical_mode_admission::gpu_logical_mode_is_supported;
use super::*;

pub(super) enum BatchContractPreflight {
    Continue { empty_two_input_noop: bool },
    Return(DynamicImage),
}

pub(super) enum BatchResolvedInputPreflight {
    Continue { contrast_mean: Option<u8> },
    Return(DynamicImage),
}

#[derive(Clone, Copy)]
pub(super) struct GpuPackedDispatchCandidates {
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_putdata: bool,
    pub(super) packed_luma_order_statistic: bool,
    pub(super) packed_native_byte_filter: bool,
    pub(super) packed_luma_pad: bool,
    pub(super) packed_la_pad: bool,
}

#[derive(Clone, Copy)]
pub(super) struct GpuPackedDispatchPlan {
    pub(super) packed_luma_point: bool,
    pub(super) packed_luma_putdata: bool,
    pub(super) packed_luma_order_statistic_requested: bool,
    pub(super) packed_luma_order_statistic: bool,
    pub(super) packed_native_byte_filter_requested: bool,
    pub(super) packed_native_byte_filter: bool,
    pub(super) packed_luma_pad_requested: bool,
    pub(super) packed_luma_pad: bool,
    pub(super) packed_la_pad_requested: bool,
    pub(super) packed_la_pad: bool,
}

pub(super) enum BatchPackedDispatchPreflight {
    Continue(GpuPackedDispatchPlan),
    Return(DynamicImage),
}

pub(super) fn plan_gpu_packed_dispatches(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
    max_workgroups_per_dimension: u32,
    candidates: GpuPackedDispatchCandidates,
) -> GpuPackedDispatchPlan {
    let packed_luma_point = candidates.packed_luma_point
        && plan_packed_point_luma_dispatch(
            image.width(),
            image.height(),
            max_workgroups_per_dimension,
        )
        .is_ok();
    let packed_luma_putdata = candidates.packed_luma_putdata
        && plan_packed_luma_dispatch(image.width(), image.height(), max_workgroups_per_dimension)
            .is_ok();
    let packed_luma_order_statistic = candidates.packed_luma_order_statistic
        && plan_packed_luma_dispatch(image.width(), image.height(), max_workgroups_per_dimension)
            .is_ok();
    let packed_native_byte_filter = candidates.packed_native_byte_filter
        && if matches!(mode, Some("LA")) {
            plan_packed_la_dispatch(image.width(), image.height(), max_workgroups_per_dimension)
                .is_ok()
        } else {
            plan_packed_luma_dispatch(image.width(), image.height(), max_workgroups_per_dimension)
                .is_ok()
        };
    let packed_luma_pad = candidates.packed_luma_pad
        && matches!(ops, [op @ PipelineOp::Pad { .. }]
        if gpu_packed_luma_pad_dispatch_supported(
            op,
            image.dimensions(),
            max_workgroups_per_dimension,
        ));
    GpuPackedDispatchPlan {
        packed_luma_point,
        packed_luma_putdata,
        packed_luma_order_statistic_requested: candidates.packed_luma_order_statistic,
        packed_luma_order_statistic,
        packed_native_byte_filter_requested: candidates.packed_native_byte_filter,
        packed_native_byte_filter,
        packed_luma_pad_requested: candidates.packed_luma_pad,
        packed_luma_pad,
        packed_la_pad_requested: candidates.packed_la_pad,
        packed_la_pad: false,
    }
}

pub(super) struct GpuBatchResizeProofs {
    pub(super) constant_bits: Option<u32>,
    pub(super) box_copy_is_exact: bool,
    pub(super) identity_is_exact: bool,
    pub(super) box_average_is_exact: bool,
    pub(super) pad_f64_is_exact: bool,
    pub(super) dyadic_is_exact: bool,
    pub(super) f64_is_exact: bool,
    pub(super) f64_ordered_is_exact: bool,
}

pub(super) fn gpu_batch_resize_proofs(
    ops: &[PipelineOp],
    image: &DynamicImage,
    mode: Option<&str>,
) -> GpuBatchResizeProofs {
    let constant_bits = gpu_f_resize_constant_bits(ops, image, mode);
    let box_copy_is_exact = gpu_f_resize_box_copy_is_exact(ops, image, mode);
    let identity_is_exact = gpu_f_resize_identity_is_exact(ops, image, mode);
    let box_average_is_exact = gpu_f_resize_box_average_is_exact(ops, image, mode);
    let pad_f64_is_exact = gpu_f_pad_f64_is_exact(ops, image, mode);
    let ordered_shader_supported = gpu_f_resize_f64_ordered_shader_is_supported(ops, image, mode);
    // The static ordered-f64 contract covers every source bit pattern and
    // avoids the per-pixel marker-6 simulation when geometry already qualifies.
    let dyadic_is_exact =
        !ordered_shader_supported && gpu_f_resize_dyadic_is_exact(ops, image, mode);
    // Use the bounded ordered-f64 shader's static coefficient/geometry
    // contract first; retain the input-driven marker-9 and marker-12 proofs
    // only when that shader cannot handle this resize.
    let f64_is_exact = !ordered_shader_supported
        && (gpu_f_resize_f64_is_exact(ops, image, mode)
            || pad_f64_is_exact
            || gpu_luma16_resize_f64_is_exact(ops, image, mode)
            || gpu_i_resize_f64_is_exact(ops, image, mode));
    let ordered_proof = ordered_shader_supported
        || (!f64_is_exact && gpu_f_resize_f64_ordered_is_exact(ops, image, mode));
    // Marker 12 is selected only if no earlier F proof owns the operation.
    // Box-average and dyadic markers use fixed-point tables; marker 12 needs
    // the four-word f64 coefficient arena.
    let f64_ordered_is_exact = ordered_proof
        && !f64_is_exact
        && constant_bits.is_none()
        && !box_copy_is_exact
        && !identity_is_exact
        && !box_average_is_exact
        && !dyadic_is_exact;
    GpuBatchResizeProofs {
        constant_bits,
        box_copy_is_exact,
        identity_is_exact,
        box_average_is_exact,
        pad_f64_is_exact,
        dyadic_is_exact,
        f64_is_exact,
        f64_ordered_is_exact,
    }
}

impl GpuPool {
    pub(super) fn preflight_packed_dispatches(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        max_workgroups_per_dimension: u32,
        mut plan: GpuPackedDispatchPlan,
    ) -> Result<BatchPackedDispatchPreflight, PilError> {
        if plan.packed_luma_pad_requested && !plan.packed_luma_pad {
            gpu_log!(
                "[GPU] dispatch preflight routed packed-L Pad to CPU: adapter workgroup limit"
            );
            return self.packed_dispatch_preflight_failure(context, ops, image, mode, allow_cpu_fallback);
        }
        plan.packed_la_pad = plan.packed_la_pad_requested
            && matches!(ops, [op @ PipelineOp::Pad { .. }]
            if gpu_packed_la_pad_dispatch_supported(
                op,
                image.dimensions(),
                max_workgroups_per_dimension,
            ));
        if plan.packed_la_pad_requested && !plan.packed_la_pad {
            gpu_log!(
                "[GPU] dispatch preflight routed packed-LA Pad to CPU: adapter workgroup limit"
            );
            return self.packed_dispatch_preflight_failure(context, ops, image, mode, allow_cpu_fallback);
        }
        if plan.packed_luma_order_statistic_requested && !plan.packed_luma_order_statistic {
            gpu_log!(
                "[GPU] dispatch preflight routed packed L order statistic to CPU: adapter workgroup limit"
            );
            return self.packed_dispatch_preflight_failure(context, ops, image, mode, allow_cpu_fallback);
        }
        if plan.packed_native_byte_filter_requested && !plan.packed_native_byte_filter {
            gpu_log!(
                "[GPU] dispatch preflight routed packed native blur to CPU: adapter workgroup limit"
            );
            return self.packed_dispatch_preflight_failure(context, ops, image, mode, allow_cpu_fallback);
        }
        Ok(BatchPackedDispatchPreflight::Continue(plan))
    }

    fn packed_dispatch_preflight_failure(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<BatchPackedDispatchPreflight, PilError> {
        self.preflight_failure(
            context,
            ops,
            image,
            mode,
            allow_cpu_fallback,
            "adapter workgroup limit",
        )
        .map(BatchPackedDispatchPreflight::Return)
    }

    pub(super) fn preflight_resolved_batch_inputs(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        auxiliary_images: &[AuxiliaryImages],
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        empty_two_input_noop: bool,
    ) -> Result<BatchResolvedInputPreflight, PilError> {
        if empty_two_input_noop {
            if !gpu_empty_two_input_inputs_are_safe(ops, image, auxiliary_images) {
                gpu_log!(
                    "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete empty-image inputs"
                );
                return self
                    .preflight_failure(
                        context,
                        ops,
                        image,
                        mode,
                        allow_cpu_fallback,
                        "unsafe or incomplete empty-image inputs",
                    )
                    .map(BatchResolvedInputPreflight::Return);
            }
            // No invocations are submitted for this valid empty-image no-op.
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(BatchResolvedInputPreflight::Return(image.clone()));
        }

        if gpu_pipeline_requires_cpu(ops, image, auxiliary_images, mode) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe or incomplete image dimensions"
            );
            return self
                .preflight_failure(
                    context,
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "unsafe or incomplete image dimensions",
                )
                .map(BatchResolvedInputPreflight::Return);
        }

        // Contrast's midpoint belongs to the current source image. A single
        // exact PutPixel prefix can be mirrored; other prefixes need host control.
        let contrast_mean = if ops
            .iter()
            .any(|op| matches!(op, PipelineOp::Contrast { .. }))
        {
            match gpu_contrast_mean_after_exact_prefix(image, ops, mode) {
                Some(mean) => Some(mean),
                None => {
                    let reason = if ops
                        .iter()
                        .position(|op| matches!(op, PipelineOp::Contrast { .. }))
                        == Some(0)
                    {
                        "Contrast midpoint is unavailable for this image layout"
                    } else {
                        "Contrast midpoint requires the current host image"
                    };
                    return self
                        .preflight_failure(context, ops, image, mode, allow_cpu_fallback, reason)
                        .map(BatchResolvedInputPreflight::Return);
                }
            }
        } else {
            None
        };

        Ok(BatchResolvedInputPreflight::Continue { contrast_mean })
    }

    pub(super) fn preflight_batch_contract(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        f_resize_constant_bits: Option<u32>,
        f_pad_f64_is_exact: bool,
    ) -> Result<BatchContractPreflight, PilError> {
        if let Some(image) = self.preflight_batch_layout(
            context,
            ops,
            image,
            mode,
            allow_cpu_fallback,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
        )? {
            return Ok(BatchContractPreflight::Return(image));
        }

        self.preflight_batch_operations(context, ops, image, mode, allow_cpu_fallback)
    }

    fn preflight_batch_layout(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        f_resize_constant_bits: Option<u32>,
        f_pad_f64_is_exact: bool,
    ) -> Result<Option<DynamicImage>, PilError> {
        // GPU shaders consume the packed L/LA/RGB/RGBA representation. P and
        // PA are safe for raw channel-wise Chops because the core exposes
        // their index/(index, alpha) samples as Luma8/LumaA8 buffers; no
        // palette expansion is involved. CMYK is additionally safe for a
        // brightness/color-saturation batch because its C/M/Y/K bytes share
        // the four-byte transport and those shaders explicitly process K.
        // Grayscale has a dedicated shader branch that fuses CMYK-to-RGB
        // arithmetic with luma. EffectSpread gathers complete packed words,
        // so it is safe for all modes backed by ordinary byte transport.
        let logical_mode_supported = gpu_logical_mode_is_supported(
            ops,
            Some(image),
            mode,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
            true,
        );
        if !logical_mode_supported {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to exact host semantic control: logical layout"
            );
            return self
                .preflight_failure(
                    context,
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "GPU logical-mode operation contract is not proven",
                )
                .map(Some);
        }

        // A palette-index PutPixel stores an index in the same Luma8 layout
        // as ordinary L. The logical mode must keep that index from being
        // treated as a visible luma sample; PA uses its separate alpha layout.
        if ops.iter().any(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: true,
                    ..
                }
            )
        }) && mode != Some("P")
        {
            return self
                .preflight_failure(
                    context,
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "palette index requires P logical mode",
                )
                .map(Some);
        }

        // The uniform mode word describes the source layout for the whole
        // batch. A non-terminal mode change would make later shaders decode
        // the new pixels with the old layout.
        if gpu_batch_has_nonterminal_mode_change(ops) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: mode-changing op is not terminal"
            );
            return self
                .preflight_failure(
                    context,
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "non-terminal mode change",
                )
                .map(Some);
        }

        if !gpu_image_layout_is_supported(image)
            && !gpu_luma16_geometry_is_supported(ops, image, mode)
            && !gpu_luma16_convert_is_supported(ops, image)
            && !gpu_luma16_paste_is_supported(ops, image)
        {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsupported native pixel layout"
            );
            return self
                .preflight_failure(
                    context,
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "unsupported native pixel layout",
                )
                .map(Some);
        }

        Ok(None)
    }

    fn preflight_batch_operations(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<BatchContractPreflight, PilError> {
        for op in ops {
            let i_mode_filter = mode == Some("I")
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                )
                && (gpu_int_filter_is_supported(ops, image)
                    || gpu_int_filter_resize_chain_is_supported(ops, image));
            if !registry::gpu_supports(op)? && !i_mode_filter {
                return self
                    .preflight_failure(
                        context,
                        ops,
                        image,
                        mode,
                        allow_cpu_fallback,
                        "no valid single-dispatch shader contract",
                    )
                    .map(BatchContractPreflight::Return);
            }
        }

        // Keep this guard at execution time as well as in `supports`: explicit
        // backend selection and future callers must not bypass shader bounds.
        if let Err(error) = validate_gpu_operations(ops, image, mode) {
            let reason = error.to_string();
            return self
                .preflight_failure(context, ops, image, mode, allow_cpu_fallback, &reason)
                .map(BatchContractPreflight::Return);
        }

        // Check dimensions before resolving nested images or initializing a
        // device. Empty two-input no-ops are the established exception.
        let empty_two_input_noop = gpu_empty_two_input_batch_is_noop(ops, image);
        if gpu_dimensions_require_cpu(ops, image, mode) && !empty_two_input_noop {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe primary image dimensions"
            );
            return self
                .preflight_failure(
                    context,
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "unsafe primary image dimensions",
                )
                .map(BatchContractPreflight::Return);
        }

        Ok(BatchContractPreflight::Continue {
            empty_two_input_noop,
        })
    }
}
