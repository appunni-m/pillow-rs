use super::*;

mod dispatch;

impl GpuInner {
    pub(super) fn encode_batch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        ops: &[PipelineOp],
        prepared: &PreparedGpuBatch,
        start_is_a: bool,
        logical_mode: Option<&str>,
        packed_luma_point: bool,
        packed_luma_putdata: bool,
        packed_luma_order_statistic: bool,
        packed_native_byte_filter: bool,
        packed_luma_pad: bool,
        packed_la_pad: bool,
        packed_luma_transform_output: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
    ) -> Result<bool, PilError> {
        let resolved = self.resolve_batch_pipelines(
            ops,
            logical_mode,
            &prepared.input_dims,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
            prepared.resources.native_cover_packed_output,
            packed_luma_transform_output,
            prepared.resources.native_rgb_packed_output.is_some(),
            prepared.resources.native_la_transform_output.is_some(),
        )?;
        BatchEncoding {
            gpu: self,
            encoder,
            ops,
            prepared,
            logical_mode,
        }
        .encode(&resolved, start_is_a)
    }
}

struct BatchEncoding<'a> {
    gpu: &'a GpuInner,
    encoder: &'a mut wgpu::CommandEncoder,
    ops: &'a [PipelineOp],
    prepared: &'a PreparedGpuBatch<'a>,
    logical_mode: Option<&'a str>,
}

impl BatchEncoding<'_> {
    fn encode(
        mut self,
        resolved: &[ResolvedPipeline],
        mut current_is_a: bool,
    ) -> Result<bool, PilError> {
        for (index, pipeline) in resolved.iter().enumerate() {
            let resize_input_dims = self.prepared.resize_source_rows[index]
                .map_or(self.prepared.input_dims[index], |(_, rows)| {
                    (self.prepared.input_dims[index].0, rows)
                });
            if matches!(pipeline, ResolvedPipeline::Skip) {
                continue;
            }
            if self.requires_separate_resize_pass(index, pipeline) {
                let ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                } = pipeline
                else {
                    unreachable!("typed resize pipeline shape changed")
                };
                current_is_a = self.encode_split_resize(
                    index,
                    horizontal,
                    vertical,
                    resize_input_dims,
                    current_is_a,
                )?;
                continue;
            }
            if self.requires_separate_typed_pad_pass(index, pipeline) {
                let ResolvedPipeline::Pad {
                    horizontal,
                    vertical,
                    place,
                    ..
                } = pipeline
                else {
                    unreachable!("typed I Pad pipeline shape changed")
                };
                let resize_dims = self.prepared.pad_resize_dims[index].ok_or_else(|| {
                    PilError::InternalError("GPU I Pad is missing its contain dimensions".into())
                })?;
                current_is_a = self.encode_split_typed_pad(
                    index,
                    horizontal,
                    vertical,
                    place,
                    resize_dims,
                    current_is_a,
                )?;
                continue;
            }
            current_is_a = encode_standard_pipeline(
                self.gpu,
                self.encoder,
                self.ops,
                self.prepared,
                index,
                pipeline,
                resize_input_dims,
                current_is_a,
            )?;
        }
        Ok(current_is_a)
    }

    fn requires_separate_resize_pass(&self, index: usize, pipeline: &ResolvedPipeline) -> bool {
        matches!(pipeline, ResolvedPipeline::Resize { .. })
            && (self.logical_mode == Some("F")
                && matches!(
                    self.ops.get(index),
                    Some(
                        PipelineOp::Resize { .. }
                            | PipelineOp::ResizeBoxed { .. }
                            | PipelineOp::Fit {
                                filter: ResampleFilter::Nearest,
                                ..
                            }
                    )
                )
                || self.logical_mode == Some("I")
                    && matches!(
                        self.ops.get(index),
                        Some(PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. })
                    )
                || matches!(
                    self.logical_mode,
                    Some("I;16" | "I;16L" | "I;16B" | "I;16N")
                ) && matches!(
                    self.ops.get(index),
                    Some(PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. })
                )
                || matches!(self.logical_mode, Some("L" | "LA" | "RGB" | "RGBA"))
                    && self.prepared.input_dims[index].1
                        > self.prepared.input_dims[index].0.saturating_mul(100)
                    && matches!(
                        self.ops.get(index),
                        Some(
                            PipelineOp::Resize {
                                filter: ResampleFilter::Nearest,
                                ..
                            } | PipelineOp::ResizeBoxed {
                                filter: ResampleFilter::Nearest,
                                ..
                            }
                        )
                    ))
    }

    fn requires_separate_typed_pad_pass(&self, index: usize, pipeline: &ResolvedPipeline) -> bool {
        matches!(self.logical_mode, Some("F" | "I"))
            && matches!(pipeline, ResolvedPipeline::Pad { .. })
            && matches!(
                self.ops.get(index),
                Some(PipelineOp::Pad { filter, .. })
                    if (self.logical_mode == Some("I")
                        && matches!(filter, ResampleFilter::Nearest))
                    || (self.logical_mode == Some("F")
                        && !matches!(filter, ResampleFilter::Nearest))
            )
    }

    fn encode_split_resize(
        &mut self,
        index: usize,
        horizontal: &CachedPipeline,
        vertical: &CachedPipeline,
        resize_input_dims: (u32, u32),
        mut current_is_a: bool,
    ) -> Result<bool, PilError> {
        // Keep separate passes for typed paths and tall nearest byte resizes.
        // Metal can otherwise race the horizontal and vertical storage-buffer
        // writes and expose a stale row to the second pass.
        let compact_vertical = self.ops.len() == 1
            && gpu_f_resize_compact_box_vertical_only_geometry(
                &self.ops[index],
                self.prepared.input_dims[index],
                self.prepared.output_dims[index],
                self.logical_mode,
            );
        if !compact_vertical {
            let mut pass = self
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("gpu_batch_compute_resize_h"),
                    timestamp_writes: None,
                });
            current_is_a = self.gpu.encode_dispatch(
                &mut pass,
                horizontal,
                index,
                current_is_a,
                &self.prepared.resources,
                resize_input_dims,
                self.prepared.output_dims[index],
            )?;
        }
        let mut pass = self
            .encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_batch_compute_resize_v"),
                timestamp_writes: None,
            });
        self.gpu.encode_dispatch(
            &mut pass,
            vertical,
            index,
            current_is_a,
            &self.prepared.resources,
            self.prepared.input_dims[index],
            self.prepared.output_dims[index],
        )
    }

    fn encode_split_typed_pad(
        &mut self,
        index: usize,
        horizontal: &CachedPipeline,
        vertical: &CachedPipeline,
        place: &CachedPipeline,
        resize_dims: (u32, u32),
        mut current_is_a: bool,
    ) -> Result<bool, PilError> {
        if resize_dims != self.prepared.input_dims[index] {
            {
                let mut pass = self
                    .encoder
                    .begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("gpu_batch_compute_typed_pad_resize_h"),
                        timestamp_writes: None,
                    });
                current_is_a = self.gpu.encode_dispatch(
                    &mut pass,
                    horizontal,
                    index,
                    current_is_a,
                    &self.prepared.resources,
                    self.prepared.input_dims[index],
                    resize_dims,
                )?;
            }
            {
                let mut pass = self
                    .encoder
                    .begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("gpu_batch_compute_typed_pad_resize_v"),
                        timestamp_writes: None,
                    });
                current_is_a = self.gpu.encode_dispatch(
                    &mut pass,
                    vertical,
                    index,
                    current_is_a,
                    &self.prepared.resources,
                    self.prepared.input_dims[index],
                    resize_dims,
                )?;
            }
        }
        if resize_dims != self.prepared.output_dims[index] {
            let mut pass = self
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("gpu_batch_compute_typed_pad_place"),
                    timestamp_writes: None,
                });
            current_is_a = self.gpu.encode_dispatch(
                &mut pass,
                place,
                index,
                current_is_a,
                &self.prepared.resources,
                resize_dims,
                self.prepared.output_dims[index],
            )?;
        }
        Ok(current_is_a)
    }
}

fn encode_standard_pipeline(
    gpu: &GpuInner,
    encoder: &mut wgpu::CommandEncoder,
    ops: &[PipelineOp],
    prepared: &PreparedGpuBatch,
    index: usize,
    pipeline: &ResolvedPipeline,
    resize_input_dims: (u32, u32),
    current_is_a: bool,
) -> Result<bool, PilError> {
    if matches!(pipeline, ResolvedPipeline::Histogram { .. })
        && matches!(
            ops.get(index),
            Some(PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. })
        )
    {
        // Clear the reusable histogram with the encoder's zero-fill instead
        // of spending a compute dispatch on 1024 atomic stores.
        encoder.clear_buffer(prepared.resources.histogram, 0, None);
    }
    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("gpu_batch_compute"),
        timestamp_writes: None,
    });
    let result = match pipeline {
        ResolvedPipeline::Blur {
            horizontal,
            vertical,
            pass_count,
        } => encode_blur_pipeline(
            gpu,
            &mut pass,
            prepared,
            index,
            horizontal,
            vertical,
            *pass_count,
            current_is_a,
        ),
        ResolvedPipeline::Resize {
            horizontal,
            vertical,
        } => encode_resize_pipeline(
            gpu,
            &mut pass,
            prepared,
            index,
            horizontal,
            vertical,
            resize_input_dims,
            current_is_a,
        ),
        ResolvedPipeline::Pad {
            horizontal,
            vertical,
            place,
            vertical_pad_rgbx,
        } => encode_pad_pipeline(
            gpu,
            &mut pass,
            prepared,
            index,
            horizontal,
            vertical,
            place,
            vertical_pad_rgbx.as_ref(),
            current_is_a,
        ),
        ResolvedPipeline::Histogram {
            clear,
            histogram,
            derive,
            remap,
        } => encode_histogram_pipeline(
            gpu,
            &mut pass,
            ops,
            prepared,
            index,
            clear,
            histogram,
            derive,
            remap,
            current_is_a,
        ),
        ResolvedPipeline::Single(cached) => gpu.encode_dispatch(
            &mut pass,
            cached,
            index,
            current_is_a,
            &prepared.resources,
            prepared.input_dims[index],
            prepared.output_dims[index],
        ),
        ResolvedPipeline::Skip => unreachable!("skip pipelines are handled before encoding"),
    }?;
    drop(pass);
    Ok(result)
}

fn encode_blur_pipeline(
    gpu: &GpuInner,
    pass: &mut wgpu::ComputePass<'_>,
    prepared: &PreparedGpuBatch,
    index: usize,
    horizontal: &CachedPipeline,
    vertical: &CachedPipeline,
    pass_count: usize,
    mut current_is_a: bool,
) -> Result<bool, PilError> {
    // Pillow GaussianBlur is three horizontal box passes followed by three
    // vertical passes; BoxBlur uses one of each. Keep intermediates on GPU.
    for _ in 0..pass_count {
        current_is_a = gpu.encode_dispatch(
            pass,
            horizontal,
            index,
            current_is_a,
            &prepared.resources,
            prepared.input_dims[index],
            prepared.output_dims[index],
        )?;
    }
    for _ in 0..pass_count {
        current_is_a = gpu.encode_dispatch(
            pass,
            vertical,
            index,
            current_is_a,
            &prepared.resources,
            prepared.input_dims[index],
            prepared.output_dims[index],
        )?;
    }
    Ok(current_is_a)
}

fn encode_resize_pipeline(
    gpu: &GpuInner,
    pass: &mut wgpu::ComputePass<'_>,
    prepared: &PreparedGpuBatch,
    index: usize,
    horizontal: &CachedPipeline,
    vertical: &CachedPipeline,
    resize_input_dims: (u32, u32),
    current_is_a: bool,
) -> Result<bool, PilError> {
    let current_is_a = gpu.encode_dispatch(
        pass,
        horizontal,
        index,
        current_is_a,
        &prepared.resources,
        resize_input_dims,
        prepared.output_dims[index],
    )?;
    gpu.encode_dispatch(
        pass,
        vertical,
        index,
        current_is_a,
        &prepared.resources,
        prepared.input_dims[index],
        prepared.output_dims[index],
    )
}

fn encode_pad_pipeline(
    gpu: &GpuInner,
    pass: &mut wgpu::ComputePass<'_>,
    prepared: &PreparedGpuBatch,
    index: usize,
    horizontal: &CachedPipeline,
    vertical: &CachedPipeline,
    place: &CachedPipeline,
    vertical_pad_rgbx: Option<&Arc<CachedPipeline>>,
    mut current_is_a: bool,
) -> Result<bool, PilError> {
    let resize_dims = prepared.pad_resize_dims[index].ok_or_else(|| {
        PilError::InternalError("GPU Pad is missing its contain dimensions".into())
    })?;
    if resize_dims != prepared.input_dims[index] {
        current_is_a = gpu.encode_dispatch(
            pass,
            horizontal,
            index,
            current_is_a,
            &prepared.resources,
            prepared.input_dims[index],
            resize_dims,
        )?;
        if let Some(vertical_pad_rgbx) = vertical_pad_rgbx {
            return gpu.encode_dispatch(
                pass,
                vertical_pad_rgbx,
                index,
                current_is_a,
                &prepared.resources,
                prepared.input_dims[index],
                prepared.output_dims[index],
            );
        }
        current_is_a = gpu.encode_dispatch(
            pass,
            vertical,
            index,
            current_is_a,
            &prepared.resources,
            prepared.input_dims[index],
            resize_dims,
        )?;
    }
    if vertical_pad_rgbx.is_none() && resize_dims != prepared.output_dims[index] {
        current_is_a = gpu.encode_dispatch(
            pass,
            place,
            index,
            current_is_a,
            &prepared.resources,
            resize_dims,
            prepared.output_dims[index],
        )?;
    }
    Ok(current_is_a)
}

fn encode_histogram_pipeline(
    gpu: &GpuInner,
    pass: &mut wgpu::ComputePass<'_>,
    ops: &[PipelineOp],
    prepared: &PreparedGpuBatch,
    index: usize,
    clear: &CachedPipeline,
    histogram: &CachedPipeline,
    derive: &CachedPipeline,
    remap: &CachedPipeline,
    mut current_is_a: bool,
) -> Result<bool, PilError> {
    if matches!(ops.get(index), Some(PipelineOp::Autocontrast { .. })) {
        current_is_a = gpu.encode_dispatch(
            pass,
            clear,
            index,
            current_is_a,
            &prepared.resources,
            prepared.input_dims[index],
            prepared.output_dims[index],
        )?;
    }
    current_is_a = gpu.encode_dispatch(
        pass,
        histogram,
        index,
        current_is_a,
        &prepared.resources,
        prepared.input_dims[index],
        prepared.output_dims[index],
    )?;
    current_is_a = gpu.encode_dispatch(
        pass,
        derive,
        index,
        current_is_a,
        &prepared.resources,
        prepared.input_dims[index],
        prepared.output_dims[index],
    )?;
    current_is_a = gpu.encode_dispatch(
        pass,
        remap,
        index,
        current_is_a,
        &prepared.resources,
        prepared.input_dims[index],
        prepared.output_dims[index],
    )?;
    Ok(current_is_a)
}
