use super::*;

impl GpuInner {
    pub(super) fn resolve_batch_pipelines(
        &self,
        ops: &[PipelineOp],
        logical_mode: Option<&str>,
        input_dims: &[(u32, u32)],
        packed_luma_point: bool,
        packed_luma_putdata: bool,
        packed_luma_order_statistic: bool,
        packed_native_byte_filter: bool,
        packed_luma_pad: bool,
        packed_la_pad: bool,
        native_sharpness_l_input: bool,
        native_sharpness_la_input: bool,
        native_sharpness_rgb_input: bool,
        native_cover_packed_output: Option<NativeCoverPackedOutputDispatch>,
        native_luma_transform_output: bool,
        native_rgb_packed_output: bool,
        native_la_transform_output: bool,
    ) -> Result<Vec<ResolvedPipeline>, PilError> {
        let mut resolved = Vec::with_capacity(ops.len());
        let mut index = 0usize;
        while index < ops.len() {
            if index + 1 < ops.len() && can_fuse_gpu_multiply_screen(ops, index) {
                let fused = self.resolve_pipeline(
                    "__internal_multiply_screen",
                    "multiply_screen.wgsl",
                    include_str!("shaders/multiply_screen.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(fused));
                resolved.push(ResolvedPipeline::Skip);
                index += 2;
                continue;
            }
            if can_fuse_gpu_alpha_composite_mirror(ops, index, logical_mode, input_dims[index]) {
                let fused = self.resolve_pipeline(
                    "__internal_alpha_composite_mirror",
                    "alpha_composite_mirror.wgsl",
                    include_str!("shaders/alpha_composite_mirror.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(fused));
                resolved.push(ResolvedPipeline::Skip);
                index += 2;
                continue;
            }

            let op = &ops[index];
            if packed_luma_putdata && matches!(op, PipelineOp::PutData { .. }) {
                let putdata = self.resolve_pipeline(
                    "__internal_put_data_luma_packed",
                    "put_data_luma_packed.wgsl",
                    include_str!("shaders/put_data_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(putdata));
                index += 1;
                continue;
            }
            if packed_luma_point && matches!(op, PipelineOp::Eval { .. }) {
                let point = self.resolve_pipeline(
                    "__internal_point_luma_packed",
                    "point_luma_packed.wgsl",
                    include_str!("shaders/point_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(point));
                index += 1;
                continue;
            }
            if native_rgb_packed_output && matches!(op, PipelineOp::Eval { .. }) {
                let point = self.resolve_pipeline(
                    "__internal_point_rgb_packed",
                    "point_rgb_packed.wgsl",
                    include_str!("shaders/point_rgb_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(point));
                index += 1;
                continue;
            }
            if native_rgb_packed_output && matches!(op, PipelineOp::MedianFilter { size: 3 }) {
                let median = self.resolve_pipeline(
                    "__internal_median_filter_3x3_rgb_packed",
                    "median_filter_3x3_rgb_packed.wgsl",
                    include_str!("shaders/median_filter_3x3_rgb_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(median));
                index += 1;
                continue;
            }
            if native_rgb_packed_output && matches!(op, PipelineOp::MaxFilter { size: 3 }) {
                let max_filter = self.resolve_pipeline(
                    "__internal_max_filter_3x3_rgb_packed",
                    "max_filter_3x3_rgb_packed.wgsl",
                    include_str!("shaders/max_filter_3x3_rgb_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(max_filter));
                index += 1;
                continue;
            }
            if native_rgb_packed_output && matches!(op, PipelineOp::MinFilter { size: 3 }) {
                let min_filter = self.resolve_pipeline(
                    "__internal_min_filter_3x3_rgb_packed",
                    "min_filter_3x3_rgb_packed.wgsl",
                    include_str!("shaders/min_filter_3x3_rgb_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(min_filter));
                index += 1;
                continue;
            }
            if packed_luma_order_statistic && matches!(op, PipelineOp::MedianFilter { size: 3 }) {
                let median = self.resolve_pipeline(
                    "__internal_median_filter_3x3_luma_packed",
                    "median_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/median_filter_3x3_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(median));
                index += 1;
                continue;
            }
            if packed_luma_order_statistic && matches!(op, PipelineOp::MaxFilter { size: 3 }) {
                let max_filter = self.resolve_pipeline(
                    "__internal_max_filter_3x3_luma_packed",
                    "max_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/max_filter_3x3_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(max_filter));
                index += 1;
                continue;
            }
            if packed_luma_order_statistic && matches!(op, PipelineOp::MinFilter { size: 3 }) {
                let min_filter = self.resolve_pipeline(
                    "__internal_min_filter_3x3_luma_packed",
                    "min_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/min_filter_3x3_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(min_filter));
                index += 1;
                continue;
            }
            if packed_luma_order_statistic
                && matches!(op, PipelineOp::RankFilter { size: 3, rank: 1 })
            {
                let rank_filter = self.resolve_pipeline(
                    "__internal_rank_filter_3x3_luma_packed",
                    "rank_filter_3x3_luma_packed.wgsl",
                    include_str!("shaders/rank_filter_3x3_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(rank_filter));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, None | Some("L"))
                && matches!(op, PipelineOp::Filter5x5 { .. })
            {
                let filter = self.resolve_pipeline(
                    "__internal_filter_5x5_luma_packed",
                    "filter_5x5_luma_packed.wgsl",
                    include_str!("shaders/filter_5x5_luma_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(filter));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::Filter3x3 { .. })
            {
                let filter = self.resolve_pipeline(
                    "__internal_filter_3x3_la_binomial_packed",
                    "filter_3x3_la_binomial_packed.wgsl",
                    include_str!("shaders/filter_3x3_la_binomial_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(filter));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::Filter5x5 { .. })
            {
                let filter = self.resolve_pipeline(
                    "__internal_filter_5x5_la_packed",
                    "filter_5x5_la_packed.wgsl",
                    include_str!("shaders/filter_5x5_la_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(filter));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::MedianFilter { size: 3 })
            {
                let median = self.resolve_pipeline(
                    "__internal_median_filter_3x3_la_packed",
                    "median_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/median_filter_3x3_la_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(median));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::RankFilter { size: 3, rank } if *rank < 9)
            {
                let rank_filter = self.resolve_pipeline(
                    "__internal_rank_filter_3x3_la_packed",
                    "rank_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/rank_filter_3x3_la_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(rank_filter));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::MaxFilter { size: 3 })
            {
                let max_filter = self.resolve_pipeline(
                    "__internal_max_filter_3x3_la_packed",
                    "max_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/max_filter_3x3_la_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(max_filter));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, Some("LA"))
                && matches!(op, PipelineOp::MinFilter { size: 3 })
            {
                let min_filter = self.resolve_pipeline(
                    "__internal_min_filter_3x3_la_packed",
                    "min_filter_3x3_la_packed.wgsl",
                    include_str!("shaders/min_filter_3x3_la_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(min_filter));
                index += 1;
                continue;
            }
            if packed_luma_order_statistic && matches!(op, PipelineOp::RankFilter { size: 9, .. }) {
                let rank = if input_dims[index].0 % 4 == 0 {
                    self.resolve_pipeline(
                        "__internal_rank_filter_9x9_luma_row_packed",
                        "rank_filter_9x9_luma_row_packed.wgsl",
                        include_str!("shaders/rank_filter_9x9_luma_row_packed.wgsl"),
                    )?
                } else {
                    self.resolve_pipeline(
                        "__internal_rank_filter_9x9_luma_packed",
                        "rank_filter_9x9_luma_packed.wgsl",
                        include_str!("shaders/rank_filter_9x9_luma_packed.wgsl"),
                    )?
                };
                resolved.push(ResolvedPipeline::Single(rank));
                index += 1;
                continue;
            }
            if native_sharpness_l_input && matches!(op, PipelineOp::Sharpness { .. }) {
                let sharpness = self.resolve_pipeline(
                    "__internal_sharpness_l_native",
                    "sharpness_l_native.wgsl",
                    include_str!("shaders/sharpness_l_native.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(sharpness));
                index += 1;
                continue;
            }
            if native_sharpness_la_input && matches!(op, PipelineOp::Sharpness { .. }) {
                let sharpness = self.resolve_pipeline(
                    "__internal_sharpness_la_native",
                    "sharpness_la_native.wgsl",
                    include_str!("shaders/sharpness_la_native.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(sharpness));
                index += 1;
                continue;
            }
            if native_sharpness_rgb_input && matches!(op, PipelineOp::Sharpness { .. }) {
                let sharpness = self.resolve_pipeline(
                    "__internal_sharpness_rgb_native",
                    "sharpness_rgb_native.wgsl",
                    include_str!("shaders/sharpness_rgb_native.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(sharpness));
                index += 1;
                continue;
            }
            if native_rgb_packed_output
                && matches!(logical_mode, None | Some("RGB"))
                && matches!(op, PipelineOp::BoxBlur { radius: 1 })
            {
                let fused = self.resolve_pipeline(
                    "__internal_blur_hv_rgb_radius_one_packed",
                    "box_blur_hv_rgb_radius_one_packed.wgsl",
                    include_str!("shaders/box_blur_hv_rgb_radius_one_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(fused));
                index += 1;
                continue;
            }
            if packed_native_byte_filter
                && matches!(logical_mode, None | Some("L"))
                && matches!(op, PipelineOp::BoxBlur { radius: 1 })
            {
                let fused = self.resolve_pipeline(
                    "__internal_blur_hv_luma_radius_one_packed",
                    "box_blur_hv_luma_radius_one_packed.wgsl",
                    include_str!("shaders/box_blur_hv_luma_radius_one_packed.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(fused));
                index += 1;
                continue;
            }
            // MedianFilter's default 3x3 window is common enough to deserve a
            // dedicated shader. The general insertion-sort shader reserves
            // four 225-value channel arrays per invocation even though this
            // path needs only nine packed samples.
            if matches!(op, PipelineOp::MedianFilter { size: 3 })
                && !matches!(logical_mode, Some("F" | "I"))
            {
                let median = self.resolve_pipeline(
                    "__internal_median_filter_3x3",
                    "median_filter_3x3.wgsl",
                    include_str!("shaders/median_filter_3x3.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Single(median));
                index += 1;
                continue;
            }
            if matches!(
                op,
                PipelineOp::Autocontrast { .. }
                    | PipelineOp::Equalize
                    | PipelineOp::EqualizeMasked { .. }
            ) {
                let clear = self.resolve_pipeline(
                    "__internal_histogram_clear",
                    "histogram_clear.wgsl",
                    include_str!("shaders/histogram_clear.wgsl"),
                )?;
                // Autocontrast and Equalize need the same per-band counts.
                // Share the workgroup-local histogram gather so a large
                // image is spread across device workgroups instead of making
                // one workgroup scan the complete image serially.
                let native_rgb_equalize =
                    native_rgb_packed_output && matches!(op, PipelineOp::Equalize);
                let native_luma_equalize = packed_luma_point && matches!(op, PipelineOp::Equalize);
                let histogram = if native_rgb_equalize {
                    self.resolve_pipeline(
                        "__internal_equalize_histogram_rgb_packed",
                        "equalize_histogram_rgb_packed.wgsl",
                        include_str!("shaders/equalize_histogram_rgb_packed.wgsl"),
                    )?
                } else if native_luma_equalize {
                    self.resolve_pipeline(
                        "__internal_equalize_histogram_l_packed",
                        "equalize_histogram_l_packed.wgsl",
                        include_str!("shaders/equalize_histogram_l_packed.wgsl"),
                    )?
                } else {
                    self.resolve_pipeline(
                        "__internal_equalize_histogram",
                        "equalize_histogram.wgsl",
                        include_str!("shaders/equalize_histogram.wgsl"),
                    )?
                };
                let derive = match op {
                    PipelineOp::Autocontrast { .. } => self.resolve_pipeline(
                        "__internal_autocontrast_lut",
                        "autocontrast_cutoff.wgsl",
                        include_str!("shaders/autocontrast_cutoff.wgsl"),
                    )?,
                    PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => self
                        .resolve_pipeline(
                            "__internal_equalize_lut",
                            "equalize_cdf.wgsl",
                            include_str!("shaders/equalize_cdf.wgsl"),
                        )?,
                    _ => unreachable!("histogram pipeline branch changed"),
                };
                let remap = if native_rgb_equalize {
                    self.resolve_pipeline(
                        "__internal_histogram_remap_rgb_packed",
                        "point_rgb_packed.wgsl",
                        include_str!("shaders/point_rgb_packed.wgsl"),
                    )?
                } else if native_luma_equalize {
                    self.resolve_pipeline(
                        "__internal_equalize_remap_l_packed",
                        "equalize_remap_l_packed.wgsl",
                        include_str!("shaders/equalize_remap_l_packed.wgsl"),
                    )?
                } else {
                    self.resolve_pipeline(
                        "__internal_histogram_remap",
                        "point_op.wgsl",
                        include_str!("shaders/point_op.wgsl"),
                    )?
                };
                resolved.push(ResolvedPipeline::Histogram {
                    clear,
                    histogram,
                    derive,
                    remap,
                });
            } else if let Some(pass_count) = Self::blur_pass_count(op) {
                // These internal variants expand one public blur operation
                // into horizontal and vertical dispatches without a host
                // materialization between them.
                let (horizontal, vertical) = if native_rgb_packed_output
                    && matches!(logical_mode, None | Some("RGB"))
                    && matches!(op, PipelineOp::BoxBlur { radius: 1 })
                {
                    (
                        self.resolve_pipeline(
                            "__internal_blur_h_rgb_packed",
                            "box_blur_h_rgb_packed.wgsl",
                            include_str!("shaders/box_blur_h_rgb_packed.wgsl"),
                        )?,
                        self.resolve_pipeline(
                            "__internal_blur_v_rgb_packed",
                            "box_blur_v_rgb_packed.wgsl",
                            include_str!("shaders/box_blur_v_rgb_packed.wgsl"),
                        )?,
                    )
                } else if packed_native_byte_filter
                    && matches!(
                        op,
                        PipelineOp::GaussianBlur { .. } | PipelineOp::BoxBlur { radius: 1 }
                    )
                {
                    if matches!(logical_mode, Some("LA")) {
                        (
                            self.resolve_pipeline(
                                "__internal_blur_h_la_packed",
                                "box_blur_h_la_packed.wgsl",
                                include_str!("shaders/box_blur_h_la_packed.wgsl"),
                            )?,
                            self.resolve_pipeline(
                                "__internal_blur_v_la_packed",
                                "box_blur_v_la_packed.wgsl",
                                include_str!("shaders/box_blur_v_la_packed.wgsl"),
                            )?,
                        )
                    } else {
                        (
                            self.resolve_pipeline(
                                "__internal_blur_h_luma_packed",
                                "box_blur_h_luma_packed.wgsl",
                                include_str!("shaders/box_blur_h_luma_packed.wgsl"),
                            )?,
                            self.resolve_pipeline(
                                "__internal_blur_v_luma_packed",
                                "box_blur_v_luma_packed.wgsl",
                                include_str!("shaders/box_blur_v_luma_packed.wgsl"),
                            )?,
                        )
                    }
                } else {
                    (
                        self.resolve_pipeline(
                            "__internal_blur_h",
                            "box_blur_h.wgsl",
                            include_str!("shaders/box_blur_h.wgsl"),
                        )?,
                        self.resolve_pipeline(
                            "__internal_blur_v",
                            "box_blur_v.wgsl",
                            include_str!("shaders/box_blur_v.wgsl"),
                        )?,
                    )
                };
                resolved.push(ResolvedPipeline::Blur {
                    horizontal,
                    vertical,
                    pass_count,
                });
            } else if matches!(op, PipelineOp::Pad { .. }) {
                let (horizontal, vertical) = if packed_luma_pad {
                    (
                        self.resolve_pipeline(
                            "__internal_resize_h_pad_luma_packed",
                            "resize_convolution_h.wgsl",
                            include_str!("shaders/resize_convolution_h.wgsl"),
                        )?,
                        self.resolve_pipeline(
                            "__internal_resize_v_pad_luma_packed",
                            "resize_convolution_v.wgsl",
                            include_str!("shaders/resize_convolution_v.wgsl"),
                        )?,
                    )
                } else {
                    (
                        self.resolve_pipeline(
                            "__internal_resize_h",
                            "resize_convolution_h.wgsl",
                            include_str!("shaders/resize_convolution_h.wgsl"),
                        )?,
                        self.resolve_pipeline(
                            "__internal_resize_v",
                            "resize_convolution_v.wgsl",
                            include_str!("shaders/resize_convolution_v.wgsl"),
                        )?,
                    )
                };
                let place = if packed_luma_pad {
                    self.resolve_pipeline(
                        "__internal_pad_luma_packed",
                        "pad_luma_packed.wgsl",
                        include_str!("shaders/pad_luma_packed.wgsl"),
                    )?
                } else if packed_la_pad {
                    self.resolve_pipeline(
                        "__internal_pad_la_packed",
                        "pad_la_packed.wgsl",
                        include_str!("shaders/pad_la_packed.wgsl"),
                    )?
                } else {
                    self.resolve_pipeline(
                        "__internal_pad_place",
                        "pad.wgsl",
                        include_str!("shaders/pad.wgsl"),
                    )?
                };
                let vertical_pad_rgbx =
                    if gpu_pad_rgbx_fused_vertical_geometry(op, input_dims[index], logical_mode)
                        .is_some()
                    {
                        Some(self.resolve_pipeline(
                            "__internal_resize_v_pad_rgbx",
                            "resize_convolution_v_pad_rgbx.wgsl",
                            include_str!("shaders/resize_convolution_v_pad_rgbx.wgsl"),
                        )?)
                    } else {
                        None
                    };
                resolved.push(ResolvedPipeline::Pad {
                    horizontal,
                    vertical,
                    place,
                    vertical_pad_rgbx,
                });
            } else if matches!(op, PipelineOp::Fit { .. } | PipelineOp::ResizeBoxed { .. }) {
                let horizontal = self.resolve_pipeline(
                    "__internal_resize_h",
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?;
                let vertical = self.resolve_pipeline(
                    "__internal_resize_v",
                    "resize_convolution_v.wgsl",
                    include_str!("shaders/resize_convolution_v.wgsl"),
                )?;
                resolved.push(ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                });
            } else if matches!(op, PipelineOp::Resize { filter, .. }
                if matches!(filter, ResampleFilter::Nearest)
                    && !gpu_resize_nearest_uses_coefficients(logical_mode))
            {
                let key = registry::variant_key(op);
                let source = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_source)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader source"
                        ))
                    })?;
                let shader_file = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_shader)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader name"
                        ))
                    })?;
                resolved.push(
                    self.resolve_pipeline(key, shader_file, source)
                        .map(ResolvedPipeline::Single)?,
                );
            } else if matches!(op, PipelineOp::Resize { .. }) {
                let horizontal = self.resolve_pipeline(
                    if native_cover_packed_output.is_some() {
                        "__internal_resize_h_cover_native_packed"
                    } else {
                        "__internal_resize_h"
                    },
                    "resize_convolution_h.wgsl",
                    include_str!("shaders/resize_convolution_h.wgsl"),
                )?;
                let vertical = if native_cover_packed_output.is_some() {
                    self.resolve_pipeline(
                        "__internal_resize_v_cover_native_packed",
                        "resize_convolution_v.wgsl",
                        include_str!("shaders/resize_convolution_v.wgsl"),
                    )?
                } else {
                    self.resolve_pipeline(
                        "__internal_resize_v",
                        "resize_convolution_v.wgsl",
                        include_str!("shaders/resize_convolution_v.wgsl"),
                    )?
                };
                resolved.push(ResolvedPipeline::Resize {
                    horizontal,
                    vertical,
                });
            } else if native_rgb_packed_output && matches!(op, PipelineOp::Transform { .. }) {
                resolved.push(ResolvedPipeline::Single(self.resolve_pipeline(
                    "__internal_transform_rgb_nearest_packed",
                    "transform_rgb_nearest_packed.wgsl",
                    include_str!("shaders/transform_rgb_nearest_packed.wgsl"),
                )?));
            } else if native_la_transform_output && matches!(op, PipelineOp::Transform { .. }) {
                resolved.push(ResolvedPipeline::Single(self.resolve_pipeline(
                    "__internal_transform_la_nearest_packed",
                    "transform_la_nearest_packed.wgsl",
                    include_str!("shaders/transform_la_nearest_packed.wgsl"),
                )?));
            } else if native_luma_transform_output && matches!(op, PipelineOp::Transform { .. }) {
                resolved.push(ResolvedPipeline::Single(self.resolve_pipeline(
                    "__internal_transform_l_nearest_packed",
                    "transform_l_nearest_packed.wgsl",
                    include_str!("shaders/transform_l_nearest_packed.wgsl"),
                )?));
            } else if gpu_transform_uses_geometry_table(op, logical_mode, input_dims[index]) {
                resolved.push(ResolvedPipeline::Single(self.resolve_pipeline(
                    "__internal_transform_geometry",
                    "transform_geometry.wgsl",
                    include_str!("shaders/transform_geometry.wgsl"),
                )?));
            } else {
                let key = registry::variant_key(op);
                let source = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_source)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader source"
                        ))
                    })?;
                let shader_file = registry::registry()?
                    .get(key)
                    .and_then(|entry| entry.gpu_shader)
                    .ok_or_else(|| {
                        PilError::ValueError(format!(
                            "GPU operation '{key}' has no registered shader name"
                        ))
                    })?;
                resolved.push(
                    self.resolve_pipeline(key, shader_file, source)
                        .map(ResolvedPipeline::Single)?,
                );
            }
            index += 1;
        }
        Ok(resolved)
    }
}
