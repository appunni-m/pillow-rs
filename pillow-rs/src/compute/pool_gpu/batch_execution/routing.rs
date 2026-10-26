use super::super::*;
use crate::raster::ColorType;

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
        let NormalizedGpuBatch {
            native_cover_input_channels,
            native_sharpness_la_patch,
            input_layouts,
            dispatch_ops,
        } = normalized;
        let ops = dispatch_ops.as_slice();
        let BatchInputLayoutCandidates {
            packed_luma_colorize,
            packed_luma_convert,
            native_alpha_convert_input,
            packed_luma_transform_input,
            native_rgb_compact_input: native_rgb_compact_input_candidate,
            native_la_transform_input,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
        } = input_layouts;
        if let Some(result) =
            self.execute_batch_segments_if_needed(ops, img, mode, allow_cpu_fallback)?
        {
            return Ok(result);
        }

        let GpuBatchResizeProofs {
            constant_bits: f_resize_constant_bits,
            box_copy_is_exact: f_resize_box_copy_is_exact,
            identity_is_exact: f_resize_identity_is_exact,
            box_average_is_exact: f_resize_box_average_is_exact,
            pad_f64_is_exact: f_pad_f64_is_exact,
            dyadic_is_exact: f_resize_dyadic_is_exact,
            f64_is_exact: f_resize_f64_is_exact,
            f64_ordered_is_exact: f_resize_f64_ordered_is_exact,
        } = gpu_batch_resize_proofs(&dispatch_ops, img, mode);

        // Pillow defines both histogram operations as identity operations for
        // an empty image. There is no valid storage-buffer invocation to
        // issue for a zero-area image, so complete this already-validated
        // no-op without entering the device path or manufacturing a CPU
        // fallback receipt.
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
            return Ok(img.clone());
        }

        // Geometry operations whose public contract includes a reducing-gap,
        // fractional crop, or typed-sample rule must publish the exact host
        // result.  The helper still performs a real GPU copy for packed byte
        // results, so this is a controlled host/GPU boundary rather than an
        // operation-level capability outcome.
        if let Some(reason) = gpu_geometry_host_control_reason(
            ops,
            img,
            mode,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact || f_resize_f64_ordered_is_exact,
        ) {
            return self.execute_exact_host_result(ops, img, mode, reason);
        }

        let empty_two_input_noop = match self.preflight_batch_contract(
            ops,
            img,
            mode,
            allow_cpu_fallback,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
        )? {
            BatchContractPreflight::Continue {
                empty_two_input_noop,
            } => empty_two_input_noop,
            BatchContractPreflight::Return(image) => return Ok(image),
        };

        // Distance one makes every EffectSpread displacement exactly zero.
        // Preserve its two RNG draws per pixel, but skip generating/uploading
        // the identity map and submitting a shader that only copies pixels.
        // Keep this after the ordinary GPU capability and dimension checks so
        // strict backend selection retains the same input contract.
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
            return Ok(img.clone());
        }

        // Resolve nested images before GPU work; a locked nested pipeline may
        // itself need the device and must surface its materialization errors.
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
            BatchResolvedInputPreflight::Return(image) => return Ok(image),
        };

        let gpu = Self::ensure_init()?;
        gpu.ensure_healthy("GPU batch start")?;
        let max_workgroups_per_dimension = gpu.device.limits().max_compute_workgroups_per_dimension;
        let packed_dispatch_candidates = GpuPackedDispatchCandidates {
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
        };
        let packed_dispatch_plan = plan_gpu_packed_dispatches(
            ops,
            img,
            mode,
            max_workgroups_per_dimension,
            packed_dispatch_candidates,
        );
        let packed_dispatch_plan = match self.preflight_packed_dispatches(
            ops,
            img,
            mode,
            allow_cpu_fallback,
            max_workgroups_per_dimension,
            packed_dispatch_plan,
        )? {
            BatchPackedDispatchPreflight::Continue(plan) => plan,
            BatchPackedDispatchPreflight::Return(image) => return Ok(image),
        };
        let GpuPackedDispatchPlan {
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
            ..
        } = packed_dispatch_plan;
        let image_pixels = CheckedDims::new(img.width(), img.height(), 1)?.total_pixels();
        let full_luma_putdata = packed_luma_putdata
            && matches!(ops, [PipelineOp::PutData { data, mode: PixelMode::L }]
                if data.len() == image_pixels);
        let limits = gpu.device.limits();
        #[cfg(target_endian = "little")]
        if matches!(mode, None | Some("RGB"))
            && matches!(img, DynamicImage::ImageRgb8(_))
            && let [
                PipelineOp::Crop {
                    left,
                    top,
                    right,
                    bottom,
                },
            ] = ops
            && let Some(dispatch) = plan_gpu_native_rgb_crop(
                img.width(),
                img.height(),
                *left,
                *top,
                *right,
                *bottom,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = gpu.acquire_buffers(dispatch.buffer_words)?;
            let result = gpu.execute_native_rgb_crop(img, *left, *top, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }
        #[cfg(target_endian = "little")]
        if let Some(bands) = gpu_native_rgb_luma_merge_bands(ops, img, mode)
            && let Some(dispatch) = plan_gpu_native_rgb_merge(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
            )
        {
            let mut buffers = gpu.acquire_buffers(dispatch.buffer_words)?;
            let result = gpu.execute_native_rgb_luma_merge(img, bands, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }
        // A compact native route can fit limits that the generic RGBA buffer
        // and 16x16 dispatch checks reject. Its planner must prove every
        // adapter and global image limit before it bypasses those checks.
        #[cfg(target_endian = "little")]
        let native_masked_byte_paste = (|| {
            let ([op], [auxiliary]) = (ops, auxiliary_images.as_slice()) else {
                return None;
            };
            let (Some(source_pixels), Some(mask_pixels)) =
                (auxiliary.second.as_deref(), auxiliary.third.as_deref())
            else {
                return None;
            };
            let (offset, logical_mode, dispatch) = plan_gpu_native_masked_byte_paste_for_op(
                op,
                img,
                source_pixels,
                mask_pixels,
                mode,
                &limits,
            )?;
            Some((source_pixels, mask_pixels, offset, logical_mode, dispatch))
        })();
        #[cfg(not(target_endian = "little"))]
        let native_masked_byte_paste: Option<(
            &DynamicImage,
            &DynamicImage,
            (i32, i32),
            &'static str,
            NativeMaskedBytePasteDispatch,
        )> = None;

        #[cfg(target_endian = "little")]
        let native_hsv_pad = plan_native_hsv_pad_dispatch(
            ops,
            img,
            mode,
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        );
        #[cfg(not(target_endian = "little"))]
        let native_hsv_pad: Option<()> = None;

        if native_masked_byte_paste.is_none()
            && native_hsv_pad.is_none()
            && !packed_luma_order_statistic
            && gpu_dispatch_dimensions_require_cpu(
                ops,
                img.dimensions(),
                limits.max_compute_workgroups_per_dimension,
                mode,
                packed_luma_pad,
            )
        {
            gpu_log!("[GPU] dispatch preflight routed batch to CPU: adapter workgroup limit");
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "adapter workgroup limit",
            );
        }
        if let Some((source_pixels, mask_pixels, offset, logical_mode, dispatch)) =
            native_masked_byte_paste
        {
            let mut buffers = gpu.acquire_buffers(dispatch.word_count)?;
            let result = gpu.execute_native_masked_byte_paste(
                img,
                source_pixels,
                mask_pixels,
                logical_mode,
                offset,
                dispatch,
                &mut buffers,
            )?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }
        #[cfg(target_endian = "little")]
        if let Some(dispatch) = native_hsv_pad {
            let mut buffers = gpu.acquire_buffers(dispatch.output.output_word_count)?;
            let result = gpu.execute_native_hsv_pad(img, dispatch, &mut buffers)?;
            gpu.recycle_buffers(buffers);
            return Ok(result);
        }

        let capacity = gpu_batch_capacity(ops, img, &auxiliary_images, mode)?;
        let packed_luma_transform_output = packed_luma_transform_input
            && plan_native_luma_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
            .is_some();
        let native_rgb_output_plan = if matches!(
            ops,
            [PipelineOp::Eval { .. }
                | PipelineOp::Equalize
                | PipelineOp::MedianFilter { size: 3 }
                | PipelineOp::MinFilter { size: 3 }
                | PipelineOp::MaxFilter { size: 3 }
                | PipelineOp::BoxBlur { radius: 1 }]
        ) {
            plan_native_rgb_point_output(
                img.width(),
                img.height(),
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
        } else {
            plan_native_rgb_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
        };
        let native_rgb_packed_output =
            native_rgb_compact_input_candidate && native_rgb_output_plan.is_some();
        // Native RGB point, equalize, and 3x3 order filters require the
        // compact output plan as well as compact input. Otherwise keep both
        // transfer boundaries on the standard RGBA contract.
        let native_rgb_compact_input = native_rgb_compact_input_candidate
            && (!matches!(
                ops,
                [PipelineOp::Eval { .. }
                    | PipelineOp::Equalize
                    | PipelineOp::MedianFilter { size: 3 }
                    | PipelineOp::MinFilter { size: 3 }
                    | PipelineOp::MaxFilter { size: 3 }
                    | PipelineOp::BoxBlur { radius: 1 }]
            ) || native_rgb_packed_output);
        let native_la_transform_output = native_la_transform_input
            && plan_native_la_transform_output(
                ops,
                limits.max_compute_workgroups_per_dimension,
                limits.max_storage_buffer_binding_size,
                limits.max_buffer_size,
                capacity,
            )
            .is_some();
        let native_cover_packed_output = gpu_native_cover_packed_output_dispatch(
            ops,
            native_cover_input_channels,
            mode,
            img.height(),
            limits.max_compute_workgroups_per_dimension,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
            capacity,
        );
        if gpu_buffer_capacity_exceeds_limits(
            capacity,
            limits.max_storage_buffer_binding_size,
            limits.max_buffer_size,
        ) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: image buffer exceeds adapter limits"
            );
            return self.preflight_failure(
                ops,
                img,
                mode,
                allow_cpu_fallback,
                "image buffer exceeds adapter limits",
            );
        }
        #[cfg(target_endian = "little")]
        if let Some(result) =
            gpu.try_execute_native_batch_routes(ops, img, mode, &auxiliary_images, &limits)?
        {
            return Ok(result);
        }

        #[cfg(target_endian = "little")]
        match gpu.try_execute_native_flip(ops, img, mode, &limits, capacity)? {
            NativeGeometryDispatchOutcome::NotApplicable => {}
            NativeGeometryDispatchOutcome::ExceedsLimits(reason) => {
                return self.preflight_failure(ops, img, mode, allow_cpu_fallback, reason);
            }
            NativeGeometryDispatchOutcome::Executed(result) => return Ok(result),
        }

        #[cfg(target_endian = "little")]
        match gpu.try_execute_native_transpose_paths(ops, img, mode, &limits, capacity)? {
            NativeGeometryDispatchOutcome::NotApplicable => {}
            NativeGeometryDispatchOutcome::ExceedsLimits(reason) => {
                return self.preflight_failure(ops, img, mode, allow_cpu_fallback, reason);
            }
            NativeGeometryDispatchOutcome::Executed(result) => return Ok(result),
        }

        let mut buffers = gpu.acquire_buffers(capacity)?;
        let native_luma16 = gpu_luma16_geometry_is_supported(ops, img, mode);
        let native_luma16_convert = gpu_luma16_convert_is_supported(ops, img);
        let native_luma16_paste = gpu_luma16_paste_is_supported(ops, img);
        let (w, h) = img.dimensions();
        let mcode = execution_mode_code(img, mode);
        let op_keys: Vec<&str> = ops.iter().map(|op| registry::variant_key(op)).collect();
        log::debug!(
            "[GPU] {} op(s) {}x{} mode={}: {:?}",
            ops.len(),
            w,
            h,
            mcode,
            op_keys
        );
        let native_extract_band_channels = gpu_native_extract_band_channels(ops, img, mode);
        let native_extract_band = native_extract_band_channels.is_some();
        let output_only_constant = matches!(ops, [PipelineOp::Constant { .. }]);
        let native_grayscale_rgb_input = gpu_native_grayscale_rgb_input(ops, img, mode);
        let native_grayscale_cmyk_input = gpu_native_grayscale_cmyk_input(ops, img, mode);
        let native_sharpness_l_input = gpu_native_sharpness_l_input(ops, img, mode);
        let native_sharpness_la_input = gpu_native_sharpness_la_input(ops, img, mode);
        let native_sharpness_rgb_input = gpu_native_sharpness_rgb_input(ops, img, mode);
        let native_reduce_rgb_input = gpu_native_reduce_rgb_input(ops, img, mode);
        let native_expand_channels = gpu_native_expand_channels(ops, img, mode);
        #[cfg(target_endian = "little")]
        let mapped_native_box_blur_input = gpu.direct_primary_readback
            && matches!(ops, [PipelineOp::BoxBlur { radius: 1 }])
            && ((packed_native_byte_filter
                && matches!(mode, None | Some("L"))
                && matches!(img, DynamicImage::ImageLuma8(image)
                    if image.as_raw().len() == (w as usize).saturating_mul(h as usize)))
                || (native_rgb_compact_input
                    && matches!(mode, None | Some("RGB"))
                    && matches!(img, DynamicImage::ImageRgb8(image)
                        if image.as_raw().len() == (w as usize).saturating_mul(h as usize).saturating_mul(3))));
        #[cfg(not(target_endian = "little"))]
        let mapped_native_box_blur_input = false;
        #[cfg(target_endian = "little")]
        let mapped_native_grayscale_input = gpu.direct_primary_readback
            && (native_grayscale_rgb_input || native_grayscale_cmyk_input);
        #[cfg(not(target_endian = "little"))]
        let mapped_native_grayscale_input = false;
        gpu_log!(
            "[GPU] step=upload start output_only_constant={output_only_constant} native_luma16={native_luma16} native_luma16_convert={native_luma16_convert} native_luma16_paste={native_luma16_paste}"
        );
        gpu.upload_batch_input(
            img,
            &mut buffers,
            GpuBatchUploadOptions {
                mode,
                native_luma16,
                native_luma16_convert,
                native_luma16_paste,
                full_luma_putdata,
                output_only_constant,
                packed_la_pad,
                packed_native_byte_filter,
                packed_luma_colorize,
                packed_luma_convert,
                native_alpha_convert_input,
                packed_luma_transform_input,
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                packed_luma_pad,
                native_la_transform_output,
                native_rgb_compact_input,
                native_extract_band_channels,
                native_grayscale_rgb_input,
                native_grayscale_cmyk_input,
                native_sharpness_l_input,
                native_sharpness_la_input,
                native_sharpness_la_patch,
                native_sharpness_rgb_input,
                native_reduce_rgb_input,
                native_expand_channels,
                native_cover_input_channels,
                mapped_native_box_blur_input,
                mapped_native_grayscale_input,
            },
        )?;
        gpu_log!(
            "[GPU] step=upload done output_only_constant={output_only_constant} native_luma16={native_luma16}"
        );
        gpu_log!("[GPU] step=execute_batch_impl start");
        let mapped_native_input = if mapped_native_box_blur_input || mapped_native_grayscale_input {
            Some(buffers.mapped_native_input.take().ok_or_else(|| {
                PilError::InternalError(
                    "mapped native upload completed without retaining its input buffer".into(),
                )
            })?)
        } else {
            None
        };
        let initial_input = mapped_native_input.as_ref().map(|input| &input.buffer);
        let (
            final_is_a,
            final_w,
            final_h,
            readback,
            mut resource_telemetry,
            dispatch_count,
            native_expand_output,
        ) = gpu.execute_batch_impl(
            ops,
            &auxiliary_images,
            w,
            h,
            mcode,
            packed_luma_colorize,
            packed_luma_convert,
            packed_luma_transform_input,
            packed_luma_transform_output,
            native_rgb_compact_input,
            native_rgb_packed_output,
            native_la_transform_output,
            packed_luma_point,
            packed_luma_putdata,
            packed_luma_order_statistic,
            packed_native_byte_filter,
            packed_luma_pad,
            packed_la_pad,
            native_extract_band,
            native_grayscale_rgb_input,
            native_sharpness_l_input,
            native_sharpness_la_input,
            native_sharpness_rgb_input,
            native_reduce_rgb_input,
            native_expand_channels,
            native_cover_input_channels,
            native_cover_packed_output,
            mode,
            contrast_mean,
            f_resize_constant_bits,
            f_resize_box_copy_is_exact,
            f_resize_identity_is_exact,
            f_resize_box_average_is_exact,
            f_resize_dyadic_is_exact,
            f_resize_f64_is_exact,
            f_resize_f64_ordered_is_exact,
            initial_input,
            &mut buffers,
        )?;
        // The ordinary image route shares the same fused shader resolver as
        // explicit GPU batches, but its per-operation telemetry was not
        // carrying the fusion count. Keep the receipt aligned with the
        // admitted dispatch plan without counting an unrelated CPU fallback.
        resource_telemetry.fused_operation_count = resource_telemetry
            .fused_operation_count
            .max(gpu_fused_operation_count(ops, mode, (w, h)));
        if let Some(input) = mapped_native_input {
            buffers.mapped_native_input = Some(input);
        }
        gpu_log!(
            "[GPU] step=execute_batch_impl done final=({},{}) is_a={}",
            final_w,
            final_h,
            final_is_a
        );
        let output_layout = GpuBatchOutputLayout::from_operations(ops, img.color());
        let native_luma16_output = output_layout.output_color_type == Some(ColorType::L16);
        // All command buffers are submitted before registering the mapping. The
        // batch owns A/B exclusively until the mapped view is dropped and the
        // selected primary or staging buffer has been unmapped.
        gpu_log!("[GPU] step=readback start");
        let readback_buffer = readback.buffer(&buffers, final_is_a);
        let result = if let Some(dispatch) = native_cover_packed_output {
            gpu.readback_to_cover_native_bytes(final_w, final_h, dispatch, readback_buffer)?
        } else if native_rgb_packed_output {
            gpu.readback_to_native_channels(final_w, final_h, 3, readback_buffer)?
        } else if packed_la_pad
            || native_la_transform_output
            || (packed_native_byte_filter && matches!(mode, Some("LA")))
        {
            gpu.readback_to_native_channels(final_w, final_h, 2, readback_buffer)?
        } else if native_luma16 {
            gpu.readback_to_luma16(final_w, final_h, readback_buffer, mode)?
        } else if native_luma16_paste || native_luma16_output {
            gpu.readback_to_luma16_numeric(final_w, final_h, readback_buffer)?
        } else if output_layout.native_luma8_extract
            || output_layout.native_luma8_grayscale
            || output_layout.native_luma8_constant
            || packed_luma_transform_output
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || native_sharpness_l_input
        {
            gpu.readback_to_luma8(final_w, final_h, readback_buffer)?
        } else if native_sharpness_la_input {
            gpu.readback_to_luma_alpha_from_rgba_transport(final_w, final_h, readback_buffer)?
        } else {
            if native_expand_output {
                let channels = native_expand_channels.ok_or_else(|| {
                    PilError::InternalError(
                        "GPU native Expand output is missing its channel count".into(),
                    )
                })?;
                gpu.readback_to_native_channels(final_w, final_h, channels, readback_buffer)?
            } else {
                gpu.readback_to_image(final_w, final_h, readback_buffer, output_layout.native_rgb)?
            }
        };
        gpu_log!("[GPU] step=readback done");
        // map_async completion proves the command buffer no longer uses the
        // working images. Return successful working sets to the bounded pool;
        // every error path drops its buffers instead of risking reuse of an
        // in-flight or device-invalid resource.
        resource_telemetry = gpu_batch_resource_telemetry(
            resource_telemetry,
            &buffers,
            &readback,
            img,
            GpuBatchResourceTelemetryOptions {
                input_dimensions: (w, h),
                output_dimensions: (final_w, final_h),
                mode,
                output_layout: &output_layout,
                output_only_constant,
                full_luma_putdata,
                native_extract_band_channels,
                native_grayscale_rgb_input,
                native_rgb_compact_input,
                native_sharpness_l_input,
                packed_native_byte_filter,
                packed_la_pad,
                packed_luma_pad,
                native_sharpness_la_input,
                native_la_transform_output,
                native_sharpness_rgb_input,
                native_reduce_rgb_input,
                native_expand_channels,
                native_cover_input_channels,
                packed_luma_colorize,
                packed_luma_convert,
                native_alpha_convert_input,
                packed_luma_transform_input,
                packed_luma_point,
                packed_luma_putdata,
                packed_luma_order_statistic,
                native_luma16_convert,
                native_luma16_paste,
                native_cover_packed_output,
                native_rgb_packed_output,
                packed_luma_transform_output,
                native_expand_output,
            },
        )?;
        crate::compute::record_pipeline_resource_telemetry(resource_telemetry);
        crate::compute::record_pipeline_dispatch_count(dispatch_count);
        if let ReadbackTarget::Staging(staging) = readback {
            gpu.recycle_staging(staging);
        }
        gpu.recycle_buffers(buffers);
        if output_layout.native_rgb
            || native_la_transform_output
            || native_sharpness_l_input
            || native_sharpness_la_input
            || native_cover_packed_output.is_some()
            || native_luma16_output
            || packed_luma_pad
            || packed_la_pad
        {
            // The final native layout was decoded directly from the mapping;
            // applying mode preservation again would reallocate it.
            return Ok(result);
        }
        if output_layout.native_luma8_extract
            || output_layout.native_luma8_grayscale
            || output_layout.native_luma8_constant
            || packed_luma_transform_output
            || packed_luma_point
            || packed_luma_putdata
            || packed_luma_order_statistic
            || packed_native_byte_filter
            || packed_luma_pad
            || packed_la_pad
        {
            return Ok(result);
        }
        if let Some(mode) = output_layout.put_alpha_mode {
            return put_alpha_output(result, mode);
        }
        if let Some(ct) = output_layout.output_color_type {
            // Bypass preserve_mode — use the override color type directly.
            gpu_result_as_color_type(result, ct)
        } else {
            Ok(crate::image::preserve_mode(img, result))
        }
    }
}
