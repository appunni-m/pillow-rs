# Repository map

Use this map to locate maintained source and generators. [Architecture](ARCHITECTURE.md)
explains ownership, and [implementation structure](IMPLEMENTATION_STRUCTURE.md)
defines the shared path layout. [Contributing](../CONTRIBUTING.md) explains the
workflow.

| Path | Responsibility |
| --- | --- |
| `pillow-rs/` | Rust image model, operations, drawing, pipelines, font integration |
| `pillow-rs-py/` | Python `PIL` facade and PyO3 conversion/delegation |
| `pillow-rs-parallel/` | Optional PyPI companion wheel for the Rayon-backed Parallel CPU extension |
| `pillow-rs-js/` | WASM bindings, npm runtime entry points, Node/browser runners |
| `pillow-rs/tests/fixtures/manifest.yaml` | Selected public contract and indexed input files |
| `scripts/` | Maintained generators, runners, receipt validators, release and documentation tools |
| `docs/` | Public guides and explicitly versioned generated evidence |
| `.github/workflows/` | Validation, benchmark, Pages, and package-release automation |
| `scripts/data/migration-authority-v0.yaml` | Hash-pinned provenance used to regenerate the public inventory |

The root Makefile owns the pinned fontdone checkout under `build/fontdone-src/`.
Contribute to fontdone in its own repository. Its FreeType checkout is a read-only
oracle; runtime implementation remains Rust. Dependency versions and revisions
are pinned in Cargo metadata and the root Makefile.

## Generated source inventory

Run `make repo-map-update` after staging new or removed maintained files, then
`make repo-map-check`. The tree excludes fixture payloads, generated reports,
private compatibility aliases, vendored oracles, and build/package outputs.
It is a navigation aid, not an API-support or coverage claim.

<!-- BEGIN GENERATED CODE TREE -->
```text
.
|-- .claude/
|   `-- skills/
|       |-- compute-backend/
|       |   |-- SKILL.md
|       |   |-- examples/
|       |   |   |-- canonical_shader.wgsl
|       |   |   `-- pool_gpu_module.rs
|       |   `-- references/
|       |       |-- backend-architecture.md
|       |       `-- shader-migration-guide.md
|       |-- fix-pil-parity/
|       |   |-- SKILL.md
|       |   `-- references/
|       |       `-- debug-patterns.md
|       `-- freetype-parity/
|           `-- SKILL.md
|-- .github/
|   `-- workflows/
|       |-- benchmark.yml
|       |-- ci.yml
|       |-- docs.yml
|       `-- release.yml
|-- AGENT.md
|-- AGENTS.md
|-- CHANGELOG.md
|-- CLAUDE.md
|-- CONTRIBUTING.md
|-- Cargo.lock
|-- Cargo.toml
|-- Makefile
|-- README.md
|-- deny.toml
|-- docs/
|   `-- REPO_MAP.md
|-- docs.mk
|-- documentation.json
|-- mkdocs.yml
|-- pillow-rs/
|   |-- Cargo.toml
|   |-- src/
|   |   |-- checked_dims.rs
|   |   |-- color.rs
|   |   |-- color_grayscale_allocation_tests.rs
|   |   |-- compute/
|   |   |   |-- automatic_segments.rs
|   |   |   |-- backend_state.rs
|   |   |   |-- eager.rs
|   |   |   |-- execution/
|   |   |   |   `-- terminals.rs
|   |   |   |-- execution.rs
|   |   |   |-- mod.rs
|   |   |   |-- phase_telemetry.rs
|   |   |   |-- pool_cpu/
|   |   |   |   |-- mod.rs
|   |   |   |   `-- ops/
|   |   |   |       |-- chops.rs
|   |   |   |       |-- color.rs
|   |   |   |       |-- color_convert_allocation_tests.rs
|   |   |   |       |-- draw.rs
|   |   |   |       |-- effects.rs
|   |   |   |       |-- enhance.rs
|   |   |   |       |-- filter/
|   |   |   |       |   |-- kernels/
|   |   |   |       |   |   |-- blur_line_steps.rs
|   |   |   |       |   |   |-- i32_cross_smooth.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- rgb_blur_ring.rs
|   |   |   |       |   |-- uniform_box.rs
|   |   |   |       |   `-- uniform_box_tests.rs
|   |   |   |       |-- filter.rs
|   |   |   |       |-- geometry/
|   |   |   |       |   |-- float_resize.rs
|   |   |   |       |   |-- float_resize_tests.rs
|   |   |   |       |   |-- kernels/
|   |   |   |       |   |   `-- transpose_rgba/
|   |   |   |       |   |       `-- x86_64.rs
|   |   |   |       |   |-- native_transpose.rs
|   |   |   |       |   `-- transpose.rs
|   |   |   |       |-- geometry.rs
|   |   |   |       |-- grayscale.rs
|   |   |   |       |-- imageops.rs
|   |   |   |       `-- mod.rs
|   |   |   |-- pool_gpu/
|   |   |   |   |-- auxiliary_images.rs
|   |   |   |   |-- batch_arena_upload.rs
|   |   |   |   |-- batch_auxiliary_ranges.rs
|   |   |   |   |-- batch_encoding/
|   |   |   |   |   `-- dispatch.rs
|   |   |   |   |-- batch_encoding.rs
|   |   |   |   |-- batch_estimates.rs
|   |   |   |   |-- batch_normalization.rs
|   |   |   |   |-- batch_operation_planning.rs
|   |   |   |   |-- batch_output.rs
|   |   |   |   |-- batch_parameters.rs
|   |   |   |   |-- batch_pipeline.rs
|   |   |   |   |-- batch_preflight.rs
|   |   |   |   |-- batch_preparation.rs
|   |   |   |   |-- batch_resize_coefficients.rs
|   |   |   |   |-- batch_segmentation.rs
|   |   |   |   |-- batch_telemetry.rs
|   |   |   |   |-- batch_upload.rs
|   |   |   |   |-- fusion.rs
|   |   |   |   |-- geometry/
|   |   |   |   |   `-- native_execution.rs
|   |   |   |   |-- geometry.rs
|   |   |   |   |-- layout_admission.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- mode.rs
|   |   |   |   |-- native_composite.rs
|   |   |   |   |-- native_dispatch.rs
|   |   |   |   |-- native_hsv_pad.rs
|   |   |   |   |-- native_input.rs
|   |   |   |   |-- native_paste/
|   |   |   |   |   |-- execution.rs
|   |   |   |   |   `-- masked_execution.rs
|   |   |   |   |-- native_paste.rs
|   |   |   |   |-- operation_admission.rs
|   |   |   |   |-- operation_support.rs
|   |   |   |   |-- point_ops/
|   |   |   |   |   |-- native_alpha.rs
|   |   |   |   |   `-- native_byte_op.rs
|   |   |   |   |-- point_ops.rs
|   |   |   |   |-- resize/
|   |   |   |   |   |-- compact_box.rs
|   |   |   |   |   |-- float_exactness.rs
|   |   |   |   |   |-- float_fast_paths.rs
|   |   |   |   |   |-- integer.rs
|   |   |   |   |   |-- integer_arithmetic.rs
|   |   |   |   |   |-- luma16.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   `-- ordered_float.rs
|   |   |   |   |-- resource_pool.rs
|   |   |   |   |-- rgb_merge.rs
|   |   |   |   |-- shaders/
|   |   |   |   |   |-- add.wgsl
|   |   |   |   |   |-- add_modulo.wgsl
|   |   |   |   |   |-- alpha_composite.wgsl
|   |   |   |   |   |-- alpha_composite_mirror.wgsl
|   |   |   |   |   |-- autocontrast.wgsl
|   |   |   |   |   |-- autocontrast_cutoff.wgsl
|   |   |   |   |   |-- autocontrast_remap.wgsl
|   |   |   |   |   |-- blend_module.wgsl
|   |   |   |   |   |-- box_blur.wgsl
|   |   |   |   |   |-- box_blur_h.wgsl
|   |   |   |   |   |-- box_blur_h_la_packed.wgsl
|   |   |   |   |   |-- box_blur_h_luma_packed.wgsl
|   |   |   |   |   |-- box_blur_h_rgb_packed.wgsl
|   |   |   |   |   |-- box_blur_hv_luma_radius_one_packed.wgsl
|   |   |   |   |   |-- box_blur_hv_rgb_radius_one_packed.wgsl
|   |   |   |   |   |-- box_blur_v.wgsl
|   |   |   |   |   |-- box_blur_v_la_packed.wgsl
|   |   |   |   |   |-- box_blur_v_luma_packed.wgsl
|   |   |   |   |   |-- box_blur_v_rgb_packed.wgsl
|   |   |   |   |   |-- brightness.wgsl
|   |   |   |   |   |-- brightness_native.wgsl
|   |   |   |   |   |-- color_3dlut.wgsl
|   |   |   |   |   |-- color_saturation.wgsl
|   |   |   |   |   |-- colorize.wgsl
|   |   |   |   |   |-- composite_module.wgsl
|   |   |   |   |   |-- composite_native.wgsl
|   |   |   |   |   |-- constant.wgsl
|   |   |   |   |   |-- contain.wgsl
|   |   |   |   |   |-- contrast.wgsl
|   |   |   |   |   |-- convert.wgsl
|   |   |   |   |   |-- cover.wgsl
|   |   |   |   |   |-- crop.wgsl
|   |   |   |   |   |-- crop_border.wgsl
|   |   |   |   |   |-- crop_native_rgb.wgsl
|   |   |   |   |   |-- darker.wgsl
|   |   |   |   |   |-- difference.wgsl
|   |   |   |   |   |-- draw.wgsl
|   |   |   |   |   |-- duplicate.wgsl
|   |   |   |   |   |-- effect_noise.wgsl
|   |   |   |   |   |-- effect_spread.wgsl
|   |   |   |   |   |-- equalize.wgsl
|   |   |   |   |   |-- equalize_cdf.wgsl
|   |   |   |   |   |-- equalize_histogram.wgsl
|   |   |   |   |   |-- equalize_histogram_l_packed.wgsl
|   |   |   |   |   |-- equalize_histogram_rgb_packed.wgsl
|   |   |   |   |   |-- equalize_remap.wgsl
|   |   |   |   |   |-- equalize_remap_l_packed.wgsl
|   |   |   |   |   |-- eval.wgsl
|   |   |   |   |   |-- expand.wgsl
|   |   |   |   |   |-- extract_band.wgsl
|   |   |   |   |   |-- filter_3x3.wgsl
|   |   |   |   |   |-- filter_3x3_la_binomial_packed.wgsl
|   |   |   |   |   |-- filter_5x5.wgsl
|   |   |   |   |   |-- filter_5x5_la_packed.wgsl
|   |   |   |   |   |-- filter_5x5_luma_packed.wgsl
|   |   |   |   |   |-- fit.wgsl
|   |   |   |   |   |-- flip.wgsl
|   |   |   |   |   |-- flip_native_byte_rows.wgsl
|   |   |   |   |   |-- gaussian_blur.wgsl
|   |   |   |   |   |-- getprojection_atomic.wgsl
|   |   |   |   |   |-- getprojection_cmyk.wgsl
|   |   |   |   |   |-- grayscale.wgsl
|   |   |   |   |   |-- hard_light.wgsl
|   |   |   |   |   |-- histogram_clear.wgsl
|   |   |   |   |   |-- invert.wgsl
|   |   |   |   |   |-- invert_chops.wgsl
|   |   |   |   |   |-- lighter.wgsl
|   |   |   |   |   |-- logical_and.wgsl
|   |   |   |   |   |-- logical_or.wgsl
|   |   |   |   |   |-- logical_xor.wgsl
|   |   |   |   |   |-- max_filter.wgsl
|   |   |   |   |   |-- max_filter_3x3_la_packed.wgsl
|   |   |   |   |   |-- max_filter_3x3_luma_packed.wgsl
|   |   |   |   |   |-- max_filter_3x3_rgb_packed.wgsl
|   |   |   |   |   |-- median_filter.wgsl
|   |   |   |   |   |-- median_filter_3x3.wgsl
|   |   |   |   |   |-- median_filter_3x3_la_packed.wgsl
|   |   |   |   |   |-- median_filter_3x3_luma_packed.wgsl
|   |   |   |   |   |-- median_filter_3x3_rgb_packed.wgsl
|   |   |   |   |   |-- merge.wgsl
|   |   |   |   |   |-- min_filter.wgsl
|   |   |   |   |   |-- min_filter_3x3_la_packed.wgsl
|   |   |   |   |   |-- min_filter_3x3_luma_packed.wgsl
|   |   |   |   |   |-- min_filter_3x3_rgb_packed.wgsl
|   |   |   |   |   |-- mirror.wgsl
|   |   |   |   |   |-- multiply.wgsl
|   |   |   |   |   |-- multiply_screen.wgsl
|   |   |   |   |   |-- offset.wgsl
|   |   |   |   |   |-- overlay.wgsl
|   |   |   |   |   |-- pad.wgsl
|   |   |   |   |   |-- pad_hsv_native_rgb.wgsl
|   |   |   |   |   |-- pad_la_packed.wgsl
|   |   |   |   |   |-- pad_luma_packed.wgsl
|   |   |   |   |   |-- paste.wgsl
|   |   |   |   |   |-- paste_native_bytes.wgsl
|   |   |   |   |   |-- paste_native_masked_l.wgsl
|   |   |   |   |   |-- paste_native_masked_la.wgsl
|   |   |   |   |   |-- paste_native_masked_pa.wgsl
|   |   |   |   |   |-- paste_native_masked_rgb.wgsl
|   |   |   |   |   |-- paste_native_masked_rgba.wgsl
|   |   |   |   |   |-- paste_native_rgb_to_rgba.wgsl
|   |   |   |   |   |-- point_luma_packed.wgsl
|   |   |   |   |   |-- point_op.wgsl
|   |   |   |   |   |-- point_rgb_packed.wgsl
|   |   |   |   |   |-- posterize.wgsl
|   |   |   |   |   |-- put_alpha.wgsl
|   |   |   |   |   |-- put_alpha_data.wgsl
|   |   |   |   |   |-- put_alpha_data_native_rgb.wgsl
|   |   |   |   |   |-- put_alpha_data_native_rgba.wgsl
|   |   |   |   |   |-- put_alpha_native_la.wgsl
|   |   |   |   |   |-- put_alpha_native_rgb.wgsl
|   |   |   |   |   |-- put_data.wgsl
|   |   |   |   |   |-- put_data_luma_packed.wgsl
|   |   |   |   |   |-- put_pixel.wgsl
|   |   |   |   |   |-- rank_filter.wgsl
|   |   |   |   |   |-- rank_filter_3x3_la_packed.wgsl
|   |   |   |   |   |-- rank_filter_3x3_luma_packed.wgsl
|   |   |   |   |   |-- rank_filter_9x9_luma_packed.wgsl
|   |   |   |   |   |-- rank_filter_9x9_luma_row_packed.wgsl
|   |   |   |   |   |-- reduce.wgsl
|   |   |   |   |   |-- relocate_native_bytes.wgsl
|   |   |   |   |   |-- remap_palette.wgsl
|   |   |   |   |   |-- resize_bilinear.wgsl
|   |   |   |   |   |-- resize_convolution_h.wgsl
|   |   |   |   |   |-- resize_convolution_v.wgsl
|   |   |   |   |   |-- resize_convolution_v_pad_rgbx.wgsl
|   |   |   |   |   |-- resize_nearest.wgsl
|   |   |   |   |   |-- rgb_to_lab.wgsl
|   |   |   |   |   |-- rotate.wgsl
|   |   |   |   |   |-- scale.wgsl
|   |   |   |   |   |-- screen.wgsl
|   |   |   |   |   |-- sharpness.wgsl
|   |   |   |   |   |-- sharpness_l_native.wgsl
|   |   |   |   |   |-- sharpness_la_native.wgsl
|   |   |   |   |   |-- sharpness_rgb_native.wgsl
|   |   |   |   |   |-- soft_light.wgsl
|   |   |   |   |   |-- solarize.wgsl
|   |   |   |   |   |-- subtract.wgsl
|   |   |   |   |   |-- subtract_modulo.wgsl
|   |   |   |   |   |-- thumbnail.wgsl
|   |   |   |   |   |-- transform.wgsl
|   |   |   |   |   |-- transform_geometry.wgsl
|   |   |   |   |   |-- transform_l_nearest_packed.wgsl
|   |   |   |   |   |-- transform_la_nearest_packed.wgsl
|   |   |   |   |   |-- transform_rgb_nearest_packed.wgsl
|   |   |   |   |   |-- transpose.wgsl
|   |   |   |   |   |-- transpose_rgb.wgsl
|   |   |   |   |   `-- transpose_rgb_tiled.wgsl
|   |   |   |   |-- stream.rs
|   |   |   |   |-- tests/
|   |   |   |   |   |-- affine_geometry.rs
|   |   |   |   |   |-- dispatch_planners.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   |-- native_cover.rs
|   |   |   |   |   |-- native_effects.rs
|   |   |   |   |   |-- native_filters.rs
|   |   |   |   |   |-- native_layout_planners.rs
|   |   |   |   |   |-- native_luma.rs
|   |   |   |   |   |-- native_luma_transfer.rs
|   |   |   |   |   |-- native_operations.rs
|   |   |   |   |   |-- native_padding.rs
|   |   |   |   |   |-- native_paste_execution.rs
|   |   |   |   |   |-- native_paste_planning.rs
|   |   |   |   |   |-- native_transform.rs
|   |   |   |   |   |-- palette_projective.rs
|   |   |   |   |   |-- pipeline_fusion.rs
|   |   |   |   |   |-- point_merge.rs
|   |   |   |   |   |-- projective_filtered.rs
|   |   |   |   |   |-- projective_nearest_execution.rs
|   |   |   |   |   |-- projective_nearest_proofs.rs
|   |   |   |   |   |-- putalpha_planners.rs
|   |   |   |   |   |-- readback.rs
|   |   |   |   |   |-- resize_compact_box.rs
|   |   |   |   |   |-- resize_float_execution.rs
|   |   |   |   |   |-- resize_float_proofs.rs
|   |   |   |   |   |-- resize_geometry.rs
|   |   |   |   |   |-- resize_integer.rs
|   |   |   |   |   |-- resize_ordered_float.rs
|   |   |   |   |   |-- resize_ordered_taps.rs
|   |   |   |   |   |-- resize_streaming.rs
|   |   |   |   |   |-- resize_thumbnail.rs
|   |   |   |   |   |-- resource_pool.rs
|   |   |   |   |   |-- rgb_compact.rs
|   |   |   |   |   |-- transpose_execution.rs
|   |   |   |   |   |-- transpose_planning.rs
|   |   |   |   |   |-- typed_geometry.rs
|   |   |   |   |   |-- typed_resize.rs
|   |   |   |   |   `-- upload_reuse.rs
|   |   |   |   |-- thumbnail.rs
|   |   |   |   |-- transform/
|   |   |   |   |   |-- geometry.rs
|   |   |   |   |   |-- projective_admission.rs
|   |   |   |   |   |-- projective_exactness.rs
|   |   |   |   |   `-- safety.rs
|   |   |   |   `-- transform.rs
|   |   |   |-- pool_simd/
|   |   |   |   |-- mod.rs
|   |   |   |   |-- ops/
|   |   |   |   |   |-- adapters/
|   |   |   |   |   |   |-- alpha_composite.rs
|   |   |   |   |   |   |-- box_blur/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- la.rs
|   |   |   |   |   |   |   |   |-- luma.rs
|   |   |   |   |   |   |   |   |-- rgb.rs
|   |   |   |   |   |   |   |   |-- rgba.rs
|   |   |   |   |   |   |   |   |-- rgba_avx2.rs
|   |   |   |   |   |   |   |   `-- rgba_neon.rs
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- box_blur.rs
|   |   |   |   |   |   |-- chops/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   `-- fused_multiply_screen.rs
|   |   |   |   |   |   |   `-- kernels.rs
|   |   |   |   |   |   |-- chops.rs
|   |   |   |   |   |   |-- color3dlut.rs
|   |   |   |   |   |   |-- colorize.rs
|   |   |   |   |   |   |-- convert/
|   |   |   |   |   |   |   |-- cmyk.rs
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- color_space.rs
|   |   |   |   |   |   |   |   |-- dispatch.rs
|   |   |   |   |   |   |   |   |-- layout.rs
|   |   |   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |   |   |-- rgba_rgb_neon.rs
|   |   |   |   |   |   |   |   `-- rgba_rgb_ssse3.rs
|   |   |   |   |   |   |   |-- tests/
|   |   |   |   |   |   |   |   `-- cmyk_packing.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- convert.rs
|   |   |   |   |   |   |-- crop.rs
|   |   |   |   |   |   |-- draw/
|   |   |   |   |   |   |   `-- kernels/
|   |   |   |   |   |   |       |-- mod.rs
|   |   |   |   |   |   |       |-- shapes.rs
|   |   |   |   |   |   |       `-- spans.rs
|   |   |   |   |   |   |-- draw.rs
|   |   |   |   |   |   |-- duplicate/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- duplicate.rs
|   |   |   |   |   |   |-- effects/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- effects.rs
|   |   |   |   |   |   |-- enhance/
|   |   |   |   |   |   |   |-- point.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- enhance.rs
|   |   |   |   |   |   |-- expand/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- expand.rs
|   |   |   |   |   |   |-- extract_band.rs
|   |   |   |   |   |   |-- filter/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- byte_layouts.rs
|   |   |   |   |   |   |   |   |-- i32_three_by_three.rs
|   |   |   |   |   |   |   |   |-- l.rs
|   |   |   |   |   |   |   |   |-- la.rs
|   |   |   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |   |   |-- rgb_blur_ring.rs
|   |   |   |   |   |   |   |   |-- three_by_three.rs
|   |   |   |   |   |   |   |   |-- uniform_rgb.rs
|   |   |   |   |   |   |   |   `-- unsharp.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- filter.rs
|   |   |   |   |   |   |-- flip.rs
|   |   |   |   |   |   |-- gaussian_blur/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- la.rs
|   |   |   |   |   |   |   |   `-- rgba_uniform.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- gaussian_blur.rs
|   |   |   |   |   |   |-- gradient.rs
|   |   |   |   |   |   |-- grayscale/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- channels.rs
|   |   |   |   |   |   |   |   |-- cmyk.rs
|   |   |   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |   |   |-- rgb_avx2.rs
|   |   |   |   |   |   |   |   |-- ycbcr.rs
|   |   |   |   |   |   |   |   `-- ycbcr_ssse3.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- grayscale.rs
|   |   |   |   |   |   |-- histogram.rs
|   |   |   |   |   |   |-- layout.rs
|   |   |   |   |   |   |-- lut/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- lut.rs
|   |   |   |   |   |   |-- mandelbrot.rs
|   |   |   |   |   |   |-- median_filter/
|   |   |   |   |   |   |   |-- extrema.rs
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- median_filter.rs
|   |   |   |   |   |   |-- merge/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- merge.rs
|   |   |   |   |   |   |-- module_blend.rs
|   |   |   |   |   |   |-- native_arithmetic.rs
|   |   |   |   |   |   |-- native_bytes.rs
|   |   |   |   |   |   |-- neighborhood.rs
|   |   |   |   |   |   |-- offset/
|   |   |   |   |   |   |   `-- kernels/
|   |   |   |   |   |   |       |-- byte.rs
|   |   |   |   |   |   |       `-- luma16.rs
|   |   |   |   |   |   |-- offset.rs
|   |   |   |   |   |   |-- pad/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- pad.rs
|   |   |   |   |   |   |-- paste/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- apply.rs
|   |   |   |   |   |   |   |   |-- blend.rs
|   |   |   |   |   |   |   |   |-- luma.rs
|   |   |   |   |   |   |   |   |-- luma16.rs
|   |   |   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |   |   `-- rgb_cmyk.rs
|   |   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |   |-- plan.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- pipeline.rs
|   |   |   |   |   |   |-- projection.rs
|   |   |   |   |   |   |-- put_alpha/
|   |   |   |   |   |   |   `-- kernels.rs
|   |   |   |   |   |   |-- put_alpha.rs
|   |   |   |   |   |   |-- put_data.rs
|   |   |   |   |   |   |-- put_pixel/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   `-- rgba_batch.rs
|   |   |   |   |   |   |   `-- kernels.rs
|   |   |   |   |   |   |-- put_pixel.rs
|   |   |   |   |   |   |-- reduce/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- reduce.rs
|   |   |   |   |   |   |-- resize/
|   |   |   |   |   |   |   |-- coordinates.rs
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- f32.rs
|   |   |   |   |   |   |   |   |-- i32/
|   |   |   |   |   |   |   |   |   `-- neon.rs
|   |   |   |   |   |   |   |   |-- i32.rs
|   |   |   |   |   |   |   |   |-- luma16.rs
|   |   |   |   |   |   |   |   |-- u8.rs
|   |   |   |   |   |   |   |   |-- u8_boxed_convolution.rs
|   |   |   |   |   |   |   |   |-- u8_convolution.rs
|   |   |   |   |   |   |   |   |-- u8_cover_i32.rs
|   |   |   |   |   |   |   |   |-- u8_nearest.rs
|   |   |   |   |   |   |   |   `-- u8_plan.rs
|   |   |   |   |   |   |   |-- support.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- resize.rs
|   |   |   |   |   |   |-- rotate/
|   |   |   |   |   |   |   |-- interpolation.rs
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- native.rs
|   |   |   |   |   |   |   |   `-- nearest.rs
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- rotate.rs
|   |   |   |   |   |   |-- thumbnail/
|   |   |   |   |   |   |   `-- kernels/
|   |   |   |   |   |   |       |-- float.rs
|   |   |   |   |   |   |       |-- fusion.rs
|   |   |   |   |   |   |       `-- integer.rs
|   |   |   |   |   |   |-- thumbnail.rs
|   |   |   |   |   |   |-- transform/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- bilinear.rs
|   |   |   |   |   |   |   |   |-- luma16.rs
|   |   |   |   |   |   |   |   |-- mesh.rs
|   |   |   |   |   |   |   |   `-- nearest.rs
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- transform.rs
|   |   |   |   |   |   |-- transpose/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- portable.rs
|   |   |   |   |   |   |   |   `-- rgba_avx2.rs
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   |-- odd_tests.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   `-- transpose.rs
|   |   |   |   |   |-- adapters.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   |-- native_channel/
|   |   |   |   |   |   `-- cmyk_ssse3.rs
|   |   |   |   |   |-- native_channel.rs
|   |   |   |   |   |-- native_filter.rs
|   |   |   |   |   |-- native_merge.rs
|   |   |   |   |   `-- scalar.rs
|   |   |   |   |-- point_batch.rs
|   |   |   |   `-- put_pixel_batch.rs
|   |   |   |-- registry/
|   |   |   |   |-- gpu_params.rs
|   |   |   |   |-- gpu_support.rs
|   |   |   |   `-- registration/
|   |   |   |       |-- chops.rs
|   |   |   |       |-- color.rs
|   |   |   |       |-- draw.rs
|   |   |   |       |-- effects.rs
|   |   |   |       |-- enhance.rs
|   |   |   |       |-- extract_band.rs
|   |   |   |       |-- filter.rs
|   |   |   |       |-- geometry.rs
|   |   |   |       |-- imageops.rs
|   |   |   |       |-- mod.rs
|   |   |   |       |-- module_fns.rs
|   |   |   |       |-- mutating.rs
|   |   |   |       |-- prelude.rs
|   |   |   |       |-- simd.rs
|   |   |   |       `-- transform.rs
|   |   |   |-- registry.rs
|   |   |   |-- telemetry.rs
|   |   |   |-- thumbnail_fusion.rs
|   |   |   `-- transpose_tests.rs
|   |   |-- data/
|   |   |   |-- lab-identity-profile.icc
|   |   |   `-- lcms-lab-lut.u16le
|   |   |-- draw/
|   |   |   `-- mod.rs
|   |   |-- error.rs
|   |   |-- font/
|   |   |   |-- courb08.luma
|   |   |   |-- courb08.pil
|   |   |   |-- default_aileron.LICENSE.txt
|   |   |   |-- default_aileron.rs
|   |   |   |-- default_aileron.ttf
|   |   |   |-- imagingft.rs
|   |   |   |-- mod.rs
|   |   |   `-- pilfont.rs
|   |   |-- format.rs
|   |   |-- image/
|   |   |   |-- encode.rs
|   |   |   `-- pipeline_admission.rs
|   |   |-- image.rs
|   |   |-- image_getchannel_receipt_tests.rs
|   |   |-- image_sequence.rs
|   |   |-- image_utils.rs
|   |   |-- lab.rs
|   |   |-- lib.rs
|   |   |-- ops/
|   |   |   |-- analysis.rs
|   |   |   |-- array.rs
|   |   |   |-- chops.rs
|   |   |   |-- convert.rs
|   |   |   |-- crop.rs
|   |   |   |-- enhance.rs
|   |   |   |-- filter.rs
|   |   |   |-- imageops.rs
|   |   |   |-- lut_hardlight.bin
|   |   |   |-- lut_overlay.bin
|   |   |   |-- lut_softlight.bin
|   |   |   |-- mod.rs
|   |   |   |-- module_fns.rs
|   |   |   |-- param_filters.rs
|   |   |   |-- paste.rs
|   |   |   |-- pil_resize.rs
|   |   |   |-- quantize.rs
|   |   |   |-- resize.rs
|   |   |   |-- rotate/
|   |   |   |   `-- nearest.rs
|   |   |   |-- rotate.rs
|   |   |   |-- split.rs
|   |   |   |-- transform.rs
|   |   |   |-- transpose.rs
|   |   |   `-- utils.rs
|   |   |-- par.rs
|   |   |-- pipeline.rs
|   |   `-- raster/
|   |       |-- buffer.rs
|   |       |-- color/
|   |       |   |-- blend.rs
|   |       |   |-- from_color.rs
|   |       |   |-- from_primitive.rs
|   |       |   |-- invert.rs
|   |       |   |-- mod.rs
|   |       |   |-- pixel_luma.rs
|   |       |   |-- pixel_rgb.rs
|   |       |   `-- types.rs
|   |       |-- dynamic.rs
|   |       |-- error.rs
|   |       |-- mod.rs
|   |       `-- traits/
|   |           |-- mod.rs
|   |           |-- pixel.rs
|   |           |-- primitive.rs
|   |           `-- view.rs
|   `-- tests/
|       `-- fixtures/
|           `-- manifest.yaml
|-- pillow-rs-js/
|   |-- Cargo.toml
|   |-- LICENSE
|   |-- README.md
|   |-- package-lock.json
|   |-- package.json
|   `-- src/
|       |-- image_io.rs
|       `-- lib.rs
|-- pillow-rs-parallel/
|   |-- LICENSE
|   |-- README.md
|   `-- pyproject.toml
|-- pillow-rs-py/
|   |-- Cargo.toml
|   |-- pyproject.toml
|   |-- python/
|   |   |-- PIL/
|   |   |   |-- Image.py
|   |   |   |-- ImageShow.py
|   |   |   `-- __init__.py
|   |   `-- pillow_rs/
|   |       |-- __init__.py
|   |       |-- enums.py
|   |       |-- gpu_batch.py
|   |       |-- image.py
|   |       |-- imagechops.py
|   |       |-- imagecolor.py
|   |       |-- imagedraw.py
|   |       |-- imageenhance.py
|   |       |-- imagefilter.py
|   |       |-- imagefont.py
|   |       |-- imageops.py
|   |       |-- imagepalette.py
|   |       |-- imagesequence.py
|   |       |-- imageshow.py
|   |       |-- imagestat.py
|   |       `-- operations.py
|   `-- src/
|       |-- getcolors.rs
|       |-- gpu_batch.rs
|       |-- lib.rs
|       |-- putdata.rs
|       |-- save_options.rs
|       `-- telemetry.rs
|-- requirements-ci.txt
|-- requirements-docs.in
|-- requirements-docs.txt
|-- rust-toolchain.toml
|-- rustfmt.toml
`-- scripts/
    |-- aggregate_migration_parity.py
    |-- audit_rust_result_methods.py
    |-- benchmark_gpu_batch.py
    |-- build_migration_parity_inputs.py
    |-- build_migration_parity_manifest.py
    |-- build_operation_source_crosswalk.py
    |-- build_perfdb_selected_run.py
    |-- build_performance_optimization_matrix.py
    |-- build_public_api_inventory.py
    |-- check_backend_boundaries.py
    |-- check_bindings.py
    |-- check_docs_browser.cjs
    |-- check_docs_examples.py
    |-- check_docs_registry.py
    |-- check_implementation_structure.py
    |-- check_migration_parity_inputs.py
    |-- check_pipeline_benchmark_budgets.py
    |-- check_public_api_boundary.py
    |-- check_python_compatibility.py
    |-- check_release_crate.py
    |-- check_release_licenses.py
    |-- check_release_parallel_wheel.py
    |-- check_release_recovery.py
    |-- check_release_status.py
    |-- check_release_wheel.py
    |-- check_repo_map.py
    |-- check_rust_file_sizes.py
    |-- check_source_contracts.py
    |-- check_workflows.py
    |-- codex-worktree-setup.sh
    |-- data/
    |   `-- migration-authority-v0.yaml
    |-- docs_benchmark_view.py
    |-- docs_evidence.py
    |-- docs_release.py
    |-- docs_site.py
    |-- generate_migration_parity_docs.py
    |-- implementation_structure/
    |   |-- __init__.py
    |   |-- audit.py
    |   `-- test_audit.py
    |-- lint.sh
    |-- migration_benchmark_adapter.py
    |-- migration_benchmark_samples.py
    |-- migration_build_witness.py
    |-- migration_execution_attestation.py
    |-- migration_execution_identity.py
    |-- migration_materialization_witness.py
    |-- migration_parity/
    |   |-- channel_inputs.py
    |   |-- display.py
    |   |-- display_inputs.py
    |   `-- parallel_cpu_inputs.py
    |-- migration_parity_inventory.py
    |-- migration_producer_attestation.py
    |-- migration_producer_identity.py
    |-- prepare_pypi_release.py
    |-- profile_migration_benchmark.py
    |-- reduce_migration_parity_cases.py
    |-- release_versions.py
    |-- report_migration_changed_line_coverage.py
    |-- report_migration_js_parity_gaps.py
    |-- report_migration_parity_region_coverage.py
    |-- report_migration_pillow_missing.py
    |-- report_optimization_goals.py
    |-- report_pipeline_benchmark_coverage.py
    |-- report_pipeline_performance.py
    |-- report_pipeline_roadmap_status.py
    |-- run_all_backend_tests.py
    |-- run_clippy_quality.py
    |-- run_migration_benchmark.py
    |-- run_migration_coverage.py
    |-- run_migration_font_native_cases.py
    |-- run_migration_imagecolor_native_cases.py
    |-- run_migration_imagecore_native_cases.py
    |-- run_migration_imagedraw_native_cases.py
    |-- run_migration_imageops_native_cases.py
    |-- run_migration_imagepalette_native_cases.py
    |-- run_migration_imagesequence_native_cases.py
    |-- run_migration_js_parity.py
    |-- run_migration_parity.py
    |-- run_migration_pillow_coverage.py
    |-- run_migration_rust_coverage.py
    |-- run_pillow_simd_benchmark.py
    |-- run_transpose_throughput.py
    |-- rust_size_baseline.json
    |-- select_docs_benchmark.py
    |-- simd_evidence.py
    |-- source_crosswalk/
    |   |-- __init__.py
    |   |-- builder.py
    |   |-- evidence.py
    |   |-- pyo3_bindings.py
    |   |-- python_bindings.py
    |   |-- rust_calls.py
    |   |-- rust_modules.py
    |   |-- rust_source.py
    |   |-- test_evidence.py
    |   `-- test_operation_source_crosswalk.py
    |-- test_benchmark_gpu_batch_contract.py
    |-- test_build_perfdb_selected_run.py
    |-- test_check_backend_boundaries.py
    |-- test_chops_affine_parity.py
    |-- test_coverage_context.py
    |-- test_docs_benchmark_selection.py
    |-- test_docs_benchmark_view.py
    |-- test_docs_evidence.py
    |-- test_docs_performance_matrix.py
    |-- test_docs_release.py
    |-- test_docs_site.py
    |-- test_font_native_cases.py
    |-- test_font_variant_parity.py
    |-- test_getdata_sequence_parity.py
    |-- test_gpu_batch.py
    |-- test_grayscale_rgb_domain.py
    |-- test_migration_benchmark_attestation.py
    |-- test_migration_benchmark_runner_identity.py
    |-- test_migration_benchmark_samples.py
    |-- test_migration_benchmark_selection.py
    |-- test_migration_build_witness.py
    |-- test_migration_execution_attestation.py
    |-- test_migration_execution_identity.py
    |-- test_migration_materialization_witness.py
    |-- test_migration_parity_attestation.py
    |-- test_migration_parity_display.py
    |-- test_migration_parity_icc_timestamp.py
    |-- test_migration_parity_selection.py
    |-- test_migration_parity_serialization.py
    |-- test_migration_parity_timing.py
    |-- test_migration_producer_attestation.py
    |-- test_migration_producer_identity.py
    |-- test_optimization_goals.py
    |-- test_parity_reduction.py
    |-- test_performance_optimization_matrix.py
    |-- test_pillow_simd_benchmark.py
    |-- test_pillow_simd_suite_compatibility.py
    |-- test_profile_migration_benchmark.py
    |-- test_receipt_state.py
    |-- test_release_tools.py
    |-- test_rust_size_policy.py
    |-- test_source_contracts.py
    |-- validate_migration_parity_contract.py
    `-- validate_migration_parity_result.py
```
<!-- END GENERATED CODE TREE -->
