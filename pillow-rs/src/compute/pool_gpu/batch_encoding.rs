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
        let mut current_is_a = start_is_a;
        for (index, pipeline) in resolved.iter().enumerate() {
            let resize_input_dims = prepared.resize_source_rows[index]
                .map_or(prepared.input_dims[index], |(_, rows)| {
                    (prepared.input_dims[index].0, rows)
                });
            if matches!(pipeline, ResolvedPipeline::Skip) {
                continue;
            }
            // Metal requires an explicit compute-pass boundary between the
            // horizontal and vertical typed resize passes.  Both passes
            // ping-pong the same two storage buffers; keeping them in one
            // pass leaves the second dispatch racing the first on some
            // drivers, producing a stale row while the host coefficients
            // themselves remain exact. Keep the boundary for F/I typed paths
            // and tall nearest byte-mode Resize/ResizeBoxed; the ordinary byte
            // path retains its grouped pass for the existing throughput
            // contract.
            if matches!(pipeline, ResolvedPipeline::Resize { .. })
                && (logical_mode == Some("F")
                    && matches!(
                        ops.get(index),
                        Some(
                            PipelineOp::Resize { .. }
                                | PipelineOp::ResizeBoxed { .. }
                                | PipelineOp::Fit {
                                    filter: ResampleFilter::Nearest,
                                    ..
                                }
                        )
                    )
                    || logical_mode == Some("I")
                        && matches!(
                            ops.get(index),
                            Some(PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. })
                        )
                    || matches!(logical_mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
                        && matches!(
                            ops.get(index),
                            Some(PipelineOp::Resize { .. } | PipelineOp::ResizeBoxed { .. })
                        )
                    || matches!(logical_mode, Some("L" | "LA" | "RGB" | "RGBA"))
                        && prepared.input_dims[index].1
                            > prepared.input_dims[index].0.saturating_mul(100)
                        && matches!(
                            ops.get(index),
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
            {
                let ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                } = pipeline
                else {
                    unreachable!("typed resize pipeline shape changed")
                };
                let compact_vertical = ops.len() == 1
                    && gpu_f_resize_compact_box_vertical_only_geometry(
                        &ops[index],
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                        logical_mode,
                    );
                if !compact_vertical {
                    let mut resize_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("gpu_batch_compute_resize_h"),
                            timestamp_writes: None,
                        });
                    current_is_a = self.encode_dispatch(
                        &mut resize_pass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        resize_input_dims,
                        prepared.output_dims[index],
                    )?;
                }
                {
                    let mut resize_pass =
                        encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                            label: Some("gpu_batch_compute_resize_v"),
                            timestamp_writes: None,
                        });
                    current_is_a = self.encode_dispatch(
                        &mut resize_pass,
                        vertical,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
                continue;
            }
            // Typed F/I Pad has the same intermediate dependency as a
            // Resize, but also appends a placement dispatch. Keep each stage
            // in its own compute pass so Metal cannot expose a stale
            // horizontal/vertical intermediate to the next stage.
            if matches!(logical_mode, Some("F" | "I"))
                && matches!(pipeline, ResolvedPipeline::Pad { .. })
                && matches!(
                    ops.get(index),
                    Some(PipelineOp::Pad {
                        filter,
                        ..
                    }) if (logical_mode == Some("I")
                        && matches!(filter, ResampleFilter::Nearest))
                        || (logical_mode == Some("F")
                            && !matches!(filter, ResampleFilter::Nearest))
                )
            {
                let ResolvedPipeline::Pad {
                    horizontal,
                    vertical,
                    place,
                    ..
                } = pipeline
                else {
                    unreachable!("typed I Pad pipeline shape changed")
                };
                let resize_dims = prepared.pad_resize_dims[index].ok_or_else(|| {
                    PilError::InternalError("GPU I Pad is missing its contain dimensions".into())
                })?;
                if resize_dims != prepared.input_dims[index] {
                    {
                        let mut resize_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("gpu_batch_compute_typed_pad_resize_h"),
                                timestamp_writes: None,
                            });
                        current_is_a = self.encode_dispatch(
                            &mut resize_pass,
                            horizontal,
                            index,
                            current_is_a,
                            &prepared.resources,
                            prepared.input_dims[index],
                            resize_dims,
                        )?;
                    }
                    {
                        let mut resize_pass =
                            encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                                label: Some("gpu_batch_compute_typed_pad_resize_v"),
                                timestamp_writes: None,
                            });
                        current_is_a = self.encode_dispatch(
                            &mut resize_pass,
                            vertical,
                            index,
                            current_is_a,
                            &prepared.resources,
                            prepared.input_dims[index],
                            resize_dims,
                        )?;
                    }
                }
                if resize_dims != prepared.output_dims[index] {
                    let mut place_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                        label: Some("gpu_batch_compute_typed_pad_place"),
                        timestamp_writes: None,
                    });
                    current_is_a = self.encode_dispatch(
                        &mut place_pass,
                        place,
                        index,
                        current_is_a,
                        &prepared.resources,
                        resize_dims,
                        prepared.output_dims[index],
                    )?;
                }
                continue;
            }
            if matches!(pipeline, ResolvedPipeline::Histogram { .. })
                && matches!(
                    ops.get(index),
                    Some(PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. })
                )
            {
                // Clear the reusable histogram with the command encoder's
                // native zero-fill operation instead of spending a compute
                // dispatch on 1024 atomic stores.
                encoder.clear_buffer(prepared.resources.histogram, 0, None);
            }
            let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_batch_compute"),
                timestamp_writes: None,
            });
            if let ResolvedPipeline::Blur {
                horizontal,
                vertical,
                pass_count,
            } = pipeline
            {
                // Pillow's GaussianBlur is three horizontal box passes
                // followed by three vertical box passes. BoxBlur is one of
                // each. Keep all passes in this compute pass and ping-pong
                // between the two image buffers; no intermediate readback or
                // CPU serialization is needed.
                for _ in 0..*pass_count {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
                for _ in 0..*pass_count {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        vertical,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
            } else if let ResolvedPipeline::Resize {
                horizontal,
                vertical,
            } = pipeline
            {
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    horizontal,
                    index,
                    current_is_a,
                    &prepared.resources,
                    resize_input_dims,
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    vertical,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
            } else if let ResolvedPipeline::Pad {
                horizontal,
                vertical,
                place,
                vertical_pad_rgbx,
            } = pipeline
            {
                let resize_dims = prepared.pad_resize_dims[index].ok_or_else(|| {
                    PilError::InternalError("GPU Pad is missing its contain dimensions".into())
                })?;
                if resize_dims != prepared.input_dims[index] {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        horizontal,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        resize_dims,
                    )?;
                    if let Some(vertical_pad_rgbx) = vertical_pad_rgbx {
                        current_is_a = self.encode_dispatch(
                            &mut cpass,
                            vertical_pad_rgbx,
                            index,
                            current_is_a,
                            &prepared.resources,
                            prepared.input_dims[index],
                            prepared.output_dims[index],
                        )?;
                        continue;
                    } else {
                        current_is_a = self.encode_dispatch(
                            &mut cpass,
                            vertical,
                            index,
                            current_is_a,
                            &prepared.resources,
                            prepared.input_dims[index],
                            resize_dims,
                        )?;
                    }
                }
                if vertical_pad_rgbx.is_none() && resize_dims != prepared.output_dims[index] {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        place,
                        index,
                        current_is_a,
                        &prepared.resources,
                        resize_dims,
                        prepared.output_dims[index],
                    )?;
                }
            } else if let ResolvedPipeline::Histogram {
                clear,
                histogram,
                derive,
                remap,
            } = pipeline
            {
                if matches!(ops.get(index), Some(PipelineOp::Autocontrast { .. })) {
                    current_is_a = self.encode_dispatch(
                        &mut cpass,
                        clear,
                        index,
                        current_is_a,
                        &prepared.resources,
                        prepared.input_dims[index],
                        prepared.output_dims[index],
                    )?;
                }
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    histogram,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    derive,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    remap,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
            } else {
                let ResolvedPipeline::Single(cached) = pipeline else {
                    unreachable!("resolved GPU pipeline variant changed during encoding")
                };
                current_is_a = self.encode_dispatch(
                    &mut cpass,
                    cached,
                    index,
                    current_is_a,
                    &prepared.resources,
                    prepared.input_dims[index],
                    prepared.output_dims[index],
                )?;
            }
            drop(cpass);
        }
        Ok(current_is_a)
    }
}
