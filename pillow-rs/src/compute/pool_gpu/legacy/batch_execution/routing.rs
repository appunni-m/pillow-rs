use super::super::*;

mod execution;

impl GpuPool {
    fn execute_exact_host_result(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        reason: &str,
    ) -> Result<DynamicImage, PilError> {
        // Keep receipts honest: this path is an exact semantic bridge, not a
        // native GPU implementation.  The marker is consumed by the outer
        // execution boundary and therefore reports the pixels as CPU-owned
        // even when the packed result is subsequently copied through a real
        // GPU dispatch for transport/parity coverage.
        crate::compute::record_pipeline_backend_fallback(format!(
            "exact host semantic control: {reason}"
        ));
        let exact = crate::compute::CpuPool.execute_batch(ops, img, mode)?;

        // Keep the GPU parity lane honest when the result can use the
        // ordinary packed transport: the host implementation determines the
        // public pixels, then a real GPU Duplicate dispatch carries those
        // bytes through the selected device backend. Typed/empty results are
        // returned directly because the packed copy would narrow their
        // sample representation or require a zero-sized storage binding.
        if exact.width() == 0 || exact.height() == 0 || !gpu_image_layout_is_supported(&exact) {
            return Ok(exact);
        }
        let copied = self
            .execute_batch_with_policy(&[PipelineOp::Duplicate], &exact, None, false)
            .unwrap_or_else(|_| exact.clone());
        Ok(crate::image::preserve_mode(&exact, copied))
    }

    pub(in crate::compute::pool_gpu) fn preflight_failure(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        reason: &str,
    ) -> Result<DynamicImage, PilError> {
        if allow_cpu_fallback {
            crate::compute::record_pipeline_backend_fallback(reason);
            let cpu = crate::compute::CpuPool;
            return cpu.execute_batch(ops, img, mode);
        }

        // An explicitly locked GPU is a parity lane, not a permission to
        // manufacture a public NotImplementedError for a valid Pillow
        // operation.  The preflight guards below describe GPU storage and
        // shader limits; they do not describe the public operation's error
        // contract.  Run the same exact Rust operation here so invalid
        // inputs produce Pillow-compatible errors and valid inputs retain
        // their pixels while the native GPU implementation is completed.
        // Automatic routing still takes the normal CPU fallback branch above.
        self.execute_exact_host_result(ops, img, mode, reason)
    }

    fn preflight_batch_route(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        proofs: &GpuBatchResizeProofs,
    ) -> Result<BatchRouteStage<bool>, PilError> {
        // Pillow defines histogram operations as identity operations on empty
        // images; there is no valid storage-buffer invocation to submit.
        if (img.width() == 0 || img.height() == 0)
            && ops.iter().all(|op| {
                matches!(
                    op,
                    PipelineOp::Autocontrast { .. }
                        | PipelineOp::Equalize
                        | PipelineOp::EqualizeMasked { .. }
                )
            })
        {
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(BatchRouteStage::Return(img.clone()));
        }

        // Some geometry operations still require host semantic control. Their
        // packed results retain the explicit GPU copy used by the parity lane.
        if let Some(reason) = gpu_geometry_host_control_reason(
            ops,
            img,
            mode,
            proofs.constant_bits,
            proofs.box_copy_is_exact,
            proofs.identity_is_exact,
            proofs.box_average_is_exact,
            proofs.dyadic_is_exact,
            proofs.f64_is_exact || proofs.f64_ordered_is_exact,
        ) {
            return self
                .execute_exact_host_result(ops, img, mode, reason)
                .map(BatchRouteStage::Return);
        }

        let empty_two_input_noop = match self.preflight_batch_contract(
            ops,
            img,
            mode,
            allow_cpu_fallback,
            proofs.constant_bits,
            proofs.pad_f64_is_exact,
        )? {
            BatchContractPreflight::Continue {
                empty_two_input_noop,
            } => empty_two_input_noop,
            BatchContractPreflight::Return(image) => {
                return Ok(BatchRouteStage::Return(image));
            }
        };

        // Distance one still advances the same two RNG draws per pixel.
        if matches!(ops, [PipelineOp::EffectSpread { distance: 1 }])
            && img.width() != 0
            && img.height() != 0
        {
            crate::compute::pool_cpu::ops::effects::effect_spread_advance_identity_rng(
                img.width(),
                img.height(),
            )?;
            crate::compute::record_pipeline_operation_path("native-identity");
            crate::compute::record_pipeline_dispatch_count(0);
            return Ok(BatchRouteStage::Return(img.clone()));
        }
        Ok(BatchRouteStage::Continue(empty_two_input_noop))
    }

    fn preflight_batch_auxiliary_inputs(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        empty_two_input_noop: bool,
    ) -> Result<BatchRouteStage<ResolvedBatchAuxiliaryInputs>, PilError> {
        // Resolve nested images before GPU work so locked nested pipelines
        // surface their materialization errors here.
        let auxiliary_images = resolve_batch_auxiliary_images(ops, img, mode)?;
        let contrast_mean = match self.preflight_resolved_batch_inputs(
            ops,
            img,
            &auxiliary_images,
            mode,
            allow_cpu_fallback,
            empty_two_input_noop,
        )? {
            BatchResolvedInputPreflight::Continue { contrast_mean } => contrast_mean,
            BatchResolvedInputPreflight::Return(image) => {
                return Ok(BatchRouteStage::Return(image));
            }
        };
        Ok(BatchRouteStage::Continue(ResolvedBatchAuxiliaryInputs {
            auxiliary_images,
            contrast_mean,
        }))
    }

    pub(in crate::compute::pool_gpu) fn execute_batch_with_policy(
        &self,
        ops: &[PipelineOp],
        img: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<DynamicImage, PilError> {
        if ops.is_empty() {
            return Ok(img.clone());
        }
        let normalized = match self.normalize_batch_operations(ops, img, mode)? {
            BatchNormalization::Continue(normalized) => normalized,
            BatchNormalization::Return(image) => return Ok(image),
        };
        let ops = normalized.dispatch_ops.as_slice();
        if let Some(result) =
            self.execute_batch_segments_if_needed(ops, img, mode, allow_cpu_fallback)?
        {
            return Ok(result);
        }
        let resize_proofs = gpu_batch_resize_proofs(&normalized.dispatch_ops, img, mode);
        let preflight =
            match self.preflight_batch_route(ops, img, mode, allow_cpu_fallback, &resize_proofs)? {
                BatchRouteStage::Continue(empty_two_input_noop) => empty_two_input_noop,
                BatchRouteStage::Return(image) => return Ok(image),
            };
        let (auxiliary_images, contrast_mean) = match self.preflight_batch_auxiliary_inputs(
            ops,
            img,
            mode,
            allow_cpu_fallback,
            preflight,
        )? {
            BatchRouteStage::Continue(inputs) => (inputs.auxiliary_images, inputs.contrast_mean),
            BatchRouteStage::Return(image) => return Ok(image),
        };
        PreparedGpuBatchRoute {
            pool: self,
            img,
            mode,
            allow_cpu_fallback,
            normalized,
            resize_proofs,
            auxiliary_images,
            contrast_mean,
        }
        .execute()
    }
}

pub(super) struct PreparedGpuBatchRoute<'a> {
    pool: &'a GpuPool,
    img: &'a DynamicImage,
    mode: Option<&'a str>,
    allow_cpu_fallback: bool,
    normalized: NormalizedGpuBatch,
    resize_proofs: GpuBatchResizeProofs,
    auxiliary_images: Vec<AuxiliaryImages>,
    contrast_mean: Option<u8>,
}

pub(super) enum BatchRouteStage<T> {
    Continue(T),
    Return(DynamicImage),
}

struct ResolvedBatchAuxiliaryInputs {
    auxiliary_images: Vec<AuxiliaryImages>,
    contrast_mean: Option<u8>,
}
