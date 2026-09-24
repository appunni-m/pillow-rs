# Repository map

Use this map to locate maintained source and generators. [Architecture](ARCHITECTURE.md)
explains ownership; [Contributing](../CONTRIBUTING.md) explains the workflow.

| Path | Responsibility |
| --- | --- |
| `pillow-rs/` | Rust image model, operations, drawing, pipelines, font integration |
| `pillow-rs-py/` | Python `PIL` facade and PyO3 conversion/delegation |
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
|   |   |-- compute/
|   |   |   |-- mod.rs
|   |   |   |-- pool_cpu/
|   |   |   |   |-- mod.rs
|   |   |   |   `-- ops/
|   |   |   |       |-- chops.rs
|   |   |   |       |-- color.rs
|   |   |   |       |-- draw.rs
|   |   |   |       |-- effects.rs
|   |   |   |       |-- enhance.rs
|   |   |   |       |-- filter.rs
|   |   |   |       |-- geometry.rs
|   |   |   |       |-- imageops.rs
|   |   |   |       `-- mod.rs
|   |   |   |-- pool_gpu/
|   |   |   |   |-- mod.rs
|   |   |   |   `-- shaders/
|   |   |   |       |-- add.wgsl
|   |   |   |       |-- add_modulo.wgsl
|   |   |   |       |-- alpha_composite.wgsl
|   |   |   |       |-- autocontrast.wgsl
|   |   |   |       |-- autocontrast_cutoff.wgsl
|   |   |   |       |-- autocontrast_histogram.wgsl
|   |   |   |       |-- autocontrast_remap.wgsl
|   |   |   |       |-- blend_module.wgsl
|   |   |   |       |-- box_blur.wgsl
|   |   |   |       |-- box_blur_h.wgsl
|   |   |   |       |-- box_blur_v.wgsl
|   |   |   |       |-- brightness.wgsl
|   |   |   |       |-- color_3dlut.wgsl
|   |   |   |       |-- color_saturation.wgsl
|   |   |   |       |-- colorize.wgsl
|   |   |   |       |-- composite_module.wgsl
|   |   |   |       |-- constant.wgsl
|   |   |   |       |-- contain.wgsl
|   |   |   |       |-- contrast.wgsl
|   |   |   |       |-- convert.wgsl
|   |   |   |       |-- cover.wgsl
|   |   |   |       |-- crop.wgsl
|   |   |   |       |-- crop_border.wgsl
|   |   |   |       |-- darker.wgsl
|   |   |   |       |-- difference.wgsl
|   |   |   |       |-- draw.wgsl
|   |   |   |       |-- duplicate.wgsl
|   |   |   |       |-- effect_noise.wgsl
|   |   |   |       |-- effect_spread.wgsl
|   |   |   |       |-- equalize.wgsl
|   |   |   |       |-- equalize_cdf.wgsl
|   |   |   |       |-- equalize_histogram.wgsl
|   |   |   |       |-- equalize_remap.wgsl
|   |   |   |       |-- eval.wgsl
|   |   |   |       |-- expand.wgsl
|   |   |   |       |-- extract_band.wgsl
|   |   |   |       |-- filter_3x3.wgsl
|   |   |   |       |-- filter_5x5.wgsl
|   |   |   |       |-- fit.wgsl
|   |   |   |       |-- flip.wgsl
|   |   |   |       |-- gaussian_blur.wgsl
|   |   |   |       |-- grayscale.wgsl
|   |   |   |       |-- hard_light.wgsl
|   |   |   |       |-- histogram_clear.wgsl
|   |   |   |       |-- invert.wgsl
|   |   |   |       |-- invert_chops.wgsl
|   |   |   |       |-- lighter.wgsl
|   |   |   |       |-- logical_and.wgsl
|   |   |   |       |-- logical_or.wgsl
|   |   |   |       |-- logical_xor.wgsl
|   |   |   |       |-- max_filter.wgsl
|   |   |   |       |-- median_filter.wgsl
|   |   |   |       |-- merge.wgsl
|   |   |   |       |-- min_filter.wgsl
|   |   |   |       |-- mirror.wgsl
|   |   |   |       |-- multiply.wgsl
|   |   |   |       |-- multiply_screen.wgsl
|   |   |   |       |-- offset.wgsl
|   |   |   |       |-- overlay.wgsl
|   |   |   |       |-- pad.wgsl
|   |   |   |       |-- paste.wgsl
|   |   |   |       |-- point_op.wgsl
|   |   |   |       |-- posterize.wgsl
|   |   |   |       |-- put_alpha.wgsl
|   |   |   |       |-- put_alpha_data.wgsl
|   |   |   |       |-- put_data.wgsl
|   |   |   |       |-- put_pixel.wgsl
|   |   |   |       |-- rank_filter.wgsl
|   |   |   |       |-- reduce.wgsl
|   |   |   |       |-- remap_palette.wgsl
|   |   |   |       |-- resize_bilinear.wgsl
|   |   |   |       |-- resize_convolution_h.wgsl
|   |   |   |       |-- resize_convolution_v.wgsl
|   |   |   |       |-- resize_nearest.wgsl
|   |   |   |       |-- rotate.wgsl
|   |   |   |       |-- scale.wgsl
|   |   |   |       |-- screen.wgsl
|   |   |   |       |-- sharpness.wgsl
|   |   |   |       |-- soft_light.wgsl
|   |   |   |       |-- solarize.wgsl
|   |   |   |       |-- subtract.wgsl
|   |   |   |       |-- subtract_modulo.wgsl
|   |   |   |       |-- thumbnail.wgsl
|   |   |   |       |-- transform.wgsl
|   |   |   |       |-- transform_geometry.wgsl
|   |   |   |       |-- transpose.wgsl
|   |   |   |       |-- transpose_rgb.wgsl
|   |   |   |       `-- transpose_rgb_tiled.wgsl
|   |   |   |-- pool_simd/
|   |   |   |   |-- mod.rs
|   |   |   |   `-- ops/
|   |   |   |       |-- adapters.rs
|   |   |   |       |-- mod.rs
|   |   |   |       `-- scalar.rs
|   |   |   `-- registry.rs
|   |   |-- draw/
|   |   |   `-- mod.rs
|   |   |-- error.rs
|   |   |-- font/
|   |   |   |-- courb08.pil
|   |   |   |-- default_aileron.LICENSE.txt
|   |   |   |-- default_aileron.rs
|   |   |   |-- default_aileron.ttf
|   |   |   |-- imagingft.rs
|   |   |   |-- mod.rs
|   |   |   `-- pilfont.rs
|   |   |-- format.rs
|   |   |-- image.rs
|   |   |-- image_sequence.rs
|   |   |-- image_utils.rs
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
|       `-- lib.rs
|-- pillow-rs-py/
|   |-- Cargo.toml
|   |-- pyproject.toml
|   |-- python/
|   |   |-- PIL/
|   |   |   |-- Image.py
|   |   |   `-- __init__.py
|   |   `-- pillow_rs/
|   |       |-- __init__.py
|   |       |-- enums.py
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
|   |       |-- imagestat.py
|   |       `-- operations.py
|   `-- src/
|       |-- lib.rs
|       `-- putdata.rs
|-- requirements-ci.txt
|-- requirements-docs.in
|-- requirements-docs.txt
|-- rust-toolchain.toml
|-- rustfmt.toml
`-- scripts/
    |-- aggregate_migration_parity.py
    |-- audit_rust_result_methods.py
    |-- build_migration_parity_inputs.py
    |-- build_migration_parity_manifest.py
    |-- check_bindings.py
    |-- check_docs_browser.cjs
    |-- check_docs_examples.py
    |-- check_docs_registry.py
    |-- check_migration_parity_inputs.py
    |-- check_pipeline_benchmark_budgets.py
    |-- check_public_api_boundary.py
    |-- check_python_compatibility.py
    |-- check_release_licenses.py
    |-- check_release_recovery.py
    |-- check_release_status.py
    |-- check_release_wheel.py
    |-- check_repo_map.py
    |-- check_workflows.py
    |-- codex-worktree-setup.sh
    |-- data/
    |   `-- migration-authority-v0.yaml
    |-- docs_benchmark_view.py
    |-- docs_evidence.py
    |-- docs_release.py
    |-- docs_site.py
    |-- generate_migration_parity_docs.py
    |-- lint.sh
    |-- migration_parity_inventory.py
    |-- prepare_pypi_release.py
    |-- profile_migration_benchmark.py
    |-- reduce_migration_parity_cases.py
    |-- release_versions.py
    |-- report_migration_changed_line_coverage.py
    |-- report_migration_js_parity_gaps.py
    |-- report_migration_parity_region_coverage.py
    |-- report_migration_pillow_missing.py
    |-- report_pipeline_benchmark_coverage.py
    |-- report_pipeline_performance.py
    |-- report_pipeline_roadmap_status.py
    |-- run_all_backend_tests.py
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
    |-- run_transpose_throughput.py
    |-- select_docs_benchmark.py
    |-- test_coverage_context.py
    |-- test_docs_benchmark_selection.py
    |-- test_docs_benchmark_view.py
    |-- test_docs_release.py
    |-- test_docs_site.py
    |-- test_font_native_cases.py
    |-- test_parity_reduction.py
    |-- test_receipt_state.py
    |-- test_release_tools.py
    |-- validate_migration_parity_contract.py
    `-- validate_migration_parity_result.py
```
<!-- END GENERATED CODE TREE -->
