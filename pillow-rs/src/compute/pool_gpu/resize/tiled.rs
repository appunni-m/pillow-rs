//! Tiled GPU execution for ordered binary64 FLOAT resizing.

use super::super::*;

impl GpuInner {
    /// Stream coefficient tiles while the ordered binary64 state stays on the
    /// device. Resample.c stores FLOAT32 only after the complete axis, never at
    /// a transport boundary; marker 15 performs that one final conversion.
    pub(in crate::compute::pool_gpu) fn execute_f_resize_tiled(
        &self,
        source: (u32, u32),
        destination: (u32, u32),
        horizontal: &FilterCoeffsF64,
        vertical: &FilterCoeffsF64,
        buffers: &BufferPool,
    ) -> Result<
        (
            bool,
            u32,
            u32,
            ReadbackTarget,
            PipelineResourceTelemetry,
            u64,
        ),
        PilError,
    > {
        let cached = self.resolve_pipeline(
            "__internal_resize_h",
            "resize_convolution_h.wgsl",
            include_str!("../shaders/resize_convolution_h.wgsl"),
        )?;
        let storage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;
        let states = create_sized_buffer(
            &self.device,
            "gpu_f_resize_tile_states",
            storage,
            GPU_F_RESIZE_TILE_PIXELS * 6 * 4,
            GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES,
        );
        let coefficients = create_sized_buffer(
            &self.device,
            "gpu_f_resize_tile_coefficients",
            storage,
            (6 + GPU_F_RESIZE_TILE_ROWS * (4 + GPU_F_RESIZE_TILE_TAPS * 4)) * 4,
            GPU_F_RESIZE_COEFFICIENT_ALIGNMENT_BYTES,
        );
        let parameters = create_sized_buffer(
            &self.device,
            "gpu_f_resize_tile_parameters",
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            8 * 4,
            256,
        );
        let mut current_is_a = true;
        let mut dimensions = source;
        let mut telemetry = PipelineResourceTelemetry::default();
        let mut dispatches = 0u64;
        for (axis, table, target) in [
            (0u32, horizontal, destination.0),
            (1, vertical, destination.1),
        ] {
            if target
                == if axis == 0 {
                    dimensions.0
                } else {
                    dimensions.1
                }
            {
                continue;
            }
            let output_dims = if axis == 0 {
                (target, dimensions.1)
            } else {
                (dimensions.0, target)
            };
            let line_count = if axis == 0 {
                dimensions.1
            } else {
                dimensions.0
            } as usize;
            let (source_buffer, destination_buffer) = if current_is_a {
                (&buffers.buf_a, &buffers.buf_b)
            } else {
                (&buffers.buf_b, &buffers.buf_a)
            };
            for row_start in (0..table.count.len()).step_by(GPU_F_RESIZE_TILE_ROWS) {
                let rows = GPU_F_RESIZE_TILE_ROWS.min(table.count.len() - row_start);
                let lines_per_tile = GPU_F_RESIZE_TILE_PIXELS / rows;
                let max_taps = table.count[row_start..row_start + rows]
                    .iter()
                    .copied()
                    .max()
                    .unwrap_or(0);
                for line_start in (0..line_count).step_by(lines_per_tile) {
                    let lines = lines_per_tile.min(line_count - line_start);
                    let header = [
                        axis,
                        row_start as u32,
                        line_start as u32,
                        rows as u32,
                        lines as u32,
                        0,
                    ];
                    for tap_start in (0..max_taps.max(1)).step_by(GPU_F_RESIZE_TILE_TAPS) {
                        let mut words = header.to_vec();
                        words[5] = tap_start as u32;
                        words.resize(6 + rows * 4, 0);
                        for row in 0..rows {
                            let index = row_start + row;
                            let total = table.count[index];
                            let end = total.min(tap_start.saturating_add(GPU_F_RESIZE_TILE_TAPS));
                            let start = total.min(tap_start);
                            let offset = words.len() as u32;
                            words[6 + row * 4..6 + (row + 1) * 4].copy_from_slice(&[
                                table.xmin[index] as u32,
                                total as u32,
                                (end - start) as u32,
                                offset,
                            ]);
                            for &weight in &table.weights[index][start..end] {
                                let parts = gpu_f64_integer_parts(weight).ok_or_else(|| {
                                    PilError::InternalError(
                                        "GPU tiled F coefficient is invalid".into(),
                                    )
                                })?;
                                words.extend([
                                    parts.mantissa as u32,
                                    (parts.mantissa >> 32) as u32,
                                    parts.exponent as u32,
                                    u32::from(parts.negative),
                                ]);
                            }
                        }
                        self.queue
                            .write_buffer(&coefficients, 0, bytemuck::cast_slice(&words));
                        telemetry.auxiliary_bytes += (words.len() * 4) as u64;
                        let params = [
                            dimensions.0,
                            dimensions.1,
                            8,
                            0,
                            output_dims.0,
                            output_dims.1,
                            1,
                            14,
                        ];
                        self.queue
                            .write_buffer(&parameters, 0, bytemuck::cast_slice(&params));
                        telemetry.parameter_bytes += 32;
                        self.submit_f_resize_tile(
                            &cached,
                            source_buffer,
                            &states,
                            &parameters,
                            &coefficients,
                            rows as u32,
                            lines as u32,
                        )?;
                        dispatches += 1;
                        if dispatches.is_multiple_of(8) {
                            self.device.poll(wgpu::Maintain::Wait);
                            self.ensure_healthy("GPU tiled F resize bounded queue")?;
                        }
                    }
                    let params = [
                        dimensions.0,
                        dimensions.1,
                        8,
                        0,
                        output_dims.0,
                        output_dims.1,
                        1,
                        15,
                    ];
                    self.queue
                        .write_buffer(&parameters, 0, bytemuck::cast_slice(&params));
                    telemetry.parameter_bytes += 32;
                    self.submit_f_resize_tile(
                        &cached,
                        &states,
                        destination_buffer,
                        &parameters,
                        &coefficients,
                        rows as u32,
                        lines as u32,
                    )?;
                    dispatches += 1;
                }
            }
            current_is_a = !current_is_a;
            dimensions = output_dims;
        }
        let size = CheckedDims::new(destination.0, destination.1, 4)?.total_bytes() as u64;
        let result_buffer = if current_is_a {
            &buffers.buf_a
        } else {
            &buffers.buf_b
        };
        let readback = self.prepare_readback(result_buffer, size)?;
        if let ReadbackTarget::Staging(staging) = &readback {
            let mut encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("gpu_f_resize_tile_readback"),
                });
            encoder.copy_buffer_to_buffer(result_buffer, 0, &staging.buffer, 0, size);
            self.queue.submit(Some(encoder.finish()));
            self.poll_device("GPU tiled F resize readback")?;
        } else if dispatches == 0 {
            // An unchanged shape may leave only a pending queue upload. Flush
            // it before mapping even when there was no compute submission.
            self.queue.submit(std::iter::empty());
            self.poll_device("GPU tiled F resize identity upload")?;
        }
        Ok((
            current_is_a,
            destination.0,
            destination.1,
            readback,
            telemetry,
            dispatches,
        ))
    }

    fn submit_f_resize_tile(
        &self,
        cached: &CachedPipeline,
        input: &wgpu::Buffer,
        output: &wgpu::Buffer,
        parameters: &wgpu::Buffer,
        coefficients: &wgpu::Buffer,
        rows: u32,
        lines: u32,
    ) -> Result<(), PilError> {
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu_f_resize_tile"),
            layout: &cached.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: output.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: parameters.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: coefficients.as_entire_binding(),
                },
            ],
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gpu_f_resize_tile"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu_f_resize_tile"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&cached.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(rows.div_ceil(16), lines.div_ceil(16), 1);
        }
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(rows.div_ceil(16)) * u64::from(lines.div_ceil(16)),
        );
        // Queue order protects both the state and the reused coefficient arena.
        // Poll between submissions so device errors surface at the owning tile.
        self.queue.submit(Some(encoder.finish()));
        self.poll_device("GPU tiled F resize submission")
    }
}
