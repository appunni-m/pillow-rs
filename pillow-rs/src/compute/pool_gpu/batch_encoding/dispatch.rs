use super::super::*;

impl GpuInner {
    fn make_bind_group(
        &self,
        cached: &CachedPipeline,
        input_buf: &wgpu::Buffer,
        output_buf: &wgpu::Buffer,
        resources: &GpuBatchResources,
        op_index: usize,
    ) -> Result<wgpu::BindGroup, PilError> {
        let params = ranged_binding(resources.params, Some(resources.params_ranges[op_index]))?;
        let second = auxiliary_binding(
            resources.img2,
            resources.fallback_img2,
            resources.img2_ranges[op_index],
        )?;
        let third = auxiliary_binding(
            resources.img3,
            resources.fallback_img3,
            resources.img3_ranges[op_index],
        )?;
        let lut = auxiliary_binding(
            resources.lut,
            resources.fallback_lut,
            resources.lut_ranges[op_index],
        )?;
        let mut entries = Vec::with_capacity(cached.num_bindings as usize);
        // Histogram control passes reuse the ordinary binding counts but
        // reinterpret the storage slots:
        //   clear:  [unused input, histogram rw, params]
        //   gather:  [image, optional mask, histogram rw, params]
        //   derive:  [histogram, generated LUT rw, params]
        // The final remap is a normal LUT shader, except that it must read the
        // generated LUT buffer rather than a host-uploaded LUT arena.
        if cached.variant_name == "__internal_histogram_clear" {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
        } else if matches!(
            cached.variant_name,
            "__internal_equalize_histogram" | "__internal_equalize_histogram_rgb_packed"
        ) {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: auxiliary_binding(
                    resources.img3,
                    resources.fallback_img3,
                    resources.img3_ranges[op_index],
                )?,
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: params,
            });
        } else if cached.variant_name == "__internal_equalize_histogram_l_packed" {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
        } else if cached.variant_name == "__internal_autocontrast_lut"
            || cached.variant_name == "__internal_equalize_lut"
        {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: resources.histogram.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: resources.fallback_lut.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
        } else if matches!(
            cached.variant_name,
            "__internal_histogram_remap" | "__internal_histogram_remap_rgb_packed"
        ) {
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: resources.fallback_lut.as_entire_binding(),
            });
        } else if matches!(
            cached.variant_name,
            "__internal_resize_h"
                | "__internal_resize_h_cover_native_packed"
                | "__internal_resize_h_pad_luma_packed"
                | "__internal_resize_v"
                | "__internal_resize_v_cover_native_packed"
                | "__internal_resize_v_pad_luma_packed"
                | "__internal_resize_v_pad_rgbx"
        ) {
            let (horizontal, vertical) = resources
                .resize_coeff_ranges
                .get(op_index)
                .and_then(Option::as_ref)
                .ok_or_else(|| {
                    PilError::InternalError(
                        "GPU resize pipeline is missing its coefficient ranges".into(),
                    )
                })?;
            let range = if matches!(
                cached.variant_name,
                "__internal_resize_h"
                    | "__internal_resize_h_cover_native_packed"
                    | "__internal_resize_h_pad_luma_packed"
            ) {
                *horizontal
            } else {
                *vertical
            };
            entries.push(wgpu::BindGroupEntry {
                binding: 0,
                resource: input_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 1,
                resource: output_buf.as_entire_binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: params,
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: auxiliary_binding(resources.img2, resources.fallback_img2, Some(range))?,
            });
        } else {
            match (cached.num_bindings, cached.is_lut, cached.is_output_only) {
                (2, _, true) => {
                    // Generator layout: [output(rw), params(uniform)].
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: params,
                    });
                }
                (5, _, _) => {
                    // 5-binding: [in_a(read), in_b(read), in_c(read), out(rw), params(uniform)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    // Storage read and read_write bindings may not alias within one
                    // compute dispatch. Keep absent optional inputs on their
                    // dedicated auxiliary buffers; shaders guard those reads with
                    // their corresponding presence parameter.
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: second,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: third,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 3,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 4,
                        resource: params,
                    });
                }
                (4, true, _) => {
                    // LUT layout: [input(read), output(rw), params(uniform), lut(read)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 3,
                        resource: lut,
                    });
                }
                (4, false, _) => {
                    // Dual-input layout: [input_a(read), input_b(read), output(rw), params(uniform)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: second,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: output_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 3,
                        resource: params,
                    });
                }
                (3, _, _) if cached.is_in_place => {
                    // AlphaComposite: [source(read), destination(read_write), params].
                    // The destination is the current ping-pong buffer, so the
                    // dispatch updates it in place and encode_batch keeps the
                    // current buffer selection unchanged.
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: second,
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params,
                    });
                }
                _ => {
                    // 2 or 3 binding: [input(read), output(rw), ...params(uniform)]
                    entries.push(wgpu::BindGroupEntry {
                        binding: 0,
                        resource: input_buf.as_entire_binding(),
                    });
                    entries.push(wgpu::BindGroupEntry {
                        binding: 1,
                        resource: output_buf.as_entire_binding(),
                    });
                    if cached.num_bindings > 2 {
                        entries.push(wgpu::BindGroupEntry {
                            binding: 2,
                            resource: params,
                        });
                    }
                }
            }
        }
        Ok(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(cached.variant_name),
            layout: &cached.bind_group_layout,
            entries: &entries,
        }))
    }

    pub(super) fn encode_dispatch(
        &self,
        cpass: &mut wgpu::ComputePass<'_>,
        cached: &CachedPipeline,
        index: usize,
        current_is_a: bool,
        resources: &GpuBatchResources,
        input_dims: (u32, u32),
        output_dims: (u32, u32),
    ) -> Result<bool, PilError> {
        let (input_buf, output_buf) = if current_is_a {
            (
                if index == 0 {
                    resources.initial_input.unwrap_or(resources.buf_a)
                } else {
                    resources.buf_a
                },
                resources.buf_b,
            )
        } else {
            (resources.buf_b, resources.buf_a)
        };
        let bind_group = self.make_bind_group(cached, input_buf, output_buf, resources, index)?;
        cpass.set_pipeline(&cached.pipeline);
        cpass.set_bind_group(0, &bind_group, &[]);
        let (dispatch_w, dispatch_h) = match cached.variant_name {
            "__internal_point_luma_packed" | "__internal_equalize_remap_l_packed" => {
                plan_packed_point_luma_dispatch(
                    input_dims.0,
                    input_dims.1,
                    self.device.limits().max_compute_workgroups_per_dimension,
                )?
            }
            "__internal_put_data_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_median_filter_3x3_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_max_filter_3x3_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_min_filter_3x3_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_rank_filter_3x3_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_filter_5x5_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_filter_5x5_la_packed" => plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_filter_3x3_la_binomial_packed" => plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_median_filter_3x3_la_packed" => plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_rank_filter_3x3_la_packed" => plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_max_filter_3x3_la_packed" => plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_min_filter_3x3_la_packed" => plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_rank_filter_9x9_luma_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_rank_filter_9x9_luma_row_packed" => plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_blur_h_luma_packed"
            | "__internal_blur_v_luma_packed"
            | "__internal_blur_hv_luma_radius_one_packed" => plan_packed_luma_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_blur_h_la_packed" | "__internal_blur_v_la_packed" => {
                plan_packed_la_dispatch(
                    output_dims.0,
                    output_dims.1,
                    self.device.limits().max_compute_workgroups_per_dimension,
                )?
            }
            "__internal_resize_h_pad_luma_packed" => plan_packed_luma_resize_dispatch(
                output_dims.0,
                input_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_resize_v_pad_luma_packed" => plan_packed_luma_resize_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_pad_luma_packed" => plan_packed_luma_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_pad_la_packed" => plan_packed_la_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "ExtractBand" | "Constant" => plan_extract_band_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "Grayscale" => plan_packed_output_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
                GRAYSCALE_WORKGROUP_SIZE,
                "GPU grayscale",
            )?,
            "__internal_sharpness_l_native" => {
                let plan = plan_sharpness_l_dispatch(
                    output_dims.0,
                    output_dims.1,
                    self.device.limits().max_compute_workgroups_per_dimension,
                )?;
                (plan.groups_x, plan.groups_y)
            }
            "Expand" if resources.native_expand_output.is_some() => {
                let plan = resources
                    .native_expand_output
                    .expect("guarded native Expand dispatch");
                (plan.groups_x, plan.groups_y)
            }
            "__internal_transform_l_nearest_packed" => {
                let plan = resources.native_luma_transform_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-L Transform shader has no checked output plan".into(),
                    )
                })?;
                (plan.groups_x, plan.groups_y)
            }
            "__internal_transform_rgb_nearest_packed" => {
                let plan = resources.native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB Transform shader has no checked output plan".into(),
                    )
                })?;
                (plan.groups_x, plan.groups_y)
            }
            "__internal_point_rgb_packed"
            | "__internal_histogram_remap_rgb_packed"
            | "__internal_median_filter_3x3_rgb_packed"
            | "__internal_min_filter_3x3_rgb_packed"
            | "__internal_max_filter_3x3_rgb_packed"
            | "__internal_blur_h_rgb_packed"
            | "__internal_blur_v_rgb_packed" => {
                let plan = resources.native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB point shader has no checked output plan".into(),
                    )
                })?;
                (plan.groups_x, plan.groups_y)
            }
            "__internal_transform_la_nearest_packed" => {
                let plan = resources.native_la_transform_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-LA Transform shader has no checked output plan".into(),
                    )
                })?;
                (plan.groups_x, plan.groups_y)
            }
            "__internal_blur_h" | "__internal_blur_v" => plan_blur_dispatch(
                output_dims.0,
                output_dims.1,
                self.device.limits().max_compute_workgroups_per_dimension,
            )?,
            "__internal_resize_h" => (output_dims.0.div_ceil(16), input_dims.1.div_ceil(16)),
            "__internal_resize_h_cover_native_packed" => {
                let plan = resources.native_cover_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed native-L Cover intermediate has no dispatch plan".into(),
                    )
                })?;
                (plan.horizontal_groups_x, plan.horizontal_groups_y)
            }
            "__internal_resize_v" => (output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)),
            "__internal_resize_v_cover_native_packed" => {
                let plan = resources.native_cover_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed native-L Cover output has no dispatch plan".into(),
                    )
                })?;
                (plan.groups_x, plan.groups_y)
            }
            "__internal_resize_v_pad_rgbx" => {
                (output_dims.0.div_ceil(16), output_dims.1.div_ceil(16))
            }
            "__internal_equalize_histogram"
            | "__internal_equalize_histogram_rgb_packed"
            | "__internal_equalize_histogram_l_packed" => {
                // About 16 packed pixels per lane amortizes each group's
                // shared histogram. Cap groups to bound global bin merges;
                // the shader grid-stride loop handles the remaining pixels.
                let pixels = u64::from(input_dims.0) * u64::from(input_dims.1);
                (pixels.div_ceil(4096).clamp(1, 256) as u32, 1)
            }
            "__internal_histogram_clear"
            | "__internal_autocontrast_lut"
            | "__internal_equalize_lut" => (1, 1),
            _ => (output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)),
        };
        crate::compute::record_gpu_shader_dispatch(
            cached.variant_name,
            cached.shader_file,
            u64::from(dispatch_w).saturating_mul(u64::from(dispatch_h)),
        );
        cpass.dispatch_workgroups(dispatch_w, dispatch_h, 1);
        let keeps_image_buffer = matches!(
            cached.variant_name,
            "__internal_histogram_clear"
                | "__internal_equalize_histogram"
                | "__internal_equalize_histogram_rgb_packed"
                | "__internal_equalize_histogram_l_packed"
                | "__internal_autocontrast_lut"
                | "__internal_equalize_lut"
        );
        Ok(if cached.is_in_place || keeps_image_buffer {
            current_is_a
        } else {
            !current_is_a
        })
    }
}
