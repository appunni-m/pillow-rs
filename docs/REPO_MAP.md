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
- `../fontdone/freetype/` is a version-pinned C FreeType oracle for fixture
  generation and diagnosis only. Runtime code must not call it.

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
- `../fontdone/`: sibling standalone package for the pure Rust
  FreeType-compatible implementation and parity harness. `pillow-rs` depends
  on it through the `fontdone` crate.

## FreeType Map

- `../fontdone/PROJECT_GOALS.md`: project-level parity goal and
  non-negotiable constraints.
- `../fontdone/FTL.TXT`, `LICENSE`, and `NOTICE.md`: FreeType license
  text and migration attribution for the standalone package.
- `../fontdone/src/font.rs`: public face/font API and high-level
  FreeType-compatible behavior.
- `../fontdone/src/scaler.rs`: size scaling and glyph load pipeline.
- `../fontdone/src/outline.rs`: outline geometry, bbox, cbox, and point
  representation.
- `../fontdone/src/render.rs` and `src/grays.rs`: bitmap render paths.
- `../fontdone/src/tt/`: TrueType parsing, glyph loading, metrics, and
  native bytecode hinting.
- `../fontdone/src/autohint/`: autohinter implementation and script
  coverage.
- `../fontdone/tests/coverage_matrix_tests.rs`: fixture matrix parity
  harness.
- `../fontdone/tests/no_runtime_ffi.rs`: guard against runtime FFI
  shortcuts.
- `../fontdone/tests/fixtures/*_matrix.json`: versioned oracle matrices.
  Do not edit these to make tests pass.
- `../fontdone/scripts/`: maintained generators, benchmark tools, and
  failure classifiers.
- `../fontdone/doc/GENERATOR_SYSTEM.md`: fixture generation contract.
- `../fontdone/doc/FONT_FIXTURE_COVERAGE_PLAN.md`: maintained plan for
  compact font fixtures, explicit public inputs, legacy-font retirement, and
  100% Rust structural coverage.
- `../fontdone/doc/FONT_FIXTURE_INVENTORY.md`: content-deduplicated active
  and deprecated font inventory with selected glyph and coverage ownership.
- `../fontdone/doc/PARITY_FAILURE_CLASSIFICATION.md`: failure bucket
  taxonomy.
- `../fontdone/doc/PERFORMANCE_BENCHMARKING.md`: Rust-vs-C FreeType
  benchmark method and reporting.

## Harness And Fixture Map

- `tests/engine.py` and `tests/test_parity.py`: root PIL parity runner.
- `scripts/generate_fixtures.py`: root PIL fixture generator.
- `scripts/coverage/`: manifest coverage computation and validation.
- `scripts/bench/`: root benchmark specification, runners, aggregation, and
  baseline comparison.
- `../fontdone/tests/coverage_matrix_tests.rs`: FreeType matrix runner.

## Cleanup Rules

These paths are not source of truth and should stay untracked or be removed
before commit:

- `target/`
- `.pytest_cache/`
- `__pycache__/`
- `pillow-rs-js/node_modules/`
- `pillow-rs-js/pkg/`
- `pillow-rs-js/pkg_node/`
- `../fontdone/freetype/build*/`

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
|       |   |-- courb08.pil
|       |   |-- default_aileron.LICENSE.txt
|       |   |-- default_aileron.rs
|       |   |-- default_aileron.ttf
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
|       |-- pipeline.rs
|       `-- raster/
|           |-- buffer.rs
|           |-- color/
|           |   |-- blend.rs
|           |   |-- from_color.rs
|           |   |-- from_primitive.rs
|           |   |-- invert.rs
|           |   |-- mod.rs
|           |   |-- pixel_luma.rs
|           |   |-- pixel_rgb.rs
|           |   `-- types.rs
|           |-- dynamic.rs
|           |-- error.rs
|           |-- mod.rs
|           `-- traits/
|               |-- mod.rs
|               |-- pixel.rs
|               |-- primitive.rs
|               `-- view.rs
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
|   |   |-- run_font_rust_with_freetype_coverage.sh
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
    |-- deprecated/
    |   |-- fixtures/
    |   |   |-- .gitignore
    |   |   |-- input/
    |   |   |   |-- images/
    |   |   |   |   `-- .gitkeep
    |   |   |   |-- jsons/
    |   |   |   |   |-- .gitkeep
    |   |   |   |   |-- Image.alpha_composite.json
    |   |   |   |   |-- Image.apply_transparency.json
    |   |   |   |   |-- Image.close.json
    |   |   |   |   |-- Image.convert.json
    |   |   |   |   |-- Image.copy.json
    |   |   |   |   |-- Image.crop.json
    |   |   |   |   |-- Image.draft.json
    |   |   |   |   |-- Image.effect_spread.json
    |   |   |   |   |-- Image.entropy.json
    |   |   |   |   |-- Image.filter.json
    |   |   |   |   |-- Image.format.json
    |   |   |   |   |-- Image.frombytes.json
    |   |   |   |   |-- Image.get_child_images.json
    |   |   |   |   |-- Image.get_flattened_data.json
    |   |   |   |   |-- Image.getbands.json
    |   |   |   |   |-- Image.getbbox.json
    |   |   |   |   |-- Image.getchannel.json
    |   |   |   |   |-- Image.getcolors.json
    |   |   |   |   |-- Image.getdata.json
    |   |   |   |   |-- Image.getexif.json
    |   |   |   |   |-- Image.getextrema.json
    |   |   |   |   |-- Image.getpalette.json
    |   |   |   |   |-- Image.getpixel.json
    |   |   |   |   |-- Image.getprojection.json
    |   |   |   |   |-- Image.getxmp.json
    |   |   |   |   |-- Image.height.json
    |   |   |   |   |-- Image.histogram.json
    |   |   |   |   |-- Image.info.json
    |   |   |   |   |-- Image.load.json
    |   |   |   |   |-- Image.mode.json
    |   |   |   |   |-- Image.open.json
    |   |   |   |   |-- Image.paste.json
    |   |   |   |   |-- Image.point.json
    |   |   |   |   |-- Image.putalpha.json
    |   |   |   |   |-- Image.putdata.json
    |   |   |   |   |-- Image.putpalette.json
    |   |   |   |   |-- Image.putpixel.json
    |   |   |   |   |-- Image.quantize.json
    |   |   |   |   |-- Image.reduce.json
    |   |   |   |   |-- Image.remap_palette.json
    |   |   |   |   |-- Image.resize.json
    |   |   |   |   |-- Image.rotate.json
    |   |   |   |   |-- Image.save.json
    |   |   |   |   |-- Image.seek.json
    |   |   |   |   |-- Image.size.json
    |   |   |   |   |-- Image.split.json
    |   |   |   |   |-- Image.tell.json
    |   |   |   |   |-- Image.thumbnail.json
    |   |   |   |   |-- Image.tobitmap.json
    |   |   |   |   |-- Image.tobytes.json
    |   |   |   |   |-- Image.transform.json
    |   |   |   |   |-- Image.transpose.json
    |   |   |   |   |-- Image.verify.json
    |   |   |   |   |-- Image.width.json
    |   |   |   |   |-- ImageChops.add.json
    |   |   |   |   |-- ImageChops.add_modulo.json
    |   |   |   |   |-- ImageChops.blend.json
    |   |   |   |   |-- ImageChops.composite.json
    |   |   |   |   |-- ImageChops.constant.json
    |   |   |   |   |-- ImageChops.darker.json
    |   |   |   |   |-- ImageChops.difference.json
    |   |   |   |   |-- ImageChops.duplicate.json
    |   |   |   |   |-- ImageChops.hard_light.json
    |   |   |   |   |-- ImageChops.invert.json
    |   |   |   |   |-- ImageChops.lighter.json
    |   |   |   |   |-- ImageChops.logical_and.json
    |   |   |   |   |-- ImageChops.logical_or.json
    |   |   |   |   |-- ImageChops.logical_xor.json
    |   |   |   |   |-- ImageChops.multiply.json
    |   |   |   |   |-- ImageChops.offset.json
    |   |   |   |   |-- ImageChops.overlay.json
    |   |   |   |   |-- ImageChops.screen.json
    |   |   |   |   |-- ImageChops.soft_light.json
    |   |   |   |   |-- ImageChops.subtract.json
    |   |   |   |   |-- ImageChops.subtract_modulo.json
    |   |   |   |   |-- ImageColor.getcolor.json
    |   |   |   |   |-- ImageColor.getrgb.json
    |   |   |   |   |-- ImageDraw.arc.json
    |   |   |   |   |-- ImageDraw.bitmap.json
    |   |   |   |   |-- ImageDraw.chord.json
    |   |   |   |   |-- ImageDraw.circle.json
    |   |   |   |   |-- ImageDraw.ellipse.json
    |   |   |   |   |-- ImageDraw.getfont.json
    |   |   |   |   |-- ImageDraw.line.json
    |   |   |   |   |-- ImageDraw.multiline_text.json
    |   |   |   |   |-- ImageDraw.multiline_textbbox.json
    |   |   |   |   |-- ImageDraw.pieslice.json
    |   |   |   |   |-- ImageDraw.point.json
    |   |   |   |   |-- ImageDraw.polygon.json
    |   |   |   |   |-- ImageDraw.rectangle.json
    |   |   |   |   |-- ImageDraw.regular_polygon.json
    |   |   |   |   |-- ImageDraw.rounded_rectangle.json
    |   |   |   |   |-- ImageDraw.shape.json
    |   |   |   |   |-- ImageDraw.text.json
    |   |   |   |   |-- ImageDraw.textbbox.json
    |   |   |   |   |-- ImageDraw.textlength.json
    |   |   |   |   |-- ImageEnhance.Brightness.json
    |   |   |   |   |-- ImageEnhance.Color.json
    |   |   |   |   |-- ImageEnhance.Contrast.json
    |   |   |   |   |-- ImageEnhance.Sharpness.json
    |   |   |   |   |-- ImageFilter.BLUR.json
    |   |   |   |   |-- ImageFilter.BoxBlur.json
    |   |   |   |   |-- ImageFilter.CONTOUR.json
    |   |   |   |   |-- ImageFilter.Color3DLUT.json
    |   |   |   |   |-- ImageFilter.DETAIL.json
    |   |   |   |   |-- ImageFilter.EDGE_ENHANCE.json
    |   |   |   |   |-- ImageFilter.EDGE_ENHANCE_MORE.json
    |   |   |   |   |-- ImageFilter.EMBOSS.json
    |   |   |   |   |-- ImageFilter.FIND_EDGES.json
    |   |   |   |   |-- ImageFilter.GaussianBlur.json
    |   |   |   |   |-- ImageFilter.Kernel.json
    |   |   |   |   |-- ImageFilter.MaxFilter.json
    |   |   |   |   |-- ImageFilter.MedianFilter.json
    |   |   |   |   |-- ImageFilter.MinFilter.json
    |   |   |   |   |-- ImageFilter.ModeFilter.json
    |   |   |   |   |-- ImageFilter.RankFilter.json
    |   |   |   |   |-- ImageFilter.SHARPEN.json
    |   |   |   |   |-- ImageFilter.SMOOTH.json
    |   |   |   |   |-- ImageFilter.SMOOTH_MORE.json
    |   |   |   |   |-- ImageFilter.UnsharpMask.json
    |   |   |   |   |-- ImageFont.FreeTypeFont.json
    |   |   |   |   |-- ImageFont.ImageFont.getbbox.json
    |   |   |   |   |-- ImageFont.ImageFont.getlength.json
    |   |   |   |   |-- ImageFont.ImageFont.getmask.json
    |   |   |   |   |-- ImageFont.ImageFont.json
    |   |   |   |   |-- ImageFont.MAX_STRING_LENGTH.json
    |   |   |   |   |-- ImageFont.TransposedFont.getbbox.json
    |   |   |   |   |-- ImageFont.TransposedFont.getlength.json
    |   |   |   |   |-- ImageFont.TransposedFont.getmask.json
    |   |   |   |   |-- ImageFont.TransposedFont.json
    |   |   |   |   |-- ImageFont.font_variant.json
    |   |   |   |   |-- ImageFont.get_variation_axes.json
    |   |   |   |   |-- ImageFont.get_variation_names.json
    |   |   |   |   |-- ImageFont.getbbox.json
    |   |   |   |   |-- ImageFont.getlength.json
    |   |   |   |   |-- ImageFont.getmask.json
    |   |   |   |   |-- ImageFont.getmask2.json
    |   |   |   |   |-- ImageFont.getmetrics.json
    |   |   |   |   |-- ImageFont.getname.json
    |   |   |   |   |-- ImageFont.load.json
    |   |   |   |   |-- ImageFont.load_default.json
    |   |   |   |   |-- ImageFont.load_default_imagefont.json
    |   |   |   |   |-- ImageFont.load_path.json
    |   |   |   |   |-- ImageFont.set_variation_by_axes.json
    |   |   |   |   |-- ImageFont.set_variation_by_name.json
    |   |   |   |   |-- ImageFont.truetype.json
    |   |   |   |   |-- ImageModule.alpha_composite.json
    |   |   |   |   |-- ImageModule.blend.json
    |   |   |   |   |-- ImageModule.composite.json
    |   |   |   |   |-- ImageModule.effect_mandelbrot.json
    |   |   |   |   |-- ImageModule.effect_noise.json
    |   |   |   |   |-- ImageModule.eval.json
    |   |   |   |   |-- ImageModule.fromarray.json
    |   |   |   |   |-- ImageModule.frombuffer.json
    |   |   |   |   |-- ImageModule.frombytes.json
    |   |   |   |   |-- ImageModule.linear_gradient.json
    |   |   |   |   |-- ImageModule.merge.json
    |   |   |   |   |-- ImageModule.new.colors.json
    |   |   |   |   |-- ImageModule.new.json
    |   |   |   |   |-- ImageModule.open.json
    |   |   |   |   |-- ImageModule.radial_gradient.json
    |   |   |   |   |-- ImageOps.autocontrast.json
    |   |   |   |   |-- ImageOps.colorize.json
    |   |   |   |   |-- ImageOps.contain.json
    |   |   |   |   |-- ImageOps.cover.json
    |   |   |   |   |-- ImageOps.crop.json
    |   |   |   |   |-- ImageOps.deform.json
    |   |   |   |   |-- ImageOps.equalize.json
    |   |   |   |   |-- ImageOps.exif_transpose.json
    |   |   |   |   |-- ImageOps.expand.json
    |   |   |   |   |-- ImageOps.fit.json
    |   |   |   |   |-- ImageOps.flip.json
    |   |   |   |   |-- ImageOps.grayscale.json
    |   |   |   |   |-- ImageOps.invert.json
    |   |   |   |   |-- ImageOps.mirror.json
    |   |   |   |   |-- ImageOps.pad.json
    |   |   |   |   |-- ImageOps.posterize.json
    |   |   |   |   |-- ImageOps.scale.json
    |   |   |   |   |-- ImageOps.solarize.json
    |   |   |   |   |-- ImagePalette.copy.json
    |   |   |   |   |-- ImagePalette.getcolor.json
    |   |   |   |   |-- ImagePalette.getdata.json
    |   |   |   |   |-- ImagePalette.save.json
    |   |   |   |   |-- ImagePalette.tobytes.json
    |   |   |   |   |-- ImageSequence.Iterator.json
    |   |   |   |   `-- ImageStat.Stat.json
    |   |   |   `-- raws/
    |   |   |       `-- .gitkeep
    |   |   `-- outputs/
    |   |       `-- jsons/
    |   |           |-- Image.format.json
    |   |           |-- Image.height.json
    |   |           |-- Image.info.json
    |   |           |-- Image.mode.json
    |   |           |-- Image.size.json
    |   |           |-- Image.width.json
    |   |           |-- ImageFont.ImageFont.getbbox.json
    |   |           |-- ImageFont.ImageFont.getlength.json
    |   |           |-- ImageFont.ImageFont.getmask.json
    |   |           |-- ImageFont.MAX_STRING_LENGTH.json
    |   |           |-- ImageFont.TransposedFont.getbbox.json
    |   |           |-- ImageFont.TransposedFont.getlength.json
    |   |           `-- ImageFont.TransposedFont.getmask.json
    |   `-- fixtures_2/
    |       |-- .gitignore
    |       |-- input/
    |       |   |-- images/
    |       |   |   `-- .gitkeep
    |       |   |-- jsons/
    |       |   |   |-- Image.alpha_composite.json
    |       |   |   |-- Image.apply_transparency.json
    |       |   |   |-- Image.close.json
    |       |   |   |-- Image.convert.json
    |       |   |   |-- Image.copy.json
    |       |   |   |-- Image.crop.json
    |       |   |   |-- Image.draft.json
    |       |   |   |-- Image.effect_spread.json
    |       |   |   |-- Image.entropy.json
    |       |   |   |-- Image.filter.json
    |       |   |   |-- Image.frombytes.json
    |       |   |   |-- Image.get_child_images.json
    |       |   |   |-- Image.get_flattened_data.json
    |       |   |   |-- Image.getbands.json
    |       |   |   |-- Image.getbbox.json
    |       |   |   |-- Image.getchannel.json
    |       |   |   |-- Image.getcolors.json
    |       |   |   |-- Image.getdata.json
    |       |   |   |-- Image.getexif.json
    |       |   |   |-- Image.getextrema.json
    |       |   |   |-- Image.getpalette.json
    |       |   |   |-- Image.getpixel.json
    |       |   |   |-- Image.getprojection.json
    |       |   |   |-- Image.getxmp.json
    |       |   |   |-- Image.histogram.json
    |       |   |   |-- Image.load.json
    |       |   |   |-- Image.open.json
    |       |   |   |-- Image.paste.json
    |       |   |   |-- Image.point.json
    |       |   |   |-- Image.putalpha.json
    |       |   |   |-- Image.putdata.json
    |       |   |   |-- Image.putpalette.json
    |       |   |   |-- Image.putpixel.json
    |       |   |   |-- Image.quantize.json
    |       |   |   |-- Image.reduce.json
    |       |   |   |-- Image.remap_palette.json
    |       |   |   |-- Image.resize.json
    |       |   |   |-- Image.rotate.json
    |       |   |   |-- Image.save.json
    |       |   |   |-- Image.seek.json
    |       |   |   |-- Image.split.json
    |       |   |   |-- Image.tell.json
    |       |   |   |-- Image.thumbnail.json
    |       |   |   |-- Image.tobitmap.json
    |       |   |   |-- Image.tobytes.json
    |       |   |   |-- Image.transform.json
    |       |   |   |-- Image.transpose.json
    |       |   |   |-- Image.verify.json
    |       |   |   |-- ImageChops.add.json
    |       |   |   |-- ImageChops.add_modulo.json
    |       |   |   |-- ImageChops.blend.json
    |       |   |   |-- ImageChops.composite.json
    |       |   |   |-- ImageChops.constant.json
    |       |   |   |-- ImageChops.darker.json
    |       |   |   |-- ImageChops.difference.json
    |       |   |   |-- ImageChops.duplicate.json
    |       |   |   |-- ImageChops.hard_light.json
    |       |   |   |-- ImageChops.invert.json
    |       |   |   |-- ImageChops.lighter.json
    |       |   |   |-- ImageChops.logical_and.json
    |       |   |   |-- ImageChops.logical_or.json
    |       |   |   |-- ImageChops.logical_xor.json
    |       |   |   |-- ImageChops.multiply.json
    |       |   |   |-- ImageChops.offset.json
    |       |   |   |-- ImageChops.overlay.json
    |       |   |   |-- ImageChops.screen.json
    |       |   |   |-- ImageChops.soft_light.json
    |       |   |   |-- ImageChops.subtract.json
    |       |   |   |-- ImageChops.subtract_modulo.json
    |       |   |   |-- ImageColor.getcolor.json
    |       |   |   |-- ImageColor.getrgb.json
    |       |   |   |-- ImageDraw.arc.json
    |       |   |   |-- ImageDraw.bitmap.json
    |       |   |   |-- ImageDraw.chord.json
    |       |   |   |-- ImageDraw.circle.json
    |       |   |   |-- ImageDraw.ellipse.json
    |       |   |   |-- ImageDraw.getfont.json
    |       |   |   |-- ImageDraw.line.json
    |       |   |   |-- ImageDraw.multiline_text.json
    |       |   |   |-- ImageDraw.multiline_textbbox.json
    |       |   |   |-- ImageDraw.pieslice.json
    |       |   |   |-- ImageDraw.point.json
    |       |   |   |-- ImageDraw.polygon.json
    |       |   |   |-- ImageDraw.rectangle.json
    |       |   |   |-- ImageDraw.regular_polygon.json
    |       |   |   |-- ImageDraw.rounded_rectangle.json
    |       |   |   |-- ImageDraw.shape.json
    |       |   |   |-- ImageDraw.text.json
    |       |   |   |-- ImageDraw.textbbox.json
    |       |   |   |-- ImageDraw.textlength.json
    |       |   |   |-- ImageEnhance.Brightness.json
    |       |   |   |-- ImageEnhance.Color.json
    |       |   |   |-- ImageEnhance.Contrast.json
    |       |   |   |-- ImageEnhance.Sharpness.json
    |       |   |   |-- ImageFilter.BLUR.json
    |       |   |   |-- ImageFilter.BoxBlur.json
    |       |   |   |-- ImageFilter.CONTOUR.json
    |       |   |   |-- ImageFilter.Color3DLUT.json
    |       |   |   |-- ImageFilter.DETAIL.json
    |       |   |   |-- ImageFilter.EDGE_ENHANCE.json
    |       |   |   |-- ImageFilter.EDGE_ENHANCE_MORE.json
    |       |   |   |-- ImageFilter.EMBOSS.json
    |       |   |   |-- ImageFilter.FIND_EDGES.json
    |       |   |   |-- ImageFilter.GaussianBlur.json
    |       |   |   |-- ImageFilter.Kernel.json
    |       |   |   |-- ImageFilter.MaxFilter.json
    |       |   |   |-- ImageFilter.MedianFilter.json
    |       |   |   |-- ImageFilter.MinFilter.json
    |       |   |   |-- ImageFilter.ModeFilter.json
    |       |   |   |-- ImageFilter.RankFilter.json
    |       |   |   |-- ImageFilter.SHARPEN.json
    |       |   |   |-- ImageFilter.SMOOTH.json
    |       |   |   |-- ImageFilter.SMOOTH_MORE.json
    |       |   |   |-- ImageFilter.UnsharpMask.json
    |       |   |   |-- ImageFont.FreeTypeFont.json
    |       |   |   |-- ImageFont.ImageFont.json
    |       |   |   |-- ImageFont.font_variant.json
    |       |   |   |-- ImageFont.get_variation_axes.json
    |       |   |   |-- ImageFont.get_variation_names.json
    |       |   |   |-- ImageFont.getbbox.json
    |       |   |   |-- ImageFont.getlength.json
    |       |   |   |-- ImageFont.getmask.json
    |       |   |   |-- ImageFont.getmask2.json
    |       |   |   |-- ImageFont.getmetrics.json
    |       |   |   |-- ImageFont.getname.json
    |       |   |   |-- ImageFont.load.json
    |       |   |   |-- ImageFont.load_default.json
    |       |   |   |-- ImageFont.load_default_imagefont.json
    |       |   |   |-- ImageFont.load_path.json
    |       |   |   |-- ImageFont.set_variation_by_axes.json
    |       |   |   |-- ImageFont.set_variation_by_name.json
    |       |   |   |-- ImageFont.truetype.json
    |       |   |   |-- ImageModule.alpha_composite.json
    |       |   |   |-- ImageModule.blend.json
    |       |   |   |-- ImageModule.composite.json
    |       |   |   |-- ImageModule.effect_mandelbrot.json
    |       |   |   |-- ImageModule.effect_noise.json
    |       |   |   |-- ImageModule.eval.json
    |       |   |   |-- ImageModule.fromarray.json
    |       |   |   |-- ImageModule.frombuffer.json
    |       |   |   |-- ImageModule.frombytes.json
    |       |   |   |-- ImageModule.linear_gradient.json
    |       |   |   |-- ImageModule.merge.json
    |       |   |   |-- ImageModule.new.colors.json
    |       |   |   |-- ImageModule.new.json
    |       |   |   |-- ImageModule.open.json
    |       |   |   |-- ImageModule.radial_gradient.json
    |       |   |   |-- ImageOps.autocontrast.json
    |       |   |   |-- ImageOps.colorize.json
    |       |   |   |-- ImageOps.contain.json
    |       |   |   |-- ImageOps.cover.json
    |       |   |   |-- ImageOps.crop.json
    |       |   |   |-- ImageOps.deform.json
    |       |   |   |-- ImageOps.equalize.json
    |       |   |   |-- ImageOps.exif_transpose.json
    |       |   |   |-- ImageOps.expand.json
    |       |   |   |-- ImageOps.fit.json
    |       |   |   |-- ImageOps.flip.json
    |       |   |   |-- ImageOps.grayscale.json
    |       |   |   |-- ImageOps.invert.json
    |       |   |   |-- ImageOps.mirror.json
    |       |   |   |-- ImageOps.pad.json
    |       |   |   |-- ImageOps.posterize.json
    |       |   |   |-- ImageOps.scale.json
    |       |   |   |-- ImageOps.solarize.json
    |       |   |   |-- ImagePalette.copy.json
    |       |   |   |-- ImagePalette.getcolor.json
    |       |   |   |-- ImagePalette.getdata.json
    |       |   |   |-- ImagePalette.save.json
    |       |   |   |-- ImagePalette.tobytes.json
    |       |   |   |-- ImageSequence.Iterator.json
    |       |   |   `-- ImageStat.Stat.json
    |       |   `-- raws/
    |       |       `-- .gitkeep
    |       `-- outputs/
    |           |-- jsons/
    |           |   |-- Image.alpha_composite.json
    |           |   |-- Image.apply_transparency.json
    |           |   |-- Image.close.json
    |           |   |-- Image.convert.json
    |           |   |-- Image.copy.json
    |           |   |-- Image.crop.json
    |           |   |-- Image.draft.json
    |           |   |-- Image.effect_spread.json
    |           |   |-- Image.entropy.json
    |           |   |-- Image.filter.json
    |           |   |-- Image.frombytes.json
    |           |   |-- Image.get_child_images.json
    |           |   |-- Image.get_flattened_data.json
    |           |   |-- Image.getbands.json
    |           |   |-- Image.getbbox.json
    |           |   |-- Image.getchannel.json
    |           |   |-- Image.getcolors.json
    |           |   |-- Image.getdata.json
    |           |   |-- Image.getexif.json
    |           |   |-- Image.getextrema.json
    |           |   |-- Image.getpalette.json
    |           |   |-- Image.getpixel.json
    |           |   |-- Image.getprojection.json
    |           |   |-- Image.getxmp.json
    |           |   |-- Image.histogram.json
    |           |   |-- Image.load.json
    |           |   |-- Image.open.json
    |           |   |-- Image.paste.json
    |           |   |-- Image.point.json
    |           |   |-- Image.putalpha.json
    |           |   |-- Image.putdata.json
    |           |   |-- Image.putpalette.json
    |           |   |-- Image.putpixel.json
    |           |   |-- Image.quantize.json
    |           |   |-- Image.reduce.json
    |           |   |-- Image.remap_palette.json
    |           |   |-- Image.resize.json
    |           |   |-- Image.rotate.json
    |           |   |-- Image.save.json
    |           |   |-- Image.seek.json
    |           |   |-- Image.split.json
    |           |   |-- Image.tell.json
    |           |   |-- Image.thumbnail.json
    |           |   |-- Image.tobitmap.json
    |           |   |-- Image.tobytes.json
    |           |   |-- Image.transform.json
    |           |   |-- Image.transpose.json
    |           |   |-- Image.verify.json
    |           |   |-- ImageChops.add.json
    |           |   |-- ImageChops.add_modulo.json
    |           |   |-- ImageChops.blend.json
    |           |   |-- ImageChops.composite.json
    |           |   |-- ImageChops.constant.json
    |           |   |-- ImageChops.darker.json
    |           |   |-- ImageChops.difference.json
    |           |   |-- ImageChops.duplicate.json
    |           |   |-- ImageChops.hard_light.json
    |           |   |-- ImageChops.invert.json
    |           |   |-- ImageChops.lighter.json
    |           |   |-- ImageChops.logical_and.json
    |           |   |-- ImageChops.logical_or.json
    |           |   |-- ImageChops.logical_xor.json
    |           |   |-- ImageChops.multiply.json
    |           |   |-- ImageChops.offset.json
    |           |   |-- ImageChops.overlay.json
    |           |   |-- ImageChops.screen.json
    |           |   |-- ImageChops.soft_light.json
    |           |   |-- ImageChops.subtract.json
    |           |   |-- ImageChops.subtract_modulo.json
    |           |   |-- ImageColor.getcolor.json
    |           |   |-- ImageColor.getrgb.json
    |           |   |-- ImageDraw.arc.json
    |           |   |-- ImageDraw.bitmap.json
    |           |   |-- ImageDraw.chord.json
    |           |   |-- ImageDraw.circle.json
    |           |   |-- ImageDraw.ellipse.json
    |           |   |-- ImageDraw.getfont.json
    |           |   |-- ImageDraw.line.json
    |           |   |-- ImageDraw.multiline_text.json
    |           |   |-- ImageDraw.multiline_textbbox.json
    |           |   |-- ImageDraw.pieslice.json
    |           |   |-- ImageDraw.point.json
    |           |   |-- ImageDraw.polygon.json
    |           |   |-- ImageDraw.rectangle.json
    |           |   |-- ImageDraw.regular_polygon.json
    |           |   |-- ImageDraw.rounded_rectangle.json
    |           |   |-- ImageDraw.shape.json
    |           |   |-- ImageDraw.text.json
    |           |   |-- ImageDraw.textbbox.json
    |           |   |-- ImageDraw.textlength.json
    |           |   |-- ImageEnhance.Brightness.json
    |           |   |-- ImageEnhance.Color.json
    |           |   |-- ImageEnhance.Contrast.json
    |           |   |-- ImageEnhance.Sharpness.json
    |           |   |-- ImageFilter.BLUR.json
    |           |   |-- ImageFilter.BoxBlur.json
    |           |   |-- ImageFilter.CONTOUR.json
    |           |   |-- ImageFilter.Color3DLUT.json
    |           |   |-- ImageFilter.DETAIL.json
    |           |   |-- ImageFilter.EDGE_ENHANCE.json
    |           |   |-- ImageFilter.EDGE_ENHANCE_MORE.json
    |           |   |-- ImageFilter.EMBOSS.json
    |           |   |-- ImageFilter.FIND_EDGES.json
    |           |   |-- ImageFilter.GaussianBlur.json
    |           |   |-- ImageFilter.Kernel.json
    |           |   |-- ImageFilter.MaxFilter.json
    |           |   |-- ImageFilter.MedianFilter.json
    |           |   |-- ImageFilter.MinFilter.json
    |           |   |-- ImageFilter.ModeFilter.json
    |           |   |-- ImageFilter.RankFilter.json
    |           |   |-- ImageFilter.SHARPEN.json
    |           |   |-- ImageFilter.SMOOTH.json
    |           |   |-- ImageFilter.SMOOTH_MORE.json
    |           |   |-- ImageFilter.UnsharpMask.json
    |           |   |-- ImageFont.FreeTypeFont.json
    |           |   |-- ImageFont.ImageFont.json
    |           |   |-- ImageFont.font_variant.json
    |           |   |-- ImageFont.get_variation_axes.json
    |           |   |-- ImageFont.get_variation_names.json
    |           |   |-- ImageFont.getbbox.json
    |           |   |-- ImageFont.getlength.json
    |           |   |-- ImageFont.getmask.json
    |           |   |-- ImageFont.getmask2.json
    |           |   |-- ImageFont.getmetrics.json
    |           |   |-- ImageFont.getname.json
    |           |   |-- ImageFont.load.json
    |           |   |-- ImageFont.load_default.json
    |           |   |-- ImageFont.load_default_imagefont.json
    |           |   |-- ImageFont.load_path.json
    |           |   |-- ImageFont.set_variation_by_axes.json
    |           |   |-- ImageFont.set_variation_by_name.json
    |           |   |-- ImageFont.truetype.json
    |           |   |-- ImageModule.alpha_composite.json
    |           |   |-- ImageModule.blend.json
    |           |   |-- ImageModule.composite.json
    |           |   |-- ImageModule.effect_mandelbrot.json
    |           |   |-- ImageModule.effect_noise.json
    |           |   |-- ImageModule.eval.json
    |           |   |-- ImageModule.fromarray.json
    |           |   |-- ImageModule.frombuffer.json
    |           |   |-- ImageModule.frombytes.json
    |           |   |-- ImageModule.linear_gradient.json
    |           |   |-- ImageModule.merge.json
    |           |   |-- ImageModule.new.colors.json
    |           |   |-- ImageModule.new.json
    |           |   |-- ImageModule.open.json
    |           |   |-- ImageModule.radial_gradient.json
    |           |   |-- ImageOps.autocontrast.json
    |           |   |-- ImageOps.colorize.json
    |           |   |-- ImageOps.contain.json
    |           |   |-- ImageOps.cover.json
    |           |   |-- ImageOps.crop.json
    |           |   |-- ImageOps.deform.json
    |           |   |-- ImageOps.equalize.json
    |           |   |-- ImageOps.exif_transpose.json
    |           |   |-- ImageOps.expand.json
    |           |   |-- ImageOps.fit.json
    |           |   |-- ImageOps.flip.json
    |           |   |-- ImageOps.grayscale.json
    |           |   |-- ImageOps.invert.json
    |           |   |-- ImageOps.mirror.json
    |           |   |-- ImageOps.pad.json
    |           |   |-- ImageOps.posterize.json
    |           |   |-- ImageOps.scale.json
    |           |   |-- ImageOps.solarize.json
    |           |   |-- ImagePalette.copy.json
    |           |   |-- ImagePalette.getcolor.json
    |           |   |-- ImagePalette.getdata.json
    |           |   |-- ImagePalette.save.json
    |           |   |-- ImagePalette.tobytes.json
    |           |   |-- ImageSequence.Iterator.json
    |           |   `-- ImageStat.Stat.json
    |           `-- raws/
    |               |-- Image.convert_Image_convert_CMYK_suite1.bin
    |               |-- Image.convert_Image_convert_F_suite1.bin
    |               |-- Image.convert_Image_convert_HSV_suite1.bin
    |               |-- Image.convert_Image_convert_I_suite1.bin
    |               |-- Image.convert_Image_convert_YCbCr_suite1.bin
    |               |-- Image.copy_Image_copy_P_suite1.bin
    |               |-- Image.crop_Image_crop_CMYK_suite1.bin
    |               |-- Image.crop_Image_crop_F_suite1.bin
    |               |-- Image.crop_Image_crop_HSV_suite1.bin
    |               |-- Image.crop_Image_crop_I_suite1.bin
    |               |-- Image.crop_Image_crop_P_suite1.bin
    |               |-- Image.crop_Image_crop_YCbCr_suite1.bin
    |               |-- Image.draft_Image_draft_CMYK_suite1_1.bin
    |               |-- Image.draft_Image_draft_P_suite1_1.bin
    |               |-- Image.effect_spread_Image_effect_spread_CMYK_suite1.bin
    |               |-- Image.effect_spread_Image_effect_spread_P_suite1.bin
    |               |-- Image.filter_Image.filter_CMYK_suite1.bin
    |               |-- Image.get_flattened_data_Image_get_flattened_data_L_suite1.typed.bin
    |               |-- Image.get_flattened_data_Image_get_flattened_data_RGB_suite1.typed.bin
    |               |-- Image.getdata_Image_getdata_LA_suite1.typed.bin
    |               |-- Image.getdata_Image_getdata_L_suite1.typed.bin
    |               |-- Image.getdata_Image_getdata_RGBA_suite1.typed.bin
    |               |-- Image.getdata_Image_getdata_RGB_suite1.typed.bin
    |               |-- Image.paste_Image_paste_CMYK_suite1_1.bin
    |               |-- Image.paste_Image_paste_P_suite1_1.bin
    |               |-- Image.point_Image_point_P_suite1.bin
    |               |-- Image.putalpha_Image_putalpha_P_suite1_1.bin
    |               |-- Image.putdata_Image_putdata_CMYK_suite1_1.bin
    |               |-- Image.putdata_Image_putdata_F_suite1_1.bin
    |               |-- Image.putdata_Image_putdata_I_suite1_1.bin
    |               |-- Image.putdata_Image_putdata_PA_suite1_1.bin
    |               |-- Image.putdata_Image_putdata_P_suite1_1.bin
    |               |-- Image.putpalette_Image_putpalette_L_suite1_1.bin
    |               |-- Image.putpalette_Image_putpalette_P_suite1_1.bin
    |               |-- Image.quantize_Image_quantize_L_suite1.bin
    |               |-- Image.quantize_Image_quantize_RGBA_suite1.bin
    |               |-- Image.quantize_Image_quantize_RGB_suite1.bin
    |               |-- Image.reduce_Image_reduce_CMYK_suite1.bin
    |               |-- Image.remap_palette_Image_remap_palette_L_suite1.bin
    |               |-- Image.remap_palette_Image_remap_palette_P_suite1.bin
    |               |-- Image.resize_Image_resize_CMYK_suite1.bin
    |               |-- Image.resize_Image_resize_HSV_suite1.bin
    |               |-- Image.resize_Image_resize_I_suite1.bin
    |               |-- Image.resize_Image_resize_P_suite1.bin
    |               |-- Image.resize_Image_resize_YCbCr_suite1.bin
    |               |-- Image.rotate_Image_rotate_CMYK_suite1.bin
    |               |-- Image.rotate_Image_rotate_P_suite1.bin
    |               |-- Image.thumbnail_Image_thumbnail_P_suite1_1.bin
    |               |-- Image.tobitmap_Image_tobitmap_1_suite1.bin
    |               |-- Image.tobytes_Image_tobytes_1_suite1.bin
    |               |-- Image.tobytes_Image_tobytes_LA_suite1.bin
    |               |-- Image.tobytes_Image_tobytes_L_suite1.bin
    |               |-- Image.tobytes_Image_tobytes_P_suite1.bin
    |               |-- Image.tobytes_Image_tobytes_RGBA_suite1.bin
    |               |-- Image.tobytes_Image_tobytes_RGB_suite1.bin
    |               |-- Image.transform_Image_transform_CMYK_suite1.bin
    |               |-- Image.transform_Image_transform_P_suite1.bin
    |               |-- Image.transpose_Image_transpose_P_suite1.bin
    |               |-- ImageChops.duplicate_ImageChops_duplicate_P_suite1.bin
    |               |-- ImageChops.invert_ImageChops.invert_P_suite1.bin
    |               |-- ImageDraw.arc_ImageDraw_arc_CMYK_suite1_1.bin
    |               |-- ImageDraw.arc_ImageDraw_arc_P_suite1_1.bin
    |               |-- ImageDraw.chord_ImageDraw_chord_CMYK_suite1_1.bin
    |               |-- ImageDraw.chord_ImageDraw_chord_P_suite1_1.bin
    |               |-- ImageDraw.circle_ImageDraw_circle_CMYK_suite1_1.bin
    |               |-- ImageDraw.circle_ImageDraw_circle_P_suite1_1.bin
    |               |-- ImageDraw.ellipse_ImageDraw_ellipse_CMYK_suite1_1.bin
    |               |-- ImageDraw.ellipse_ImageDraw_ellipse_P_suite1_1.bin
    |               |-- ImageDraw.line_ImageDraw_line_CMYK_suite1_1.bin
    |               |-- ImageDraw.line_ImageDraw_line_P_suite1_1.bin
    |               |-- ImageDraw.multiline_text_ImageDraw_multiline_text_CMYK_suite1_1.bin
    |               |-- ImageDraw.multiline_text_ImageDraw_multiline_text_P_suite1_1.bin
    |               |-- ImageDraw.pieslice_ImageDraw_pieslice_CMYK_suite1_1.bin
    |               |-- ImageDraw.pieslice_ImageDraw_pieslice_P_suite1_1.bin
    |               |-- ImageDraw.point_ImageDraw_point_CMYK_suite1_1.bin
    |               |-- ImageDraw.point_ImageDraw_point_P_suite1_1.bin
    |               |-- ImageDraw.polygon_ImageDraw_polygon_CMYK_suite1_1.bin
    |               |-- ImageDraw.polygon_ImageDraw_polygon_P_suite1_1.bin
    |               |-- ImageDraw.rectangle_ImageDraw_rectangle_CMYK_suite1_1.bin
    |               |-- ImageDraw.rectangle_ImageDraw_rectangle_P_suite1_1.bin
    |               |-- ImageDraw.regular_polygon_ImageDraw_regular_polygon_CMYK_suite1_1.bin
    |               |-- ImageDraw.regular_polygon_ImageDraw_regular_polygon_P_suite1_1.bin
    |               |-- ImageDraw.rounded_rectangle_ImageDraw_rounded_rectangle_CMYK_suite1_1.bin
    |               |-- ImageDraw.rounded_rectangle_ImageDraw_rounded_rectangle_P_suite1_1.bin
    |               |-- ImageDraw.text_ImageDraw_text_CMYK_suite1_1.bin
    |               |-- ImageDraw.text_ImageDraw_text_P_suite1_1.bin
    |               |-- ImageFilter.BLUR_ImageFilter_BLUR_CMYK_suite1.bin
    |               |-- ImageFilter.SHARPEN_ImageFilter.SHARPEN_HSV_suite1.bin
    |               |-- ImageFilter.SHARPEN_ImageFilter.SHARPEN_YCbCr_suite1.bin
    |               |-- ImageFilter.SHARPEN_ImageFilter_SHARPEN_CMYK_suite1.bin
    |               |-- ImageFilter.SHARPEN_ImageFilter_SHARPEN_I_suite1.bin
    |               |-- ImageModule.composite_ImageModule.composite_P_suite1.bin
    |               |-- ImageModule.eval_ImageModule.eval_1_suite1.bin
    |               |-- ImageModule.eval_ImageModule.eval_CMYK_suite1.bin
    |               |-- ImageModule.eval_ImageModule.eval_P_suite1.bin
    |               |-- ImageModule.frombytes_frombytes_CMYK_suite1.bin
    |               |-- ImageModule.frombytes_frombytes_P_suite1.bin
    |               |-- ImageModule.new.colors_cmyk_50x50_suite1.bin
    |               |-- ImageModule.new.colors_float_50x50_suite1.bin
    |               |-- ImageModule.new.colors_hsv_50x50_suite1.bin
    |               |-- ImageModule.new.colors_int32_50x50_suite1.bin
    |               |-- ImageModule.new.colors_ycbcr_50x50_suite1.bin
    |               |-- ImageModule.new_ImageModule_new_CMYK_suite1.bin
    |               |-- ImageModule.new_ImageModule_new_F_suite1.bin
    |               |-- ImageModule.new_ImageModule_new_HSV_suite1.bin
    |               |-- ImageModule.new_ImageModule_new_I_suite1.bin
    |               |-- ImageModule.new_ImageModule_new_P_suite1.bin
    |               |-- ImageModule.new_ImageModule_new_YCbCr_suite1.bin
    |               |-- ImageModule.open_ImageModule.open_CMYK_suite1.bin
    |               |-- ImageModule.open_ImageModule.open_F_suite1.bin
    |               |-- ImageModule.open_ImageModule.open_I_suite1.bin
    |               |-- ImageModule.open_ImageModule.open_P_suite1.bin
    |               |-- ImagePalette.tobytes_ImagePalette_tobytes_L_suite1.bin
    |               |-- ImagePalette.tobytes_ImagePalette_tobytes_P_suite1.bin
    |               `-- ImagePalette.tobytes_ImagePalette_tobytes_RGB_suite1.bin
    |-- engine.py
    |-- fixture_coverage.py
    |-- oracles/
    |   `-- image_open_inputs.json
    |-- test_apply_transparency_oracle.py
    |-- test_drawing_oracle.py
    |-- test_eval_errors.py
    |-- test_fromarray_descriptor_oracle.py
    |-- test_imagefont_facade_oracle.py
    |-- test_pa_mutations.py
    |-- test_parity.py
    |-- test_paste_oracle.py
    `-- test_putdata_parity.py
```
<!-- END GENERATED CODE TREE -->
