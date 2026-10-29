use super::*;

impl GpuPool {
    pub(super) fn execute_batch_segments_if_needed(
        &self,
        context: &ExecutionContext,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<Option<DynamicImage>, PilError> {
        // A packed dispatch has one logical layout uniform. Operations such
        // as Grayscale, ExtractBand, and PutAlpha change that layout for the
        // following node, so they cannot share a dispatch with later work.
        // F resize chains likewise require materialized FLOAT32 words between
        // public resize calls, so split before deriving proof flags.
        let segment_boundary = gpu_first_nonterminal_mode_change(ops).or_else(|| {
            (mode == Some("F")
                && ops.len() > 1
                && ops.iter().all(|op| matches!(op, PipelineOp::Resize { .. })))
            .then_some(0)
        });
        let Some(mode_index) = segment_boundary else {
            return Ok(None);
        };

        let (prefix, suffix) = ops.split_at(mode_index + 1);
        let prefix_result =
            self.execute_batch_with_policy(context, prefix, image, mode, allow_cpu_fallback)?;
        // A recursive segment publishes its own receipt counters. Drain them
        // before executing the suffix so the outer benchmark sample can
        // retain one aggregate rather than only the final segment's counts.
        let prefix_override = crate::compute::take_pipeline_backend_override();
        let prefix_resource = crate::compute::take_pipeline_resource_telemetry();
        let prefix_dispatch = crate::compute::take_pipeline_dispatch_count().unwrap_or(0);
        if suffix.is_empty() {
            if let Some(prefix_override) = prefix_override {
                crate::compute::restore_pipeline_backend_override(prefix_override);
            }
            if let Some(resource) = prefix_resource {
                crate::compute::record_pipeline_resource_telemetry(resource);
            }
            crate::compute::record_pipeline_dispatch_count(prefix_dispatch);
            return Ok(Some(prefix_result));
        }

        let next_mode = gpu_logical_mode_after_op(mode, &prefix[mode_index], &prefix_result);
        let suffix_result = self.execute_batch_with_policy(
            context,
            suffix,
            &prefix_result,
            next_mode.as_deref(),
            allow_cpu_fallback,
        )?;
        let suffix_override = crate::compute::take_pipeline_backend_override();
        let suffix_resource = crate::compute::take_pipeline_resource_telemetry();
        let suffix_dispatch = crate::compute::take_pipeline_dispatch_count().unwrap_or(0);
        // A host-controlled prefix may feed a native GPU suffix, but its CPU
        // override must not mislabel the terminal executor. Preserve the
        // diagnostic reason while reporting the final segment's backend. If
        // the suffix also fell back, its terminal CPU override wins unchanged.
        if let Some(suffix_override) = suffix_override {
            crate::compute::restore_pipeline_backend_override(suffix_override);
        } else if let Some((_, reason)) = prefix_override {
            crate::compute::restore_pipeline_backend_override((Backend::Gpu, reason));
        }
        let mut resource = None;
        crate::compute::merge_pipeline_resource_telemetry(&mut resource, prefix_resource);
        crate::compute::merge_pipeline_resource_telemetry(&mut resource, suffix_resource);
        if let Some(resource) = resource {
            crate::compute::record_pipeline_resource_telemetry(resource);
        }
        crate::compute::record_pipeline_dispatch_count(
            prefix_dispatch.saturating_add(suffix_dispatch),
        );
        Ok(Some(suffix_result))
    }
}
