/// Find the largest safe vertical stack for one explicitly queued image batch.
/// The bound includes the median filter's duplicated edge rows and proves the
/// same byte capacity, workgroup grid, and shader-work limits used by normal
/// GPU execution before the caller allocates the stacked host image.
fn gpu_batch_group_limit_for_limits(
    op: &PipelineOp,
    logical_mode: &str,
    (width, height): (u32, u32),
    requested: usize,
    max_storage_buffer_binding_size: u32,
    max_buffer_size: u64,
    max_workgroups_per_dimension: u32,
) -> usize {
    if requested == 0 || width == 0 || height == 0 {
        return 0;
    }

    let multiply = matches!(op, PipelineOp::Multiply { .. });
    let native_brightness = matches!(op, PipelineOp::Brightness { .. });
    let native_invert = matches!(op, PipelineOp::Invert);
    let native_expand = matches!(op, PipelineOp::Expand { .. });
    let native_composite = matches!(
        op,
        PipelineOp::CompositeModule {
            mask_alpha: false,
            ..
        }
    );
    let masked_paste = matches!(
        op,
        PipelineOp::Paste {
            mask: Some(_),
            mask_alpha: false,
            ..
        }
    );
    let channels = if multiply
        || masked_paste
        || native_brightness
        || native_invert
        || native_expand
        || native_composite
    {
        match logical_mode {
            "L" => Some(1u64),
            "LA" => Some(2),
            "RGB" => Some(3),
            "RGBA" => Some(4),
            _ => None,
        }
    } else {
        Some(1)
    };
    let Some(channels) = channels else {
        return 0;
    };
    let halo = match op {
        PipelineOp::MedianFilter { size: 3 }
        | PipelineOp::MaxFilter { size: 3 }
        | PipelineOp::RankFilter { size: 3, rank: 1 } => 2u32,
        PipelineOp::ExtractBand { .. }
        | PipelineOp::Grayscale
        | PipelineOp::CompositeModule { .. }
        | PipelineOp::Invert
        | PipelineOp::Brightness { .. }
        | PipelineOp::Multiply { .. }
        | PipelineOp::Paste {
            mask: Some(_),
            mask_alpha: false,
            ..
        }
        | PipelineOp::Color3DLut { .. }
        | PipelineOp::Expand { .. } => 0,
        _ => return 0,
    };
    if native_expand && !cfg!(target_endian = "little") {
        return 0;
    }
    let Some(per_image_height) = height.checked_add(halo) else {
        return 0;
    };

    let is_safe = |count: usize| {
        let Ok(count) = u32::try_from(count) else {
            return false;
        };
        let (stacked_dimensions, output_dimensions) = if let PipelineOp::Expand { border, .. } = op
        {
            let Some(border_twice) = border.checked_mul(2) else {
                return false;
            };
            let Some(output_width) = width.checked_add(border_twice) else {
                return false;
            };
            let Some(output_height) = height
                .checked_add(border_twice)
                .and_then(|image_height| image_height.checked_mul(count))
            else {
                return false;
            };
            let Some(stacked_height) = output_height.checked_sub(border_twice) else {
                return false;
            };
            ((width, stacked_height), (output_width, output_height))
        } else {
            let Some(stacked_height) = per_image_height.checked_mul(count) else {
                return false;
            };
            let dimensions = (width, stacked_height);
            (dimensions, dimensions)
        };
        let input_pixels = u64::from(stacked_dimensions.0) * u64::from(stacked_dimensions.1);
        let output_pixels = u64::from(output_dimensions.0) * u64::from(output_dimensions.1);
        // The ordinary filter/extract layouts address one packed u32 per
        // pixel. Native-byte Multiply and Brightness address four stored
        // samples per word, so their device-buffer bounds use the source
        // mode's actual byte width.
        let buffer_words = if native_expand {
            let Some(source_bytes) = input_pixels.checked_mul(channels) else {
                return false;
            };
            let Some(output_bytes) = output_pixels.checked_mul(channels) else {
                return false;
            };
            if source_bytes == 0
                || source_bytes > u64::from(u32::MAX)
                || output_bytes == 0
                || output_bytes > u64::from(u32::MAX)
                || plan_native_expand_output_dispatch(output_bytes, max_workgroups_per_dimension)
                    .is_none()
            {
                return false;
            }
            // The ordinary pipeline's BufferPool is sized in pixels even
            // though native Expand transfers compact bytes. Keep its static
            // capacity and device binding bound here; the separate compact
            // readback planner above proves the native output word grid.
            input_pixels.max(output_pixels)
        } else if masked_paste {
            let Some(source_bytes) = input_pixels.checked_mul(channels) else {
                return false;
            };
            let Some(words) = source_bytes
                .div_ceil(4)
                .checked_add(input_pixels.div_ceil(4))
            else {
                return false;
            };
            words
        } else if multiply || native_brightness || native_invert || native_composite {
            let Some(sample_bytes) = input_pixels.checked_mul(channels) else {
                return false;
            };
            sample_bytes.div_ceil(4)
        } else {
            input_pixels
        };
        let Ok(buffer_capacity) = u32::try_from(buffer_words) else {
            return false;
        };
        if input_pixels == 0
            || output_pixels == 0
            || (native_expand
                && (input_pixels > u64::from(GPU_BUFFER_CAPACITY)
                    || output_pixels > u64::from(GPU_BUFFER_CAPACITY)))
            || (masked_paste && input_pixels > u64::from(GPU_BUFFER_CAPACITY))
            || buffer_words > u64::from(GPU_BUFFER_CAPACITY)
            || gpu_buffer_capacity_exceeds_limits(
                buffer_capacity,
                max_storage_buffer_binding_size,
                max_buffer_size,
            )
        {
            return false;
        }

        if masked_paste {
            // Use the same byte-aligned work-item and 1D/2D dispatch planner
            // as the native masked-Paste executor. The general batch grid
            // below is not equivalent for L/LA/RGB: those shaders reject a
            // flat workgroup count above the device's per-dimension limit.
            let Ok(bytes_per_pixel) = u8::try_from(channels) else {
                return false;
            };
            if plan_gpu_native_masked_byte_paste(
                width,
                stacked_dimensions.1,
                width,
                stacked_dimensions.1,
                width,
                stacked_dimensions.1,
                bytes_per_pixel,
                max_workgroups_per_dimension,
                max_storage_buffer_binding_size,
                max_buffer_size,
            )
            .is_none()
            {
                return false;
            }
        }

        if multiply || native_brightness || native_invert {
            let words = buffer_capacity;
            let columns = words.min(1024);
            let rows = words.div_ceil(columns);
            if columns.div_ceil(16) > max_workgroups_per_dimension
                || rows.div_ceil(16) > max_workgroups_per_dimension
            {
                return false;
            }
        }

        if native_composite
            && plan_native_composite_dispatch(buffer_capacity, max_workgroups_per_dimension)
                .is_none()
        {
            // The compact Composite shader uses its own 2D packed-word grid;
            // cap the group before allocating a stack the selected adapter
            // cannot dispatch.
            return false;
        }

        let ops = [op.clone()];
        !gpu_dispatch_dimensions_require_cpu(
            &ops,
            stacked_dimensions,
            max_workgroups_per_dimension,
            Some(logical_mode),
            false,
        ) && !gpu_shader_work_requires_cpu(
            op,
            stacked_dimensions,
            output_dimensions,
            Some(logical_mode),
        )
    };

    let mut low = 0usize;
    let mut high = requested;
    while low < high {
        let middle = low + (high - low).div_ceil(2);
        if is_safe(middle) {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
}

/// Return the safe queued-group length for the actual selected GPU adapter.
/// Adapter initialization or unavailable GPU support simply disables grouping.
pub(crate) fn gpu_batch_group_limit(
    op: &PipelineOp,
    logical_mode: &str,
    dimensions: (u32, u32),
    requested: usize,
) -> usize {
    let Ok(gpu) = GpuPool::ensure_init() else {
        return 0;
    };
    if gpu.failure_detail().is_some() {
        return 0;
    }
    let limits = gpu.device.limits();
    gpu_batch_group_limit_for_limits(
        op,
        logical_mode,
        dimensions,
        requested,
        limits.max_storage_buffer_binding_size,
        limits.max_buffer_size,
        limits.max_compute_workgroups_per_dimension,
    )
}


    /// Check an established byte mode without decoding or executing its image.
    /// Geometry lowering and typed sample proofs need the concrete source and
    /// remain unknown here, rather than rejecting potentially valid GPU work.
    pub(crate) fn validate_submission_known_mode(
        operation: &PipelineOp,
        mode: &str,
    ) -> Result<(), PilError> {
        if matches!(mode, "I" | "F" | "I;16" | "I;16L" | "I;16B" | "I;16N")
            || matches!(
                operation,
                PipelineOp::Resize { .. }
                    | PipelineOp::ResizeBoxed { .. }
                    | PipelineOp::Rotate { .. }
                    | PipelineOp::Thumbnail { .. }
                    | PipelineOp::Scale { .. }
                    | PipelineOp::Contain { .. }
                    | PipelineOp::Cover { .. }
                    | PipelineOp::Fit { .. }
                    | PipelineOp::Pad { .. }
                    | PipelineOp::Transform { .. }
            )
        {
            return Ok(());
        }
        let color = match mode {
            "1" | "L" | "P" => Some(crate::raster::ColorType::L8),
            "LA" | "La" | "PA" => Some(crate::raster::ColorType::La8),
            "RGB" | "HSV" | "YCbCr" | "LAB" => Some(crate::raster::ColorType::Rgb8),
            "RGBA" | "RGBa" | "RGBX" | "CMYK" => Some(crate::raster::ColorType::Rgba8),
            _ => None,
        };
        let other_is_lab = matches!(operation, PipelineOp::BlendModule { other, .. }
            if other.known_mode().as_deref() == Some("LAB"));
        if color
            .is_some_and(|color| gpu_operation_color_requires_cpu(operation, color, other_is_lab))
            || !gpu_logical_mode_is_supported(
                std::slice::from_ref(operation),
                None,
                Some(mode),
                None,
                false,
                false,
            )
        {
            return Err(PilError::NotImplementedError(format!(
                "GPU does not natively support {} in mode {mode}",
                registry::variant_key(operation)
            )));
        }
        Ok(())
    }

    /// Reject a known native-mode mismatch before a queued job is accepted.
    /// Content-dependent geometry and device allocation remain execution-time
    /// checks; this does not initialize a device or materialize a lazy source.
    pub(crate) fn validate_submission_mode(
        ops: &[PipelineOp],
        image: &DynamicImage,
        mode: &str,
    ) -> Result<(), PilError> {
        // The device gate sees lowered geometry. For example, Thumbnail on
        // P/1 becomes a nearest Resize; checking its public descriptor would
        // incorrectly reject an operation the existing GPU plan supports.
        let lowered = expand_gpu_geometry_ops(ops, image, image.dimensions(), Some(mode));
        let ops = lowered.as_slice();
        let f_constant = (mode == "F")
            .then(|| gpu_f_resize_constant_bits(ops, image, Some(mode)))
            .flatten();
        let f_pad_exact = mode == "F" && gpu_f_pad_f64_is_exact(ops, image, Some(mode));
        if !gpu_logical_mode_is_supported(
            ops,
            Some(image),
            Some(mode),
            f_constant,
            f_pad_exact,
            false,
        ) {
            return Err(PilError::NotImplementedError(format!(
                "GPU does not natively support this pipeline in mode {mode}"
            )));
        }
        if !gpu_batch_has_nonterminal_mode_change(ops) {
            for operation in ops {
                // Blend's gate may query a lazy auxiliary image's mode.
                // Leave that contextual check at execution unless the
                // operand is loaded, so validation never computes pixels.
                if let PipelineOp::BlendModule { other, .. } = operation
                    && !matches!(other.as_ref(), crate::Image::Loaded(_))
                {
                    continue;
                }
                if gpu_operation_mode_requires_cpu(operation, image) {
                    return Err(PilError::NotImplementedError(format!(
                        "GPU does not natively support {} in mode {mode}",
                        registry::variant_key(operation)
                    )));
                }
            }
        }
        Ok(())
    }


    #[test]
    fn explicit_gpu_batch_planner_caps_static_device_and_dispatch_boundaries() {
        let extract_band = PipelineOp::ExtractBand { index: 3 };
        let default_limits = (u32::MAX, u64::MAX, 65_535);

        // Expand adds its border to both axes, keeps mode-native byte
        // channels, and packs fill rows between source images. Bound input
        // and output storage plus the adapter's actual dispatch grid.
        let expand = PipelineOp::Expand {
            border: 1,
            fill: (11, 23, 37, 49),
        };
        for mode in ["L", "LA", "RGB", "RGBA"] {
            let cap =
                gpu_batch_group_limit_for_limits(&expand, mode, (4, 3), 100, 600, 600, 65_535);
            assert_eq!(cap, 5, "wrong bounded native Expand cap for {mode}");
            // BufferPool capacities are pixel-sized u32 storage even when
            // Expand's actual upload/readback remains a compact 1–4 byte
            // native image. The planner must respect that allocation size.
            let buffer_bytes = 6u64 * 5 * cap as u64 * 4;
            assert!(buffer_bytes <= 600);
            assert!(
                6u64 * 5 * (cap as u64 + 1) * 4 > 600,
                "next {mode} group must exceed the device buffer limit"
            );
        }
        // The border participates in output dispatch dimensions. One 14×14
        // source fits a one-workgroup device after expansion, while two do
        // not; the planner must cap the group before allocation.
        assert_eq!(
            gpu_batch_group_limit_for_limits(&expand, "L", (14, 14), 2, u32::MAX, u64::MAX, 1,),
            1
        );
        let overflowing_expand = PipelineOp::Expand {
            border: u32::MAX,
            fill: (0, 0, 0, 0),
        };
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &overflowing_expand,
                "RGBA",
                (4, 3),
                2,
                u32::MAX,
                u64::MAX,
                65_535,
            ),
            0
        );

        let cap = gpu_batch_group_limit_for_limits(
            &extract_band,
            "RGBA",
            (1024, 768),
            22,
            default_limits.0,
            default_limits.1,
            default_limits.2,
        );
        assert_eq!(cap, 21);
        assert!(1024u64 * 768 * cap as u64 <= u64::from(GPU_BUFFER_CAPACITY));
        assert!(1024u64 * 768 * (cap as u64 + 1) > u64::from(GPU_BUFFER_CAPACITY));

        // Grayscale reads one native source pixel per shader invocation and
        // emits one L sample. YCbCr uses three-byte input triples, but the
        // per-pixel u32 buffer bound remains conservative for both source and
        // output while retaining the logical mode needed for direct Y copy.
        let grayscale = PipelineOp::Grayscale;
        for mode in ["L", "LA", "RGB", "RGBA", "YCbCr"] {
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &grayscale,
                    mode,
                    (1024, 768),
                    22,
                    default_limits.0,
                    default_limits.1,
                    default_limits.2,
                ),
                21,
                "wrong bounded Grayscale cap for native mode {mode}"
            );
        }

        // 3x3 MaxFilter batches carry one clamped top and bottom row per
        // image. The group limit includes those halo rows in buffer sizing.
        let max_filter_cap = gpu_batch_group_limit_for_limits(
            &PipelineOp::MaxFilter { size: 3 },
            "RGB",
            (1024, 768),
            22,
            default_limits.0,
            default_limits.1,
            default_limits.2,
        );
        assert_eq!(max_filter_cap, 21);
        assert!(1024u64 * 770 * max_filter_cap as u64 <= u64::from(GPU_BUFFER_CAPACITY));
        assert!(1024u64 * 770 * (max_filter_cap as u64 + 1) > u64::from(GPU_BUFFER_CAPACITY));

        // Native-L RankFilter(3, rank=1) uses the same one-row-per-image
        // halos, and its L shader addresses one packed word per pixel.
        let rank_filter_cap = gpu_batch_group_limit_for_limits(
            &PipelineOp::RankFilter { size: 3, rank: 1 },
            "L",
            (1024, 768),
            22,
            default_limits.0,
            default_limits.1,
            default_limits.2,
        );
        assert_eq!(rank_filter_cap, 21);
        assert!(1024u64 * 770 * rank_filter_cap as u64 <= u64::from(GPU_BUFFER_CAPACITY));
        assert!(1024u64 * 770 * (rank_filter_cap as u64 + 1) > u64::from(GPU_BUFFER_CAPACITY));
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &PipelineOp::RankFilter { size: 3, rank: 0 },
                "L",
                (1024, 768),
                22,
                default_limits.0,
                default_limits.1,
                default_limits.2,
            ),
            0
        );

        let storage_limited_cap = gpu_batch_group_limit_for_limits(
            &extract_band,
            "RGBA",
            (1024, 768),
            8,
            16 * 1024 * 1024,
            16 * 1024 * 1024,
            65_535,
        );
        assert_eq!(storage_limited_cap, 5);

        // Multiply transports the stored bytes as independent packed samples:
        // its safe batch size depends on the native mode's byte width.
        for (mode, expected_cap) in [("L", 85), ("LA", 42), ("RGB", 28), ("RGBA", 21)] {
            let image = Image::new(1024, 768, mode, (0, 0, 0, 0)).unwrap();
            let multiply = PipelineOp::Multiply {
                other: Arc::new(image),
            };
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &multiply,
                    mode,
                    (1024, 768),
                    100,
                    default_limits.0,
                    default_limits.1,
                    default_limits.2,
                ),
                expected_cap,
                "wrong native-byte Multiply cap for {mode}"
            );
        }

        // Brightness batches use the same packed-byte transport sizes as
        // Multiply, while retaining the brightness shader's own dispatch.
        for (mode, expected_cap, expected_storage_cap) in
            [("L", 85, 21), ("LA", 42, 10), ("RGB", 28, 7)]
        {
            let brightness = PipelineOp::Brightness { factor: 0.5 };
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &brightness,
                    mode,
                    (1024, 768),
                    100,
                    default_limits.0,
                    default_limits.1,
                    default_limits.2,
                ),
                expected_cap,
                "wrong native-byte Brightness cap for {mode}"
            );
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &brightness,
                    mode,
                    (1024, 768),
                    100,
                    16 * 1024 * 1024,
                    16 * 1024 * 1024,
                    default_limits.2,
                ),
                expected_storage_cap,
                "wrong device-storage Brightness cap for {mode}"
            );
        }

        // ImageOps.invert shares the packed native-byte shader path, so a
        // stacked group is bounded by stored bytes and packed-word workgroups,
        // not by one logical work item per pixel.
        for (mode, expected_cap, expected_storage_cap) in [("L", 85, 21), ("RGB", 28, 7)] {
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &PipelineOp::Invert,
                    mode,
                    (1024, 768),
                    100,
                    default_limits.0,
                    default_limits.1,
                    default_limits.2,
                ),
                expected_cap,
                "wrong native-byte ImageOps.invert cap for {mode}"
            );
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &PipelineOp::Invert,
                    mode,
                    (1024, 768),
                    100,
                    16 * 1024 * 1024,
                    16 * 1024 * 1024,
                    default_limits.2,
                ),
                expected_storage_cap,
                "wrong device-storage ImageOps.invert cap for {mode}"
            );
        }

        let multiply_rgba = PipelineOp::Multiply {
            other: Arc::new(Image::new(1024, 768, "RGBA", (0, 0, 0, 0)).unwrap()),
        };
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &multiply_rgba,
                "RGBA",
                (1024, 768),
                8,
                16 * 1024 * 1024,
                16 * 1024 * 1024,
                65_535,
            ),
            5,
            "Multiply cap must respect the selected device storage limit"
        );

        // Masked Paste's shared source/mask binding contains both native
        // source samples and one L-mask byte per output pixel. Its group cap
        // must account for both buffers and use the same dispatch layout as
        // the native masked-Paste executor.
        for (mode, expected_unbounded, expected_storage_limited) in [
            ("L", 21, 10),
            ("LA", 10, 7),
            ("RGB", 21, 5),
            ("RGBA", 17, 4),
        ] {
            let source = Arc::new(Image::new(1024, 768, mode, (0, 0, 0, 0)).unwrap());
            let mask = Arc::new(Image::new(1024, 768, "L", (255, 0, 0, 0)).unwrap());
            let paste = PipelineOp::Paste {
                source,
                x: 0,
                y: 0,
                w: 1024,
                h: 768,
                mask: Some(mask),
                mask_alpha: false,
            };
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &paste,
                    mode,
                    (1024, 768),
                    22,
                    default_limits.0,
                    default_limits.1,
                    default_limits.2,
                ),
                expected_unbounded,
                "wrong static masked-Paste cap for {mode}"
            );
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &paste,
                    mode,
                    (1024, 768),
                    22,
                    16 * 1024 * 1024,
                    16 * 1024 * 1024,
                    default_limits.2,
                ),
                expected_storage_limited,
                "wrong device-storage masked-Paste cap for {mode}"
            );
        }

        let la_batch_height = 768 * 10;
        assert!(
            plan_gpu_native_masked_byte_paste(
                1024,
                la_batch_height,
                1024,
                la_batch_height,
                1024,
                la_batch_height,
                2,
                default_limits.2,
                default_limits.0,
                default_limits.1,
            )
            .is_some()
        );
        let over_limit_la_height = 768 * 11;
        assert!(
            plan_gpu_native_masked_byte_paste(
                1024,
                over_limit_la_height,
                1024,
                over_limit_la_height,
                1024,
                over_limit_la_height,
                2,
                default_limits.2,
                default_limits.0,
                default_limits.1,
            )
            .is_none()
        );

        // The batch planner must also honor adapters whose workgroup limit is
        // below the static default. L uses one flat workgroup per 256 pixels;
        // LA uses two bytes per pixel and reaches the boundary twice as early.
        let small_limit_paste = |mode: &str| {
            let source = Arc::new(Image::new(1000, 4, mode, (0, 0, 0, 0)).unwrap());
            let mask = Arc::new(Image::new(1000, 4, "L", (255, 0, 0, 0)).unwrap());
            PipelineOp::Paste {
                source,
                x: 0,
                y: 0,
                w: 1000,
                h: 4,
                mask: Some(mask),
                mask_alpha: false,
            }
        };
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &small_limit_paste("L"),
                "L",
                (1000, 4),
                5,
                u32::MAX,
                u64::MAX,
                63,
            ),
            4,
            "L Paste batch must cap at the native flat-dispatch boundary"
        );
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &small_limit_paste("LA"),
                "LA",
                (1000, 4),
                3,
                u32::MAX,
                u64::MAX,
                63,
            ),
            2,
            "LA Paste batch must cap at the native flat-dispatch boundary"
        );

        // A 1000×500 L image yields a generic 63×63 grid when two images are
        // stacked, but the packed-byte Multiply grid needs 64 groups in X.
        // The planner must reject it for an adapter capped at 63 groups.
        let multiply_l = PipelineOp::Multiply {
            other: Arc::new(Image::new(1000, 500, "L", (0, 0, 0, 0)).unwrap()),
        };
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &multiply_l,
                "L",
                (1000, 500),
                2,
                u32::MAX,
                u64::MAX,
                63,
            ),
            0,
            "packed-word dispatch must respect each adapter workgroup dimension"
        );
        let brightness_l = PipelineOp::Brightness { factor: 0.5 };
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &brightness_l,
                "L",
                (1000, 500),
                2,
                u32::MAX,
                u64::MAX,
                63,
            ),
            0,
            "Brightness packed-word dispatch must respect each adapter workgroup dimension"
        );
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &PipelineOp::Invert,
                "L",
                (1000, 500),
                2,
                u32::MAX,
                u64::MAX,
                63,
            ),
            0,
            "ImageOps.invert packed-word dispatch must respect each adapter workgroup dimension"
        );

        // Each 1x16384 image needs exactly an 8x8 ExtractBand grid. Two
        // stacked images would require 8x16 groups and exceed this adapter.
        assert_eq!(
            gpu_batch_group_limit_for_limits(
                &extract_band,
                "L",
                (1, 16_384),
                2,
                u32::MAX,
                u64::MAX,
                8,
            ),
            1
        );

        let median_filter = PipelineOp::MedianFilter { size: 3 };
        let median_cap = gpu_batch_group_limit_for_limits(
            &median_filter,
            "RGBA",
            (1024, 768),
            9,
            default_limits.0,
            default_limits.1,
            default_limits.2,
        );
        assert_eq!(median_cap, 8);
        let safe_height = 770u64 * median_cap as u64;
        let over_height = 770u64 * (median_cap as u64 + 1);
        assert!(1024 * safe_height * 324 <= super::MAX_GPU_SHADER_WORK_ITEMS);
        assert!(1024 * over_height * 324 > super::MAX_GPU_SHADER_WORK_ITEMS);
    }


    #[test]
    fn composite_batch_planner_respects_each_auxiliary_buffer_limit() {
        let default_limits = (u32::MAX, u64::MAX, 65_535);
        for (mode, channels, default_cap, storage_cap) in [
            ("L", 1u64, 22u32, 21),
            ("LA", 2, 22, 10),
            ("RGB", 3, 22, 7),
            ("RGBA", 4, 21, 5),
        ] {
            let composite = PipelineOp::CompositeModule {
                other: Arc::new(Image::new(1024, 768, mode, (0, 0, 0, 0)).unwrap()),
                mask: Arc::new(Image::new(1024, 768, "L", (0, 0, 0, 0)).unwrap()),
                mask_alpha: false,
            };
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &composite,
                    mode,
                    (1024, 768),
                    22,
                    default_limits.0,
                    default_limits.1,
                    default_limits.2,
                ),
                usize::try_from(default_cap).expect("small GPU group cap fits usize"),
                "wrong native Composite workgroup cap for {mode}"
            );
            assert_eq!(
                gpu_batch_group_limit_for_limits(
                    &composite,
                    mode,
                    (1024, 768),
                    22,
                    16 * 1024 * 1024,
                    16 * 1024 * 1024,
                    default_limits.2,
                ),
                usize::try_from(storage_cap).expect("small GPU group cap fits usize"),
                "Composite must respect the selected device's storage-binding limit for {mode}"
            );

            let bytes_per_image = 1024u64 * 768 * channels;
            let word_count = bytes_per_image.div_ceil(4);
            assert!(
                super::plan_native_composite_dispatch(
                    u32::try_from(word_count * u64::from(default_cap))
                        .expect("default Composite batch fits u32 words"),
                    default_limits.2,
                )
                .is_some(),
                "safe {mode} Composite group must fit the adapter's 2D dispatch grid"
            );
        }
    }


        if crate::compute::gpu_native_execution_required() {
            return Err(PilError::NotImplementedError(format!(
                "native GPU pipeline cannot execute this image: {reason}"
            )));
        }

        if crate::compute::gpu_native_execution_required()
            && ops
                .iter()
                .any(crate::compute::pool_gpu_operation_uses_host_pixels)
        {
            return Err(PilError::NotImplementedError(
                "native GPU pipeline cannot copy host-generated operation results".into(),
            ));
        }
