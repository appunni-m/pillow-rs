use super::*;

pub(super) enum BatchContractPreflight {
    Continue { empty_two_input_noop: bool },
    Return(DynamicImage),
}

impl GpuPool {
    pub(super) fn preflight_batch_contract(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        f_resize_constant_bits: Option<u32>,
        f_pad_f64_is_exact: bool,
    ) -> Result<BatchContractPreflight, PilError> {
        if let Some(image) = self.preflight_batch_layout(
            ops,
            image,
            mode,
            allow_cpu_fallback,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
        )? {
            return Ok(BatchContractPreflight::Return(image));
        }

        self.preflight_batch_operations(ops, image, mode, allow_cpu_fallback)
    }

    fn preflight_batch_layout(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
        f_resize_constant_bits: Option<u32>,
        f_pad_f64_is_exact: bool,
    ) -> Result<Option<DynamicImage>, PilError> {
        // GPU shaders consume the packed L/LA/RGB/RGBA representation. P and
        // PA are safe for raw channel-wise Chops because the core exposes
        // their index/(index, alpha) samples as Luma8/LumaA8 buffers; no
        // palette expansion is involved. CMYK is additionally safe for a
        // brightness/color-saturation batch because its C/M/Y/K bytes share
        // the four-byte transport and those shaders explicitly process K.
        // Grayscale has a dedicated shader branch that fuses CMYK-to-RGB
        // arithmetic with luma. EffectSpread gathers complete packed words,
        // so it is safe for all modes backed by ordinary byte transport.
        let logical_mode_supported = gpu_logical_mode_is_supported(
            ops,
            Some(image),
            mode,
            f_resize_constant_bits,
            f_pad_f64_is_exact,
            true,
        );
        if !logical_mode_supported {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to exact host semantic control: logical layout"
            );
            return self
                .preflight_failure(
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "GPU logical-mode operation contract is not proven",
                )
                .map(Some);
        }

        // A palette-index PutPixel stores an index in the same Luma8 layout
        // as ordinary L. The logical mode must keep that index from being
        // treated as a visible luma sample; PA uses its separate alpha layout.
        if ops.iter().any(|op| {
            matches!(
                op,
                PipelineOp::PutPixel {
                    palette_index: true,
                    ..
                }
            )
        }) && mode != Some("P")
        {
            return self
                .preflight_failure(
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "palette index requires P logical mode",
                )
                .map(Some);
        }

        // The uniform mode word describes the source layout for the whole
        // batch. A non-terminal mode change would make later shaders decode
        // the new pixels with the old layout.
        if gpu_batch_has_nonterminal_mode_change(ops) {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: mode-changing op is not terminal"
            );
            return self
                .preflight_failure(
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "non-terminal mode change",
                )
                .map(Some);
        }

        if !gpu_image_layout_is_supported(image)
            && !gpu_luma16_geometry_is_supported(ops, image, mode)
            && !gpu_luma16_convert_is_supported(ops, image)
            && !gpu_luma16_paste_is_supported(ops, image)
        {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsupported native pixel layout"
            );
            return self
                .preflight_failure(
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "unsupported native pixel layout",
                )
                .map(Some);
        }

        Ok(None)
    }

    fn preflight_batch_operations(
        &self,
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: Option<&str>,
        allow_cpu_fallback: bool,
    ) -> Result<BatchContractPreflight, PilError> {
        for op in ops {
            let i_mode_filter = mode == Some("I")
                && matches!(
                    op,
                    PipelineOp::Filter3x3 { .. } | PipelineOp::Filter5x5 { .. }
                )
                && (gpu_int_filter_is_supported(ops, image)
                    || gpu_int_filter_resize_chain_is_supported(ops, image));
            if !registry::gpu_supports(op)? && !i_mode_filter {
                return self
                    .preflight_failure(
                        ops,
                        image,
                        mode,
                        allow_cpu_fallback,
                        "no valid single-dispatch shader contract",
                    )
                    .map(BatchContractPreflight::Return);
            }
        }

        // Keep this guard at execution time as well as in `supports`: explicit
        // backend selection and future callers must not bypass shader bounds.
        if let Err(error) = validate_gpu_operations(ops, image, mode) {
            let reason = error.to_string();
            return self
                .preflight_failure(ops, image, mode, allow_cpu_fallback, &reason)
                .map(BatchContractPreflight::Return);
        }

        // Check dimensions before resolving nested images or initializing a
        // device. Empty two-input no-ops are the established exception.
        let empty_two_input_noop = gpu_empty_two_input_batch_is_noop(ops, image);
        if gpu_dimensions_require_cpu(ops, image, mode) && !empty_two_input_noop {
            gpu_log!(
                "[GPU] dispatch preflight routed batch to CPU: unsafe primary image dimensions"
            );
            return self
                .preflight_failure(
                    ops,
                    image,
                    mode,
                    allow_cpu_fallback,
                    "unsafe primary image dimensions",
                )
                .map(BatchContractPreflight::Return);
        }

        Ok(BatchContractPreflight::Continue {
            empty_two_input_noop,
        })
    }
}
