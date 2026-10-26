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
        let bindings = BindGroupBuilder {
            cached,
            input_buf,
            output_buf,
            resources,
            op_index,
            params,
            second,
            third,
            lut,
        };
        let special = bindings.append_histogram_bindings(&mut entries)?
            || bindings.append_resize_bindings(&mut entries)?;
        if !special {
            bindings.append_standard_bindings(&mut entries);
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
        let (dispatch_w, dispatch_h) =
            self.plan_batch_dispatch(cached, resources, input_dims, output_dims)?;
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

    fn plan_batch_dispatch(
        &self,
        cached: &CachedPipeline,
        resources: &GpuBatchResources,
        input_dims: (u32, u32),
        output_dims: (u32, u32),
    ) -> Result<(u32, u32), PilError> {
        if let Some(plan) = self.plan_packed_dispatch(cached, input_dims, output_dims)? {
            return Ok(plan);
        }
        if let Some(plan) = self.plan_native_dispatch(cached, resources, output_dims)? {
            return Ok(plan);
        }
        if let Some(plan) = self.plan_resize_dispatch(cached, resources, input_dims, output_dims)? {
            return Ok(plan);
        }
        if let Some(plan) = Self::plan_histogram_dispatch(cached.variant_name, input_dims) {
            return Ok(plan);
        }
        Ok((output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)))
    }

    fn plan_packed_dispatch(
        &self,
        cached: &CachedPipeline,
        input_dims: (u32, u32),
        output_dims: (u32, u32),
    ) -> Result<Option<(u32, u32)>, PilError> {
        let max_workgroups = self.device.limits().max_compute_workgroups_per_dimension;
        let plan = match cached.variant_name {
            "__internal_point_luma_packed" | "__internal_equalize_remap_l_packed" => Some(
                plan_packed_point_luma_dispatch(input_dims.0, input_dims.1, max_workgroups)?,
            ),
            "__internal_put_data_luma_packed"
            | "__internal_median_filter_3x3_luma_packed"
            | "__internal_max_filter_3x3_luma_packed"
            | "__internal_min_filter_3x3_luma_packed"
            | "__internal_rank_filter_3x3_luma_packed"
            | "__internal_filter_5x5_luma_packed"
            | "__internal_rank_filter_9x9_luma_packed"
            | "__internal_rank_filter_9x9_luma_row_packed" => Some(plan_packed_luma_dispatch(
                input_dims.0,
                input_dims.1,
                max_workgroups,
            )?),
            "__internal_filter_5x5_la_packed"
            | "__internal_filter_3x3_la_binomial_packed"
            | "__internal_median_filter_3x3_la_packed"
            | "__internal_rank_filter_3x3_la_packed"
            | "__internal_max_filter_3x3_la_packed"
            | "__internal_min_filter_3x3_la_packed" => Some(plan_packed_la_dispatch(
                input_dims.0,
                input_dims.1,
                max_workgroups,
            )?),
            "__internal_blur_h_luma_packed"
            | "__internal_blur_v_luma_packed"
            | "__internal_blur_hv_luma_radius_one_packed" => Some(plan_packed_luma_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
            )?),
            "__internal_blur_h_la_packed" | "__internal_blur_v_la_packed" => Some(
                plan_packed_la_dispatch(output_dims.0, output_dims.1, max_workgroups)?,
            ),
            "__internal_resize_h_pad_luma_packed" => Some(plan_packed_luma_resize_dispatch(
                output_dims.0,
                input_dims.1,
                max_workgroups,
            )?),
            "__internal_resize_v_pad_luma_packed" => Some(plan_packed_luma_resize_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
            )?),
            "__internal_pad_luma_packed" => Some(plan_packed_luma_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
            )?),
            "__internal_pad_la_packed" => Some(plan_packed_la_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
            )?),
            _ => None,
        };
        Ok(plan)
    }

    fn plan_native_dispatch(
        &self,
        cached: &CachedPipeline,
        resources: &GpuBatchResources,
        output_dims: (u32, u32),
    ) -> Result<Option<(u32, u32)>, PilError> {
        let max_workgroups = self.device.limits().max_compute_workgroups_per_dimension;
        let plan = match cached.variant_name {
            "ExtractBand" | "Constant" => Some(plan_extract_band_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
            )?),
            "Grayscale" => Some(plan_packed_output_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
                GRAYSCALE_WORKGROUP_SIZE,
                "GPU grayscale",
            )?),
            "__internal_sharpness_l_native" => {
                let plan = plan_sharpness_l_dispatch(output_dims.0, output_dims.1, max_workgroups)?;
                Some((plan.groups_x, plan.groups_y))
            }
            "Expand" if resources.native_expand_output.is_some() => {
                let plan = resources
                    .native_expand_output
                    .expect("guarded native Expand dispatch");
                Some((plan.groups_x, plan.groups_y))
            }
            "__internal_transform_l_nearest_packed" => {
                let plan = resources.native_luma_transform_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-L Transform shader has no checked output plan".into(),
                    )
                })?;
                Some((plan.groups_x, plan.groups_y))
            }
            "__internal_transform_rgb_nearest_packed" => {
                let plan = resources.native_rgb_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-RGB Transform shader has no checked output plan".into(),
                    )
                })?;
                Some((plan.groups_x, plan.groups_y))
            }
            "__internal_transform_la_nearest_packed" => {
                let plan = resources.native_la_transform_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed-LA Transform shader has no checked output plan".into(),
                    )
                })?;
                Some((plan.groups_x, plan.groups_y))
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
                Some((plan.groups_x, plan.groups_y))
            }
            "__internal_blur_h" | "__internal_blur_v" => Some(plan_blur_dispatch(
                output_dims.0,
                output_dims.1,
                max_workgroups,
            )?),
            _ => None,
        };
        Ok(plan)
    }

    fn plan_resize_dispatch(
        &self,
        cached: &CachedPipeline,
        resources: &GpuBatchResources,
        input_dims: (u32, u32),
        output_dims: (u32, u32),
    ) -> Result<Option<(u32, u32)>, PilError> {
        let plan = match cached.variant_name {
            "__internal_resize_h" => Some((output_dims.0.div_ceil(16), input_dims.1.div_ceil(16))),
            "__internal_resize_h_cover_native_packed" => {
                let plan = resources.native_cover_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed native-L Cover intermediate has no dispatch plan".into(),
                    )
                })?;
                Some((plan.horizontal_groups_x, plan.horizontal_groups_y))
            }
            "__internal_resize_v" | "__internal_resize_v_pad_rgbx" => {
                Some((output_dims.0.div_ceil(16), output_dims.1.div_ceil(16)))
            }
            "__internal_resize_v_cover_native_packed" => {
                let plan = resources.native_cover_packed_output.ok_or_else(|| {
                    PilError::InternalError(
                        "packed native-L Cover output has no dispatch plan".into(),
                    )
                })?;
                Some((plan.groups_x, plan.groups_y))
            }
            _ => None,
        };
        Ok(plan)
    }

    fn plan_histogram_dispatch(variant_name: &str, input_dims: (u32, u32)) -> Option<(u32, u32)> {
        match variant_name {
            "__internal_equalize_histogram"
            | "__internal_equalize_histogram_rgb_packed"
            | "__internal_equalize_histogram_l_packed" => {
                // About 16 packed pixels per lane amortizes each group's
                // shared histogram. Cap groups to bound global bin merges;
                // the shader grid-stride loop handles the remaining pixels.
                let pixels = u64::from(input_dims.0) * u64::from(input_dims.1);
                Some((pixels.div_ceil(4096).clamp(1, 256) as u32, 1))
            }
            "__internal_histogram_clear"
            | "__internal_autocontrast_lut"
            | "__internal_equalize_lut" => Some((1, 1)),
            _ => None,
        }
    }
}

struct BindGroupBuilder<'a> {
    cached: &'a CachedPipeline,
    input_buf: &'a wgpu::Buffer,
    output_buf: &'a wgpu::Buffer,
    resources: &'a GpuBatchResources<'a>,
    op_index: usize,
    params: wgpu::BindingResource<'a>,
    second: wgpu::BindingResource<'a>,
    third: wgpu::BindingResource<'a>,
    lut: wgpu::BindingResource<'a>,
}

impl<'a> BindGroupBuilder<'a> {
    fn append_histogram_bindings(
        &self,
        entries: &mut Vec<wgpu::BindGroupEntry<'a>>,
    ) -> Result<bool, PilError> {
        // Histogram control passes reuse ordinary binding counts but
        // reinterpret storage slots. The final remap reads the generated LUT
        // buffer instead of the host-uploaded LUT arena.
        match self.cached.variant_name {
            "__internal_histogram_clear" => {
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.resources.histogram.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.clone(),
                });
            }
            "__internal_equalize_histogram" | "__internal_equalize_histogram_rgb_packed" => {
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.third.clone(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.resources.histogram.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.params.clone(),
                });
            }
            "__internal_equalize_histogram_l_packed" => {
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.resources.histogram.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.clone(),
                });
            }
            "__internal_autocontrast_lut" | "__internal_equalize_lut" => {
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.resources.histogram.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.resources.fallback_lut.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.clone(),
                });
            }
            "__internal_histogram_remap" | "__internal_histogram_remap_rgb_packed" => {
                entries.push(wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.input_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.output_buf.as_entire_binding(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.params.clone(),
                });
                entries.push(wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.resources.fallback_lut.as_entire_binding(),
                });
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn append_resize_bindings(
        &self,
        entries: &mut Vec<wgpu::BindGroupEntry<'a>>,
    ) -> Result<bool, PilError> {
        if !matches!(
            self.cached.variant_name,
            "__internal_resize_h"
                | "__internal_resize_h_cover_native_packed"
                | "__internal_resize_h_pad_luma_packed"
                | "__internal_resize_v"
                | "__internal_resize_v_cover_native_packed"
                | "__internal_resize_v_pad_luma_packed"
                | "__internal_resize_v_pad_rgbx"
        ) {
            return Ok(false);
        }
        let (horizontal, vertical) = self
            .resources
            .resize_coeff_ranges
            .get(self.op_index)
            .and_then(Option::as_ref)
            .ok_or_else(|| {
                PilError::InternalError(
                    "GPU resize pipeline is missing its coefficient ranges".into(),
                )
            })?;
        let range = if matches!(
            self.cached.variant_name,
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
            resource: self.input_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.output_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: self.params.clone(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: auxiliary_binding(
                self.resources.img2,
                self.resources.fallback_img2,
                Some(range),
            )?,
        });
        Ok(true)
    }

    fn append_standard_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        match (
            self.cached.num_bindings,
            self.cached.is_lut,
            self.cached.is_output_only,
        ) {
            (2, _, true) => self.append_generator_bindings(entries),
            (5, _, _) => self.append_five_bindings(entries),
            (4, true, _) => self.append_lut_bindings(entries),
            (4, false, _) => self.append_dual_input_bindings(entries),
            (3, _, _) if self.cached.is_in_place => self.append_in_place_bindings(entries),
            _ => self.append_default_bindings(entries),
        }
    }

    fn append_generator_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        // Generator layout: [output(rw), params(uniform)].
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.output_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.params.clone(),
        });
    }

    fn append_five_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        // [in_a(read), in_b(read), in_c(read), out(rw), params(uniform)].
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.input_buf.as_entire_binding(),
        });
        // Storage read and read_write bindings may not alias in one dispatch.
        // Keep absent optional inputs on dedicated auxiliary buffers; shader
        // presence parameters guard their reads.
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.second.clone(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: self.third.clone(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: self.output_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 4,
            resource: self.params.clone(),
        });
    }

    fn append_lut_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        // LUT layout: [input(read), output(rw), params(uniform), lut(read)].
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.input_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.output_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: self.params.clone(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: self.lut.clone(),
        });
    }

    fn append_dual_input_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        // Dual-input layout: [input_a(read), input_b(read), output(rw), params].
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.input_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.second.clone(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: self.output_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 3,
            resource: self.params.clone(),
        });
    }

    fn append_in_place_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        // AlphaComposite reads its source and updates the current destination
        // ping-pong buffer in place; encode_batch keeps that buffer selected.
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.second.clone(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.input_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 2,
            resource: self.params.clone(),
        });
    }

    fn append_default_bindings(&self, entries: &mut Vec<wgpu::BindGroupEntry<'a>>) {
        // Two or three bindings: [input(read), output(rw), params?].
        entries.push(wgpu::BindGroupEntry {
            binding: 0,
            resource: self.input_buf.as_entire_binding(),
        });
        entries.push(wgpu::BindGroupEntry {
            binding: 1,
            resource: self.output_buf.as_entire_binding(),
        });
        if self.cached.num_bindings > 2 {
            entries.push(wgpu::BindGroupEntry {
                binding: 2,
                resource: self.params.clone(),
            });
        }
    }
}
