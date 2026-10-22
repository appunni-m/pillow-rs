use super::*;

#[derive(Clone, Copy)]
pub(super) struct BatchEstimateOptions<'a> {
    pub(super) uniform_alignment: usize,
    pub(super) storage_alignment: usize,
    pub(super) packed_luma_putdata: bool,
    pub(super) logical_mode: Option<&'a str>,
    pub(super) f_resize_f64_is_exact: bool,
    pub(super) f_resize_f64_ordered_is_exact: bool,
}

pub(super) struct BatchExecutionEstimates {
    pub(super) resource_bytes: Vec<usize>,
    pub(super) shader_work_items: Vec<u64>,
}

impl GpuInner {
    pub(super) fn estimate_batch_resources(
        &self,
        ops: &[PipelineOp],
        auxiliary_images: &[AuxiliaryImages],
        dimensions: (u32, u32),
        mode: u32,
        options: BatchEstimateOptions<'_>,
        buffers: &BufferPool,
    ) -> Result<BatchExecutionEstimates, PilError> {
        let mut op_modes = Vec::with_capacity(ops.len());
        let mut current_mode = mode;
        for op in ops {
            op_modes.push(current_mode);
            current_mode = gpu_mode_after_op(current_mode, op);
        }

        let mut resource_source_dims = Vec::with_capacity(ops.len());
        let mut resource_w = dimensions.0;
        let mut resource_h = dimensions.1;
        for op in ops {
            resource_source_dims.push((resource_w, resource_h));
            let next = match op_output_dims(op, resource_w, resource_h) {
                Some(dimensions) => dimensions,
                None if op_has_explicit_output_dimensions(op) => {
                    return Err(PilError::ValueError(format!(
                        "GPU operation '{}' has no safe source dimensions",
                        registry::variant_key(op)
                    )));
                }
                None => (resource_w, resource_h),
            };
            (resource_w, resource_h) = next;
        }
        let resource_bytes = ops
            .iter()
            .zip(auxiliary_images.iter())
            .enumerate()
            .map(|(index, (op, auxiliary))| {
                self.estimate_resource_bytes(
                    op,
                    auxiliary,
                    buffers,
                    options.uniform_alignment,
                    options.storage_alignment,
                    resource_source_dims[index],
                    op_modes[index],
                    options.packed_luma_putdata,
                    options.logical_mode,
                    options.f_resize_f64_is_exact,
                    options.f_resize_f64_ordered_is_exact,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut work_w = dimensions.0;
        let mut work_h = dimensions.1;
        let shader_work_items = ops
            .iter()
            .map(|op| {
                let next = match op_output_dims(op, work_w, work_h) {
                    Some(dimensions) => dimensions,
                    None if op_has_explicit_output_dimensions(op) => {
                        return Err(PilError::ValueError(format!(
                            "GPU operation '{}' has no safe work dimensions",
                            registry::variant_key(op)
                        )));
                    }
                    None => (work_w, work_h),
                };
                let work = gpu_shader_work_items(op, (work_w, work_h), next, options.logical_mode)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{}' has no bounded shader work estimate",
                            registry::variant_key(op)
                        ))
                    })?;
                (work_w, work_h) = next;
                Ok(work)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(BatchExecutionEstimates {
            resource_bytes,
            shader_work_items,
        })
    }
}
