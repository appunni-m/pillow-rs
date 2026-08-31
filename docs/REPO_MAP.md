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
- `build/fontdone-src/freetype/` is a version-pinned C FreeType oracle for
  fixture generation and diagnosis only. Runtime code must not call it.

## Primary Source Of Truth

- `CLAUDE.md` with symlinked `AGENTS.md` and `AGENT.md`: default agent
  briefing, project constraints, skill routing, and workflow policy.
- `Makefile`: root build, test, lint, fixture, benchmark, coverage, and CI
  entrypoint.
- `Cargo.toml`: workspace membership, edition, lint policy, and shared cargo
  settings.
- `rust-toolchain.toml`: pinned toolchain and required components.
- `pillow-rs/tests/fixtures/manifest.yaml`: the fixed
  `migration-parity/manifest@2` public-surface specification. Parity, coverage,
  and benchmark inputs are indexed from this manifest; generated results are
  not stored here.
- `docs/REPO_MAP.md`: this maintained ownership and code tree map.

## Workspace Crates

- `pillow-rs/`: pure Rust image operations and shared image model.
  `src/image.rs`, `src/pipeline.rs`, `src/ops/`, `src/draw/`, and
  `src/font/imagingft.rs` are the primary implementation paths.
- `pillow-rs-py/`: PyO3 binding crate. `src/lib.rs` exposes Rust to Python;
  `python/pillow_rs/` must stay a thin Python surface.
- `pillow-rs-js/`: wasm-bindgen binding crate plus browser/node test runners.
- `build/fontdone-src/`: pinned GitHub checkout for the standalone pure Rust
  FreeType-compatible implementation and parity harness. `pillow-rs` depends
  on the same pinned `fontdone` revision through Cargo.

## FreeType Map

- `build/fontdone-src/PROJECT_GOALS.md`: project-level parity goal and
  non-negotiable constraints.
- `build/fontdone-src/FTL.TXT`, `LICENSE`, and `NOTICE.md`: FreeType license
  text and migration attribution for the standalone package.
- `build/fontdone-src/src/font.rs`: public face/font API and high-level
  FreeType-compatible behavior.
- `build/fontdone-src/src/scaler.rs`: size scaling and glyph load pipeline.
- `build/fontdone-src/src/outline.rs`: outline geometry, bbox, cbox, and point
  representation.
- `build/fontdone-src/src/render.rs` and `src/grays.rs`: bitmap render paths.
- `build/fontdone-src/src/tt/`: TrueType parsing, glyph loading, metrics, and
  native bytecode hinting.
- `build/fontdone-src/src/autohint/`: autohinter implementation and script
  coverage.
- `build/fontdone-src/tests/coverage_matrix_tests.rs`: fixture matrix parity
  harness.
- `build/fontdone-src/tests/no_runtime_ffi.rs`: guard against runtime FFI
  shortcuts.
- `build/fontdone-src/tests/fixtures/*_matrix.json`: versioned oracle matrices.
  Do not edit these to make tests pass.
- `build/fontdone-src/scripts/`: maintained generators, benchmark tools, and
  failure classifiers.
- `build/fontdone-src/doc/GENERATOR_SYSTEM.md`: fixture generation contract.
- `build/fontdone-src/doc/FONT_FIXTURE_COVERAGE_PLAN.md`: maintained plan for
  compact font fixtures, explicit public inputs, legacy-font retirement, and
  100% Rust structural coverage.
- `build/fontdone-src/doc/FONT_FIXTURE_INVENTORY.md`: content-deduplicated active
  and deprecated font inventory with selected glyph and coverage ownership.
- `build/fontdone-src/doc/PARITY_FAILURE_CLASSIFICATION.md`: failure bucket
  taxonomy.
- `build/fontdone-src/doc/PERFORMANCE_BENCHMARKING.md`: Rust-vs-C FreeType
  benchmark method and reporting.

## Harness And Fixture Map

- `scripts/run_all_backend_tests.py`: maintained CPU/SIMD/GPU/JS campaign
  orchestrator. GPU children use bounded process groups, and full GPU parity
  runs by default after the smoke gate.
- `scripts/run_migration_parity.py`: live source/target runner for indexed
  input-only workflows.
- `scripts/run_migration_coverage.py`: target coverage producer for indexed
  coverage plans.
- `scripts/run_migration_benchmark.py`: correctness-gated benchmark producer
  for indexed workloads.
- `pillow-rs/tests/fixtures/inputs/`: the active parity, coverage, and
  benchmark input documents; expected values and run status are prohibited.
- `deprecated/migration-parity-v0/`: read-only provenance archive for the
  retired fixture/oracle suites, old manifest, and old coverage/benchmark
  tooling. It is not an active test root.
- `docs/gpu-crash-audit-20260811.md`: current GPU loop, resource, lazy-pipeline,
  watchdog, and adapter-backed verification record.
- `build/fontdone-src/tests/coverage_matrix_tests.rs`: separate FreeType matrix
  runner.

## Cleanup Rules

These paths are not source of truth and should stay untracked or be removed
before commit:

- `target/`
- `.pytest_cache/`
- `__pycache__/`
- `pillow-rs-js/node_modules/`
- `pillow-rs-js/pkg/`
- `pillow-rs-js/pkg_node/`
- `build/fontdone-src/freetype/build*/`

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
|   |   |   |       |-- effect_mandelbrot.wgsl
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
|   |   |   |       |-- linear_gradient.wgsl
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
|   |   |   |       |-- quantize.wgsl
|   |   |   |       |-- radial_gradient.wgsl
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
|   |   |   |       `-- transpose.wgsl
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
|   |-- package-lock.json
|   |-- package.json
|   `-- src/
|       `-- lib.rs
|-- pillow-rs-py/
|   |-- Cargo.toml
|   |-- pyproject.toml
|   |-- python/
|   |   |-- RSPIL/
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
|-- rust-toolchain.toml
|-- rustfmt.toml
`-- scripts/
    |-- aggregate_migration_parity.py
    |-- audit_rust_result_methods.py
    |-- build_migration_parity_inputs.py
    |-- build_migration_parity_manifest.py
    |-- check_bindings.py
    |-- check_migration_parity_inputs.py
    |-- check_pipeline_benchmark_budgets.py
    |-- check_public_api_boundary.py
    |-- check_repo_map.py
    |-- codex-worktree-setup.sh
    |-- generate_migration_parity_docs.py
    |-- lint.sh
    |-- migration_parity_inventory.py
    |-- profile_migration_benchmark.py
    |-- report_migration_js_parity_gaps.py
    |-- report_migration_parity_region_coverage.py
    |-- report_migration_pillow_missing.py
    |-- report_pipeline_benchmark_coverage.py
    |-- report_pipeline_performance.py
    |-- report_pipeline_roadmap_status.py
    |-- review_migration_parity_cases.py
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
    |-- test_receipt_state.py
    |-- validate_migration_parity_contract.py
    `-- validate_migration_parity_result.py
```
<!-- END GENERATED CODE TREE -->
