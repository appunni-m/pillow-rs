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
|   |   |-- color/
|   |   |   |-- conversion.rs
|   |   |   |-- grayscale.rs
|   |   |   |-- mod.rs
|   |   |   |-- palette.rs
|   |   |   |-- parse.rs
|   |   |   |-- photo_ycc.rs
|   |   |   `-- resolve.rs
|   |   |-- compute/
|   |   |   |-- backend_state.rs
|   |   |   |-- eager/
|   |   |   |   `-- generators.rs
|   |   |   |-- eager.rs
|   |   |   |-- execution/
|   |   |   |   `-- terminals.rs
|   |   |   |-- execution.rs
|   |   |   |-- execution_context.rs
|   |   |   |-- helpers/
|   |   |   |   |-- effects/
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   `-- random.rs
|   |   |   |   |-- enhance.rs
|   |   |   |   |-- filters/
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   `-- parameters.rs
|   |   |   |   |-- geometry/
|   |   |   |   |   |-- affine.rs
|   |   |   |   |   `-- mod.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- put_data.rs
|   |   |   |   `-- resampling/
|   |   |   |       |-- luma16_samples.rs
|   |   |   |       |-- mod.rs
|   |   |   |       |-- rounding.rs
|   |   |   |       `-- thumbnail.rs
|   |   |   |-- mod.rs
|   |   |   |-- phase_telemetry.rs
|   |   |   |-- pool_cpu/
|   |   |   |   |-- batch_execution.rs
|   |   |   |   |-- executor.rs
|   |   |   |   |-- helpers/
|   |   |   |   |   |-- bytes/
|   |   |   |   |   |   |-- hidden_lanes.rs
|   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |-- native_invert.rs
|   |   |   |   |   |   |-- preblend.rs
|   |   |   |   |   |   |-- unpremultiply.rs
|   |   |   |   |   |   `-- vector_copy.rs
|   |   |   |   |   |-- channel/
|   |   |   |   |   |   |-- gather.rs
|   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |-- neon.rs
|   |   |   |   |   |   |-- rgba.rs
|   |   |   |   |   |   |-- ssse3.rs
|   |   |   |   |   |   `-- wide.rs
|   |   |   |   |   |-- dynamic_image.rs
|   |   |   |   |   |-- execution/
|   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   `-- prepared.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   `-- quantization/
|   |   |   |   |       |-- fixed_palette.rs
|   |   |   |   |       |-- maxcoverage.rs
|   |   |   |   |       |-- median_cut.rs
|   |   |   |   |       |-- mod.rs
|   |   |   |   |       |-- octree.rs
|   |   |   |   |       |-- rgb_source.rs
|   |   |   |   |       |-- rgba_palette.rs
|   |   |   |   |       |-- rgba_source.rs
|   |   |   |   |       `-- web_palette.rs
|   |   |   |   |-- legacy/
|   |   |   |   |   |-- batch_execution.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   |-- ops/
|   |   |   |   |   |   |-- chops.rs
|   |   |   |   |   |   |-- color.rs
|   |   |   |   |   |   |-- color3dlut.rs
|   |   |   |   |   |   |-- color_convert_allocation_tests.rs
|   |   |   |   |   |   |-- composite/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- composite.rs
|   |   |   |   |   |   |-- crop.rs
|   |   |   |   |   |   |-- draw/
|   |   |   |   |   |   |   |-- arcs.rs
|   |   |   |   |   |   |   |-- canvas.rs
|   |   |   |   |   |   |   |-- clipping.rs
|   |   |   |   |   |   |   |-- dispatch.rs
|   |   |   |   |   |   |   |-- ellipse.rs
|   |   |   |   |   |   |   |-- operations.rs
|   |   |   |   |   |   |   |-- polygon.rs
|   |   |   |   |   |   |   |-- primitives.rs
|   |   |   |   |   |   |   `-- rounded_rect.rs
|   |   |   |   |   |   |-- draw.rs
|   |   |   |   |   |   |-- effects.rs
|   |   |   |   |   |   |-- enhance.rs
|   |   |   |   |   |   |-- eval/
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- eval.rs
|   |   |   |   |   |   |-- filter/
|   |   |   |   |   |   |   |-- blur/
|   |   |   |   |   |   |   |   |-- box_rows.rs
|   |   |   |   |   |   |   |   |-- line.rs
|   |   |   |   |   |   |   |   |-- radius_one/
|   |   |   |   |   |   |   |   |   `-- vertical.rs
|   |   |   |   |   |   |   |   |-- radius_one.rs
|   |   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |   |-- blur.rs
|   |   |   |   |   |   |   |-- convolution.rs
|   |   |   |   |   |   |   |-- filter3x3/
|   |   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |   |-- filter3x3.rs
|   |   |   |   |   |   |   |-- filter5x5/
|   |   |   |   |   |   |   |   |-- int32/
|   |   |   |   |   |   |   |   |   |-- bounded_binomial.rs
|   |   |   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |   |   |-- int32.rs
|   |   |   |   |   |   |   |   |-- native_bytes/
|   |   |   |   |   |   |   |   |   |-- binomial.rs
|   |   |   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |   |   `-- native_bytes.rs
|   |   |   |   |   |   |   |-- filter5x5.rs
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   |-- blur_line_steps.rs
|   |   |   |   |   |   |   |   |-- i32_cross_smooth.rs
|   |   |   |   |   |   |   |   `-- mod.rs
|   |   |   |   |   |   |   |-- rank/
|   |   |   |   |   |   |   |   |-- float.rs
|   |   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |   |-- rank.rs
|   |   |   |   |   |   |   |-- rgb_blur_ring.rs
|   |   |   |   |   |   |   |-- uniform_box.rs
|   |   |   |   |   |   |   |-- uniform_box_tests.rs
|   |   |   |   |   |   |   `-- uniform_image.rs
|   |   |   |   |   |   |-- filter.rs
|   |   |   |   |   |   |-- geometry.rs
|   |   |   |   |   |   |-- grayscale.rs
|   |   |   |   |   |   |-- imageops/
|   |   |   |   |   |   |   |-- colorize.rs
|   |   |   |   |   |   |   |-- expand.rs
|   |   |   |   |   |   |   |-- geometry.rs
|   |   |   |   |   |   |   |-- histogram.rs
|   |   |   |   |   |   |   |-- pad/
|   |   |   |   |   |   |   |   |-- native.rs
|   |   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |   |-- pad.rs
|   |   |   |   |   |   |   `-- pixel.rs
|   |   |   |   |   |   |-- imageops.rs
|   |   |   |   |   |   |-- mod.rs
|   |   |   |   |   |   |-- paste/
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   |-- layout.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- paste.rs
|   |   |   |   |   |   |-- put.rs
|   |   |   |   |   |   |-- reduce/
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   `-- rgb.rs
|   |   |   |   |   |   |   |-- kernels.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- reduce.rs
|   |   |   |   |   |   |-- resize/
|   |   |   |   |   |   |   |-- filter.rs
|   |   |   |   |   |   |   |-- float.rs
|   |   |   |   |   |   |   |-- float_resize.rs
|   |   |   |   |   |   |   |-- float_resize_tests.rs
|   |   |   |   |   |   |   |-- integer.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- resize.rs
|   |   |   |   |   |   |-- rotate/
|   |   |   |   |   |   |   |-- affine/
|   |   |   |   |   |   |   |   `-- coordinates.rs
|   |   |   |   |   |   |   |-- affine.rs
|   |   |   |   |   |   |   |-- cubic.rs
|   |   |   |   |   |   |   |-- scalar.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- rotate.rs
|   |   |   |   |   |   |-- thumbnail/
|   |   |   |   |   |   |   |-- fusion.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- thumbnail.rs
|   |   |   |   |   |   |-- transform/
|   |   |   |   |   |   |   |-- affine.rs
|   |   |   |   |   |   |   |-- projective.rs
|   |   |   |   |   |   |   |-- scalar/
|   |   |   |   |   |   |   |   `-- projective.rs
|   |   |   |   |   |   |   |-- scalar.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   |-- transform.rs
|   |   |   |   |   |   |-- transpose/
|   |   |   |   |   |   |   |-- arch.rs
|   |   |   |   |   |   |   |-- kernels/
|   |   |   |   |   |   |   |   `-- transpose_rgba/
|   |   |   |   |   |   |   |       `-- x86_64.rs
|   |   |   |   |   |   |   |-- native.rs
|   |   |   |   |   |   |   `-- tests.rs
|   |   |   |   |   |   `-- transpose.rs
|   |   |   |   |   `-- pool_simd/
|   |   |   |   |       |-- batch_execution.rs
|   |   |   |   |       |-- mod.rs
|   |   |   |   |       |-- ops/
|   |   |   |   |       |   |-- adapters/
|   |   |   |   |       |   |   |-- alpha_composite.rs
|   |   |   |   |       |   |   |-- box_blur/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- la.rs
|   |   |   |   |       |   |   |   |   |-- luma.rs
|   |   |   |   |       |   |   |   |   |-- rgb.rs
|   |   |   |   |       |   |   |   |   |-- rgba.rs
|   |   |   |   |       |   |   |   |   |-- rgba_avx2.rs
|   |   |   |   |       |   |   |   |   `-- rgba_neon.rs
|   |   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- box_blur.rs
|   |   |   |   |       |   |   |-- chops/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- fused_multiply_screen.rs
|   |   |   |   |       |   |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |   `-- kernels.rs
|   |   |   |   |       |   |   |-- chops.rs
|   |   |   |   |       |   |   |-- color3dlut.rs
|   |   |   |   |       |   |   |-- colorize.rs
|   |   |   |   |       |   |   |-- convert/
|   |   |   |   |       |   |   |   |-- alpha.rs
|   |   |   |   |       |   |   |   |-- cmyk.rs
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- color_space.rs
|   |   |   |   |       |   |   |   |   |-- dispatch/
|   |   |   |   |       |   |   |   |   |   |-- layout.rs
|   |   |   |   |       |   |   |   |   |   |-- luma.rs
|   |   |   |   |       |   |   |   |   |   `-- typed.rs
|   |   |   |   |       |   |   |   |   |-- dispatch.rs
|   |   |   |   |       |   |   |   |   |-- layout.rs
|   |   |   |   |       |   |   |   |   |-- mod.rs
|   |   |   |   |       |   |   |   |   |-- rgba_rgb_neon.rs
|   |   |   |   |       |   |   |   |   `-- rgba_rgb_ssse3.rs
|   |   |   |   |       |   |   |   |-- tests/
|   |   |   |   |       |   |   |   |   `-- cmyk_packing.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- convert.rs
|   |   |   |   |       |   |   |-- crop.rs
|   |   |   |   |       |   |   |-- draw/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- mod.rs
|   |   |   |   |       |   |   |   |   |-- shapes/
|   |   |   |   |       |   |   |   |   |   `-- rounded_rect.rs
|   |   |   |   |       |   |   |   |   |-- shapes.rs
|   |   |   |   |       |   |   |   |   |-- spans/
|   |   |   |   |       |   |   |   |   |   `-- line.rs
|   |   |   |   |       |   |   |   |   `-- spans.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- draw.rs
|   |   |   |   |       |   |   |-- duplicate/
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- duplicate.rs
|   |   |   |   |       |   |   |-- effects/
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- effects.rs
|   |   |   |   |       |   |   |-- enhance/
|   |   |   |   |       |   |   |   |-- point.rs
|   |   |   |   |       |   |   |   |-- sharpness.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- enhance.rs
|   |   |   |   |       |   |   |-- expand/
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- expand.rs
|   |   |   |   |       |   |   |-- extract_band.rs
|   |   |   |   |       |   |   |-- filter/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- byte_layouts.rs
|   |   |   |   |       |   |   |   |   |-- i32_three_by_three.rs
|   |   |   |   |       |   |   |   |   |-- l.rs
|   |   |   |   |       |   |   |   |   |-- la.rs
|   |   |   |   |       |   |   |   |   |-- mod.rs
|   |   |   |   |       |   |   |   |   |-- rgb_blur_ring.rs
|   |   |   |   |       |   |   |   |   |-- three_by_three.rs
|   |   |   |   |       |   |   |   |   |-- uniform_rgb.rs
|   |   |   |   |       |   |   |   |   `-- unsharp.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- filter.rs
|   |   |   |   |       |   |   |-- flip.rs
|   |   |   |   |       |   |   |-- gaussian_blur/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- la.rs
|   |   |   |   |       |   |   |   |   `-- rgba_uniform.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- gaussian_blur.rs
|   |   |   |   |       |   |   |-- gradient.rs
|   |   |   |   |       |   |   |-- grayscale/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- channels.rs
|   |   |   |   |       |   |   |   |   |-- cmyk.rs
|   |   |   |   |       |   |   |   |   |-- mod.rs
|   |   |   |   |       |   |   |   |   |-- rgb_avx2.rs
|   |   |   |   |       |   |   |   |   |-- ycbcr.rs
|   |   |   |   |       |   |   |   |   `-- ycbcr_ssse3.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- grayscale.rs
|   |   |   |   |       |   |   |-- histogram.rs
|   |   |   |   |       |   |   |-- layout.rs
|   |   |   |   |       |   |   |-- lut/
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- lut.rs
|   |   |   |   |       |   |   |-- mandelbrot.rs
|   |   |   |   |       |   |   |-- median_filter/
|   |   |   |   |       |   |   |   |-- extrema.rs
|   |   |   |   |       |   |   |   |-- float.rs
|   |   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- median_filter.rs
|   |   |   |   |       |   |   |-- merge/
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- merge.rs
|   |   |   |   |       |   |   |-- module_blend.rs
|   |   |   |   |       |   |   |-- native_arithmetic.rs
|   |   |   |   |       |   |   |-- native_bytes.rs
|   |   |   |   |       |   |   |-- neighborhood.rs
|   |   |   |   |       |   |   |-- offset/
|   |   |   |   |       |   |   |   `-- kernels/
|   |   |   |   |       |   |   |       |-- byte.rs
|   |   |   |   |       |   |   |       `-- luma16.rs
|   |   |   |   |       |   |   |-- offset.rs
|   |   |   |   |       |   |   |-- pad/
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- pad.rs
|   |   |   |   |       |   |   |-- paste/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- apply/
|   |   |   |   |       |   |   |   |   |   |-- cmyk_l.rs
|   |   |   |   |       |   |   |   |   |   |-- generic.rs
|   |   |   |   |       |   |   |   |   |   |-- l.rs
|   |   |   |   |       |   |   |   |   |   |-- la.rs
|   |   |   |   |       |   |   |   |   |   |-- pa.rs
|   |   |   |   |       |   |   |   |   |   |-- rgb.rs
|   |   |   |   |       |   |   |   |   |   `-- rgba.rs
|   |   |   |   |       |   |   |   |   |-- apply.rs
|   |   |   |   |       |   |   |   |   |-- blend.rs
|   |   |   |   |       |   |   |   |   |-- luma.rs
|   |   |   |   |       |   |   |   |   |-- luma16.rs
|   |   |   |   |       |   |   |   |   |-- mod.rs
|   |   |   |   |       |   |   |   |   `-- rgb_cmyk.rs
|   |   |   |   |       |   |   |   |-- mod.rs
|   |   |   |   |       |   |   |   |-- plan.rs
|   |   |   |   |       |   |   |   |-- rgb_to_rgba.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- pipeline.rs
|   |   |   |   |       |   |   |-- projection.rs
|   |   |   |   |       |   |   |-- put_alpha/
|   |   |   |   |       |   |   |   `-- kernels.rs
|   |   |   |   |       |   |   |-- put_alpha.rs
|   |   |   |   |       |   |   |-- put_data.rs
|   |   |   |   |       |   |   |-- put_pixel/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   `-- rgba_batch.rs
|   |   |   |   |       |   |   |   `-- kernels.rs
|   |   |   |   |       |   |   |-- put_pixel.rs
|   |   |   |   |       |   |   |-- reduce/
|   |   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- reduce.rs
|   |   |   |   |       |   |   |-- resize/
|   |   |   |   |       |   |   |   |-- aspect.rs
|   |   |   |   |       |   |   |   |-- coordinates.rs
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- f32/
|   |   |   |   |       |   |   |   |   |   `-- boxed.rs
|   |   |   |   |       |   |   |   |   |-- f32.rs
|   |   |   |   |       |   |   |   |   |-- i32/
|   |   |   |   |       |   |   |   |   |   `-- neon.rs
|   |   |   |   |       |   |   |   |   |-- i32.rs
|   |   |   |   |       |   |   |   |   |-- luma16.rs
|   |   |   |   |       |   |   |   |   |-- u8.rs
|   |   |   |   |       |   |   |   |   |-- u8_boxed_convolution.rs
|   |   |   |   |       |   |   |   |   |-- u8_convolution.rs
|   |   |   |   |       |   |   |   |   |-- u8_cover_i32.rs
|   |   |   |   |       |   |   |   |   |-- u8_nearest.rs
|   |   |   |   |       |   |   |   |   `-- u8_plan.rs
|   |   |   |   |       |   |   |   |-- support/
|   |   |   |   |       |   |   |   |   |-- aspect.rs
|   |   |   |   |       |   |   |   |   `-- resize.rs
|   |   |   |   |       |   |   |   |-- support.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- resize.rs
|   |   |   |   |       |   |   |-- rotate/
|   |   |   |   |       |   |   |   |-- interpolation.rs
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- native/
|   |   |   |   |       |   |   |   |   |   |-- bilinear.rs
|   |   |   |   |       |   |   |   |   |   |-- nearest.rs
|   |   |   |   |       |   |   |   |   |   `-- typed_bilinear.rs
|   |   |   |   |       |   |   |   |   |-- native.rs
|   |   |   |   |       |   |   |   |   `-- nearest.rs
|   |   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- rotate.rs
|   |   |   |   |       |   |   |-- support.rs
|   |   |   |   |       |   |   |-- thumbnail/
|   |   |   |   |       |   |   |   `-- kernels/
|   |   |   |   |       |   |   |       |-- float.rs
|   |   |   |   |       |   |   |       |-- fusion.rs
|   |   |   |   |       |   |   |       `-- integer.rs
|   |   |   |   |       |   |   |-- thumbnail.rs
|   |   |   |   |       |   |   |-- transform/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- bilinear/
|   |   |   |   |       |   |   |   |   |   |-- affine.rs
|   |   |   |   |       |   |   |   |   |   `-- projective.rs
|   |   |   |   |       |   |   |   |   |-- bilinear.rs
|   |   |   |   |       |   |   |   |   |-- luma16.rs
|   |   |   |   |       |   |   |   |   |-- mesh.rs
|   |   |   |   |       |   |   |   |   |-- nearest/
|   |   |   |   |       |   |   |   |   |   |-- affine.rs
|   |   |   |   |       |   |   |   |   |   `-- projective.rs
|   |   |   |   |       |   |   |   |   `-- nearest.rs
|   |   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   |-- transform.rs
|   |   |   |   |       |   |   |-- transpose/
|   |   |   |   |       |   |   |   |-- kernels/
|   |   |   |   |       |   |   |   |   |-- portable.rs
|   |   |   |   |       |   |   |   |   `-- rgba_avx2.rs
|   |   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |   |       |   |   |   |-- odd_tests.rs
|   |   |   |   |       |   |   |   `-- tests.rs
|   |   |   |   |       |   |   `-- transpose.rs
|   |   |   |   |       |   |-- adapters.rs
|   |   |   |   |       |   |-- mod.rs
|   |   |   |   |       |   |-- native_channel/
|   |   |   |   |       |   |   `-- cmyk_ssse3.rs
|   |   |   |   |       |   |-- native_channel.rs
|   |   |   |   |       |   |-- native_filter.rs
|   |   |   |   |       |   `-- native_merge.rs
|   |   |   |   |       |-- point_batch.rs
|   |   |   |   |       `-- put_pixel_batch.rs
|   |   |   |   |-- mod.rs
|   |   |   |   `-- ops/
|   |   |   |       |-- analysis/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- projection_impl.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- positive_writes.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- projection_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- projection_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- projection_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- bbox.rs
|   |   |   |       |   |   |-- bbox_execution.rs
|   |   |   |       |   |   |-- extrema.rs
|   |   |   |       |   |   |-- extrema_bytes.rs
|   |   |   |       |   |   |-- extrema_bytes_execution.rs
|   |   |   |       |   |   |-- extrema_execution.rs
|   |   |   |       |   |   |-- get_channel_support.rs
|   |   |   |       |   |   |-- get_data.rs
|   |   |   |       |   |   |-- get_data_execution.rs
|   |   |   |       |   |   |-- get_data_support.rs
|   |   |   |       |   |   |-- getcolors.rs
|   |   |   |       |   |   |-- getcolors_execution.rs
|   |   |   |       |   |   |-- getcolors_support.rs
|   |   |   |       |   |   |-- histogram_execution.rs
|   |   |   |       |   |   |-- histogram_samples.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- pixel_access.rs
|   |   |   |       |   |   |-- projection.rs
|   |   |   |       |   |   |-- projection_support.rs
|   |   |   |       |   |   |-- projection_vector.rs
|   |   |   |       |   |   |-- split_support.rs
|   |   |   |       |   |   |-- stat.rs
|   |   |   |       |   |   `-- stat_execution.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- projection_impl.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- projection_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- projection_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- projection_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extrema_bytes_impl.rs
|   |   |   |       |   |   |   |-- get_data_impl.rs
|   |   |   |       |   |   |   |-- get_pixel_impl.rs
|   |   |   |       |   |   |   |-- getbbox_impl.rs
|   |   |   |       |   |   |   |-- getcolors_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- stat_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- extrema_bytes_impl.rs
|   |   |   |       |       |   |-- get_data_impl.rs
|   |   |   |       |       |   |-- get_pixel_impl.rs
|   |   |   |       |       |   |-- getbbox_impl.rs
|   |   |   |       |       |   |-- getcolors_impl.rs
|   |   |   |       |       |   |-- histogram_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- stat_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- chops/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- binary.rs
|   |   |   |       |   |   |-- dispatch.rs
|   |   |   |       |   |   |-- fusion.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- offset/
|   |   |   |       |   |   |   |-- byte.rs
|   |   |   |       |   |   |   |-- default.rs
|   |   |   |       |   |   |   |-- luma16.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   |-- three_band.rs
|   |   |   |       |   |   |-- unary.rs
|   |   |   |       |   |   `-- vector.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- ops.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- ops.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           `-- mod.rs
|   |   |   |       |-- color/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- to_rgb.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- convert/
|   |   |   |       |   |   |   |-- alpha.rs
|   |   |   |       |   |   |   |-- binary.rs
|   |   |   |       |   |   |   |-- four_byte_layout.rs
|   |   |   |       |   |   |   |-- four_byte_to_luma_alpha.rs
|   |   |   |       |   |   |   |-- high_depth.rs
|   |   |   |       |   |   |   |-- lab_target.rs
|   |   |   |       |   |   |   |-- luma16.rs
|   |   |   |       |   |   |   |-- luma8.rs
|   |   |   |       |   |   |   |-- matrix.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- native_rgb.rs
|   |   |   |       |   |   |   |-- palette.rs
|   |   |   |       |   |   |   |-- physical_lanes.rs
|   |   |   |       |   |   |   |-- pixel.rs
|   |   |   |       |   |   |   |-- premultiplied.rs
|   |   |   |       |   |   |   |-- prepared.rs
|   |   |   |       |   |   |   |-- target_compositions.rs
|   |   |   |       |   |   |   `-- wide.rs
|   |   |   |       |   |   |-- lab_kernel.rs
|   |   |   |       |   |   |-- lab_to_srgb.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- remap_palette.rs
|   |   |   |       |   |   `-- strategy.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- unpremultiply.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- to_srgb.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- convert.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- remap_palette_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- cmyk.rs
|   |   |   |       |   |   |   |-- expand_color.rs
|   |   |   |       |   |   |   |-- hsv.rs
|   |   |   |       |   |   |   |-- layout.rs
|   |   |   |       |   |   |   |-- luma_alpha.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- typed.rs
|   |   |   |       |   |   |   |-- vector.rs
|   |   |   |       |   |   |   `-- ycbcr.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- remap_palette_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- remap_palette_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- remap_palette_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- cmyk.rs
|   |   |   |       |   |   |   |-- expand_color.rs
|   |   |   |       |   |   |   |-- hsv.rs
|   |   |   |       |   |   |   |-- layout.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- premultiply.rs
|   |   |   |       |   |   |   |-- typed.rs
|   |   |   |       |   |   |   |-- vector.rs
|   |   |   |       |   |   |   `-- ycbcr.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- i16.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- convert.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- remap_palette_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- remap_palette_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- remap_palette_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- remap_palette_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- convert.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- convert_lab_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- layout.rs
|   |   |   |       |   |   |   |-- luma_alpha.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- rgba.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rgba_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- alpha.rs
|   |   |   |       |   |   |   |-- luma_alpha.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- convert.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- convert_lab_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- luma_alpha.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- opaque_padding.rs
|   |   |   |       |   |   |   |-- premultiply.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- convert.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   |-- convert_lab_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- convert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- convert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- convert_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- color3dlut/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- color3dlut_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- lut.rs
|   |   |   |       |   |   |-- lut_vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- strategy.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- color3dlut_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- color3dlut_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- color3dlut_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- color3dlut_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- color3dlut_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- avx2/
|   |   |   |       |       |   |-- color3dlut_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- color3dlut_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- color3dlut_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- color3dlut_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- color3dlut_impl.rs
|   |   |   |       |           `-- mod.rs
|   |   |   |       |-- composite/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- merge_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- alpha_composite.rs
|   |   |   |       |   |   |-- alpha_composite_vector.rs
|   |   |   |       |   |   |-- blend/
|   |   |   |       |   |   |   |-- avx_fma.rs
|   |   |   |       |   |   |   `-- neon.rs
|   |   |   |       |   |   |-- blend.rs
|   |   |   |       |   |   |-- composite/
|   |   |   |       |   |   |   |-- bytes.rs
|   |   |   |       |   |   |   |-- execute.rs
|   |   |   |       |   |   |   |-- mask.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- numeric.rs
|   |   |   |       |   |   |   `-- output.rs
|   |   |   |       |   |   |-- composite_module_support.rs
|   |   |   |       |   |   |-- merge.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- native_bytes.rs
|   |   |   |       |   |   `-- strategy.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- alpha_composite_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- alpha_composite_impl.rs
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- alpha_composite_impl.rs
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- alpha_composite_impl.rs
|   |   |   |       |   |       |-- merge_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- merge_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- alpha_composite_impl.rs
|   |   |   |       |   |   |   |-- alpha_composite_mirror_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- alpha_composite_mirror.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- alpha_composite_impl.rs
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- alpha_composite_impl.rs
|   |   |   |       |   |   |   |-- merge_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- alpha_composite_impl.rs
|   |   |   |       |   |       |-- merge_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- operations.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- operations.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- crop/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- crop.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   `-- vector_copy.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- crop_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- crop_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- crop_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- crop_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- crop_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- crop_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- draw/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- arcs.rs
|   |   |   |       |   |   |-- bitmap_mask.rs
|   |   |   |       |   |   |-- bitmap_region.rs
|   |   |   |       |   |   |-- canvas.rs
|   |   |   |       |   |   |-- clipping.rs
|   |   |   |       |   |   |-- dispatch.rs
|   |   |   |       |   |   |-- ellipse.rs
|   |   |   |       |   |   |-- floodfill.rs
|   |   |   |       |   |   |-- gray_alpha_bitmap.rs
|   |   |   |       |   |   |-- gray_alpha_text.rs
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- i16_bitmap.rs
|   |   |   |       |   |   |-- i16_text.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- native.rs
|   |   |   |       |   |   |-- numeric_text.rs
|   |   |   |       |   |   |-- polygon.rs
|   |   |   |       |   |   |-- primitives.rs
|   |   |   |       |   |   |-- raw_four_band_bitmap.rs
|   |   |   |       |   |   |-- rounded_rect.rs
|   |   |   |       |   |   |-- simd/
|   |   |   |       |   |   |   |-- admission.rs
|   |   |   |       |   |   |   |-- arithmetic.rs
|   |   |   |       |   |   |   |-- execute.rs
|   |   |   |       |   |   |   |-- kernels/
|   |   |   |       |   |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |   |-- shapes/
|   |   |   |       |   |   |   |   |   `-- rounded_rect.rs
|   |   |   |       |   |   |   |   |-- shapes.rs
|   |   |   |       |   |   |   |   |-- span_writer.rs
|   |   |   |       |   |   |   |   |-- spans/
|   |   |   |       |   |   |   |   |   `-- line.rs
|   |   |   |       |   |   |   |   `-- spans.rs
|   |   |   |       |   |   |   |-- layout.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- strategy.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   |-- support.rs
|   |   |   |       |   |   |-- text_coords.rs
|   |   |   |       |   |   |-- text_mask.rs
|   |   |   |       |   |   |-- text_result.rs
|   |   |   |       |   |   |-- three_band_bitmap.rs
|   |   |   |       |   |   `-- three_band_text.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- bitmap_impl.rs
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- text_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- draw_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- draw_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- bitmap_impl.rs
|   |   |   |       |       |   |-- draw_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- text_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- draw_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- draw_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- draw_impl.rs
|   |   |   |       |           `-- mod.rs
|   |   |   |       |-- effects/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       |-- linear_gradient_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- generator.rs
|   |   |   |       |   |   |-- linear_gradient_vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- noise.rs
|   |   |   |       |   |   |-- noise_vector.rs
|   |   |   |       |   |   |-- pixel_buffer.rs
|   |   |   |       |   |   |-- radial_gradient.rs
|   |   |   |       |   |   |-- spread.rs
|   |   |   |       |   |   |-- spread_vector.rs
|   |   |   |       |   |   `-- strategy.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       |-- linear_gradient_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mandelbrot_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mandelbrot_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mandelbrot.rs
|   |   |   |       |   |   |   |-- mandelbrot_vector.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mandelbrot_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mandelbrot_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       |-- linear_gradient_impl.rs
|   |   |   |       |   |       |-- mandelbrot_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- radial_gradient_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       |-- linear_gradient_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radial_gradient_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   |-- linear_gradient_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       |-- linear_gradient_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- effect_noise_impl.rs
|   |   |   |       |   |   |   |-- effect_spread_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- effect_noise_impl.rs
|   |   |   |       |   |       |-- effect_spread_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- effect_noise_impl.rs
|   |   |   |       |       |   |-- effect_spread_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- effect_noise_impl.rs
|   |   |   |       |       |   |-- effect_spread_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- effect_noise_impl.rs
|   |   |   |       |       |   |-- effect_spread_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- effect_noise_impl.rs
|   |   |   |       |           |-- effect_spread_impl.rs
|   |   |   |       |           `-- mod.rs
|   |   |   |       |-- enhance/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- contrast_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- sharpness_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- color_saturation.rs
|   |   |   |       |   |   |-- contrast.rs
|   |   |   |       |   |   |-- contrast_route.rs
|   |   |   |       |   |   |-- contrast_vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- native.rs
|   |   |   |       |   |   |-- sharpness.rs
|   |   |   |       |   |   |-- sharpness_vector.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   `-- vector.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- contrast_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- sharpness_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- contrast_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- sharpness_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- contrast_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- sharpness_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- contrast_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- sharpness_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- brightness_impl.rs
|   |   |   |       |   |   |   |-- color_saturation_impl.rs
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- contrast_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- sharpness_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- contrast_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- sharpness_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- brightness_impl.rs
|   |   |   |       |       |   |-- contrast_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- sharpness_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- eval/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- point_impl.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- lut.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- point_float.rs
|   |   |   |       |   |   |-- scalar.rs
|   |   |   |       |   |   |-- scalar_point.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   `-- vector.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- point_impl.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- point_impl.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- point_impl.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- point_impl.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- point_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_float_impl.rs
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_float_impl.rs
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_float_impl.rs
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- eval_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- eval_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- filter/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rank_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- rank_order.rs
|   |   |   |       |   |   |   |-- rank_vector.rs
|   |   |   |       |   |   |   `-- uniform.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- blur/
|   |   |   |       |   |   |   |-- box_rows.rs
|   |   |   |       |   |   |   |-- line.rs
|   |   |   |       |   |   |   |-- radius_one/
|   |   |   |       |   |   |   |   `-- vertical.rs
|   |   |   |       |   |   |   |-- radius_one.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- blur.rs
|   |   |   |       |   |   |-- byte_filter.rs
|   |   |   |       |   |   |-- convolution.rs
|   |   |   |       |   |   |-- extrema.rs
|   |   |   |       |   |   |-- filter3x3.rs
|   |   |   |       |   |   |-- filter5x5/
|   |   |   |       |   |   |   |-- native_bytes/
|   |   |   |       |   |   |   |   `-- binomial.rs
|   |   |   |       |   |   |   `-- native_bytes.rs
|   |   |   |       |   |   |-- filter5x5.rs
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- i16_kernel.rs
|   |   |   |       |   |   |-- kernels/
|   |   |   |       |   |   |   |-- blur_line_steps.rs
|   |   |   |       |   |   |   |-- filter3x3_vector.rs
|   |   |   |       |   |   |   |-- filter5x5_rgb_vector.rs
|   |   |   |       |   |   |   |-- filter5x5_vector.rs
|   |   |   |       |   |   |   `-- rank_vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- mode_filter.rs
|   |   |   |       |   |   |-- mode_leaf.rs
|   |   |   |       |   |   |-- rank.rs
|   |   |   |       |   |   |-- rank_vector.rs
|   |   |   |       |   |   |-- rgb_blur_ring.rs
|   |   |   |       |   |   |-- uniform_box.rs
|   |   |   |       |   |   |-- uniform_image.rs
|   |   |   |       |   |   `-- unsharp.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- filter3x3_identity.rs
|   |   |   |       |   |   |   |-- filter3x3_impl.rs
|   |   |   |       |   |   |   |-- filter3x3_kernel.rs
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   |-- filter5x5_kernel.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter3x3_impl.rs
|   |   |   |       |   |   |   |-- filter5x5_bounded_binomial.rs
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- i32_cross_smooth_kernel.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rank_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- binomial5x5_kernel.rs
|   |   |   |       |   |   |   |-- box_blur_radius_one.rs
|   |   |   |       |   |   |   |-- filter5x5.rs
|   |   |   |       |   |   |   |-- gaussian_blur_radius_one.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- rank3x3.rs
|   |   |   |       |   |   |   |-- rank_window.rs
|   |   |   |       |   |   |   `-- uniform5x5_kernel.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- binomial5x5_kernel.rs
|   |   |   |       |   |   |   |-- filter5x5.rs
|   |   |   |       |   |   |   |-- gaussian_blur_radius_one.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- single_outlier5x5_kernel.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- binomial3x3_kernel.rs
|   |   |   |       |   |   |   |-- filter3x3_impl.rs
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- radius_one_blur.rs
|   |   |   |       |   |   |   `-- unsharp_blend.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- gaussian_blur_radius_seven.rs
|   |   |   |       |   |   |   |-- gaussian_blur_radius_seven_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter5x5_impl.rs
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- gaussian_blur_radius_seven.rs
|   |   |   |       |   |   |   |-- gaussian_blur_uniform.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- radius_one_blur.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- gaussian_blur_radius_seven.rs
|   |   |   |       |   |   |   |-- gaussian_blur_radius_seven_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filter_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- mode_filter_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- filter_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- mode_filter_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- geometry/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- dimensions.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- pad_dimensions.rs
|   |   |   |       |   |   |-- pad_hidden_lanes.rs
|   |   |   |       |   |   |-- pad_native_vector.rs
|   |   |   |       |   |   |-- pad_samples.rs
|   |   |   |       |   |   `-- pad_strategies.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_window.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_window.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- pad_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- pad_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- pad_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- pad_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- pad_impl.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           `-- pad_impl.rs
|   |   |   |       |-- grayscale/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- grayscale_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- cmyk.rs
|   |   |   |       |   |   |-- interleaved.rs
|   |   |   |       |   |   |-- luma.rs
|   |   |   |       |   |   `-- mod.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- grayscale_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   |-- grayscale_wide_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- grayscale_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- grayscale_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- grayscale_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- grayscale_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- grayscale_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- grayscale_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- grayscale_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- grayscale_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       |-- ssse3/
|   |   |   |       |       |   |-- grayscale_impl.rs
|   |   |   |       |       |   |-- grayscale_wide_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- grayscale_impl.rs
|   |   |   |       |           `-- mod.rs
|   |   |   |       |-- imageops/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- autocontrast.rs
|   |   |   |       |   |   |-- colorize.rs
|   |   |   |       |   |   |-- equalize.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- expand_edges.rs
|   |   |   |       |   |   |-- expand_vector.rs
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- histogram.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- native_bytes.rs
|   |   |   |       |   |   |-- pixel.rs
|   |   |   |       |   |   |-- point_lut.rs
|   |   |   |       |   |   |-- point_support.rs
|   |   |   |       |   |   |-- quantize.rs
|   |   |   |       |   |   |-- quantize_rgb_source.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   `-- wrapper_routes.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- autocontrast_impl.rs
|   |   |   |       |   |   |   |-- colorize_impl.rs
|   |   |   |       |   |   |   |-- equalize_impl.rs
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   |-- quantize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- colorize_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- colorize_impl.rs
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- colorize_impl.rs
|   |   |   |       |   |       |-- histogram_impl.rs
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- posterize_impl.rs
|   |   |   |       |   |       `-- solarize_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- autocontrast_impl.rs
|   |   |   |       |   |   |   |-- equalize_impl.rs
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   |-- quantize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- autocontrast_impl.rs
|   |   |   |       |   |   |   |-- equalize_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- autocontrast_impl.rs
|   |   |   |       |   |   |   |-- equalize_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- autocontrast_impl.rs
|   |   |   |       |   |       |-- equalize_impl.rs
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- posterize_impl.rs
|   |   |   |       |   |       `-- solarize_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- autocontrast_impl.rs
|   |   |   |       |   |   |   |-- equalize_impl.rs
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   |-- quantize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- histogram_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- posterize_impl.rs
|   |   |   |       |   |   |   `-- solarize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- histogram_impl.rs
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- posterize_impl.rs
|   |   |   |       |   |       `-- solarize_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- invert_impl.rs
|   |   |   |       |   |       `-- mod.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- expand_impl.rs
|   |   |   |       |   |   |   |-- invert_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- quantize_impl.rs
|   |   |   |       |   |   |-- expand.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- expand_impl.rs
|   |   |   |       |       |   |-- invert_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- quantize_impl.rs
|   |   |   |       |       |-- expand.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- mod.rs
|   |   |   |       |-- paste/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- cmyk_row.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- bytes.rs
|   |   |   |       |   |   |-- contracts.rs
|   |   |   |       |   |   |-- i16.rs
|   |   |   |       |   |   |-- i16_vector.rs
|   |   |   |       |   |   |-- kernels/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rgb_l_mask_neon.rs
|   |   |   |       |   |   |-- layout.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- plan.rs
|   |   |   |       |   |   |-- region.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   |-- telemetry.rs
|   |   |   |       |   |   |-- typed.rs
|   |   |   |       |   |   `-- vector.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- luma_row.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- la_row.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- pa_row.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rgba_to_rgb.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rgb_to_rgba.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- paste_impl.rs
|   |   |   |       |   |   |   `-- rgba_row.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- paste_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- paste_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- paste_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- paste_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- paste_impl.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           `-- paste_impl.rs
|   |   |   |       |-- put/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_alpha_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- alpha_mask.rs
|   |   |   |       |   |   |-- extract_band.rs
|   |   |   |       |   |   |-- extract_band_strategy.rs
|   |   |   |       |   |   |-- extract_band_vector.rs
|   |   |   |       |   |   |-- imaging_core_band.rs
|   |   |   |       |   |   |-- imaging_core_mode.rs
|   |   |   |       |   |   |-- imaging_core_set_mode.rs
|   |   |   |       |   |   |-- masked_store.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- put_alpha.rs
|   |   |   |       |   |   |-- put_alpha_data.rs
|   |   |   |       |   |   |-- put_alpha_data_strategy.rs
|   |   |   |       |   |   |-- put_alpha_data_vector.rs
|   |   |   |       |   |   |-- put_alpha_strategy.rs
|   |   |   |       |   |   |-- put_alpha_vector.rs
|   |   |   |       |   |   |-- put_data.rs
|   |   |   |       |   |   |-- put_data_l16_bytes.rs
|   |   |   |       |   |   |-- put_data_strategy.rs
|   |   |   |       |   |   |-- put_data_value_at.rs
|   |   |   |       |   |   |-- put_data_vector.rs
|   |   |   |       |   |   |-- put_pixel.rs
|   |   |   |       |   |   |-- put_pixel_strategy.rs
|   |   |   |       |   |   |-- put_pixel_vector.rs
|   |   |   |       |   |   |-- scalar_write_support.rs
|   |   |   |       |   |   `-- value_at.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_helper.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- put_data_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- put_data_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- put_data_impl.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_alpha_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_data_value_at_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_alpha_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_data_value_at_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_data_value_at_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_alpha_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_data_value_at_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_data_value_at_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- put_alpha_data_impl.rs
|   |   |   |       |   |   |   |-- put_alpha_impl.rs
|   |   |   |       |   |   |   |-- put_data_impl.rs
|   |   |   |       |   |   |   `-- put_pixel_impl.rs
|   |   |   |       |   |   |-- ssse3/
|   |   |   |       |   |   |   |-- extract_band_impl.rs
|   |   |   |       |   |   |   `-- mod.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- extract_band_impl.rs
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       |-- put_alpha_data_impl.rs
|   |   |   |       |   |       |-- put_alpha_impl.rs
|   |   |   |       |   |       |-- put_data_impl.rs
|   |   |   |       |   |       `-- put_pixel_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- extract_band_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   |-- put_data_impl.rs
|   |   |   |       |       |   `-- put_pixel_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- extract_band_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   |-- put_data_impl.rs
|   |   |   |       |       |   `-- put_pixel_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- extract_band_impl.rs
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   |-- put_data_impl.rs
|   |   |   |       |       |   `-- put_pixel_impl.rs
|   |   |   |       |       |-- ssse3/
|   |   |   |       |       |   |-- extract_band_impl.rs
|   |   |   |       |       |   `-- mod.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- extract_band_impl.rs
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           |-- put_data_impl.rs
|   |   |   |       |           `-- put_pixel_impl.rs
|   |   |   |       |-- reduce/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- storage.rs
|   |   |   |       |   |   |-- kernels/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- native_bytes/
|   |   |   |       |   |   |   |-- general.rs
|   |   |   |       |   |   |   |-- kernels/
|   |   |   |       |   |   |   |   |-- avx2.rs
|   |   |   |       |   |   |   |   `-- wide.rs
|   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rgb.rs
|   |   |   |       |   |   |-- native_samples.rs
|   |   |   |       |   |   |-- scalar32.rs
|   |   |   |       |   |   `-- strategy.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- reduce_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- reduce_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- avx2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- reduce_impl.rs
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- reduce_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- reduce_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- reduce_impl.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           `-- reduce_impl.rs
|   |   |   |       |-- resize/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filtered.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filtered.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- nearest.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- alpha_samples.rs
|   |   |   |       |   |   |-- boxed_crop.rs
|   |   |   |       |   |   |-- byte_samples.rs
|   |   |   |       |   |   |-- coefficients.rs
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- luma16.rs
|   |   |   |       |   |   |-- luma16_vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- mode_dispatch.rs
|   |   |   |       |   |   |-- native_byte_vector/
|   |   |   |       |   |   |   |-- active_rows.rs
|   |   |   |       |   |   |   |-- bounded_i32.rs
|   |   |   |       |   |   |   |-- boxed.rs
|   |   |   |       |   |   |   |-- constants.rs
|   |   |   |       |   |   |   |-- convolution.rs
|   |   |   |       |   |   |   |-- execute.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- plan.rs
|   |   |   |       |   |   |   `-- telemetry.rs
|   |   |   |       |   |   |-- native_nearest.rs
|   |   |   |       |   |   |-- pad_window.rs
|   |   |   |       |   |   |-- prepared.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   |-- support.rs
|   |   |   |       |   |   |-- tall_boxed.rs
|   |   |   |       |   |   |-- typed_f32.rs
|   |   |   |       |   |   |-- typed_nearest.rs
|   |   |   |       |   |   `-- typed_u16.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filtered.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- nearest.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- streaming.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- kernels.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- plan.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- filtered.rs
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- vector.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- resize_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- resize_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- resize_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- resize_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- resize_impl.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           `-- resize_impl.rs
|   |   |   |       |-- rotate/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- byte.rs
|   |   |   |       |   |   |-- cubic.rs
|   |   |   |       |   |   |-- geometry.rs
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- i16.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- nearest.rs
|   |   |   |       |   |   |-- numeric.rs
|   |   |   |       |   |   |-- scalar.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   |-- vector.rs
|   |   |   |       |   |   |-- vector_bilinear.rs
|   |   |   |       |   |   |-- vector_interpolation.rs
|   |   |   |       |   |   |-- vector_nearest.rs
|   |   |   |       |   |   |-- vector_numeric.rs
|   |   |   |       |   |   `-- vector_typed_bilinear.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- luma_a16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- rgb16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- rgba16/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba32f/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- rotate_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- rotate_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- avx2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- rotate_impl.rs
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- rotate_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- rotate_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- rotate_impl.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           `-- rotate_impl.rs
|   |   |   |       |-- thumbnail/
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- mode_strategy.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- thumbnail_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- thumbnail_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   |-- strategy.rs
|   |   |   |       |   |   |   `-- streaming.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- putpixel_thumbnail.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   `-- rgba8/
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       `-- ops.rs
|   |   |   |       |-- transform/
|   |   |   |       |   |-- cmyk8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- f32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- helpers/
|   |   |   |       |   |   |-- affine.rs
|   |   |   |       |   |   |-- alpha.rs
|   |   |   |       |   |   |-- architecture_support.rs
|   |   |   |       |   |   |-- byte.rs
|   |   |   |       |   |   |-- byte_mode.rs
|   |   |   |       |   |   |-- byte_nearest.rs
|   |   |   |       |   |   |-- hidden_lanes.rs
|   |   |   |       |   |   |-- luma16.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- projective.rs
|   |   |   |       |   |   |-- scalar/
|   |   |   |       |   |   |   `-- projective.rs
|   |   |   |       |   |   |-- scalar.rs
|   |   |   |       |   |   |-- strategy.rs
|   |   |   |       |   |   |-- vector_affine_bilinear.rs
|   |   |   |       |   |   |-- vector_affine_nearest.rs
|   |   |   |       |   |   |-- vector_luma16.rs
|   |   |   |       |   |   |-- vector_projective_nearest.rs
|   |   |   |       |   |   `-- vector_samples.rs
|   |   |   |       |   |-- hsv8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- i16/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- i16b/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- i16l/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- i16n/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- i32/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- la_premultiplied8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- lab8/
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   `-- ops.rs
|   |   |   |       |   |-- luma8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- luma_a8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- mod.rs
|   |   |   |       |   |-- one/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- ops.rs
|   |   |   |       |   |-- palette8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- palette_alpha8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- rgb8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- rgb_premultiplied8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- rgba8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   |-- rgbx8/
|   |   |   |       |   |   |-- avx/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- avx2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- default/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- helpers/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- mod.rs
|   |   |   |       |   |   |-- neon/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   |-- ops.rs
|   |   |   |       |   |   |-- sse2/
|   |   |   |       |   |   |   |-- mod.rs
|   |   |   |       |   |   |   `-- transform_impl.rs
|   |   |   |       |   |   `-- wasm_simd128/
|   |   |   |       |   |       |-- mod.rs
|   |   |   |       |   |       `-- transform_impl.rs
|   |   |   |       |   `-- ycbcr8/
|   |   |   |       |       |-- avx/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- transform_impl.rs
|   |   |   |       |       |-- avx2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- transform_impl.rs
|   |   |   |       |       |-- default/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- transform_impl.rs
|   |   |   |       |       |-- helpers/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- transform_impl.rs
|   |   |   |       |       |-- mod.rs
|   |   |   |       |       |-- neon/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- transform_impl.rs
|   |   |   |       |       |-- ops.rs
|   |   |   |       |       |-- sse2/
|   |   |   |       |       |   |-- mod.rs
|   |   |   |       |       |   `-- transform_impl.rs
|   |   |   |       |       `-- wasm_simd128/
|   |   |   |       |           |-- mod.rs
|   |   |   |       |           `-- transform_impl.rs
|   |   |   |       `-- transpose/
|   |   |   |           |-- cmyk8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- f32/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- helpers/
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- native_luma16.rs
|   |   |   |           |   |-- native_mirror.rs
|   |   |   |           |   |-- native_transpose.rs
|   |   |   |           |   |-- pixel.rs
|   |   |   |           |   `-- strategy.rs
|   |   |   |           |-- hsv8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- i16/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- i16b/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- i16l/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- i16n/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- i32/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- la_premultiplied8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- lab8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- luma8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- luma_a16/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- luma_a8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- mod.rs
|   |   |   |           |-- one/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- ops.rs
|   |   |   |           |-- palette8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- palette_alpha8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- rgb16/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- rgb32f/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- rgb8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- rgb_premultiplied8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- rgba16/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- rgba32f/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   `-- ops.rs
|   |   |   |           |-- rgba8/
|   |   |   |           |   |-- avx2/
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   |-- rotate270_tiled.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           |-- rgbx8/
|   |   |   |           |   |-- default/
|   |   |   |           |   |   |-- flip_impl.rs
|   |   |   |           |   |   |-- mirror_impl.rs
|   |   |   |           |   |   |-- mod.rs
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- mod.rs
|   |   |   |           |   |-- neon/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   |-- ops.rs
|   |   |   |           |   |-- sse2/
|   |   |   |           |   |   `-- transpose_impl.rs
|   |   |   |           |   `-- wasm_simd128/
|   |   |   |           |       `-- transpose_impl.rs
|   |   |   |           `-- ycbcr8/
|   |   |   |               |-- default/
|   |   |   |               |   |-- flip_impl.rs
|   |   |   |               |   |-- mirror_impl.rs
|   |   |   |               |   |-- mod.rs
|   |   |   |               |   `-- transpose_impl.rs
|   |   |   |               |-- mod.rs
|   |   |   |               |-- neon/
|   |   |   |               |   `-- transpose_impl.rs
|   |   |   |               |-- ops.rs
|   |   |   |               |-- sse2/
|   |   |   |               |   `-- transpose_impl.rs
|   |   |   |               `-- wasm_simd128/
|   |   |   |                   `-- transpose_impl.rs
|   |   |   |-- pool_gpu/
|   |   |   |   |-- auxiliary_images.rs
|   |   |   |   |-- batch_arena_upload.rs
|   |   |   |   |-- batch_auxiliary_ranges.rs
|   |   |   |   |-- batch_encoding/
|   |   |   |   |   `-- dispatch.rs
|   |   |   |   |-- batch_encoding.rs
|   |   |   |   |-- batch_estimates.rs
|   |   |   |   |-- batch_execution/
|   |   |   |   |   |-- routing/
|   |   |   |   |   |   |-- execution/
|   |   |   |   |   |   |   `-- standard.rs
|   |   |   |   |   |   `-- execution.rs
|   |   |   |   |   `-- routing.rs
|   |   |   |   |-- batch_execution.rs
|   |   |   |   |-- batch_native_routes.rs
|   |   |   |   |-- batch_normalization.rs
|   |   |   |   |-- batch_operation_planning.rs
|   |   |   |   |-- batch_operations.rs
|   |   |   |   |-- batch_output.rs
|   |   |   |   |-- batch_parameters.rs
|   |   |   |   |-- batch_pipeline/
|   |   |   |   |   `-- blur_sources.rs
|   |   |   |   |-- batch_pipeline.rs
|   |   |   |   |-- batch_preflight.rs
|   |   |   |   |-- batch_preparation.rs
|   |   |   |   |-- batch_resize_coefficients.rs
|   |   |   |   |-- batch_segmentation.rs
|   |   |   |   |-- batch_telemetry.rs
|   |   |   |   |-- batch_upload/
|   |   |   |   |   `-- packing.rs
|   |   |   |   |-- batch_upload.rs
|   |   |   |   |-- device.rs
|   |   |   |   |-- fusion.rs
|   |   |   |   |-- geometry/
|   |   |   |   |   |-- native_execution/
|   |   |   |   |   |   |-- flip.rs
|   |   |   |   |   |   `-- transpose.rs
|   |   |   |   |   `-- native_execution.rs
|   |   |   |   |-- geometry.rs
|   |   |   |   |-- layout_admission.rs
|   |   |   |   |-- logical_mode_admission.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- mode.rs
|   |   |   |   |-- native_composite.rs
|   |   |   |   |-- native_dispatch.rs
|   |   |   |   |-- native_hsv_pad.rs
|   |   |   |   |-- native_input/
|   |   |   |   |   `-- channels.rs
|   |   |   |   |-- native_input.rs
|   |   |   |   |-- native_paste/
|   |   |   |   |   |-- execution/
|   |   |   |   |   |   `-- rgb_to_rgba.rs
|   |   |   |   |   |-- execution.rs
|   |   |   |   |   |-- masked_execution/
|   |   |   |   |   |   `-- byte_paste.rs
|   |   |   |   |   `-- masked_execution.rs
|   |   |   |   |-- native_paste.rs
|   |   |   |   |-- operation_admission/
|   |   |   |   |   |-- auxiliary_modes.rs
|   |   |   |   |   |-- safety.rs
|   |   |   |   |   `-- work_items.rs
|   |   |   |   |-- operation_admission.rs
|   |   |   |   |-- operation_support.rs
|   |   |   |   |-- pipeline_cache.rs
|   |   |   |   |-- point_ops/
|   |   |   |   |   |-- native_alpha.rs
|   |   |   |   |   `-- native_byte_op.rs
|   |   |   |   |-- point_ops.rs
|   |   |   |   |-- projection.rs
|   |   |   |   |-- readback.rs
|   |   |   |   |-- resize/
|   |   |   |   |   |-- compact_box.rs
|   |   |   |   |   |-- float_exactness/
|   |   |   |   |   |   `-- word_paths.rs
|   |   |   |   |   |-- float_exactness.rs
|   |   |   |   |   |-- float_exactness_f64.rs
|   |   |   |   |   |-- float_fast_paths.rs
|   |   |   |   |   |-- integer.rs
|   |   |   |   |   |-- integer_arithmetic.rs
|   |   |   |   |   |-- luma16.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   |-- ordered_float.rs
|   |   |   |   |   `-- tiled.rs
|   |   |   |   |-- resource_pool/
|   |   |   |   |   `-- auxiliary_cache.rs
|   |   |   |   |-- resource_pool.rs
|   |   |   |   |-- rgb_merge/
|   |   |   |   |   `-- execution.rs
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
|   |   |   |   |   |-- paste_native_masked_bytes.wgsl
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
|   |   |   |   |-- stream/
|   |   |   |   |   |-- dispatch.rs
|   |   |   |   |   |-- encoding/
|   |   |   |   |   |   `-- stage_encoding.rs
|   |   |   |   |   |-- encoding.rs
|   |   |   |   |   |-- graph/
|   |   |   |   |   |   |-- layout.rs
|   |   |   |   |   |   `-- resources.rs
|   |   |   |   |   |-- graph.rs
|   |   |   |   |   |-- paste.rs
|   |   |   |   |   `-- types.rs
|   |   |   |   |-- stream.rs
|   |   |   |   |-- thumbnail.rs
|   |   |   |   |-- transform/
|   |   |   |   |   |-- geometry.rs
|   |   |   |   |   |-- projective_admission/
|   |   |   |   |   |   `-- geometry_words.rs
|   |   |   |   |   |-- projective_admission.rs
|   |   |   |   |   |-- projective_exactness.rs
|   |   |   |   |   `-- safety.rs
|   |   |   |   `-- transform.rs
|   |   |   |-- registry/
|   |   |   |   |-- gpu_params.rs
|   |   |   |   |-- gpu_support.rs
|   |   |   |   |-- prepared.rs
|   |   |   |   |-- registration/
|   |   |   |   |   |-- chops.rs
|   |   |   |   |   |-- color.rs
|   |   |   |   |   |-- composite.rs
|   |   |   |   |   |-- draw.rs
|   |   |   |   |   |-- effects.rs
|   |   |   |   |   |-- enhance.rs
|   |   |   |   |   |-- extract_band.rs
|   |   |   |   |   |-- filter.rs
|   |   |   |   |   |-- geometry.rs
|   |   |   |   |   |-- imageops.rs
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   |-- module_fns.rs
|   |   |   |   |   |-- mutating.rs
|   |   |   |   |   |-- prelude.rs
|   |   |   |   |   `-- transform.rs
|   |   |   |   |-- support/
|   |   |   |   |   |-- mode.rs
|   |   |   |   |   |-- operation.rs
|   |   |   |   |   |-- request/
|   |   |   |   |   |   |-- contracts/
|   |   |   |   |   |   |   `-- request.rs
|   |   |   |   |   |   |-- contracts.rs
|   |   |   |   |   |   |-- facts.rs
|   |   |   |   |   |   |-- guard_assessment.rs
|   |   |   |   |   |   |-- guards/
|   |   |   |   |   |   |   |-- core.rs
|   |   |   |   |   |   |   |-- draw.rs
|   |   |   |   |   |   |   |-- mutation.rs
|   |   |   |   |   |   |   `-- resize.rs
|   |   |   |   |   |   `-- guards.rs
|   |   |   |   |   |-- request.rs
|   |   |   |   |   |-- route.rs
|   |   |   |   |   `-- target.rs
|   |   |   |   `-- support.rs
|   |   |   |-- registry.rs
|   |   |   |-- telemetry/
|   |   |   |   |-- eager_cpu.rs
|   |   |   |   `-- origins.rs
|   |   |   `-- telemetry.rs
|   |   |-- data/
|   |   |   |-- lab-identity-profile.icc
|   |   |   |-- lcms-lab-lut.u16le
|   |   |   `-- srgb-output-profile.icc
|   |   |-- draw/
|   |   |   |-- bitmap.rs
|   |   |   |-- color_input.rs
|   |   |   |-- curves.rs
|   |   |   |-- floodfill.rs
|   |   |   |-- geometry.rs
|   |   |   |-- line_joint.rs
|   |   |   |-- mod.rs
|   |   |   |-- primitives.rs
|   |   |   |-- rounded_rectangle.rs
|   |   |   |-- rounded_rectangle_program.rs
|   |   |   |-- text/
|   |   |   |   `-- composition.rs
|   |   |   |-- text.rs
|   |   |   |-- text_lines.rs
|   |   |   `-- text_mask.rs
|   |   |-- error.rs
|   |   |-- font/
|   |   |   |-- courb08.luma
|   |   |   |-- courb08.pil
|   |   |   |-- default_aileron.LICENSE.txt
|   |   |   |-- default_aileron.rs
|   |   |   |-- default_aileron.ttf
|   |   |   |-- imagingft/
|   |   |   |   |-- layout.rs
|   |   |   |   |-- render/
|   |   |   |   |   `-- bitmap.rs
|   |   |   |   |-- render.rs
|   |   |   |   `-- variation.rs
|   |   |   |-- imagingft.rs
|   |   |   |-- mod.rs
|   |   |   |-- pilfont.rs
|   |   |   |-- public_api.rs
|   |   |   |-- text_layout.rs
|   |   |   |-- text_layout_callbacks.rs
|   |   |   |-- text_output.rs
|   |   |   |-- text_value.rs
|   |   |   `-- text_wrap.rs
|   |   |-- format.rs
|   |   |-- image/
|   |   |   |-- backend.rs
|   |   |   |-- byte_export.rs
|   |   |   |-- channel_access.rs
|   |   |   |-- codec_adapter.rs
|   |   |   |-- color_count/
|   |   |   |   |-- pillow_table.rs
|   |   |   |   `-- specialized.rs
|   |   |   |-- color_count.rs
|   |   |   |-- construction/
|   |   |   |   |-- dynamic.rs
|   |   |   |   |-- empty.rs
|   |   |   |   |-- frombytes/
|   |   |   |   |   |-- layout/
|   |   |   |   |   |   `-- rgb_alpha.rs
|   |   |   |   |   |-- layout.rs
|   |   |   |   |   `-- transform.rs
|   |   |   |   |-- frombytes.rs
|   |   |   |   |-- internal_samples.rs
|   |   |   |   `-- palette_policy.rs
|   |   |   |-- construction.rs
|   |   |   |-- data_access/
|   |   |   |   |-- get.rs
|   |   |   |   `-- put.rs
|   |   |   |-- data_access.rs
|   |   |   |-- encode.rs
|   |   |   |-- entropy.rs
|   |   |   |-- materialization/
|   |   |   |   |-- derived.rs
|   |   |   |   |-- lifecycle.rs
|   |   |   |   |-- palette_output.rs
|   |   |   |   `-- receipts.rs
|   |   |   |-- materialization.rs
|   |   |   |-- metadata.rs
|   |   |   |-- mode_boundary.rs
|   |   |   |-- palette.rs
|   |   |   |-- pipeline_admission.rs
|   |   |   |-- pipeline_ops/
|   |   |   |   |-- dimensions.rs
|   |   |   |   `-- mode.rs
|   |   |   |-- pipeline_ops.rs
|   |   |   |-- pixel_access/
|   |   |   |   |-- palette.rs
|   |   |   |   `-- palette_components.rs
|   |   |   |-- pixel_access.rs
|   |   |   |-- projection/
|   |   |   |   `-- putpixel_pipeline.rs
|   |   |   |-- projection.rs
|   |   |   |-- statistics/
|   |   |   |   `-- result.rs
|   |   |   `-- statistics.rs
|   |   |-- image.rs
|   |   |-- image_sequence.rs
|   |   |-- image_utils.rs
|   |   |-- lab.rs
|   |   |-- lib.rs
|   |   |-- ops/
|   |   |   |-- analysis/
|   |   |   |   |-- bbox.rs
|   |   |   |   `-- histogram.rs
|   |   |   |-- analysis.rs
|   |   |   |-- array.rs
|   |   |   |-- chops.rs
|   |   |   |-- convert/
|   |   |   |   |-- binary.rs
|   |   |   |   |-- dispatch.rs
|   |   |   |   |-- input.rs
|   |   |   |   |-- luma16.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- mode.rs
|   |   |   |   |-- operation.rs
|   |   |   |   `-- palette.rs
|   |   |   |-- crop.rs
|   |   |   |-- enhance.rs
|   |   |   |-- filter.rs
|   |   |   |-- imageops/
|   |   |   |   |-- color.rs
|   |   |   |   |-- colorize/
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   `-- ops.rs
|   |   |   |   |-- equalize/
|   |   |   |   |   |-- mod.rs
|   |   |   |   |   `-- ops.rs
|   |   |   |   |-- exif.rs
|   |   |   |   |-- geometry/
|   |   |   |   |   |-- callbacks.rs
|   |   |   |   |   |-- expand.rs
|   |   |   |   |   |-- fit_callbacks.rs
|   |   |   |   |   |-- pad.rs
|   |   |   |   |   |-- pad_route.rs
|   |   |   |   |   `-- scale_callbacks.rs
|   |   |   |   |-- geometry.rs
|   |   |   |   `-- point.rs
|   |   |   |-- imageops.rs
|   |   |   |-- lut_hardlight.bin
|   |   |   |-- lut_overlay.bin
|   |   |   |-- lut_softlight.bin
|   |   |   |-- mod.rs
|   |   |   |-- module_fns/
|   |   |   |   `-- point.rs
|   |   |   |-- module_fns.rs
|   |   |   |-- param_filters.rs
|   |   |   |-- paste/
|   |   |   |   |-- color.rs
|   |   |   |   |-- input.rs
|   |   |   |   |-- operations.rs
|   |   |   |   `-- types.rs
|   |   |   |-- paste.rs
|   |   |   |-- pil_resize/
|   |   |   |   |-- alpha.rs
|   |   |   |   |-- coefficients.rs
|   |   |   |   |-- filters.rs
|   |   |   |   |-- float.rs
|   |   |   |   |-- horizontal_passes.rs
|   |   |   |   |-- luma16.rs
|   |   |   |   |-- samples.rs
|   |   |   |   `-- vertical_passes.rs
|   |   |   |-- pil_resize.rs
|   |   |   |-- quantize/
|   |   |   |   |-- input.rs
|   |   |   |   |-- mod.rs
|   |   |   |   `-- operations.rs
|   |   |   |-- resize/
|   |   |   |   `-- reducing_gap.rs
|   |   |   |-- resize.rs
|   |   |   |-- rotate.rs
|   |   |   |-- split.rs
|   |   |   |-- transform/
|   |   |   |   |-- fill.rs
|   |   |   |   `-- public.rs
|   |   |   |-- transform.rs
|   |   |   |-- transpose.rs
|   |   |   `-- utils.rs
|   |   |-- par.rs
|   |   |-- path.rs
|   |   |-- pipeline/
|   |   |   `-- support.rs
|   |   |-- pipeline.rs
|   |   |-- raster/
|   |   |   |-- buffer/
|   |   |   |   |-- iterators.rs
|   |   |   |   `-- mutable.rs
|   |   |   |-- buffer.rs
|   |   |   |-- color/
|   |   |   |   |-- blend.rs
|   |   |   |   |-- from_color.rs
|   |   |   |   |-- from_primitive.rs
|   |   |   |   |-- invert.rs
|   |   |   |   |-- mod.rs
|   |   |   |   |-- pixel_luma.rs
|   |   |   |   |-- pixel_rgb.rs
|   |   |   |   `-- types.rs
|   |   |   |-- dynamic/
|   |   |   |   |-- mode.rs
|   |   |   |   `-- transform.rs
|   |   |   |-- dynamic.rs
|   |   |   |-- error.rs
|   |   |   |-- identities.rs
|   |   |   |-- mod.rs
|   |   |   |-- mode.rs
|   |   |   `-- traits/
|   |   |       |-- mod.rs
|   |   |       |-- pixel.rs
|   |   |       |-- primitive.rs
|   |   |       `-- view.rs
|   |   `-- test_api.rs
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
|       |-- backend.rs
|       |-- color.rs
|       |-- draw/
|       |   |-- bitmap.rs
|       |   |-- shapes.rs
|       |   `-- text.rs
|       |-- draw.rs
|       |-- effects.rs
|       |-- font.rs
|       |-- image/
|       |   |-- filters.rs
|       |   `-- pixels.rs
|       |-- image.rs
|       |-- image_chops.rs
|       |-- image_io.rs
|       |-- image_ops.rs
|       |-- inputs/
|       |   |-- common.rs
|       |   |-- draw.rs
|       |   |-- filter.rs
|       |   |-- font.rs
|       |   |-- new_color.rs
|       |   |-- paste.rs
|       |   |-- pixel.rs
|       |   |-- rotate.rs
|       |   `-- transform.rs
|       |-- inputs.rs
|       |-- lib.rs
|       |-- palette.rs
|       |-- pipeline_telemetry.rs
|       |-- statistics.rs
|       `-- values.rs
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
|   |       |-- imagedraw2.py
|   |       |-- imageenhance.py
|   |       |-- imagefilter.py
|   |       |-- imagefont.py
|   |       |-- imageops.py
|   |       |-- imagepalette.py
|   |       |-- imagepath.py
|   |       |-- imagesequence.py
|   |       |-- imageshow.py
|   |       |-- imagestat.py
|   |       |-- imagetext.py
|   |       `-- operations.py
|   `-- src/
|       |-- backend.rs
|       |-- draw/
|       |   |-- custom_font_mask.rs
|       |   |-- floodfill.rs
|       |   |-- input.rs
|       |   |-- rounded_rectangle.rs
|       |   |-- text_layout.rs
|       |   `-- text_lines.rs
|       |-- draw.rs
|       |-- errors.rs
|       |-- font/
|       |   |-- pilfont.rs
|       |   |-- text_layout.rs
|       |   `-- text_wrap.rs
|       |-- font.rs
|       |-- getcolors.rs
|       |-- gpu_batch.rs
|       |-- image/
|       |   |-- analysis.rs
|       |   |-- arrays.rs
|       |   |-- constructors.rs
|       |   |-- creation.rs
|       |   |-- data.rs
|       |   |-- mod.rs
|       |   |-- operations.rs
|       |   |-- point.rs
|       |   |-- properties.rs
|       |   |-- put_alpha.rs
|       |   |-- sequence.rs
|       |   `-- transform.rs
|       |-- image_chops.rs
|       |-- image_color.rs
|       |-- image_filter.rs
|       |-- image_ops/
|       |   |-- autocontrast.rs
|       |   |-- colorize.rs
|       |   |-- equalize.rs
|       |   |-- exif.rs
|       |   |-- expand.rs
|       |   |-- pad.rs
|       |   |-- point.rs
|       |   `-- size_route.rs
|       |-- image_ops.rs
|       |-- image_path.rs
|       |-- inputs/
|       |   |-- analysis.rs
|       |   |-- color_lut.rs
|       |   |-- filters.rs
|       |   |-- float_buffer.rs
|       |   |-- images.rs
|       |   |-- masks.rs
|       |   |-- ops.rs
|       |   |-- paths.rs
|       |   `-- pixels.rs
|       |-- inputs.rs
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
    |-- check_large_files.py
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
    |   `-- audit.py
    |-- lint.sh
    |-- list_migration_gpu_adapters.py
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
    |   |-- fault_contracts.py
    |   |-- gpu_batch_inputs.py
    |   |-- mode_size_matrix.py
    |   |-- pipeline_contract.py
    |   |-- quantize_inputs.py
    |   `-- target_output_contracts.py
    |-- migration_parity_inventory.py
    |-- migration_producer_attestation.py
    |-- migration_producer_identity.py
    |-- prepare_pypi_release.py
    |-- profile_migration_benchmark.py
    |-- public_api_boundary_baseline.json
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
    |   `-- rust_source.py
    |-- test_chops_affine_parity.py
    |-- test_font_variant_parity.py
    |-- test_getdata_sequence_parity.py
    |-- test_gpu_batch.py
    |-- test_grayscale_rgb_domain.py
    |-- validate_migration_parity_contract.py
    `-- validate_migration_parity_result.py
```
<!-- END GENERATED CODE TREE -->
