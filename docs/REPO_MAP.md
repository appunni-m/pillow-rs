# Repository Map

This is the maintained index for important files in this workspace. It should
answer two questions for a new contributor or agent:

- Where does a change belong?
- Which files are source of truth versus generated, vendored, or historical?

Update this file when source ownership, harness ownership, or project-goal
documents move. The generated tree below is validated by `make repo-map-check`
and refreshed with `make repo-map-update`.

## Maintenance Rules

- Use `Makefile` targets for normal workflows. If a repeated workflow has no
  target, add one instead of documenting a raw command.
- Keep runtime implementation in Rust crates. Binding crates stay thin.
- Do not treat generated outputs, caches, build directories, or local package
  installs as repository source.
- Keep fixture generators and parity harnesses documented as maintained system
  components, not one-off scripts.
- `pillow-rs-freetype/freetype/` is a version-pinned C FreeType oracle for
  fixture generation and diagnosis only. Runtime code must not call it.

## Primary Source Of Truth

- `CLAUDE.md` with symlinked `AGENTS.md` and `AGENT.md`: default agent
  briefing, project constraints, skill routing, and workflow policy.
- `Makefile`: root build, test, lint, fixture, benchmark, coverage, and CI
  entrypoint.
- `Cargo.toml`: workspace membership, edition, lint policy, and shared cargo
  settings.
- `rust-toolchain.toml`: pinned toolchain and required components.
- `manifest.yaml`: PIL-style operation inventory for generated stubs, coverage,
  and fixture work.
- `docs/REPO_MAP.md`: this maintained ownership and code tree map.

## Workspace Crates

- `pillow-rs/`: pure Rust image operations and shared image model.
  `src/image.rs`, `src/pipeline.rs`, `src/ops/`, `src/draw/`, and
  `src/font/imagingft.rs` are the primary implementation paths.
- `pillow-rs-py/`: PyO3 binding crate. `src/lib.rs` exposes Rust to Python;
  `python/pillow_rs/` must stay a thin Python surface.
- `pillow-rs-js/`: wasm-bindgen binding crate plus browser/node test runners.
- `pillow-rs-freetype/`: monorepo path for the standalone `freetype` package:
  pure Rust FreeType-compatible implementation and parity harness.
  `PROJECT_GOALS.md`, `Makefile`, `src/`, `tests/`, `scripts/`, license files,
  and selected `doc/` files are the maintained surface.

## FreeType Map

- `pillow-rs-freetype/PROJECT_GOALS.md`: project-level parity goal and
  non-negotiable constraints.
- `pillow-rs-freetype/FTL.TXT`, `LICENSE`, and `NOTICE.md`: FreeType license
  text and migration attribution for the standalone package.
- `pillow-rs-freetype/src/font.rs`: public face/font API and high-level
  FreeType-compatible behavior.
- `pillow-rs-freetype/src/scaler.rs`: size scaling and glyph load pipeline.
- `pillow-rs-freetype/src/outline.rs`: outline geometry, bbox, cbox, and point
  representation.
- `pillow-rs-freetype/src/render.rs` and `src/grays.rs`: bitmap render paths.
- `pillow-rs-freetype/src/tt/`: TrueType parsing, glyph loading, metrics, and
  native bytecode hinting.
- `pillow-rs-freetype/src/autohint/`: autohinter implementation and script
  coverage.
- `pillow-rs-freetype/tests/coverage_matrix_tests.rs`: fixture matrix parity
  harness.
- `pillow-rs-freetype/tests/no_runtime_ffi.rs`: guard against runtime FFI
  shortcuts.
- `pillow-rs-freetype/tests/fixtures/*_matrix.json`: versioned oracle matrices.
  Do not edit these to make tests pass.
- `pillow-rs-freetype/scripts/`: maintained generators, benchmark tools, and
  failure classifiers.
- `pillow-rs-freetype/doc/GENERATOR_SYSTEM.md`: fixture generation contract.
- `pillow-rs-freetype/doc/FONT_FIXTURE_COVERAGE_PLAN.md`: maintained plan for
  compact font fixtures, explicit public inputs, legacy-font retirement, and
  100% Rust structural coverage.
- `pillow-rs-freetype/doc/FONT_FIXTURE_INVENTORY.md`: content-deduplicated active
  and deprecated font inventory with selected glyph and coverage ownership.
- `pillow-rs-freetype/doc/PARITY_FAILURE_CLASSIFICATION.md`: failure bucket
  taxonomy.
- `pillow-rs-freetype/doc/PERFORMANCE_BENCHMARKING.md`: Rust-vs-C FreeType
  benchmark method and reporting.

## Harness And Fixture Map

- `tests/engine.py` and `tests/test_parity.py`: root PIL parity runner.
- `scripts/generate_fixtures.py`: root PIL fixture generator.
- `scripts/coverage/`: manifest coverage computation and validation.
- `scripts/bench/`: root benchmark specification, runners, aggregation, and
  baseline comparison.
- `pillow-rs-freetype/tests/coverage_matrix_tests.rs`: FreeType matrix runner.

## Cleanup Rules

These paths are not source of truth and should stay untracked or be removed
before commit:

- `target/`
- `.pytest_cache/`
- `__pycache__/`
- `pillow-rs-js/node_modules/`
- `pillow-rs-js/pkg/`
- `pillow-rs-js/pkg_node/`
- `pillow-rs-freetype/freetype/build*/`

Historical planning documents may be useful as archaeology, but implementation
work should be guided by current Makefiles, `CLAUDE.md`, this map, and active
crate-level project docs. If a historical document contains current guidance,
promote that guidance into one of those maintained files and then delete or
archive the stale document.

## Maintained Code Tree

The tree below is generated from tracked source, harness, script, and control
files. It intentionally excludes vendored C FreeType, fixture payloads,
generated reports, build outputs, and package installs.

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
|       `-- ci.yml
|-- AGENT.md
|-- AGENTS.md
|-- CLAUDE.md
|-- CONTRIBUTING.md
|-- Cargo.lock
|-- Cargo.toml
|-- Makefile
|-- README.md
|-- deny.toml
|-- docs/
|   `-- REPO_MAP.md
|-- manifest.yaml
|-- pillow-rs/
|   |-- Cargo.toml
|   `-- src/
|       |-- checked_dims.rs
|       |-- color.rs
|       |-- compute/
|       |   |-- backend_op.rs
|       |   |-- mod.rs
|       |   |-- op_def.rs
|       |   |-- pool_cpu/
|       |   |   |-- mod.rs
|       |   |   `-- ops/
|       |   |       |-- chops.rs
|       |   |       |-- color.rs
|       |   |       |-- draw.rs
|       |   |       |-- effects.rs
|       |   |       |-- enhance.rs
|       |   |       |-- filter.rs
|       |   |       |-- geometry.rs
|       |   |       |-- imageops.rs
|       |   |       `-- mod.rs
|       |   |-- pool_gpu/
|       |   |   |-- mod.rs
|       |   |   `-- shaders/
|       |   |       |-- add.wgsl
|       |   |       |-- add_modulo.wgsl
|       |   |       |-- alpha_composite.wgsl
|       |   |       |-- autocontrast.wgsl
|       |   |       |-- autocontrast_cutoff.wgsl
|       |   |       |-- autocontrast_histogram.wgsl
|       |   |       |-- autocontrast_remap.wgsl
|       |   |       |-- blend.wgsl
|       |   |       |-- blend_module.wgsl
|       |   |       |-- box_blur.wgsl
|       |   |       |-- box_blur_h.wgsl
|       |   |       |-- box_blur_v.wgsl
|       |   |       |-- brightness.wgsl
|       |   |       |-- color_3dlut.wgsl
|       |   |       |-- color_saturation.wgsl
|       |   |       |-- colorize.wgsl
|       |   |       |-- composite.wgsl
|       |   |       |-- composite_module.wgsl
|       |   |       |-- constant.wgsl
|       |   |       |-- contain.wgsl
|       |   |       |-- contrast.wgsl
|       |   |       |-- convert.wgsl
|       |   |       |-- cover.wgsl
|       |   |       |-- crop.wgsl
|       |   |       |-- crop_border.wgsl
|       |   |       |-- darker.wgsl
|       |   |       |-- difference.wgsl
|       |   |       |-- duplicate.wgsl
|       |   |       |-- effect_mandelbrot.wgsl
|       |   |       |-- effect_noise.wgsl
|       |   |       |-- equalize.wgsl
|       |   |       |-- equalize_cdf.wgsl
|       |   |       |-- equalize_histogram.wgsl
|       |   |       |-- equalize_remap.wgsl
|       |   |       |-- eval.wgsl
|       |   |       |-- expand.wgsl
|       |   |       |-- extract_band.wgsl
|       |   |       |-- filter_3x3.wgsl
|       |   |       |-- filter_5x5.wgsl
|       |   |       |-- fit.wgsl
|       |   |       |-- flip.wgsl
|       |   |       |-- gaussian_blur.wgsl
|       |   |       |-- grayscale.wgsl
|       |   |       |-- hard_light.wgsl
|       |   |       |-- histogram_clear.wgsl
|       |   |       |-- invert.wgsl
|       |   |       |-- invert_chops.wgsl
|       |   |       |-- lighter.wgsl
|       |   |       |-- linear_gradient.wgsl
|       |   |       |-- logical_and.wgsl
|       |   |       |-- logical_or.wgsl
|       |   |       |-- logical_xor.wgsl
|       |   |       |-- max_filter.wgsl
|       |   |       |-- median_filter.wgsl
|       |   |       |-- merge.wgsl
|       |   |       |-- min_filter.wgsl
|       |   |       |-- mirror.wgsl
|       |   |       |-- multiply.wgsl
|       |   |       |-- offset.wgsl
|       |   |       |-- overlay.wgsl
|       |   |       |-- pad.wgsl
|       |   |       |-- paste.wgsl
|       |   |       |-- point_op.wgsl
|       |   |       |-- posterize.wgsl
|       |   |       |-- put_alpha.wgsl
|       |   |       |-- put_data.wgsl
|       |   |       |-- put_pixel.wgsl
|       |   |       |-- quantize.wgsl
|       |   |       |-- radial_gradient.wgsl
|       |   |       |-- rank_filter.wgsl
|       |   |       |-- reduce.wgsl
|       |   |       |-- remap_palette.wgsl
|       |   |       |-- resize_bilinear.wgsl
|       |   |       |-- resize_nearest.wgsl
|       |   |       |-- rotate.wgsl
|       |   |       |-- scale.wgsl
|       |   |       |-- screen.wgsl
|       |   |       |-- sharpness.wgsl
|       |   |       |-- soft_light.wgsl
|       |   |       |-- solarize.wgsl
|       |   |       |-- subtract.wgsl
|       |   |       |-- subtract_modulo.wgsl
|       |   |       |-- thumbnail.wgsl
|       |   |       |-- transform.wgsl
|       |   |       `-- transpose.wgsl
|       |   |-- pool_simd/
|       |   |   |-- mod.rs
|       |   |   `-- ops/
|       |   |       |-- adapters.rs
|       |   |       |-- arm.rs
|       |   |       |-- mod.rs
|       |   |       |-- scalar.rs
|       |   |       `-- x86.rs
|       |   `-- registry.rs
|       |-- draw/
|       |   `-- mod.rs
|       |-- error.rs
|       |-- font/
|       |   |-- courb08.pil.b64
|       |   |-- courb08.png.b64
|       |   |-- default_aileron.LICENSE.txt
|       |   |-- default_aileron.b64
|       |   |-- default_aileron.rs
|       |   |-- imagingft.rs
|       |   |-- mod.rs
|       |   `-- pilfont.rs
|       |-- format.rs
|       |-- image.rs
|       |-- image_utils.rs
|       |-- lib.rs
|       |-- ops/
|       |   |-- analysis.rs
|       |   |-- array.rs
|       |   |-- chops.rs
|       |   |-- convert.rs
|       |   |-- crop.rs
|       |   |-- enhance.rs
|       |   |-- filter.rs
|       |   |-- imageops.rs
|       |   |-- lut_hardlight.bin
|       |   |-- lut_overlay.bin
|       |   |-- lut_softlight.bin
|       |   |-- mod.rs
|       |   |-- module_fns.rs
|       |   |-- param_filters.rs
|       |   |-- paste.rs
|       |   |-- pil_resize.rs
|       |   |-- quantize.rs
|       |   |-- resize.rs
|       |   |-- rotate.rs
|       |   |-- split.rs
|       |   |-- transform.rs
|       |   |-- transpose.rs
|       |   `-- utils.rs
|       |-- par.rs
|       `-- pipeline.rs
|-- pillow-rs-freetype/
|   |-- AGENTS.md
|   |-- CONTRIBUTING.md
|   |-- Cargo.lock
|   |-- Cargo.toml
|   |-- FTL.TXT
|   |-- LICENSE
|   |-- Makefile
|   |-- NOTICE.md
|   |-- PROJECT_GOALS.md
|   |-- README.md
|   |-- deny.toml
|   |-- examples/
|   |   |-- bench_ops.rs
|   |   |-- debug_glyph.rs
|   |   `-- trace_glyph.rs
|   |-- scripts/
|   |   |-- audit_api_abi.py
|   |   |-- bench_freetype.py
|   |   |-- bench_ft_ops.c
|   |   |-- build_autohint_script_fixtures.py
|   |   |-- build_cff_fixtures.py
|   |   |-- build_cmap_fixtures.py
|   |   |-- build_cpal_palette_fixtures.py
|   |   |-- build_ft.sh
|   |   |-- build_ftmm_future_variable_fixtures.py
|   |   |-- build_fvar_fixtures.py
|   |   |-- build_gasp_fixtures.py
|   |   |-- build_gzip_fixtures.py
|   |   |-- build_hinter_edge_fixtures.py
|   |   |-- build_metric_fixtures.py
|   |   |-- build_mvar_fixtures.py
|   |   |-- build_name_fixtures.py
|   |   |-- build_post_fixtures.py
|   |   |-- build_render_fixtures.py
|   |   |-- build_sbit_fixtures.py
|   |   |-- build_sfnt_fixtures.py
|   |   |-- build_type1_fixtures.py
|   |   |-- build_unified_oracle.py
|   |   |-- check_public_api_inputs.py
|   |   |-- extract_blues.py
|   |   |-- fetch_ft.sh
|   |   |-- gen_unified_oracle.c
|   |   |-- generate_malformed_bdf_fixtures.py
|   |   |-- generate_public_constants.py
|   |   |-- generate_winfnt_fixtures.py
|   |   `-- report_pending_route_buckets.py
|   |-- src/
|   |   |-- api.rs
|   |   |-- autohint/
|   |   |   |-- ALGORITHMS.md
|   |   |   |-- blue_strings.rs
|   |   |   |-- cjk.rs
|   |   |   |-- coverage.rs
|   |   |   |-- globals.rs
|   |   |   |-- globals_data.rs
|   |   |   |-- latin.rs
|   |   |   |-- loader.rs
|   |   |   |-- mod.rs
|   |   |   |-- script.rs
|   |   |   `-- types.rs
|   |   |-- casts.rs
|   |   |-- error.rs
|   |   |-- ffi/
|   |   |   |-- constants.rs
|   |   |   |-- convert.rs
|   |   |   |-- generated_constants.rs
|   |   |   |-- handles.rs
|   |   |   |-- mod.rs
|   |   |   `-- types.rs
|   |   |-- fixed.rs
|   |   |-- font.rs
|   |   |-- grays.rs
|   |   |-- lib.rs
|   |   |-- outline.rs
|   |   |-- render.rs
|   |   |-- scaler.rs
|   |   |-- tables.rs
|   |   `-- tt/
|   |       |-- cff.rs
|   |       |-- cmap.rs
|   |       |-- fvar.rs
|   |       |-- gasp.rs
|   |       |-- glyf.rs
|   |       |-- gvar.rs
|   |       |-- hdmx.rs
|   |       |-- head.rs
|   |       |-- hhea.rs
|   |       |-- hinter/
|   |       |   |-- exec.rs
|   |       |   |-- gs.rs
|   |       |   |-- iup.rs
|   |       |   |-- mod.rs
|   |       |   |-- tables.rs
|   |       |   `-- zone.rs
|   |       |-- hmtx.rs
|   |       |-- hvar.rs
|   |       |-- kern.rs
|   |       |-- loca.rs
|   |       |-- maxp.rs
|   |       |-- mod.rs
|   |       |-- mvar.rs
|   |       |-- name.rs
|   |       |-- os2.rs
|   |       |-- post.rs
|   |       |-- sbit.rs
|   |       |-- varstore.rs
|   |       |-- vhea.rs
|   |       `-- vmtx.rs
|   `-- tests/
|       |-- data/
|       |   |-- interface_map.json
|       |   `-- perf_operation_matrix.json
|       |-- direct_ft_compare.rs
|       |-- manifest.yaml
|       |-- pipe_trace.rs
|       |-- support/
|       |   `-- generated_constant_lookup.rs
|       `-- unified_fixture_parity.rs
|-- pillow-rs-js/
|   |-- Cargo.toml
|   |-- bench_page/
|   |   |-- bench_harness.mjs
|   |   |-- bench_runner.js
|   |   `-- index.html
|   |-- package-lock.json
|   |-- package.json
|   |-- src/
|   |   `-- lib.rs
|   `-- tests/
|       |-- apply_transparency_oracle.mjs
|       |-- browser/
|       |   |-- test.html
|       |   `-- wasm_browser.test.mjs
|       |-- codec_feature_matrix.mjs
|       |-- color3dlut_oracle.mjs
|       |-- drawing_oracle.mjs
|       |-- eval_oracle.mjs
|       |-- execution_engine.mjs
|       |-- fromarray_descriptor_oracle.mjs
|       |-- image_open_oracle.mjs
|       |-- oracle_contract.mjs
|       |-- oracle_corpus.mjs
|       |-- paste_oracle.mjs
|       |-- run_wasm_test.mjs
|       |-- tobytes_oracle.mjs
|       `-- wasm_backend.mjs
|-- pillow-rs-py/
|   |-- Cargo.toml
|   |-- pyproject.toml
|   |-- python/
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
|       `-- lib.rs
|-- rust-toolchain.toml
|-- rustfmt.toml
|-- scripts/
|   |-- analyze_palette_rotate.py
|   |-- audit_rust_result_methods.py
|   |-- bench/
|   |   |-- bench_aggregate.py
|   |   |-- bench_all.sh
|   |   |-- bench_baseline_add.py
|   |   |-- bench_browser.mjs
|   |   |-- bench_cache.py
|   |   |-- bench_full.py
|   |   |-- bench_gen_spec.py
|   |   |-- bench_manifest.py
|   |   |-- bench_native_cpu.py
|   |   |-- bench_pillow_baseline.py
|   |   |-- bench_reference_images/
|   |   |   `-- gen.py
|   |   |-- bench_spec.json
|   |   |-- bench_spec.py
|   |   |-- bench_unified.py
|   |   |-- bench_wasm_cpu.mjs
|   |   `-- compare_benchmarks.py
|   |-- build_and_test.sh
|   |-- check_bindings.py
|   |-- check_public_api_boundary.py
|   |-- check_repo_map.py
|   |-- ci_coverage.sh
|   |-- compare_font_coverage.sh
|   |-- coverage/
|   |   |-- __init__.py
|   |   |-- compute_coverage.py
|   |   |-- fix_manifest_modes.py
|   |   |-- generate_multi_backend_coverage.py
|   |   |-- generate_wasm_coverage.py
|   |   |-- ops_registry.py
|   |   |-- run_apply_transparency_rust_coverage.sh
|   |   |-- run_drawing_rust_coverage.sh
|   |   |-- run_font_rust_coverage.sh
|   |   |-- run_image_backend_rust_coverage.sh
|   |   |-- run_image_open_rust_coverage.sh
|   |   |-- run_imagingft_rust_coverage.sh
|   |   |-- run_paste_rust_coverage.sh
|   |   |-- run_point_rust_coverage.sh
|   |   |-- run_python_abi_rust_coverage.sh
|   |   |-- run_python_wrapper_coverage.sh
|   |   `-- validate_coverage.py
|   |-- generate_eval_error_oracle.py
|   |-- generate_fixtures.py
|   |-- generate_fromarray_descriptor_oracle.py
|   |-- generate_image_backend_operation_fixtures.py
|   |-- generate_palette_save_fixture_inputs.py
|   |-- generate_point_fixture_inputs.py
|   |-- generate_putdata_fixture_inputs.py
|   |-- generate_stubs.py
|   |-- lint.sh
|   `-- migrate_fixtures.py
`-- tests/
    |-- conftest.py
    |-- engine.py
    |-- fixture_coverage.py
    |-- oracles/
    |   `-- image_open_inputs.json
    |-- test_apply_transparency_oracle.py
    |-- test_drawing_oracle.py
    |-- test_eval_errors.py
    |-- test_fromarray_descriptor_oracle.py
    |-- test_pa_mutations.py
    |-- test_parity.py
    |-- test_paste_oracle.py
    `-- test_putdata_parity.py
```
<!-- END GENERATED CODE TREE -->
