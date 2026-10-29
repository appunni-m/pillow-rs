use super::*;

pub(super) struct GpuBatchPreparationOutput<'a> {
    pub(super) uniform_alignment: usize,
    pub(super) storage_alignment: usize,
    pub(super) direct_img2_bytes: &'a [u8],
    pub(super) params_arena: Vec<u32>,
    pub(super) params_ranges: Vec<BufferRange>,
    pub(super) img2_arena: Vec<u32>,
    pub(super) img2_ranges: Vec<Option<BufferRange>>,
    pub(super) img3_arena: Vec<u32>,
    pub(super) img3_ranges: Vec<Option<BufferRange>>,
    pub(super) lut_arena: Vec<u32>,
    pub(super) lut_ranges: Vec<Option<BufferRange>>,
    pub(super) resize_coeff_ranges: Vec<Option<(BufferRange, BufferRange)>>,
    pub(super) input_dims: Vec<(u32, u32)>,
    pub(super) output_dims: Vec<(u32, u32)>,
    pub(super) resize_source_rows: Vec<Option<(u32, u32)>>,
    pub(super) pad_resize_dims: Vec<Option<(u32, u32)>>,
    pub(super) native_expand_output: Option<NativeExpandOutputDispatch>,
    pub(super) native_luma_transform_output: Option<NativeExpandOutputDispatch>,
    pub(super) native_rgb_packed_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_la_transform_output: Option<NativePackedChannelOutputDispatch>,
    pub(super) native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
    pub(super) final_dims: (u32, u32),
}

impl GpuInner {
    pub(super) fn finalize_prepared_batch<'a>(
        &self,
        buffers: &'a mut BufferPool,
        auxiliary_cache: &GpuAuxiliaryCache,
        output: GpuBatchPreparationOutput<'_>,
    ) -> Result<PreparedGpuBatch<'a>, PilError> {
        buffers.params_arena.ensure_capacity(
            &self.device,
            "gpu_batch_params",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            output
                .params_arena
                .len()
                .saturating_mul(std::mem::size_of::<u32>()),
            output.uniform_alignment,
        );
        self.queue.write_buffer(
            &buffers.params_arena.buffer,
            0,
            bytemuck::cast_slice(&output.params_arena),
        );

        let img2 = self.write_batch_auxiliary_arena(
            &mut buffers.img2_arena,
            "gpu_batch_img2",
            &auxiliary_cache.img2_values,
            &output.img2_arena,
            output.direct_img2_bytes,
            output.storage_alignment,
        );
        let img3 = self.write_batch_auxiliary_arena(
            &mut buffers.img3_arena,
            "gpu_batch_img3",
            &auxiliary_cache.img3_values,
            &output.img3_arena,
            &[],
            output.storage_alignment,
        );
        let lut = self.write_batch_auxiliary_arena(
            &mut buffers.lut_arena,
            "gpu_batch_lut",
            &auxiliary_cache.lut_values,
            &output.lut_arena,
            &[],
            output.storage_alignment,
        );
        let resource_telemetry = PipelineResourceTelemetry {
            parameter_bytes: (output.params_arena.len() * std::mem::size_of::<u32>()) as u64,
            auxiliary_bytes: ((output.img2_arena.len()
                + output.img3_arena.len()
                + output.lut_arena.len())
                * std::mem::size_of::<u32>()) as u64
                + output.direct_img2_bytes.len() as u64,
            ..PipelineResourceTelemetry::default()
        };

        Ok(PreparedGpuBatch {
            resources: GpuBatchResources {
                buf_a: &buffers.buf_a,
                buf_b: &buffers.buf_b,
                initial_input: None,
                fallback_img2: &buffers.buf_img2,
                fallback_img3: &buffers.buf_img3,
                fallback_lut: &buffers.lut_buf,
                histogram: &buffers.histogram_buf,
                params: &buffers.params_arena.buffer,
                params_ranges: output.params_ranges,
                img2,
                img2_ranges: output.img2_ranges,
                resize_coeff_ranges: output.resize_coeff_ranges,
                img3,
                img3_ranges: output.img3_ranges,
                lut,
                lut_ranges: output.lut_ranges,
                native_expand_output: output.native_expand_output,
                native_luma_transform_output: output.native_luma_transform_output,
                native_rgb_packed_output: output.native_rgb_packed_output,
                native_la_transform_output: output.native_la_transform_output,
                native_cover_packed_output: output.native_cover_packed_output,
            },
            input_dims: output.input_dims,
            output_dims: output.output_dims,
            resize_source_rows: output.resize_source_rows,
            pad_resize_dims: output.pad_resize_dims,
            final_dims: output.final_dims,
            resource_telemetry,
        })
    }

    fn write_batch_auxiliary_arena<'a>(
        &self,
        arena: &'a mut ReusableGpuBuffer,
        label: &'static str,
        cached_values: &[u32],
        operation_values: &[u32],
        direct_bytes: &[u8],
        storage_alignment: usize,
    ) -> Option<&'a wgpu::Buffer> {
        let arena_bytes = (cached_values.len() + operation_values.len())
            .saturating_mul(std::mem::size_of::<u32>());
        let required_bytes = arena_bytes.max(direct_bytes.len());
        if required_bytes == 0 {
            return None;
        }

        let previous_capacity = arena.capacity_bytes;
        arena.ensure_capacity(
            &self.device,
            label,
            wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            required_bytes,
            storage_alignment,
        );
        if arena.capacity_bytes != previous_capacity && !cached_values.is_empty() {
            self.queue
                .write_buffer(&arena.buffer, 0, bytemuck::cast_slice(cached_values));
        }
        if !operation_values.is_empty() {
            self.queue.write_buffer(
                &arena.buffer,
                std::mem::size_of_val(cached_values) as u64,
                bytemuck::cast_slice(operation_values),
            );
        }
        // The direct RGBA composite pair owns the whole image2 arena and
        // intentionally overwrites the otherwise-unused packed image payload.
        if !direct_bytes.is_empty() {
            self.queue.write_buffer(&arena.buffer, 0, direct_bytes);
        }
        Some(&arena.buffer)
    }
}
