# CPU refactor source and evidence inventory

Status: baseline inventory captured; the 98 CPU and 182 SIMD files are preserved below pool_cpu/legacy; maintained implementation is in progress.
Baseline commit: 3f9dfd4036caa061eabcf2255c9f34a745df9c7e
Working checkout: `/Users/lazytrot/work/pillow-rs`, branch `main` at
`3f9dfd4036caa061eabcf2255c9f34a745df9c7e`. The active tree contains the
in-progress CPU migration and pre-existing staged legacy archive renames;
these changes have been preserved. Eval, ImageOps Expand, Crop, and Effects source work
is recorded below. No build, formatter, checkstyle, lint, test, parity, benchmark,
or coverage command has run.
Related Kata issues: pillow-rs#3ptb (active CPU goal), pillow-rs#brg0 (related parity/performance findings; remains open).

This inventory records the source and evidence denominator before rebuilding pool_cpu. It is a migration ledger, not a claim that the listed mode literals prove support. Each function's exact mode support, output identity, rejection behavior, shared helpers, and architecture fallback must be confirmed from its implementation before it is copied into the maintained tree. The source path lists are taken from the baseline Git tree, so every archived file can be checked against 3f9dfd403.

## 1. Baseline and preservation boundary

- pool_cpu: 98 tracked Rust files: 2 pool-level files and 96 files under ops.
- pool_simd: 182 tracked Rust files: 4 pool-level files and 178 files under ops.
- PipelineOp: 84 baseline variants assigned to 21 operation families. Exact variant ownership is frozen in Section 5 of IMPLEMENTATION_STRUCTURE.md.
- No pool_parallel existed at baseline.
- At the baseline, raster::DynamicImage has ten storage variants and no exact mode tag. The public Pillow image wrapper carries logical mode outside that enum. The maintained enum expansion is in progress and has not been compiled or validated.
- The baseline raster storage variants are ImageLuma8, ImageLumaA8, ImageRgb8, ImageRgba8, ImageLuma16, ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, and ImageRgba32F. This layout cannot distinguish multiple Pillow identities sharing a storage buffer.
- Current mode carriers to reconcile: `LoadedData.explicit_mode` and `Image::Pipeline.explicit_mode` use `Option<raster::Mode>`; `decoded_mode` remains image-slash-star codec metadata. `PipelineModeCache` and `known_pipeline_op_mode` also store and propagate `Mode`; compatibility accessors project to Pillow spelling. `Image::from_dynamic` now accepts `Option<Mode>`, while `Draw::new` still accepts a string override (TODO at its boundary). `Image::mode()` remains the Pillow-compatible string result. `PipelineOp` keeps typed `ColorMode` and `PixelMode` inputs; Merge still retains `logical_mode: String`. CPU/SIMD leaf functions still receive string modes on compatibility routes. Keep destination modes and per-operation mode parameters where they express operation inputs, and use the exact mode tag on `DynamicImage` for materialized sources.
- Existing ColorMode is declared in pipeline/support.rs and includes Mode1, P, PA, L, LA, LaPremultiplied, RGB, RGBA, RgbPremultiplied, Rgbx, CMYK, YCbCr, HSV, I, F, and one aggregate I16. Existing PixelMode covers L, LA, RGB, RGBA, P, PA, CMYK, Mode1, YCbCr, HSV, I, and F. Both need an explicit mapping to the exact raster mode identities, including four distinct I;16 variants; neither is the image identity source.
- Mode boundaries to update include raster/dynamic.rs construction/decode adapters and transforms; image.rs LoadedData, PalettedData, Pipeline and caches; image construction/metadata/palette modules; pipeline.rs operation descriptors and transition planning; compute routing and every mode-string consumer in CPU/GPU compatibility paths; and the bindings' existing mode/decode handoff. Preserve image-slash-star ownership of EncodedImage and DecodedImage.
- The target has 25 exact Rust identities: 20 existing Pillow mode identities plus five internal high-depth representations. The authoritative variant and folder mapping is in [Implementation structure, Section 2](IMPLEMENTATION_STRUCTURE.md#2-mode-bearing-raster); this inventory does not define a separate path grammar.
- Design amendment: every operation exposes its own typed support contract and composes mode and architecture strategies. The declaration binds support to executable selection and names guards, output identity, build availability, and fallback/rejection reasons. The top-level descriptor and actual prepared selection follow [Implementation structure, Section 3](IMPLEMENTATION_STRUCTURE.md#3-injected-execution-context). This is required target behavior, not a claim that the in-progress source implements it.
- Ownership amendment: pillow-rs owns the exact Mode selector and mode-bearing DynamicImage. image-slash-star retains codec types and format metadata, translated by Pillow-owned decode/encode adapters. Python parses Pillow strings before typed core calls. See [the mode ownership contract](IMPLEMENTATION_STRUCTURE.md#21-mode-ownership-and-codec-boundaries); implementation remains in progress and unvalidated.
- Crosswalk source: revision 3f9dfd4036caa061eabcf2255c9f34a745df9c7e; 922 input files, SHA-256 d2268b046270c9a7f5816e9549baf797cdd338f5ecb12126ec5bfbffdd2daf6e. Static analysis found 84 variants and no unresolved module edges, but it does not prove runtime reachability or execution.
- Crosswalk surface summary: 214 declared public operations, 214 mapped to Python bindings, 108 PyO3 core API calls mapped, 94 direct receiver-method routes mapped, 160 Python-native calls mapped to registered PyO3 methods, and 81 registry keys. It reports zero unresolved public binding calls, pipeline variants, module edges, fixture operation references, or benchmark-to-parity references. These are static mappings; dynamic callbacks and execution paths remain review items.
- Worktrees inventoried: main at the baseline, /Users/lazytrot/.codex/worktrees/cpu-benchmark-baseline/pillow-rs at c5104cbaa (detached, dirty and preserved), and /private/tmp/pillow-rs-cpu-baseline-e843 at e843f864 (detached and preserved). The main tree was [ahead 1] of origin/main; 144 refs under refs/codex were preserved. No worktree/ref was pruned.
- Kata issue pillow-rs#3ptb tracks the current implementation and remaining completion gates. The continuation goal is recorded in [CPU refactor plan](CPU_REFACTOR_PLAN.md#goal-prompt); preserve in-progress work when continuing it.

## 2. Archive moves and active ownership

Stage 1 preservation is complete: all 98 pool_cpu paths below moved intact under pool_cpu/legacy/, preserving each listed path relative to pool_cpu. All 182 pool_simd paths moved intact under pool_cpu/legacy/pool_simd/, preserving each path relative to pool_simd. Both archives stay outside the compiled module graph. Exact supported/rejected mode contracts and output transitions will be recorded in this ledger from the preserved source before each family's maintained implementation is written.

Active CPU operation ownership follows the family table in IMPLEMENTATION_STRUCTURE.md. In particular, old pool_cpu single-file families are split into family orchestration, exact mode implementations, and Default or actual architecture implementation files. Existing SIMD architecture kernels are folded into the same family/mode owner. The table below gives the frozen source directory groups and target owner; individual functions are accounted for when the family source is reviewed, before the maintained implementation is finalized.

CPU paths already match the intended family names except for the old geometry and imageops split: all PipelineOp ownership is the explicit 21-family mapping in IMPLEMENTATION_STRUCTURE.md. Pool-level batch execution becomes executor/batch orchestration; the old pool and ops mod.rs files are archive-only. The allocation test source is test material and has no maintained runtime destination.

The 43 old SIMD adapter groups are assigned to these CPU families. Subdirectories beneath each adapter stay with that family unless the source review proves that a helper is shared at a narrower valid scope.

| SIMD source group | CPU destination owner |
| --- | --- |
| alpha_composite, merge, module_blend, native_merge | composite |
| box_blur, filter, gaussian_blur, median_filter, neighborhood, native_filter | filter |
| chops, duplicate, native_arithmetic, offset | chops |
| color3dlut | color3dlut |
| convert | color |
| colorize, expand, histogram, lut | imageops |
| crop | crop |
| draw | draw |
| effects, gradient, mandelbrot | effects and direct image-generation entry points |
| enhance | enhance |
| extract_band, native_channel, put_alpha, put_data, put_pixel, put_pixel_batch | put |
| flip, transpose | transpose |
| grayscale | grayscale |
| pad | geometry |
| paste | paste |
| reduce | reduce |
| resize | resize; Pad remains geometry-owned, with shared kernels placed only after source/caller review |
| rotate | rotate |
| transform | transform |
| adapters.rs, layout, native_bytes, pipeline, projection, support, point_batch, batch_execution, pool facade, ops facade | registry/executor or the narrow shared helper owner established from imports and callers; these are not independent operation families |

### CPU source groups
### (ops module) (1 files)
- pillow-rs/src/compute/pool_cpu/ops/mod.rs

### (pool support) (2 files)
- pillow-rs/src/compute/pool_cpu/batch_execution.rs
- pillow-rs/src/compute/pool_cpu/mod.rs

### chops (1 files)
- pillow-rs/src/compute/pool_cpu/ops/chops.rs

### color (1 files)
- pillow-rs/src/compute/pool_cpu/ops/color.rs

### color3dlut (1 files)
- pillow-rs/src/compute/pool_cpu/ops/color3dlut.rs

### color_convert_allocation_tests (1 files)
- pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs

### composite (2 files)
- pillow-rs/src/compute/pool_cpu/ops/composite.rs
- pillow-rs/src/compute/pool_cpu/ops/composite/tests.rs

### crop (1 files)
- pillow-rs/src/compute/pool_cpu/ops/crop.rs

### draw (10 files)
- pillow-rs/src/compute/pool_cpu/ops/draw.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/arcs.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/canvas.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/clipping.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/dispatch.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/ellipse.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/operations.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/polygon.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/primitives.rs
- pillow-rs/src/compute/pool_cpu/ops/draw/rounded_rect.rs

### effects (1 files)
- pillow-rs/src/compute/pool_cpu/ops/effects.rs

### enhance (1 files)
- pillow-rs/src/compute/pool_cpu/ops/enhance.rs

### eval (2 files)
- pillow-rs/src/compute/pool_cpu/ops/eval.rs
- pillow-rs/src/compute/pool_cpu/ops/eval/tests.rs

### filter (27 files)
- pillow-rs/src/compute/pool_cpu/ops/filter.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/box_rows.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/line.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one/vertical.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/convolution.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/bounded_binomial.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes/binomial.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/blur_line_steps.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/i32_cross_smooth.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/mod.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rank.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/float.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/rgb_blur_ring.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_box.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_box_tests.rs
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_image.rs

### geometry (1 files)
- pillow-rs/src/compute/pool_cpu/ops/geometry.rs

### grayscale (1 files)
- pillow-rs/src/compute/pool_cpu/ops/grayscale.rs

### imageops (9 files)
- pillow-rs/src/compute/pool_cpu/ops/imageops.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/colorize.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/expand.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/geometry.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/histogram.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/native.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/tests.rs
- pillow-rs/src/compute/pool_cpu/ops/imageops/pixel.rs

### paste (4 files)
- pillow-rs/src/compute/pool_cpu/ops/paste.rs
- pillow-rs/src/compute/pool_cpu/ops/paste/kernels.rs
- pillow-rs/src/compute/pool_cpu/ops/paste/layout.rs
- pillow-rs/src/compute/pool_cpu/ops/paste/tests.rs

### put (1 files)
- pillow-rs/src/compute/pool_cpu/ops/put.rs

### reduce (4 files)
- pillow-rs/src/compute/pool_cpu/ops/reduce.rs
- pillow-rs/src/compute/pool_cpu/ops/reduce/kernels.rs
- pillow-rs/src/compute/pool_cpu/ops/reduce/kernels/rgb.rs
- pillow-rs/src/compute/pool_cpu/ops/reduce/tests.rs

### resize (7 files)
- pillow-rs/src/compute/pool_cpu/ops/resize.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/filter.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/float.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize_tests.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/integer.rs
- pillow-rs/src/compute/pool_cpu/ops/resize/tests.rs

### rotate (6 files)
- pillow-rs/src/compute/pool_cpu/ops/rotate.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine/coordinates.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/cubic.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/scalar.rs
- pillow-rs/src/compute/pool_cpu/ops/rotate/tests.rs

### thumbnail (3 files)
- pillow-rs/src/compute/pool_cpu/ops/thumbnail.rs
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/fusion.rs
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/tests.rs

### transform (6 files)
- pillow-rs/src/compute/pool_cpu/ops/transform.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/affine.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/projective.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar/projective.rs
- pillow-rs/src/compute/pool_cpu/ops/transform/tests.rs

### transpose (5 files)
- pillow-rs/src/compute/pool_cpu/ops/transpose.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/arch.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/kernels/transpose_rgba/x86_64.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/native.rs
- pillow-rs/src/compute/pool_cpu/ops/transpose/tests.rs

### SIMD source groups
### (pool support) (4 files)
- pillow-rs/src/compute/pool_simd/batch_execution.rs
- pillow-rs/src/compute/pool_simd/mod.rs
- pillow-rs/src/compute/pool_simd/point_batch.rs
- pillow-rs/src/compute/pool_simd/put_pixel_batch.rs

### alpha_composite.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/alpha_composite.rs

### box_blur (8 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/luma.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgb.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_avx2.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_neon.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/tests.rs

### box_blur.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur.rs

### chops (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels/fused_multiply_screen.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels/tests.rs

### chops.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/chops.rs

### color3dlut.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/color3dlut.rs

### colorize.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/colorize.rs

### convert (13 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/alpha.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/cmyk.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/color_space.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/layout.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/luma.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/typed.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/layout.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests/cmyk_packing.rs

### convert.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs

### crop.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/crop.rs

### draw (6 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes/rounded_rect.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans/line.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/tests.rs

### draw.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/draw.rs

### duplicate (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/duplicate/tests.rs

### duplicate.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/duplicate.rs

### effects (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/effects/tests.rs

### effects.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/effects.rs

### enhance (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/point.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/sharpness.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/tests.rs

### enhance.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance.rs

### expand (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/expand/tests.rs

### expand.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/expand.rs

### extract_band.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs

### filter (10 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/byte_layouts.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/i32_three_by_three.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/l.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/rgb_blur_ring.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/three_by_three.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/uniform_rgb.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/unsharp.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/tests.rs

### filter.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs

### flip.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/flip.rs

### gaussian_blur (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/rgba_uniform.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/tests.rs

### gaussian_blur.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs

### gradient.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/gradient.rs

### grayscale (7 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/channels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/cmyk.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/rgb_avx2.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr_ssse3.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/tests.rs

### grayscale.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs

### histogram.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/histogram.rs

### layout.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/layout.rs

### lut (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/lut/tests.rs

### lut.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/lut.rs

### mandelbrot.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/mandelbrot.rs

### median_filter (4 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/extrema.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/float.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/tests.rs

### median_filter.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter.rs

### merge (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/merge/tests.rs

### merge.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs

### module_blend.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/module_blend.rs

### native/shared: adapters (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters.rs

### native/shared: mod (1 files)
- pillow-rs/src/compute/pool_simd/ops/mod.rs

### native/shared: native_channel (2 files)
- pillow-rs/src/compute/pool_simd/ops/native_channel.rs
- pillow-rs/src/compute/pool_simd/ops/native_channel/cmyk_ssse3.rs

### native/shared: native_filter (1 files)
- pillow-rs/src/compute/pool_simd/ops/native_filter.rs

### native/shared: native_merge (1 files)
- pillow-rs/src/compute/pool_simd/ops/native_merge.rs

### native_arithmetic.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/native_arithmetic.rs

### native_bytes.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/native_bytes.rs

### neighborhood.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/neighborhood.rs

### offset (2 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/byte.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/luma16.rs

### offset.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/offset.rs

### pad (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/pad/tests.rs

### pad.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/pad.rs

### paste (17 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/cmyk_l.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/generic.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/l.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/la.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/pa.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgb.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgba.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/blend.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma16.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/rgb_cmyk.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/mod.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/plan.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/rgb_to_rgba.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/tests.rs

### pipeline.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/pipeline.rs

### projection.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/projection.rs

### put_alpha (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha/kernels.rs

### put_alpha.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha.rs

### put_data.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_data.rs

### put_pixel (2 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels/rgba_batch.rs

### put_pixel.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel.rs

### reduce (2 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/tests.rs

### reduce.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce.rs

### resize (17 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/aspect.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/coordinates.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32/boxed.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_convolution.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_cover_i32.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_plan.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/aspect.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/resize.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/tests.rs

### resize.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs

### rotate (8 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/interpolation.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/bilinear.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/typed_bilinear.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/tests.rs

### rotate.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate.rs

### support.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/support.rs

### thumbnail (3 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs

### thumbnail.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs

### transform (10 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/affine.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/projective.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/luma16.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/mesh.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/affine.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/projective.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/tests.rs

### transform.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transform.rs

### transpose (5 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/portable.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/odd_tests.rs
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/tests.rs

### transpose.rs (1 files)
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose.rs


## 3. Mode and pixel-storage inventory

The raster-owned mode identity needs to be carried by the materialized DynamicImage variant itself. Do not infer it from channels, ColorType, buffer contents, or a separate string. Decode metadata from image-slash-star must be adapted at the raster boundary; it must not be added to image-slash-star's type. Lazy pipeline operations must use the source mode and explicitly account for mode-changing operations before materialization.

The current storage-level DynamicImage cases are not equivalent to the 20 Pillow modes. Source review must classify:
- all image construction and decode boundaries;
- raw-byte imports, frombuffer/fromarray-style adapters, and explicit conversion;
- mode-preserving crop, flip/transpose, resize, rotate, transform, and thumbnail;
- mode-changing conversion, LAB conversion, palette remap, grayscale, band extraction, alpha insertion, and other output-mode operations;
- secondary image, mask, merge-band, paste, and composite input identities;
- palette index/alpha metadata, premultiplied alpha, RGBX filler bytes, LAB conventions, I and F byte carriers, and I;16 byte order;
- all error/rejection outcomes and all high-depth representations.
A branch on a mode string or PixelMode in baseline code is a review lead, not a complete support contract.

### Literal mode candidates found in baseline source

These are strings found in production CPU/SIMD files, excluding test files. They are search evidence only; absence of a literal does not imply unsupported behavior, and a literal can be a comment, error, test helper, or validation route. Reconcile each candidate against the function body and public caller during migration.

CPU candidate modes by source family:
- (pool support): L, LA, RGB, RGBA, F
- color: 1, L, LA, La, P, RGB, RGBA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N
- composite: P, RGB, LAB
- draw: I;16, I;16B, I;16N, F
- enhance: L, LA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- eval: 1, L, P
- filter: L, LA, RGB, RGBA, I, F
- grayscale: 1, L, La, RGBa, CMYK, YCbCr, HSV, I, F
- imageops: 1, L, LA, P, PA, RGB, RGBA, RGBX, CMYK, HSV, F
- paste: 1, L, LA, La, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, LAB, HSV, I;16, I;16L, I;16B, I;16N, I, F
- reduce: La, PA, RGB, RGBA, RGBa, RGBX, CMYK, I, F
- resize: 1, LA, La, RGBA, RGBa, I, F
- rotate: 1, L, LA, La, P, PA, RGBa, RGBX, CMYK, I;16, I;16B, I;16N, I, F
- thumbnail: 1, P, RGB, RGBa, RGBX, CMYK, I, F
- transform: 1, L, LA, La, P, PA, RGB, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16B, I;16N, I, F

SIMD candidate modes by source family:
- (pool support): L, LA, P, PA, RGB, RGBA, I;16, I;16L, I;16B, I;16N, I, F
- alpha_composite.rs: LA, RGBA
- box_blur.rs: L, RGB, RGBA
- chops.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- colorize.rs: L
- convert: LA, La, RGBA, RGBa
- convert.rs: 1, L, RGB, RGBA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N
- draw.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, F
- effects.rs: 1, L, LA, P, PA, RGB, RGBA, CMYK
- enhance: L, LA, RGB, RGBA, RGBX, YCbCr, HSV
- enhance.rs: L, LA, RGB, RGBA, RGBX, CMYK, YCbCr, HSV
- extract_band.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- filter: L, LA
- filter.rs: 1, L, LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I
- flip.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, F
- gaussian_blur.rs: L, LA, RGB
- gradient.rs: 1, L, P, I, F
- grayscale.rs: L, LA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr
- histogram.rs: 1, L, RGB
- layout.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N, I, F
- lut.rs: 1, L, LA, P, RGB, RGBA, CMYK, YCbCr, HSV
- median_filter: F
- median_filter.rs: LA, F
- merge.rs: L, LA, P, RGB, RGBA, CMYK
- module_blend.rs: L, LA, La, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV
- offset: I;16, I;16L, I;16B, I;16N
- pad.rs: L, LA, P, RGB, RGBA, RGBX, CMYK, I, F
- paste: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N, I, F
- pipeline.rs: 1, L, LA, La, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, LAB, HSV, I;16, I, F
- put_alpha.rs: L, LA, P, PA, RGB, RGBA, RGBX, CMYK
- put_pixel.rs: RGB, RGBA, LAB
- reduce.rs: L, LA, RGB, RGBA, CMYK, YCbCr, HSV
- resize: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N, I, F
- resize.rs: LA, La, PA, RGBA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N, I, F
- rotate: PA, RGBA, I, F
- rotate.rs: 1, P, PA, RGBa, RGBX, CMYK, I;16, I;16L, I;16B, I;16N, I, F
- thumbnail: RGB
- thumbnail.rs: 1, P, RGB, RGBa, RGBX, CMYK, I, F
- transform: L, I;16, I;16B, I;16N
- transform.rs: L, LA, RGB, RGBA, CMYK, YCbCr, HSV, I;16, I;16L, I;16B, I;16N
- transpose.rs: 1, L, LA, P, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, HSV, I, F

## 4. CPU/SIMD architecture and parallel inventory

### Reduce family mode and architecture migration (source review pending)

The new operation owner is present at pillow-rs/src/compute/pool_cpu/ops/reduce.
Its top-level ops.rs classifies every one of the 25 exact Mode identities,
assesses request factors, selects a declared CPU strategy, and prepares the
selected mode implementation. The CPU registry now calls that prepared entry
point. Every mode has a same-mode serial Default strategy using its native
carrier and returning an exact mode-bearing DynamicImage. L, LA, RGB, RGBA,
CMYK, YCbCr, and HSV also declare exact-mode AVX2 strategies for the archived
native-byte vector reducer. The shared AVX2 helper uses bounded AVX2 fixed-point
lanes, while the portable wide-lane helper is selected as SSE2, NEON, or wasm
SIMD128 when built and supported. Each mode declaration supplies its native
channel and alpha semantics. RGB, YCbCr, and HSV retain the 3-byte 2x2 vector
block; RGB retains the 3x5 fast block and sparse 2x2 path. Both vector kernels
retain premultiplication and per-edge rounding rules, while partial row tails
use the same mode's Default helper. The special 3-byte blocks require eight
complete source blocks; incomplete right edges use the bounded general path.

| Exact source identity | Maintained strategy |
| --- | --- |
| One, ImageLuma8 | Native one-byte reducer with mode-preserving output; ImageLuma8 declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageLumaA8 | Native LA bytes with archived premultiply/average/unpremultiply arithmetic; declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageLaPremultiplied8 | Native LA bytes averaged without straight-alpha premultiplication. |
| ImagePalette8, ImagePaletteAlpha8 | Explicit identity-only strategy; actual reduction is rejected by the factor contract. |
| ImageRgb8 | Native RGB reducer retaining the archived dense-even and sparse 2x2 paths plus the general RGB row kernel; AVX2 and portable SSE2/NEON/wasm SIMD handle lane reduction, including the 2x2 and 3x5 blocks. |
| ImageRgba8 | Native RGBA reducer retaining premultiplied-alpha arithmetic and its specialized block sum; declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageRgbPremultiplied8, ImageRgbx8 | Four-byte native strategies with mode-specific alpha/channel semantics and no cross-mode conversion. |
| ImageCmyk8 | Four-byte native samples with no alpha semantics; declares AVX2 and portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageYCbCr8, ImageHsv8 | Three-byte native reducers; samples are averaged without color-space transforms and declare AVX2 plus portable SSE2/NEON/wasm SIMD lane strategies. |
| ImageLab8 | Three-byte native reducer; samples are averaged without color-space transforms. |
| ImageI32, ImageF32 | Scalar four-byte carriers with the archived I wrapping and F ordered arithmetic, returned under their exact mode variants. |
| ImageI16, ImageI16L, ImageI16B, ImageI16N | Native typed u16 reduction that preserves each exact I;16 identity. This is core-only and marked NO_PILLOW_SUPPORT. |
| ImageLumaA16, ImageRgb16, ImageRgba16, ImageRgb32F, ImageRgba32F | Native typed high-depth reduction that preserves each internal identity. These are core-only and marked NO_PILLOW_SUPPORT. |

Remaining Reduce work is explicit in the operation owner: share the I/F scalar
helpers and RGB sparse-row facts with the later Thumbnail owner, and move
archived Rayon scheduling to pool_parallel. Same-mode Default strategies
remain available on every mode. This migration is source-only; no build,
formatting, lint, test, parity, or benchmark verification has run. The archived
SIMD instructions and guards are in
pool_cpu/legacy/pool_simd/ops/adapters/reduce.rs and its reduce/kernels files.

Architecture evidence below records baseline target-feature annotations and runtime probes. The real instruction set is determined by the implementation body and its safety contract, not by a filename alone. Move each true specialization to its exact architecture directory, retain the required compile-time gates, and route through injected Architecture only after the centralized capability policy guarantees execution safety. Portable shared kernels remain in Default or the narrow shared-helper location; do not create empty architecture folders.

CPU target-feature/probe sites:
- ops/filter/filter3x3.rs:473: target_feature = "neon"
- ops/filter/filter5x5.rs:220: #[cfg(target_feature = "neon")]
- ops/filter/filter5x5/int32.rs:36: std::is_x86_feature_detected!("fma").then_some(Self { _private: () })

SIMD target-feature/probe sites:
- ops/adapters.rs:49: target_feature = "neon"
- ops/adapters/box_blur.rs:220: std::is_x86_feature_detected!("avx2")
- ops/adapters/box_blur/kernels/rgba_avx2.rs:14: #[target_feature(enable = "avx2")]
- ops/adapters/box_blur/kernels/rgba_avx2.rs:95: #[target_feature(enable = "avx2")]
- ops/adapters/box_blur/kernels/rgba_avx2.rs:161: if !std::is_x86_feature_detected!("avx2") {
- ops/adapters/convert/cmyk.rs:3: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:6: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:8: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:10: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:22: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:26: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:32: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:66: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:78: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/cmyk.rs:107: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/cmyk.rs:116: let ssse3_available = std::is_x86_feature_detected!("ssse3");
- ops/adapters/convert/kernels/layout.rs:8: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:135: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/kernels/layout.rs:379: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/kernels/layout.rs:417: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:433: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/convert/kernels/layout.rs:453: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/layout.rs:484: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/mod.rs:6: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/mod.rs:30: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/convert/kernels/rgba_rgb_ssse3.rs:18: || !std::is_x86_feature_detected!("ssse3")
- ops/adapters/convert/kernels/rgba_rgb_ssse3.rs:30: #[target_feature(enable = "ssse3")]
- ops/adapters/convert/kernels/rgba_rgb_ssse3.rs:99: if !std::is_x86_feature_detected!("ssse3") {
- ops/adapters/duplicate.rs:3: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/duplicate.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:12: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:31: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:44: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:94: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:96: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/duplicate.rs:108: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/duplicate.rs:110: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/enhance/sharpness.rs:274: target_feature = "fma",
- ops/adapters/enhance/sharpness.rs:275: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/enhance/sharpness.rs:281: target_feature = "fma",
- ops/adapters/enhance/sharpness.rs:282: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/extract_band.rs:73: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/extract_band.rs:85: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/extract_band.rs:300: if !std::is_x86_feature_detected!("ssse3") {
- ops/adapters/extract_band.rs:337: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/extract_band.rs:373: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/filter/kernels/three_by_three.rs:4: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/filter/kernels/three_by_three.rs:311: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/filter/kernels/three_by_three.rs:390: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:61: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:76: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:111: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:212: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/channels.rs:218: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/grayscale/kernels/cmyk.rs:5: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/cmyk.rs:7: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/cmyk.rs:10: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:12: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:14: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/cmyk.rs:20: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:36: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:62: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:80: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/cmyk.rs:91: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/rgb_avx2.rs:11: if !std::is_x86_feature_detected!("avx2") {
- ops/adapters/grayscale/kernels/rgb_avx2.rs:120: #[target_feature(enable = "avx2")]
- ops/adapters/grayscale/kernels/rgb_avx2.rs:178: if !std::is_x86_feature_detected!("avx2") {
- ops/adapters/grayscale/kernels/ycbcr.rs:24: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/ycbcr.rs:74: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/ycbcr.rs:79: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/ycbcr.rs:85: if std::is_x86_feature_detected!("ssse3") {
- ops/adapters/grayscale/kernels/ycbcr.rs:92: all(target_arch = "aarch64", target_feature = "neon"),
- ops/adapters/grayscale/kernels/ycbcr.rs:122: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/grayscale/kernels/ycbcr.rs:136: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/grayscale/kernels/ycbcr_ssse3.rs:11: if source.len() % 3 != 0 || !std::is_x86_feature_detected!("ssse3") {
- ops/adapters/grayscale/kernels/ycbcr_ssse3.rs:20: #[target_feature(enable = "ssse3")]
- ops/adapters/lut.rs:380: target_feature = "neon",
- ops/adapters/lut.rs:428: target_feature = "neon",
- ops/adapters/lut.rs:530: target_feature = "neon",
- ops/adapters/lut.rs:546: target_feature = "neon",
- ops/adapters/lut.rs:679: target_feature = "neon",
- ops/adapters/merge.rs:140: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/merge.rs:148: if channels == 3 && pixels >= 16 && std::is_x86_feature_detected!("ssse3") {
- ops/adapters/module_blend.rs:174: all(target_arch = "aarch64", target_feature = "neon"),
- ops/adapters/module_blend.rs:177: target_feature = "fma"
- ops/adapters/paste/kernels/luma.rs:18: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:119: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:121: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/luma.rs:123: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:204: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:270: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:325: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:353: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:400: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:434: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/luma.rs:436: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:107: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:196: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:205: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:208: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:358: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:367: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:370: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/paste/kernels/rgb_cmyk.rs:459: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:21: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:30: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:32: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:38: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:58: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:81: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:103: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:126: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:134: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:136: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:142: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:296: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:300: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:339: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:363: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:369: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:382: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:412: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:414: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:427: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:446: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:452: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:455: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:500: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:502: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32.rs:517: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/resize/kernels/i32.rs:523: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/resize/kernels/i32/neon.rs:50: // by the module's compile-time target_feature gate; the four accumulators
- ops/adapters/resize/kernels/i32/neon.rs:129: // AArch64 NEON is enabled by the module's compile-time target_feature gate.
- ops/adapters/rotate/interpolation.rs:61: target_feature = "fma",
- ops/adapters/rotate/interpolation.rs:62: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/rotate/interpolation.rs:68: target_feature = "fma",
- ops/adapters/rotate/interpolation.rs:69: all(target_arch = "aarch64", target_feature = "neon")
- ops/adapters/thumbnail/kernels/float.rs:75: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/thumbnail/kernels/float.rs:254: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/adapters/thumbnail/kernels/float.rs:273: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/thumbnail/kernels/float.rs:298: #[cfg(not(all(target_arch = "aarch64", target_feature = "neon")))]
- ops/adapters/transpose/kernels/rgba_avx2.rs:27: || !std::is_x86_feature_detected!("avx2")
- ops/adapters/transpose/kernels/rgba_avx2.rs:95: #[target_feature(enable = "avx2")]
- ops/native_channel.rs:9: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:11: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:14: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:70: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:74: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:78: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:98: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:102: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:106: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:130: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:177: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:189: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:233: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:244: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:280: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel.rs:317: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_channel/cmyk_ssse3.rs:30: #[target_feature(enable = "ssse3")]
- ops/native_channel/cmyk_ssse3.rs:69: if !std::is_x86_feature_detected!("ssse3") {
- ops/native_merge.rs:8: all(target_arch = "aarch64", target_feature = "neon")
- ops/native_merge.rs:13: all(target_arch = "aarch64", target_feature = "neon")
- ops/native_merge.rs:66: #[target_feature(enable = "ssse3")]
- ops/native_merge.rs:134: #[target_feature(enable = "ssse3")]
- ops/native_merge.rs:233: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
- ops/native_merge.rs:361: #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]

Architecture filename candidates in pool_simd include: rgba_avx2 box blur, NEON and SSSE3 RGBA-to-RGB conversion, AVX2 RGB grayscale, SSSE3 YCbCr grayscale, NEON integer resize, AVX2 RGBA transpose, and SSSE3 CMYK channel projection. Source review must also classify portable channel kernels and FMA-sensitive paths that are not named after an architecture.

Parallel/Rayon source that must be removed from maintained CPU and assigned to the later Parallel CPU work only after its own stage:
- mod.rs: lines 213, 230
- ops/chops.rs: lines 9, 111, 158
- ops/composite.rs: lines 162, 239, 256
- ops/crop.rs: lines 49
- ops/effects.rs: lines 10, 26, 36
- ops/enhance.rs: lines 7, 23, 33
- ops/filter.rs: lines 37, 39
- ops/filter/blur.rs: lines 14, 16, 19, 23, 29, 60, 79, 90, 101, 123, 152, 214, 220, 222, 226, 230, 243, 246, 259, 330, 360, 405, 481, 497, 523, 525, 527, 529, 531, 533, 535, 537
- ops/filter/blur/box_rows.rs: lines 3, 5, 12, 55, 95, 183, 263
- ops/filter/blur/line.rs: lines 185, 188, 266
- ops/filter/blur/radius_one.rs: lines 4, 7, 11, 18, 35, 126, 221, 324, 431, 514, 561
- ops/filter/blur/radius_one/vertical.rs: lines 5, 69, 120, 124, 151, 180
- ops/filter/filter3x3.rs: lines 11, 206, 230, 312, 382, 400, 471, 495
- ops/filter/filter5x5.rs: lines 15, 18, 21, 26, 29, 32, 34, 68, 77, 91, 103, 123, 136, 194, 218, 242
- ops/filter/filter5x5/int32.rs: lines 7, 12, 17, 20, 28, 33, 40, 62, 76, 153, 158, 171, 189, 215, 297, 398, 467, 619, 638, 705, 712, 721, 745
- ops/filter/filter5x5/int32/bounded_binomial.rs: lines 1, 26, 55, 79
- ops/filter/filter5x5/native_bytes.rs: lines 4, 6, 90, 100, 116, 134, 202, 236, 265, 427, 512, 516
- ops/filter/filter5x5/native_bytes/binomial.rs: lines 16, 108, 210, 272
- ops/filter/rank.rs: lines 174, 192, 208, 224, 308, 317, 381, 390, 508, 512, 581, 590
- ops/filter/rank/float.rs: lines 57, 92, 222, 224
- ops/imageops.rs: lines 30, 36
- ops/imageops/expand.rs: lines 234, 243, 276
- ops/imageops/pad.rs: lines 32, 184, 454, 469, 494, 525, 557
- ops/imageops/pad/native.rs: lines 271, 286, 580, 593, 613
- ops/imageops/pixel.rs: lines 35, 37, 45, 47, 67, 93, 123, 404, 417, 426
- ops/paste.rs: lines 8, 83, 109
- ops/paste/kernels.rs: lines 3, 154, 181
- ops/reduce.rs: lines 582, 609
- ops/reduce/kernels/rgb.rs: lines 113, 134, 185, 204
- ops/resize/float.rs: lines 18, 158, 276, 294, 357, 379, 428, 448, 470, 497, 687, 697, 708
- ops/resize/float_resize.rs: lines 3, 5, 7, 9, 13, 38, 64, 141, 153
- ops/resize/integer.rs: lines 4, 47, 68, 85, 92, 114, 170, 228, 306, 321, 336, 348, 368, 372, 398, 400, 434, 437, 497, 500, 512
- ops/rotate/affine.rs: lines 6, 18, 61, 82, 103, 106, 127, 203, 305
- ops/rotate/affine/coordinates.rs: lines 1, 14, 38, 79, 86
- ops/thumbnail.rs: lines 9, 19, 28, 70, 210, 462, 480
- ops/transpose.rs: lines 11, 14, 20, 50, 91, 101, 179, 198
- ops/transpose/arch.rs: lines 6
- ops/transpose/native.rs: lines 83, 95, 97
Parallel/Rayon source evidence in the preserved SIMD implementation:
- ops/adapters.rs: lines 47, 179, 361
- ops/adapters/alpha_composite.rs: lines 419, 434
- ops/adapters/box_blur/kernels.rs: lines 3, 5, 7, 13, 413, 444, 479, 498, 526, 548, 556, 579, 582, 586
- ops/adapters/chops.rs: lines 15, 643, 664
- ops/adapters/chops/kernels.rs: lines 3, 80, 91, 140, 151, 476, 492
- ops/adapters/convert.rs: lines 25
- ops/adapters/convert/kernels/layout.rs: lines 497
- ops/adapters/convert/kernels/mod.rs: lines 15
- ops/adapters/enhance/sharpness.rs: lines 245, 254
- ops/adapters/filter.rs: lines 505, 707
- ops/adapters/filter/kernels/byte_layouts.rs: lines 3, 192, 199
- ops/adapters/filter/kernels/i32_three_by_three.rs: lines 165, 177
- ops/adapters/filter/kernels/mod.rs: lines 5, 38
- ops/adapters/filter/kernels/three_by_three.rs: lines 606, 618, 719, 731
- ops/adapters/grayscale/kernels/cmyk.rs: lines 3, 126, 142, 158
- ops/adapters/lut.rs: lines 3, 386, 405
- ops/adapters/median_filter.rs: lines 606, 628
- ops/adapters/median_filter/extrema.rs: lines 219, 236, 331, 342
- ops/adapters/median_filter/float.rs: lines 7, 201, 230
- ops/adapters/native_bytes.rs: lines 242, 244, 259
- ops/adapters/paste/kernels/apply.rs: lines 19
- ops/adapters/paste/kernels/apply/l.rs: lines 4, 57, 78
- ops/adapters/paste/kernels/apply/la.rs: lines 4, 64, 85
- ops/adapters/paste/kernels/apply/pa.rs: lines 4, 63, 84
- ops/adapters/paste/kernels/apply/rgb.rs: lines 4, 84, 105
- ops/adapters/paste/rgb_to_rgba.rs: lines 3, 84, 109
- ops/adapters/put_alpha.rs: lines 11, 395, 419, 475
- ops/adapters/put_alpha/kernels.rs: lines 3, 314, 332
- ops/adapters/reduce.rs: lines 10, 52, 494, 537
- ops/adapters/resize/kernels/u8.rs: lines 3, 5, 20, 225, 231, 266, 268, 270, 280, 289, 301, 331, 425, 429, 436, 461, 555
- ops/adapters/resize/kernels/u8_boxed_convolution.rs: lines 3, 13, 252, 271, 311, 319, 351, 444, 452, 480
- ops/adapters/rotate.rs: lines 6
- ops/adapters/rotate/kernels.rs: lines 4, 11
- ops/adapters/rotate/kernels/native/nearest.rs: lines 74
- ops/adapters/transpose.rs: lines 5, 13, 18, 85, 222, 224, 313
- ops/adapters/transpose/kernels.rs: lines 9, 14
- ops/adapters/transpose/kernels/portable.rs: lines 3, 5, 7, 9, 13, 18, 258, 273, 286, 310, 317, 339, 348, 408, 418, 470, 497, 505, 529, 540, 599, 612
The archive preserves these paths unchanged; no active CPU executor may retain feature-based scheduling or parallel flags.

## 5. Registry, context, eager, GPU, and wrapper callsites

The current Backend enum contains Cpu, Gpu, and Simd; registry entries hold separate CpuOpFn and SimdOpFn pointers; backend_state builds independent CPU/GPU/SIMD pools. Architecture is not yet an injected operation context. CPU and SIMD leaf signatures accept Option<&str> mode. These declarations and all their call edges must be replaced by the context-owned route described by the structure guide.

Direct production references from compute modules to pool_cpu/pool_simd and their function-pointer interfaces:
- pillow-rs/src/compute/automatic_segments.rs: 1 references at lines 3
- pillow-rs/src/compute/eager.rs: 7 references at lines 92, 101, 107, 502, 531, 573, 621
- pillow-rs/src/compute/execution.rs: 6 references at lines 315, 355, 508, 610, 706, 715
- pillow-rs/src/compute/mod.rs: 4 references at lines 331, 332, 336, 339
- pillow-rs/src/compute/pool_gpu/auxiliary_images.rs: 6 references at lines 46, 148, 168, 296, 298, 309
- pillow-rs/src/compute/pool_gpu/batch_execution/routing.rs: 1 references at lines 126
- pillow-rs/src/compute/pool_gpu/batch_normalization.rs: 1 references at lines 249
- pillow-rs/src/compute/pool_gpu/batch_operation_planning.rs: 2 references at lines 204, 273
- pillow-rs/src/compute/pool_gpu/fusion.rs: 1 references at lines 58
- pillow-rs/src/compute/pool_gpu/logical_mode_admission.rs: 1 references at lines 115
- pillow-rs/src/compute/pool_gpu/mode.rs: 1 references at lines 252
- pillow-rs/src/compute/pool_gpu/native_input.rs: 1 references at lines 331
- pillow-rs/src/compute/pool_gpu/point_ops.rs: 1 references at lines 83
- pillow-rs/src/compute/pool_gpu/stream/graph/layout.rs: 1 references at lines 321
- pillow-rs/src/compute/registry.rs: 5 references at lines 54, 58, 66, 74, 80
- pillow-rs/src/compute/registry/registration/imageops.rs: 1 references at lines 151
- pillow-rs/src/compute/registry/registration/mod.rs: 4 references at lines 3, 13, 27, 37
- pillow-rs/src/compute/registry/registration/prelude.rs: 9 references at lines 4, 11, 14, 18, 23, 26, 31, 35, 40
- pillow-rs/src/compute/thumbnail_fusion.rs: 2 references at lines 14, 31

Context and routing files to audit as one connected graph:
- pillow-rs/src/compute/mod.rs: Backend and BackendImpl definitions; active pools and public dispatch exports.
- pillow-rs/src/compute/backend_state.rs: enabled backends, priority and pool construction.
- pillow-rs/src/compute/registry.rs and pillow-rs/src/compute/registry/registration/: operation function types, entries, registration and family exports.
- pillow-rs/src/compute/execution.rs, pillow-rs/src/compute/automatic_segments.rs, and pillow-rs/src/compute/execution/terminals.rs: selection, capability/admission, segment execution and terminal materialization.
- pillow-rs/src/compute/eager.rs: direct point, projection, enhancement, and gradient/generator routes.
- pillow-rs/src/compute/pool_cpu/batch_execution.rs and pillow-rs/src/compute/thumbnail_fusion.rs: mode flow and nested/fused CPU work.
- pillow-rs/src/pipeline.rs, pillow-rs/src/image.rs, and raster constructors: exact mode identity, planned transitions and materialized results.
- GPU code that imports CPU helpers: preserve GPU ownership and adapt only interfaces/type compatibility required by the shared raster/context change. Do not route GPU execution to CPU parallel scheduling.
- Python/JS bindings and decode adapters: preserve existing host validation and keep Pillow-owned metadata out of image-slash-star.
- All first-party tests that call CPU/SIMD leaf functions may need mechanical API rewiring after active code changes; do not change test cases, expected values, or assertions.

## 6. Frozen verification scope

### CPU parity

Authority: pillow-rs/tests/fixtures/manifest.yaml, SHA-256 2c07ff46a18e90530a948273b8b7aca2706ea3a7d3bb7689a668aacd427f7fa4.
- Oracle: Pillow 12.2.0 on CPython 3.12.
- CPU target profile: python-cpu with pillow-rs-py/default and pillow-rs/default; no parallel feature.
- Input set: all 27 files named by input_index.parity, with no case or surface filters.
- Total manifest cases: 17408. CPU-applicable denominator: 17407.
- The sole non-CPU case excluded by the python-cpu profile is PIL.GpuBatchExecutor.submit.fault-contract.device-failure-at-admission. It belongs to the GPU fault-contract profile.
- Canonical sorted CPU case IDs are SHA-256 8da7b964ad255904d23d15df85f29135f2bf1f9647038330429a9972acde721b. The all-case sorted ID hash is c2df09d7dbd4861621ca99ac4687638163038e3e20ce2fcb9ac70bebdf5944ae. The sorted set is derived from the pinned manifest inputs; it is not a sampled list.
- Source/build identity and execution receipts must be generated only after the complete source migration. Full CPU parity command is recorded in Stage 7 of CPU_REFACTOR_PLAN.md. No parity was run for this inventory.

### CPU benchmarks

Authority: all 28 files named by input_index.benchmark in the same manifest.
- Workload count: 1032 unique workloads with python-cpu subjects; no workload filtering.
- Sorted workload ID SHA-256: 639e358082276c924343618d5a3b09c6945de012c91c1a80ac5a8ee2121ccb84.
- Benchmark input bundle SHA-256: 61753fbdf807e62b08579e65ee272daf3ca3425a6d2347ec95dee299f2d33e77, computed from sorted path and file-digest pairs.
- Freeze the existing measurement contract from each workload: samples, warmups, iterations, cache state, materialization boundary and parity gate remain unchanged.
- No new baseline campaign has been run for this inventory. Preserve compatible baseline evidence if it exists; otherwise measure the preserved baseline only after CPU static quality/compilation and parity pass, as required by the plan.
- The quiet repeat allowance and per-operation acceptance limits are in Stage 8 of CPU_REFACTOR_PLAN.md. Keep each mode/workload separately attributable to CPU Default or its selected CPU architecture.

## 7. Completion bookkeeping

Before any validation command, update this ledger with exact baseline-source to maintained-function paths, reviewed mode/output behavior, each architecture's instruction requirements and fallback, and every direct/eager caller rewire. Mark source review complete only when all 98 CPU files, all 182 SIMD files, all 84 pipeline variants, all direct callsites, and all supported/rejected mode paths are accounted for. Then proceed to the ordered static, CPU parity, and CPU performance gates in CPU_REFACTOR_PLAN.md.

### Maintained source progress: incomplete and unvalidated

- Reduce output contract decision (2026-10-29): implement typed `u16`
  averaging for `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N`,
  preserving the exact source variant in the reduced `DynamicImage`. The
  archived byte-oriented 8-bit output is not the maintained contract because
  it cannot coexist with exact mode identity. The prepared Reduce family,
  support declarations, typed kernel, and caller rewiring remain pending; the
  owning TODOs are in `pool_cpu/ops/mod.rs` and
  `compute/registry/registration/geometry.rs`.
- Native-mode implementation rule (2026-10-29): do not cast, convert, or retag
  an image merely to reuse another mode's strategy. Each strategy must process
  its exact native representation and declare its output identity. Explicit
  mode conversions remain only when they are part of the operation's defined
  Pillow behavior. The high-depth Grayscale Default owners now read their
  native samples directly and write the L result without an intermediate RGB8
  raster. Put wrappers still have cross-mode conversions under an owning TODO;
  review and replace any conversion used only for kernel reuse before source
  completion.

- `raster::Mode` now names the same 25 variants as `DynamicImage`. The materialized image returns its exact enum identity, including internal high-depth representations. Alpha classification uses that identity. No separate mutable materialized mode field was added.
- Buffer-preserving adapter retagging moved to `raster/dynamic/mode.rs`. Pillow spelling adaptation moved out of the raster selector to the image compatibility boundary. `LoadedData` and lazy pipeline explicit-mode fields, pipeline mode cache, and mode-planner propagation now use `Mode`; mode-sensitive metadata, palette, pixel-access, resize, filter, transform, quantize-admission, and backend gates use exact identities. `Image::from_dynamic` now accepts `Option<Mode>`. Remaining TODOs cover string-taking construction/binding-facing APIs, `Draw::new`, mode-changing pipeline construction, prepared-executor projection, the encoded-header fallback in `Image::mode()`, and string-based operation/leaf paths in conversion, resize, filtering, quantize, merge, gradient, and pixel mutation. Strings therefore remain in core compatibility paths.
- Codec translation now lives in the Pillow-owned image adapter. Existing raster codec conversion methods forward there for compatibility. Bilevel, palette, CMYK, integer, and float decode branches retain their existing byte/sample handling and produce exact raster variants. image-slash-star source is unchanged.
- ExecutionContext captures independently detected instruction capabilities once and can check precise architecture requirements, including FMA. CPU pipeline batches pass the prepared context and wrapper-resolved P/PA palette inputs to the maintained serial pool; registry-wide admission, fusion, terminal, and nested-operation injection remain incomplete.
- Crop now has maintained mode/default implementations for all 25 identities, an exhaustive operation-level support declaration, and composed prepared mode/architecture strategies. Its receipt retains requested backend/architecture, concrete implementation identity, and Default fallback reason. All crop strategies preserve the input identity. The five internal high-depth modes have explicit CoreOnly support and nearby `NO_PILLOW_SUPPORT` comments. Module files contain declarations/exports only.
- Crop's serial native-byte kernel preserves the original CPU row-copy loop, dimension checks, allocation accounting, and shape-error messages. Typed high-depth strategies call the existing typed raster crop helper directly. Neither helper chooses mode/backend/architecture or schedules parallel work.
- The archived SIMD crop adapter's I;16-family copy uses the original 16-byte vector copy and padded tail. That kernel now runs through declared SSE2, NEON, and Wasm SIMD128 strategies for the four exact Pillow I;16 identities, preserves their mode variants, and retains vector/tail telemetry. Other native byte crop modes use their existing Default row-copy strategy; the archived SIMD adapter's ordinary byte-copy route and error compatibility remain pending caller reconciliation and retirement. `CropBorder` has its own imageops support identity and prepared callback; it declares the same I;16 architecture choices so its receipt matches the composed crop plan. Its declared request contract preserves width-first/height-second errors; nonempty requests compose the exact-mode crop strategy, and half-size borders use typed empty `DynamicImage::crop_imm` outputs. Its separate SIMD adapter remains registered pending compatibility-route retirement.
- The identical 16-byte copy block/tail kernel is shared from `pool_cpu/helpers/bytes/vector_copy.rs` by Crop, Expand, and PutData; each owner keeps its own typed request checks, row traversal, error mapping, and telemetry.
- Crop, CropBorder, grayscale, and ExtractBand now register concrete preparation callbacks returning the registry's closed `PreparedOperation` enum. Their execution consumes the already selected family plan. Remaining registry registrations still use the old callback signature and require migration with their owners; this source tree is intentionally incomplete and has not been compiled.
- `OperationInputs` borrows the exact raster and resolved palette dependencies without cloning pixels or adding a mode tag. Eager and single-operation lazy grayscale supply the resolved palette; ExtractBand supplies explicit absence because its native band copy does not consult palette entries. The maintained CPU batch receives wrapper palette metadata and supplies it only for current P/PA rasters; remaining palette-changing pipeline stages and non-batch operation callers still need operation-owned metadata integration.
- `PreparedExecution` retains the centrally resolved context. The eager CPU image callback receives its context rather than reconstructing it in the operation. The maintained CPU batch now receives that context and prepares each operation against the current mode-bearing raster. Palette dependencies, mode-changing metadata, support admission, fusion, terminals, nested inputs, and remaining eager helpers still need end-to-end injection. Separate SIMD routing and registry compatibility paths remain to be retired. No CPU completion, compilation, parity, or performance milestone is claimed.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `pool_cpu/legacy/ops/crop.rs::execute_crop` | Crop operation orchestration prepares one exact mode strategy; coordinate subtraction and normalized-coordinate precondition remain in the shared crop request preparation. |
| Serial row-copy branch of `execute_crop` | Shared typed `ByteCrop`, composed by each native-byte mode's Default implementation; original row order and span copying preserved. Rayon branch remains reference-only for later Parallel CPU work. |
| Typed fallback through `DynamicImage::crop_imm` | Each high-depth Default strategy calls the original typed raster `crop_buffer` helper, preserving sample order and arithmetic. |
| `pool_cpu/legacy/pool_simd/ops/adapters/crop.rs` | I;16-family native-byte vector copy moved into exact CPU mode/architecture strategies; source-preserving semantics and telemetry retained. Ordinary byte-copy fast path and caller/error reconciliation remain pending. `CropBorder` has a prepared CPU imageops owner; its independent SIMD registration remains pending retirement. |

### Grayscale source progress: incomplete and unvalidated

- The family entry point classifies every exact raster identity. It declares the existing `ImageLaPremultiplied8` (`La`) rejection and prepares concrete strategies for the other 24 identities. Every successful result is `ImageLuma8`. This is source implementation progress, not a verified Pillow support claim.
- Default mode implementations retain the archived source handling: nonzero bilevel samples become white; LA copies its luma; RGB/RGBA/RGBX use the existing rounded fixed-point loop; RGBa first applies the existing integer unpremultiplication; CMYK keeps the fused ink subtraction; HSV keeps its lookup-table RGB conversion; YCbCr keeps packed-word Y extraction and the original fallback; I/F preserve their raw little-endian scalar clipping. LAB retains the archived byte-carrier route pending the full parity gate rather than introducing a replacement algorithm during migration. `I;16*` inputs apply the archived per-sample clip directly from native `u16` storage. Internal LA16/RGB16/RGBA16/RGB32F/RGBA32F inputs convert each native sample directly into the luma output kernel without creating an intermediate RGB8 image; the conversion currently preserves the original channel scaling and alpha-drop behavior.
- Palette strategies require explicitly resolved wrapper metadata. P retains its existing palette expansion or absent-palette index path; PA retains expansion through an empty palette when no colors are attached. Palette alpha and per-pixel alpha keep their existing separate roles. A single lazy P/PA Grayscale op now reaches its prepared strategy before wrapper expansion; broader wrapper-owned operation dispatch still requires rewiring.
- RGB AVX2 and NEON, YCbCr SSSE3 and NEON, CMYK NEON, and LA/RGBA/RGBX/RGBa NEON implementations have concrete mode/architecture owners. RGB AVX2 independently declares SSE2, SSSE3, and AVX2. Existing vector arithmetic, bounded blocks, scalar/padded tails, and vector counters were retained. CMYK receives a prepared NEON token rather than constructing a SIMD level per block.
- The archived interleaved RGB/LA/four-byte vector traversal, channel gathering, and packing are shared once by named NEON, x86, and Wasm SIMD128 strategies. The non-NEON CMYK math and serial block/tail traversal are preserved in a shared family helper. The original portable channel gather serves YCbCr on 32-bit x86 and Wasm; the archived x86_64 scalar gather fallback is not claimed as a vector specialization. LA names its actual SSE2/SSSE3 byte path rather than claiming AVX2 arithmetic. The four I;16 identities share the same original clipped-RGB helper. Scalar I/F and CMYK loops were moved to typed Default owners and existing color facades delegate to those loops, avoiding a second maintained implementation. HSV and RGBa gained typed buffer entry points for their existing conversion helpers.
- Operation support now distinguishes fixed versus preserved output identity, precise instruction requirements, native-layout guards, semantic source-length errors, and unavailable build/hardware/request fallbacks. Contextual assessment reports ready, rejected, or deferred. Unknown source length, unresolved palette metadata, or an unknown CropBorder request cannot be reported as an executable selection. Crop and CropBorder use the same assessment type and prepared-selection carrier.
- The execution context supplies a centralized permitted-implementation relation while checking instruction capabilities independently, including SSE2 and statically enabled Wasm SIMD128. Operation strategies do not probe hardware. Selection retains the first preferred declaration's failure reason when a narrower specialization or Default is chosen; later candidates do not overwrite it. Backend/context injection outside the new families remains incomplete.
- The internal LA16/RGB16/RGBA16/RGB32F/RGBA32F grayscale routes are explicitly CoreOnly and carry nearby `NO_PILLOW_SUPPORT` comments; they consume native samples and produce L without an intermediate mode raster.
- Grayscale registry preparation and eager CPU callback wiring are present. Tiny-RGB eager execution uses the prepared CPU strategy for CPU Default and the compatibility SIMD request; the latter passes the detected architecture through `ExecutionContext`. Single-op lazy P/PA grayscale now passes the original mode-bearing raster and resolved `PaletteInput` into that same prepared strategy before wrapper-wide expansion. The shared telemetry frame retains vector and scalar-tail counters, and mode-conversion accounting compares exact `Mode` identities. `compute/eager.rs::record_cpu_arch_operation_telemetry` still needs to publish the selected architecture in the receipt because `PipelineTelemetry` currently records backend/path; that route has an explicit source TODO. Serial pool/batch integration, broader palette dependencies and transitions, and final source review across all target configurations remain pending. Named x86/Wasm routes and capability metadata have not been compiled or executed. Grayscale is not marked complete. Existing string-mode compatibility callers and scalar helper placement remain part of the full migration review.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `pool_cpu/legacy/ops/grayscale.rs::op_grayscale` | `grayscale/ops.rs` composes explicit mode strategies and preserves the La rejection; mode Default owners contain the original scalar behavior. |
| `native_ycbcr_luma` and `native_ycbcr_luma_packed_blocks` | `grayscale/ycbcr8/default/grayscale_impl.rs`; original packed byte extraction, dimension eligibility, fallback, and allocation accounting retained. |
| `color/grayscale.rs::pil_grayscale_inner` native branches | Typed Default owners use the existing shared scalar luma loop; the old facade remains for unmigrated callers and is not a substitute for completing their mode/context migration. |
| `color/conversion.rs::i_to_l` and `f_to_l` scalar loops | `grayscale/i32/default/grayscale_impl.rs` and `grayscale/f32/default/grayscale_impl.rs`; original color facades delegate to the typed loops. |
| `color/grayscale.rs::cmyk_to_grayscale` loop | `grayscale/cmyk8/default/grayscale_impl.rs`; the exact source-length rejection is checked during preparation from the same contract used by assessment. |
| `color/conversion.rs::hsv_to_rgb` and `ops/pil_resize/alpha.rs::unpremultiply_alpha` RGB branch | Shared typed buffer helpers preserve existing HSV factors and integer alpha restoration; grayscale's HSV/RGBa strategies inject their exact source buffers. |
| Wrapper palette expansion before color-dependent operations | P/PA Default preparation composes the existing expansion helpers with typed luma conversion; single-op lazy grayscale uses it directly, while remaining color-dependent callers are pending. |
| Archived SIMD `grayscale/kernels/rgb_avx2.rs` | `grayscale/rgb8/avx2/grayscale_impl.rs`; masks, arithmetic, short-input behavior, and bounded vector tail retained behind the prepared feature contract. |
| Archived SIMD RGB NEON functions in `channels.rs` | `grayscale/rgb8/neon/grayscale_impl.rs`; original specialized RGB loop and padded tail retained. |
| Archived SIMD `ycbcr_ssse3.rs` and NEON extraction in `ycbcr.rs` | Exact `ycbcr8/ssse3` and `ycbcr8/neon` implementation files; leaf feature probes removed, load bounds retained. |
| Archived SIMD NEON math and serial vector traversal in `cmyk.rs` | `grayscale/cmyk8/neon/grayscale_impl.rs`; explicit prepared token, unchanged arithmetic; Rayon scheduling remains archive-only for later Parallel CPU work. |
| Archived SIMD interleaved two-/three-/four-channel loops in `channels.rs`, channel masks, and pack helper | `grayscale/helpers/interleaved.rs::{simd_pack_u16x16,grayscale_block,grayscale_interleaved}` and the shared CPU `helpers/channel/gather.rs::gather_interleaved_channel`; concrete grayscale strategies preserve their original loops. The lane gather is shared with put's native NEON extraction. RGB NEON keeps its dedicated optimized loop. |
| Non-NEON `cmyk_grayscale_block` and serial traversal in archived `grayscale/kernels/cmyk.rs` | `grayscale/helpers/cmyk.rs::{block,execute}`; typed CMYK SSE2/SSSE3/AVX2/Wasm implementations compose the original arithmetic, zero-initialized output, padded tail, and saturating counters. Parallel scheduling remains archived. |
| Portable branch of `legacy/ops/imageops/colorize.rs::gather_native_channel` reached by archived YCbCr | Shared CPU `helpers/channel/wide.rs::gather`, called by YCbCr's Wasm and 32-bit x86 owners and put's matching native byte implementations. x86_64 retains its dedicated SSSE3 implementation or serial Default. |
| 32-bit x86 RGB generic-wide branch of archived `native_grayscale_bytes` | `rgb8/avx2/grayscale_wide_impl.rs` plus RGB SSE2/SSSE3 owners retain the generic loop. x86_64 AVX2 continues using its archived intrinsics, rather than replacing them with that loop. |

The following is source-level declaration bookkeeping, not executed architecture
coverage. Inspection used the pinned local `wide` 1.6.1 implementation:
`i8x16::swizzle_relaxed`, `u16x16` storage/widening/arithmetic, and
`u8x16::narrow_i16x8`. Whole-binary target requirements still apply.

| Grayscale strategy | Declared build and instruction contract |
| --- | --- |
| RGB x86_64 AVX2 | Compiled on x86_64 with a target-feature entry point; independently requires SSE2, SSSE3, and AVX2 before preparation selects it. |
| RGB 32-bit x86 AVX2; RGBA/RGBX/RGBa/CMYK wide AVX2 | The original wide loop is compiled with static AVX2; requires SSE2 and AVX2, plus SSSE3 when the build enables its shuffle branch. |
| RGB/RGBA/RGBX/RGBa/CMYK wide SSSE3 | Static SSSE3 without static AVX2; requires SSE2 and SSSE3. |
| RGB/RGBA/RGBX/RGBa/CMYK wide SSE2 | Static SSE2 without static SSSE3 or AVX2; retains the archived scalar shuffle with vector arithmetic/packing rather than inventing an intrinsic kernel. |
| LA wide byte gather | Static SSSE3 selects the SSSE3 declaration; otherwise static SSE2 selects SSE2. No AVX2 arithmetic or AVX2-named LA implementation is declared. |
| YCbCr x86 | x86_64 uses the existing target-feature SSSE3 kernel. 32-bit x86 uses its archived portable gather in the static SSE2/SSSE3 build. The x86_64 non-SSSE3 path remains serial Default. |
| RGB/LA/RGBA/RGBX/RGBa/CMYK/YCbCr Wasm | Requires a wasm32 build with static SIMD128; the injected context records that build capability without a leaf probe. |
| Existing NEON strategies | Require the AArch64 NEON build and captured NEON capability; CMYK gets its token once during preparation. |
| No implemented vector target | Uses the exact mode's serial Default. The old generic array-emulation source remains in the archive; it is not registered as a fictitious architecture. |

### ExtractBand source progress: incomplete and unvalidated

- `put/ops.rs` exposes `EXTRACT_BAND_SUPPORT` and concrete prepared strategies for all 25 raster identities. Each result is `ImageLuma8`. This describes the existing internal PipelineOp, whose defensive index behavior remains available to direct internal callers. The public `Image::getchannel` wrapper still owns selector validation, the I/F rejection, and the P-mode identity copy with retained palette. Its remaining string-based metadata boundary is migration debt, not a new typed-boundary completion claim.
- Mode declarations expose the original index rules: luma replicates into the first three bands and returns opaque alpha for higher indices; RGB selects its first three bands and returns opaque alpha for higher indices; LA/PA and four-byte carriers clamp to their final native band. The four I;16 identities now clip their native u16 samples directly to L bytes, preserving the original 0..255 clipping and opaque-alpha fallback without constructing an intermediate RGBA image. Internal LA16/RGB16/RGBA16/RGB32F/RGBA32F strategies read their native sample types directly and use the existing numeric-to-byte conversion; internal LA16 selects its actual luma or alpha band. The previous high-depth RGBA conversion path was removed from ExtractBand.
- Serial defaults preserve logical-raster truncation for trailing storage, four-pixel LA/RGBA unrolling, scalar tails, empty-image handling, allocation order/counters, and existing shape errors. Typed forwarding is inlined; the actual loops remain in their implementation/shared helper owners.
- Existing vector byte movement is composed for LA, PA, RGB, RGBA, RGBa, RGBX, CMYK, YCbCr, and HSV. x86_64 SSSE3 requires independently captured SSE2 and SSSE3 and uses the original bounded intrinsic gather. 32-bit x86 retains the original static SSSE3 or SSE2 wide gather. Wasm requires static SIMD128. NEON retains its original two-/three-channel gathers and four-channel structure load, with its token prepared once rather than probing in a leaf. Byte-only gathers do not gain an AVX2 label.
- The declaration's `ExtractBandBytes` guard uses typed `RequestFacts::ExtractBand { index }` alongside dimensions/source length. Empty or overlong storage and out-of-native-range indices select the same mode's Default. Missing index/content facts remain deferred when needed; known-ineligible layouts can select Default without materialization. Mode-only metadata never invents a parameter value.
- Shared gathers moved to CPU `helpers/channel`: grayscale and put reuse actual lane-selection/wide algorithms. The old private color gather adapters were removed after inspecting active callers. Their existing boundary test moved with the shared SSSE3 kernel; its inputs/assertions remain unchanged and its call now establishes the raw helper's safety contract explicitly. The two archived CPU ExtractBand regressions were restored under put with an injected Default context; their shapes, selectors, and assertions remain unchanged. Archived SIMD adapter tests still require relocation with the final API review.
- CPU eager ExtractBand now reaches the typed prepared registry callback. The public wrapper's two Cargo-parallel-feature exclusions were removed so enabling that feature does not change serial CPU eager admission. Separate SIMD eager extraction, the pool facade/batch executor, shared metadata transitions, registry-wide assessment APIs, and the other put operations remain incomplete. No build or test has run; this is not a completed put-family or CPU checkpoint.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/color.rs::op_extract_band` luma and RGB branches | Exact mode Default owners compose `put/helpers/extract_band.rs::{luma,rgb}` with original logical-raster collection and opaque-alpha behavior. |
| Native LA/RGBA branches of `op_extract_band` | Mode Default owners call the original four-pixel algorithm through the compile-time-layout `grouped` helper; matching raw carriers retain their native bytes. |
| High-depth branch of `op_extract_band` | Four I;16 Default owners use `clipped_luma16`; internal LA16/RGB16/RGBA16/RGB32F/RGBA32F owners use `native` to extract and convert only the selected native sample. No intermediate RGBA raster is created. |
| SSSE3 functions formerly in `color/grayscale.rs` | Shared CPU `helpers/channel/ssse3.rs`; original allocation, masks, bounds, stores, initialization, and counters remain. Production callers receive eligibility from prepared strategies rather than probing hardware in an adapter. |
| Archived `native_channel.rs` NEON into-buffer functions and structure load | Shared CPU `helpers/channel/neon.rs`, composed by the concrete put mode/NEON owners; unused allocating facades were not recreated. |
| `legacy/pool_simd/ops/adapters/extract_band.rs` output allocation and accounting | `put/helpers/extract_band_vector.rs`; original RGB allocation versus two-/four-channel into-buffer boundaries and vector/tail receipts remain. Admission is now the linked typed declaration. |
| Existing direct SSSE3 and archived CPU ExtractBand tests | `helpers/channel/tests.rs` and `put/tests/extract_band.rs`; source relocation/signature adaptation only, not executed evidence. |

The structure guide remains the authority for exact maintained paths. This ledger records function ownership and remaining work, not a competing path grammar. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for these changes.

### PutPixel source progress: incomplete and unvalidated

- `put/ops.rs` now exposes separate `PUT_PIXEL_SUPPORT` and `PUT_PIXEL_BATCH_SUPPORT` descriptors. Their linked declaration explicitly classifies all 25 identities: the 16 byte-backed modes retain native writes and exact output identity, while the nine high-depth identities retain `NotImplementedError("putpixel not supported for this image type")`. This is the internal byte-write contract; scalar I/F and I;16 public mutation paths still require the wrapper review.
- Each supported mode has typed serial single-write and ordered batch implementations. The existing batch algorithm clones once, checks each operation and coordinate in source order, and preserves last-write-wins behavior. Default preparation does not scan the batch before that original loop.
- The existing masked byte store is composed by 15 modes' SSE2, NEON, and wasm SIMD128 owners. Premultiplied La remains Default-only, matching the archived vector adapter's admission. LAB retains its PutPixel-specific vector admission rather than inheriting drawing support. No AVX/SSSE3-named store was invented for this SSE2-width bitwise implementation.
- RGBA alone declares the existing vector batch strategy. Its original requirements remain explicit: at least two writes, one complete vector, exact raster length, only in-bounds PutPixel descriptors, and no palette-index tags. Preparation inspects these facts only when a declared architecture is available under the captured build/hardware/policy; metadata queries can defer unknown run facts. The plan borrows the inspected immutable operations and executes its retained concrete choice without a retry or reselection.
- Source review against pinned `wide` 1.6.1 confirmed `u8x16` storage and bitwise operations use static SSE2, AArch64 NEON, or wasm SIMD128 branches. Other targets retain serial Default. The vector helper preserves global byte phase, padded short stores, the original checked offset errors, logical-raster truncation, and vector counters. Shared typed helpers contain no mode inference, hardware probes, or parallel scheduling.
- The PutPixel registry callback now constructs the family-owned prepared plan. The old PutPixel facade import was removed. The CPU pool/batch executor, thumbnail fusion priority, per-operation selection receipts, and public wrapper transitions are still pending; registering the plan does not establish end-to-end execution.
- Two archived vector regressions were relocated to `put/tests/put_pixel.rs`. Vector-edge shapes, write values/order, byte comparison, and unsafe-run rejection assertions remain. The former string override `Some("RGB")` is represented by an exact RGB variant with the same bytes and dimensions, since a contradictory mode override no longer belongs in compute. The archived pool resource/telemetry regression still awaits executor integration. No test has run.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/put.rs::op_put_pixel` | Typed mode `default/put_pixel_impl.rs` owners compose `put/helpers/put_pixel.rs::write`; unchanged native sample placement, one clone, and exact mode identity. |
| `legacy/ops/put.rs::{apply_put_pixel_batch,op_put_pixel_batch}` | The same Default owners compose the ordered generic `write_batch` helper with concrete pixel constructors; original descriptor/bounds errors and last-write-wins order remain. |
| Archived vector `native_putpixel_layout` and shape/image admission | Linked mode and architecture declarations in `put/ops.rs`, mode `ops.rs`, and `put/helpers/put_pixel_strategy.rs`; typed source identity replaces string/layout guesses. |
| Archived `simd_put_pixel_native` | `put/helpers/put_pixel_vector.rs::single`, reached through exact mode/architecture implementation owners; original masked store and finalization preserved. |
| Archived nonblending `draw_line_vector_block` branch | `put/helpers/masked_store.rs`; compile-time native channel width removes the unused blending decision from PutPixel without changing source-byte phase, masks, or arithmetic. Drawing migration must reconcile reuse with its blending implementation later. |
| Archived `simd_rgba_putpixel_batch_supported` | `put/helpers/put_pixel.rs::rgba_run_facts` plus `StrategyGuard::RgbaPixelWriteRun`; the same mode declaration drives assessment and preparation. |
| Archived `simd_rgba_putpixel_batch` and `kernels/rgba_batch.rs::apply_rgba_batch` | Typed RGBA architecture owners compose `put/helpers/put_pixel_vector.rs::{batch,apply_rgba_batch}`; immutable admitted writes retain order, one frame clone, and vector-store telemetry. |
| Archived PutPixel inline vector tests | Two source regressions relocated to `put/tests/put_pixel.rs`; pool-level telemetry/resource test remains pending with the executor. |

Remaining put-family work includes the CPU batch executor and thumbnail fusion connection, typed wrapper boundaries, and complete test/caller reconciliation. PutData and both alpha operations have source owners, but remain unvalidated and are not end-to-end complete. The structure guide remains the placement authority. No build, formatter, checkstyle, lint, test, parity, benchmark, coverage, commit, or push has run.

### PutData source progress: incomplete and unvalidated

- Source review confirmed that PutData's payload layout can differ from the source image in direct core calls. The archived CPU operation converts to the requested byte layout in that case; GPU admission explicitly retains this CPU fallback. The refactor therefore retains a typed payload parameter independently of the source variant rather than removing it as a redundant mode hint.
- `put/ops.rs::PUT_DATA_SUPPORT` now classifies every source identity through the same declaration that binds its concrete prepared plan. All 25 sources have serial implementations. Their payload contract enumerates the 14 canonical byte-mode identities represented by the existing normalization interface, including RGBA's RGBa/RGBX aliases. The 11 other typed payload identities have an explicit rejection; this does not broaden the public wrapper's accepted strings or replace its I;16 mutation path.
- Each mode's Default implementation receives its exact buffer type and a prepared payload strategy. It retains the original nonempty full-payload replacement paths for matching native L/LA/RGB/RGBA layouts, the separate index/color-space/raw-four-byte clone-and-prefix paths, cross-layout typed conversions, I;16 clipping, prefix truncation, untouched suffix, zero-size behavior, and `putdata: buffer error` failure. No source-mode or channel inference occurs inside the conversion or copy loops.
- Output relations are explicit in the mode contract: a matching native byte layout preserves the exact source identity; a cross-layout or high-depth conversion returns the corresponding L/LA/RGB/RGBA carrier. The same payload declaration generates accepted identities, compatible native widths, payload strategy construction, and the inspectable output table.
- All 16 byte-backed source identities have native-copy SSE2, NEON, and wasm SIMD128 owners. This retains the archived adapter's logical-mode fallback, including its La/LAB physical-copy admission through an explicitly supplied compatible payload. The nine high-depth sources use their serial strategy. Exact source length and compatible payload identity guard specialization; a missing/ineligible architecture selects that same source mode's Default strategy. The original u32 byte-count error, full-payload single copy, partial source/prefix copies, padded vector tail, counters, and empty finalization remain in `put/helpers/put_data_vector.rs`.
- Registry PutData registration now returns its family-owned prepared plan. Its legacy `op_put_data` facade import was removed. The old `PipelineOp` still carries `PixelMode`; a temporary explicit `PixelMode::raster_mode` adapter at registration supplies canonical Mode to CPU without reconstructing the source identity. Removing the older pipeline payload enum and migrating its producers/GPU consumers remains required before CPU/type-boundary completion; this adapter is migration debt, not the intended final API.
- Typed native byte finalization is now shared through `image_utils::typed_bytes_to_buffer`. The existing raw-byte facade delegates to this same length/truncation/shape implementation, and PutPixel no longer duplicates it. Concrete operation strategies construct the exact result variants. Original raw-byte error strings and checked nonempty/empty policies are retained.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/put.rs::op_put_data` complete-payload native branches | Exact source mode `default/put_data_impl.rs` owners compose `put/helpers/put_data.rs::native`; nonempty replacement and fallback construction order retained. |
| `op_put_data` partial and cross-layout branches | Typed Default owners compose `put/helpers/put_data.rs::{converted,Prefix::apply}` with concrete clone/ConvertBuffer functions; no DynamicImage reclassification in the loop. |
| Existing I;16-to-byte raster conversions reached by `op_put_data` | Four endian-identity owners compose the original clipped sample traversal through `put/helpers/put_data.rs::clipped`. |
| Archived vector `native_put_data_layout` and shape/image admission | Mode-owned `PUT_DATA_SUPPORT`, `ByteDataPayload`, and `PutDataNative` declarations bind source identity, explicit compatible payloads, exact native storage, and prepared architecture selection. |
| Archived `simd_put_data` and `native_bytes.rs::copy_native_bytes` | `put/helpers/put_data_vector.rs`, composed by exact source/architecture owners; original allocation and native byte traversal retained. Reconcile broader native-copy reuse as other families migrate. |
| `image_utils.rs::raw_bytes_to_image_with_empty` shape finalization and PutPixel's interim duplicate | One typed buffer finalizer in the existing canonical utility, reused by both raw facade and typed PutPixel/PutData owners. |

### PutAlpha and PutAlphaData source progress: incomplete and unvalidated

- Each operation now has an exhaustive 25-mode entry-point declaration, explicit outputs, typed source buffers, and a concrete Default owner. Constant alpha keeps native L/LA/P/PA layouts, promotes RGB/CMYK/other byte carriers as before, clips I;16 samples through the existing helper, and retains the CMYK MULDIV255 conversion path.
- The original architecture loops are placed in mode-owned folders: constant alpha has vector owners for L, P, RGB, RGBA, RGBX, and CMYK; image-backed alpha has L/LA/P/PA/RGB/RGBX/CMYK owners. The same declaration binds build and architecture requirements, native-byte guards, minimum vector groups, and same-mode Default fallback. RGBA mask replacement remains in Default because its archived “SIMD” path only cloned and wrote alpha bytes; it does not claim a vector implementation.
- Secondary masks retain their exact 25-mode identity in a borrowed strategy selected before pixel processing. The Default contract converts masks using the original typed-to-L behavior and keeps zip/prefix output semantics. Architecture owners require the archived native L-mask shape and exact source storage. The large RGB-mask row path, complete vector groups, scalar tails, and original receipt counters are retained.
- CMYK alpha calls the shared serial RGB conversion helper, so CPU execution does not enter the optional Rayon route. The shared u16-to-byte vector packing primitive is now owned by the CPU channel helper and reused by grayscale and alpha kernels.
- Registry callbacks now prepare opaque operation plans and no longer import the old alpha operation facades. Existing alpha kernel assertions were relocated with their original inputs and expectations; they were not run.
- The maintained serial CPU pool and prepared batch dispatcher now receive routed context and execute registered operation plans in order. The Pillow wrapper passes resolved palette metadata through prepared CPU batches, including automatic SIMD-to-CPU recovery; each operation receives it only while its current raster identity is P or PA. Source-accounted fusion and terminal integration, canonical pipeline/binding mode boundaries, metadata transitions, registry-wide assessment, remaining operation families, and full source/caller review are outstanding. No compiler, formatter, checkstyle, lint, test, parity, benchmark, coverage, commit, or push has run; this source work is incomplete and unvalidated.

### Transpose-family source progress: incomplete and unvalidated

- `transpose/ops.rs` declares exhaustive operation-level support for Flip, Mirror, and Transpose with typed mode dispatch, prepared plans, registry callbacks, and receipts. Every exact raster identity has a Default owner and preserves its mode tag. Pipeline registration no longer passes a Pillow mode string into these operations.
- The byte-backed mode owners use checked native row/pixel copying and fall back to the same generic typed image transforms for malformed or nonstandard storage. High-depth owners preserve typed sample values. Palette indices, premultiplied samples, color-space bytes, numeric carriers, and byte-order mode identities are moved without conversion.
- Transpose covers all seven methods with exact-mode Default owners and typed method preparation. Its native byte path retains checked dimensions, same-dimension row copying, serial/tiled axis swaps, the original 32-pixel tile threshold, and the RGBA 16-pixel tile size. Typed modes use the original mode-preserving pixel-coordinate transforms.
- An AVX2 RGBA mode strategy now consumes the injected architecture plus exact native layout, dimensions, and axis-swap method guards. Its `rgba8/avx2` kernel ports the archived AVX2 tile transform and handles incomplete edges through byte copies. This path has not been compiled or parity-checked.
- The archived wide-lane Mirror kernel is shared in `transpose/helpers/native_mirror.rs`; `One`, `ImageLuma8`, and `ImagePalette8`; `ImageLumaA8` and `ImagePaletteAlpha8`; `ImageRgb8`, `ImageHsv8`, and `ImageYCbCr8`; and `ImageRgba8`, `ImageCmyk8`, `ImageRgbPremultiplied8`, `ImageRgbx8`, `ImageI32`, and `ImageF32` bind it through exact-mode declarations for SSE2, NEON, and Wasm SIMD128. Each prepared strategy restores its own `DynamicImage` variant. RGBA uses the same helper in its custom AVX2-transpose strategy owner. Native row length and channel-group guards select these implementations, with the same-mode Default mirror as fallback. The `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` Flip/Mirror native-copy routes now use `transpose/helpers/native_luma16.rs`, preserving typed samples and exact output identities under SSE2, NEON, and Wasm SIMD128 strategies with same-mode Default fallback. The packed-word RGBA Rotate270 kernel now runs in the RGBA Default owner behind the original x86_64, 32-pixel tile threshold, and four-pixel divisibility guards; it has no ISA requirement and remains a Default implementation detail. The archived portable `wide` transpose tile, gathered-row, and scalar-edge paths are migrated to `transpose/helpers/native_transpose.rs` and selected by exact-mode SSE2, NEON, and Wasm SIMD128 strategies, with same-mode Default fallback. Its conditional Rayon row collection is excluded from CPU and remains for the later `pool_parallel` stage. The duplicate SIMD byte-Flip compatibility route must be retired after CPU routing reaches the prepared owner. Registry/executor and other callers remain part of the broader incomplete migration. No compiler, formatter, lint, test, parity, benchmark, or coverage command has run.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/imageops/pixel.rs::{flip_native_rows,op_flip}` | Exact-mode Default implementations under `transpose/<mode>/default/flip_impl.rs`, sharing `transpose/helpers/pixel.rs::flip_vertical_bytes`; generic typed fallback preserves the old fallback behavior. |
| `legacy/ops/imageops/pixel.rs::{mirror_byte_row,mirror_byte_rows,op_mirror}` | Exact-mode Default implementations under `transpose/<mode>/default/mirror_impl.rs`, sharing checked native byte reversal; typed fallback matches the original generic horizontal transform. |
| `legacy/ops/transpose.rs::{execute_transpose,transpose_bytes_serial,transpose_bytes_tiled,transpose_bytes_rows}` | `transpose/helpers/pixel.rs` plus each mode's `default/transpose_impl.rs`; serial byte layout and tiling behavior are retained without Rayon. |
| `PipelineOp::{Transpose,Flip,Mirror}` registry callbacks | `transpose/ops.rs` prepares the exact mode strategy with injected `ExecutionContext`; registration no longer supplies a string mode. |
| `legacy/ops/transpose/kernels/transpose_rgba/x86_64.rs` | Migrated to `transpose/rgba8/default/rotate270_tiled.rs`; it runs only under the original x86_64, tile-size, and divisibility guards and remains part of the Default strategy because it has no ISA requirement. |
| `legacy/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs` | Ported to `transpose/rgba8/avx2/transpose_impl.rs`; declared through the `TransposeRgbaAvx2` guard. Source is unvalidated. |
| `legacy/pool_simd/ops/adapters/flip.rs::simd_flip` byte route | It delegates to `img.flipv()` and has no distinct vector kernel; retire the duplicate compatibility route after prepared CPU routing is active. Its I;16 native row-copy route is in `transpose/helpers/native_luma16.rs`. |
| `legacy/pool_simd/ops/adapters/flip.rs::{simd_flip,simd_mirror}` I;16 native-copy paths | `transpose/helpers/native_luma16.rs`, selected by the exact `ImageI16`, `ImageI16L`, `ImageI16B`, and `ImageI16N` Flip/Mirror strategy declarations. Native byte length must be exact and nonempty; each mode's same-mode Default implementation remains the fallback. |
| `legacy/pool_simd/ops/adapters/flip.rs::simd_mirror` byte-backed modes | `transpose/helpers/native_mirror.rs`, selected by exact per-mode declarations for `One`, `ImageLuma8`, `ImagePalette8`, `ImageLumaA8`, `ImagePaletteAlpha8`, `ImageRgb8`, `ImageHsv8`, `ImageYCbCr8`, `ImageRgba8`, `ImageCmyk8`, `ImageRgbPremultiplied8`, `ImageRgbx8`, `ImageI32`, and `ImageF32`; RGBA uses its custom strategy owner. |
| `legacy/pool_simd/ops/adapters/transpose/kernels/portable.rs::{transpose_gathered_row,transpose_native_tile,transpose_native_row_group,transpose_native_tiled}` | Portable wide-lane tile, gathered-row, and scalar-edge paths are in `transpose/helpers/native_transpose.rs`, reached through exact-mode SSE2, NEON, and Wasm SIMD128 owners with same-mode Default fallback. The legacy conditional Rayon row scheduling and collection helpers remain archived for the later `pool_parallel` stage. |
| `legacy/pool_simd/ops/adapters/transpose.rs::rgba_avx2` and `kernels/rgba_avx2.rs` | Ported to `transpose/rgba8/avx2/transpose_impl.rs`; declared through the `TransposeRgbaAvx2` guard. Source is unvalidated. |

### ImageOps source progress: incomplete and unvalidated

- Public ImageOps mode admission in `ops/imageops/point.rs` now matches typed `Mode` identities rather than Pillow strings. Exact mode lookup uses the wrapper's cached identity; when a lazy pipeline has no identity yet, the current materialization fallback is explicitly pending removal once all pipeline transitions preserve a known exact mode.
- `imageops/ops.rs` owns an exhaustive classifier for all 25 exact modes. The 14 8-bit Pillow modes are supported by distinct mode folders and Default strategies; each bytewise transform keeps the exact input mode identity. Numeric I/F and internal high-depth carriers preserve the archived `from_raw` shape-error class and message for their corresponding L, LA, RGB, or RGBA channel count.
- CropBorder and Colorize also declare typed support for the five internal high-depth identities as CoreOnly routes, with nearby `NO_PILLOW_SUPPORT` comments. CropBorder preserves the exact raster mode; Colorize declares its explicit RGB output. Neither route is part of Pillow parity.
- Each mode's `default/invert_impl.rs` performs the same `255 - byte` transform as `legacy/ops/imageops/pixel.rs::op_invert`; the scalar byte loop is shared in `imageops/helpers/pixel.rs`. Strategy preparation receives `ExecutionContext`, and registry preparation now returns the closed `PreparedOperation` plan.
- The archived native-byte vector path is represented under the eight exact modes it admitted, with SSE2, NEON, and Wasm SIMD128 strategy declarations where each implementation is built. Architecture wrappers share `imageops/helpers/native_bytes.rs`; its exact-length copy and trailing-storage row path retain the archive's channel masks, alpha handling, output allocation boundary, and vector/tail telemetry. SIMD-requested LA/PA/RGBA preserves alpha as in the archived adapter; CMYK inverts all four channels. The active operation's separately selected Default implementations remain the scalar `255 - byte` path.
- TODO: retire the separate `registration/simd.rs` Invert adapter after every pipeline, eager, fusion, and compatibility route reaches the injected CPU architecture strategy.
- Posterize and Solarize now declare all 25 modes at `imageops/ops.rs`, with prepared CPU Default strategies for native L and RGB. Posterize admission carries the normalized bit count; P retains its `NotImplementedError`, and other unsupported identities retain Pillow's mode-specific `OsError`. TODO: palette-index LUT strategies for P remain deferred until their palette/identity contract is migrated; the existing public rejection stays in place. Their Default kernels are under `luma8/default` and `rgb8/default`; registration now prepares these plans instead of calling legacy direct-result functions. L/RGB also declare SSE2, NEON, and Wasm SIMD128 implementations using the exact-mode `NativePointwiseBytes` guard; prepared execution matches the selected architecture. The archived vector transforms now live in `imageops/helpers/native_bytes.rs` and architecture-owned mode wrappers, retaining padded vector tails and vector telemetry.
- TODO: preserve Solarize's high-threshold and custom-comparison LUT route through `eval_replicated` when the pending Eval CPU owner is migrated; those requests deliberately use Eval rather than `PipelineOp::Solarize`.
- TODO: retire the separate Posterize/Solarize SIMD registrations only after eager, fusion, and compatibility routes use the injected CPU context. Their pointwise kernels are now in the L/RGB architecture owners; retain this route TODO until those callers are reconciled.
- `Autocontrast` and `Equalize`/`EqualizeMasked` now have operation-owned support declarations and prepared CPU callbacks. L and RGB use their exact Default kernels and guarded SSE2, NEON, or Wasm SIMD128 kernels. P is explicitly rejected by Autocontrast with the existing `NotImplementedError`; Equalize expands P through the injected palette to RGB and selects a declared RGB data kernel under the P strategy. Histogram arithmetic remains in `imageops/helpers/histogram.rs`; ImageOps mask facts accept exact `One`/L identities and preserve the mode-before-size error order. TODO: retire the separate histogram SIMD registrations only after eager, fusion, and compatibility callers are rewired to these prepared CPU strategies. `Colorize` now has an all-mode prepared CPU Default owner plus native-L SSE2, NEON, and Wasm SIMD128 strategies; its independent SIMD registration still needs retirement after callers migrate. `CropBorder` now uses a prepared imageops callback; its separate SIMD adapter still needs retirement.
- `Expand` now has an operation-owned support declaration for all 25 exact source identities. Each mode has a Default owner under its own folder, performs border fill and row copying in its native sample representation, and returns the same exact `DynamicImage` variant. P/PA preserve their index samples; typed I;16 and internal high-depth modes never route through an 8-bit mode. The five internal high-depth modes are core-only and carry nearby `NO_PILLOW_SUPPORT` comments; fill bytes become same-valued native numeric samples. `ExpandedDimensions` exposes source layout, border facts, and checked output allocation through the same contract used during preparation. Registry preparation now enters the closed CPU `PreparedExpand` plan instead of the archived direct callback.
- The archived 128-bit fill/copy implementation now lives in `imageops/helpers/expand_vector.rs` and is shared by exact-mode byte strategies. The same declaration selects SSE2, NEON, or Wasm SIMD128 when built and available. Its `ExpandVector` guard enables full border fills for one- and two-byte layouts; three- and four-byte layouts use it for border-zero identity copies, matching the archived vector work. Other requests select the same-mode Default implementation. The vector kernel constructs the original exact variant rather than inferring output mode from channel count.
- The archived CPU Expand cases and SIMD byte-fill/copy regressions now live in `imageops/tests/expand.rs`; mode-string overrides were replaced with exact `DynamicImage` variants, and the old contradictory carrier/mode checks are no longer constructible. These tests have not been run.
- TODO: retire the independent `registration/simd.rs` Expand adapter after eager, fusion, and compatibility routes use the injected CPU context. Its duplicated vector kernel is already represented by the prepared CPU strategy; route retirement is not claimed.
- The ImageOps implementation and support matrix have not been compiled, parity-checked, or benchmarked. No source-completion claim is made.

| Preserved source/function | Maintained destination and disposition |
| --- | --- |
| `legacy/ops/imageops/pixel.rs::op_invert` | `imageops/<exact-mode>/default/invert_impl.rs`, with bytewise arithmetic in `imageops/helpers/pixel.rs`; successful output preserves the exact `DynamicImage` variant. |
| `legacy/ops/imageops/pixel.rs::{op_posterize,op_solarize}` | `imageops/luma8/default/{posterize_impl,solarize_impl}.rs` and `imageops/rgb8/default/{posterize_impl,solarize_impl}.rs`; exact-mode prepared plans own admission and serial execution. Archived vector kernels are folded into each mode's SSE2/NEON/Wasm SIMD128 wrappers via `imageops/helpers/native_bytes.rs`. The independent SIMD registration remains a TODO until legacy callers are retired. |
| `legacy/ops/imageops/colorize.rs::{colorize_lut,op_colorize}` | Scalar LUT construction/execution is in `imageops/helpers/colorize.rs`, with all-mode preparation in `imageops/ops.rs`; the archived vector kernel is composed by native-L SSE2, NEON, and Wasm SIMD128 wrappers. Exact nonempty native-L bytes guard architecture selection, and the 65,536-pixel path split, padded tails, and vector telemetry are retained. Independent SIMD registration/caller retirement remains TODO. |
| `legacy/ops/imageops/{histogram.rs,pixel.rs}::{autocontrast_lut,equalize_lut,op_autocontrast,op_equalize_with_mask}` | `imageops/helpers/histogram.rs` retains histogram arithmetic; `autocontrast.rs` and `equalize.rs` declare mode support, mask admission, and prepared selection. L/RGB Default kernels live under their mode folders. Equalize P expands the injected palette to RGB and declares the resulting architecture route. |
| `legacy/ops/imageops/expand.rs::op_expand` and `legacy/pool_simd/ops/adapters/expand.rs::simd_expand` | `imageops/expand.rs` declares all 25 source-mode routes; every mode binds its contract to `default/expand_impl.rs`, and the 14 Pillow byte modes compose the shared `imageops/helpers/expand_vector.rs` kernel for guarded SSE2, NEON, and Wasm SIMD128 execution. The exact mode owner restores its own output variant. TODO in `imageops/ops.rs` and `registration/simd.rs`: retire the independent SIMD adapter after eager, fusion, and compatibility callers use the prepared CPU context. |
| Archived Expand CPU and SIMD regressions | Migrated to `imageops/tests/expand.rs` with the original native fill bytes, output dimensions, byte comparisons, and architecture-request comparisons; exact mode tags replace string overrides. Not executed. |
| `legacy/pool_simd/ops/adapters/histogram.rs` | Histogram LUT application now runs through the L/RGB architecture-owned wrappers and shared `imageops/helpers/histogram.rs` vector kernel; TODO in `imageops/ops.rs` and `registration/simd.rs`: retire direct SIMD routing after eager, fusion, and compatibility callers use prepared CPU plans. |
| `legacy/pool_simd/ops/adapters/effects.rs::{native_invert,native_invert_copy,native_invert_in_place}` | `imageops/<mode>/{sse2,neon,wasm_simd128}/invert_impl.rs` composes the shared native-byte kernel under `NativeBytePrefix`; separate registry/eager route retirement remains TODO. |
| Remaining ImageOps `PipelineOp` variants in the active TODO at `imageops/ops.rs` | Add exact mode/architecture support declarations, prepared callbacks, output/rejection rules, and callers for the operations still listed there. |

### Color3DLut source progress: incomplete and unvalidated

- `color3dlut/ops.rs` now exposes an exhaustive source-mode contract: RGB, RGBA, and CMYK have Pillow-defined strategies; YCbCr, LAB, and HSV have native `CoreOnly` strategies marked `NO_PILLOW_SUPPORT`; the remaining 19 identities retain the existing `image has wrong mode` rejection. The declaration requires an explicit RGB/RGBA/CMYK target and enforces each target's channel capacity. Its output rule is the exact typed target mode. The three core-only source modes apply the table directly to their native three-byte samples without converting or retagging the source.
- The original signed 10.6 table preparation, 18.15 coordinate scaling, trilinear interpolation order, RGBA/CMYK fourth-byte behavior, and RGB output projection are in `color3dlut/helpers/lut.rs`; mode-owned Default files bind each source identity to that shared scalar kernel. CMYK output now uses the exact `ImageCmyk8` variant.
- The prepared CPU registry callback enters the exact mode strategy. RGB, RGBA, and CMYK each declare Default and the available architecture strategy. The archived eight-lane fixed-point interpolation kernel is shared by those mode owners; its dimensions, channel count, exact table length, native source byte length, and eight-pixel threshold are now part of architecture selection, with Default fallback for ineligible requests. YCbCr, LAB, and HSV declare their own Default and available architecture owners with the same native three-byte kernel; their Pillow-external support is explicitly `CoreOnly` and excluded from Pillow parity. The Rust `Image::color3dlut` API accepts `Option<Mode>`; the Python and JavaScript adapters convert external strings to the typed enum before calling it. Source review found the public image method builds a `PipelineOp::Color3DLut` and reaches `prepare_pipeline` through the CPU registry; there is no separate active direct CPU callback. GPU code maps typed `Mode` values to packed mode codes at its parameter boundary. TODOs remain to remove the separate SIMD Color3DLut adapter and update GPU source-mode snapshot encoding to consume the prepared mode transition. Those paths are not claimed complete.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### Eval source progress: prepared CPU modes implemented; caller migration pending

- `eval/ops.rs` now exposes an exhaustive contract for the 25 exact identities. `One`, `L`, `LA`, `La`, `P`, `PA`, `RGB`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, and `HSV` have native byte-LUT Default strategies with one table per native band. `I`, `F`, all four Pillow I;16 identities, and the five internal high-depth identities retain explicit rejection because `PipelineOp::Eval` accepts byte LUTs; direct scalar point transforms remain a separate pending route.
- Each admitted mode's contract includes an exact `256 * native_channels` LUT length and exact native source-byte dimensions. Its Default implementation preserves the input variant and applies the archived affine-LUT or per-band lookup loop directly to that mode's bytes. P/PA index bytes remain native; the image wrapper continues to own palette metadata. The per-mode Default modules now follow the explicit `mode/default/eval_impl.rs` layout.
- The nine identities admitted by the archived SIMD LUT adapter (`One`, `L`, `P`, `LA`, `RGB`, `HSV`, `YCbCr`, `RGBA`, and `CMYK`) declare SSE2, NEON, and Wasm SIMD128 strategies beside their exact mode contracts. A shared family helper owns the reusable channel LUT kernel; little-endian AArch64 RGB retains the archived NEON TBL4 and affine paths, and identity LUTs retain the native-copy path. Other admitted byte modes use their same-mode Default strategy when no architecture specialization is declared. The shared SIMD kernel contains no Rayon scheduling.
- Registry preparation enters the typed Eval plan, but the separate SIMD registration remains a TODO until eager, fusion, and compatibility callers use prepared CPU plans. The archived Rayon row scheduler is reserved for `pool_parallel`. Direct `eval_point_transform` and `eval_float` still need a typed native-sample owner.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run for this source.

### Explicit pending-route TODO ledger

The following maintained source TODOs are open migration routes, not support
claims or completion evidence. Each TODO stays beside its current owner; the
baseline source groups are recorded in Sections 2 and 8. A route is removed
from this table only after its maintained implementation and callers have been
reviewed. New gaps found during that review must be added here and marked with
a source TODO before moving on.

Core support is permitted to exceed Pillow. Supported internal high-depth
identities are core extensions, not Pillow parity cases; their family support
descriptors report `CoreOnly`, and their operation owners carry nearby
`NO_PILLOW_SUPPORT` comments. Existing TODOs for typed native semantics remain
open until their owner-level review removes any conversion used only to share
an implementation.

| Maintained TODO location | Pending route |
| --- | --- |
| `compute/pool_cpu/ops/mod.rs` | Create active owners and rewire callers for `resize`, `thumbnail`, `color`, `filter`, `geometry`, `chops`, `enhance`, `paste`, `composite`, `transform`, and `draw`. |
| `compute/pool_cpu/ops/eval/ops.rs` | Retire the separate SIMD registration after all callers use prepared CPU plans; move archived Rayon scheduling to `pool_parallel`; migrate direct scalar `eval_point_transform` and `eval_float` calls to a native typed owner. |
| `compute/pool_cpu/mod.rs`, `compute/pool_cpu/batch_execution.rs` | Serial prepared dispatch, routed-context plumbing, and wrapper-resolved P/PA palette handoff are present. Complete source-accounted fusion/terminal helpers; then finish admission, eager/lazy compatibility, and nested context routes. |
| `compute/mod.rs` | Retire the separate SIMD pool and its adapter re-exports after each remaining route uses a CPU architecture strategy. |
| `compute/execution.rs` | Replace mixed CPU/SIMD pipeline segments, mode transitions, and SIMD recovery with CPU architecture selection and injected prepared execution. |
| `compute/automatic_segments.rs` | Replace the separate CPU/SIMD segment rules with CPU architecture-aware prepared strategies and explicit mode transitions. |
| `compute/eager.rs` (module TODO) | Migrate remaining eager SIMD adapters for projections, generators, filters, and other direct image operations to CPU family strategies selected by context. |
| `compute/pool_cpu/ops/put/ops.rs` | Connect prepared mutation plans to pool/batch execution and thumbnail fusion; complete their metadata and eager caller routes. |
| `compute/pool_cpu/ops/put/ops.rs` | Audit the noted cross-mode conversions; implement native-mode paths or retain only conversions required by the operation's Pillow semantics. |
| `compute/registry/registration/mod.rs` | Replace direct-result registrations for `chops`, `color`, `draw`, `effects`, `enhance`, `filter`, `geometry`, `module_fns`, and `transform` with prepared family plans. |
| `compute/registry/registration/prelude.rs` | Retire the direct CPU facade import map after every remaining registration uses its family-owned prepared callback. |
| `compute/registry/registration/imageops.rs` | Migrate the remaining `Contain`, `Cover`, `Fit`, `Pad`, and `Scale` registrations to prepared owners; Expand now uses its prepared CPU owner. |
| `compute/registry/registration/simd.rs` | Fold remaining SIMD adapter implementations into CPU architecture owners, rewire every pipeline/eager/fusion/compatibility caller, then retire independent SIMD registrations. |
| `compute/pool_cpu/ops/crop/ops.rs` | Reconcile ordinary native-byte crop adapter behavior and retire the separate SIMD registration after all callers use injected CPU mode/architecture strategies. |
| `compute/pool_cpu/ops/color3dlut/ops.rs` | Fold the remaining Color3DLut SIMD adapter into its CPU architecture strategy and retire the separate registration. |
| `compute/pool_cpu/ops/grayscale/ops.rs` | Finish remaining wrapper output-mode transitions and route the serial pool plus remaining direct callers through the prepared callback with injected context. |
| `compute/pool_cpu/ops/imageops/ops.rs`, `compute/registry/registration/simd.rs` | Retire the independent SIMD Expand adapter after callers use the prepared CPU context; its vector fill/copy kernel is now shared by the exact-mode CPU strategies. Native Default strategies cover all 25 exact identities. Keep any Rayon implementation in Parallel CPU. Also add palette-index Posterize/Solarize LUT strategies when palette identity semantics are migrated, and retire Colorize, histogram, and point-operation compatibility routes only after callers use the prepared CPU context. Colorize now has native-L SSE2/NEON/Wasm SIMD128 strategies; Posterize/Solarize and Autocontrast/Equalize L/RGB architecture kernels are also present. |
| `compute/pool_cpu/ops/imageops/ops.rs` (`prepare_crop_border`) | Keep the CropBorder architecture receipt aligned with the composed Crop plan, then retire the separately registered SIMD CropBorder adapter after its caller routes are migrated. |
| `compute/pool_cpu/ops/transpose/ops.rs` | Native-byte transpose, including tiled/gathered axis swaps and same-dimension methods, is declared for the archived native-byte and I;16 identities under SSE2, NEON, and Wasm SIMD128; RGBA AVX2 remains preferred for eligible axis swaps. Keep archived parallel collection kernels for the later `pool_parallel` stage; retire the duplicate byte-Flip compatibility route after callers migrate. |
| `compute/pool_cpu/ops/rotate/ops.rs` | The maintained owner declares all 25 exact modes with serial Default strategies, including typed native `u16`/`f32` paths for the five internal identities, each marked `NO_PILLOW_SUPPORT`. The serial three-byte contiguous-copy and RGBA unrolled nearest paths are restored. All byte-backed exact modes declare native-byte nearest strategies for AVX2, SSE2, NEON, and Wasm SIMD128, including raw-byte `I`/`F` carriers; byte modes declare their archived bilinear vector kernel where supported, including typed I/F arithmetic. Still route pipeline/eager/compatibility callers through this owner. Core-only extensions remain valid when marked `NO_PILLOW_SUPPORT` and excluded from Pillow parity. |
| `compute/registry/registration/imageops.rs` | Route remaining eager ImageOps through prepared CPU plans and preserve operation-specific behavior and evidence. |
| `compute/eager.rs::record_cpu_arch_operation_telemetry` | Extend operation receipts to report the selected CPU `Architecture`; the current receipt preserves requested backend, Default fallback, and in-flight vector/tail evidence but has no architecture field. |
| `image/materialization.rs` | Keep typed prepared plans flowing through the serial pool and preserve resolved operation dependencies on wrapper-to-pool routes. Single-operation lazy P/PA grayscale now prepares before wrapper-wide expansion. |
| `image/construction.rs`, `image/mode_boundary.rs`, `image/pixel_access.rs` | Convert remaining core string-taking constructors, generated-image and pixel-mutation entry points to typed `Mode`; keep Pillow string parsing at host/API boundaries. |
| `image.rs` | Remove the remaining fallback mode identity and reconcile exact materialized and cached pipeline mode ownership. |
| `draw/mod.rs` | Accept typed `Mode` for the drawing entry point and keep Pillow string parsing at the host boundary. |
| `image/metadata.rs` | Derive exact typed mode from encoded headers/decode state before the compatibility string fallback. |
| `ops/convert/operation.rs` | Take typed source and destination mode identities in the core conversion operation. |
| `ops/enhance.rs` | Replace Pillow-string mode projections and branches with exact typed mode strategies. |
| `ops/filter.rs` | Classify filter support from exact `Mode` identities. |
| `ops/imageops/point.rs::mode_identity` | Remove the materialization fallback after all lazy pipelines carry an exact cached `Mode` through admission and transitions. |
| `ops/imageops/point.rs::solarize_with_input` | Preserve the high-threshold/custom-comparison LUT route through the prepared Eval CPU owner instead of the legacy direct Eval callback. |
| `ops/module_fns.rs` | Accept typed modes for merge and gradient inputs after host conversion. |
| `ops/param_filters.rs` | Keep parameter-filter mode selection on `known_mode_identity()` and typed strategy dispatch. |
| `ops/quantize/operations.rs` | Pass typed mode identity into quantize leaf selectors. |
| `ops/resize.rs` | Pass exact `Mode` identity through the existing resampling implementation. |
| `pipeline/support.rs` | Replace remaining mode string metadata and compatibility parsing with typed `Mode` at host boundaries. |
| `pipeline.rs` | Replace Merge's `logical_mode` string with typed `Mode`; remove the GPU source-mode snapshot after GPU parameter encoding uses the prepared mode transition. |
| `compute/registry/registration/simd.rs`, `compute/pool_cpu/ops/color3dlut/ops.rs`, `compute/pool_cpu/ops/imageops/ops.rs` | Keep the listed independent SIMD compatibility routes until their corresponding prepared CPU architecture strategy and every caller are migrated; Colorize's all-mode Default and native-L architecture strategies exist, but its independent route remains as recorded in its source TODO. |
| `compute/registry/registration/simd.rs`, `compute/pool_cpu/ops/eval/ops.rs` | Retire the separate Eval SIMD adapter after its vector LUT kernels are bound to the injected CPU architecture strategy and all callers use prepared CPU plans. The exact-mode kernel declarations are now present; caller migration remains open. |

## 8. Baseline exported function declarations

This source scan lists every pub-visible function declaration in maintained baseline CPU and SIMD source, excluding test modules. It includes family entry points and helpers exposed to sibling modules; source visibility alone does not imply runtime reachability. The declared operation-to-family map remains Section 5 of IMPLEMENTATION_STRUCTURE.md, while the caller-reference list above identifies pool-level production callsites. Preserve or retire each listed declaration only after its consumers and feature/architecture configurations have been accounted for.

### CPU (313 declarations across 73 files)

- pillow-rs/src/compute/pool_cpu/batch_execution.rs: 7: execute_batch
- pillow-rs/src/compute/pool_cpu/ops/chops.rs: 326: op_chops_add, 344: op_chops_subtract, 361: op_chops_multiply, 365: op_chops_screen, 374: op_chops_darker, 378: op_chops_lighter, 382: op_chops_difference, 389: op_chops_overlay, 393: op_chops_hard_light, 400: op_chops_soft_light, 407: op_chops_add_modulo, 414: op_chops_subtract_modulo, 421: op_chops_logical_and, 430: op_chops_logical_or, 439: op_chops_logical_xor, 448: op_chops_constant, 457: op_chops_offset, 461: op_chops_duplicate, 465: op_chops_invert
- pillow-rs/src/compute/pool_cpu/ops/color.rs: 18: op_convert, 397: op_convert_lab, 403: op_remap_palette, 451: op_extract_band
- pillow-rs/src/compute/pool_cpu/ops/color3dlut.rs: 17: op_color3dlut
- pillow-rs/src/compute/pool_cpu/ops/composite.rs: 43: op_alpha_composite, 98: op_alpha_composite_mirror, 264: op_merge, 381: op_blend_module, 573: op_composite_module
- pillow-rs/src/compute/pool_cpu/ops/crop.rs: 14: execute_crop
- pillow-rs/src/compute/pool_cpu/ops/draw/arcs.rs: 8: draw_clipped_ellipse, 22: draw_arc_on_canvas, 59: draw_chord_on_canvas, 99: draw_pieslice_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/canvas.rs: 20: from_image, 37: into_image, 47: set_alpha_blend_rgb, 53: set_l16_ink_order, 131: l16_draw_uses_big_endian, 140: draw_native
- pillow-rs/src/compute/pool_cpu/ops/draw/clipping.rs: 22: union_intervals, 37: intersect_intervals, 51: clip_intervals, 118: normalize_angles, 136: clip, 140: and, 144: or, 148: arc_clip_state, 226: chord_clip_state, 248: chord_line_clip_state, 274: pie_side_clip_state, 290: pie_clip_state, 313: for_each_clipped_ellipse_span
- pillow-rs/src/compute/pool_cpu/ops/draw/dispatch.rs: 12: draw_op_on_canvas, 221: is_draw_op, 238: execute_draw_batch
- pillow-rs/src/compute/pool_cpu/ops/draw/ellipse.rs: 95: new, 125: next, 180: draw_hline, 208: for_each_ellipse_span, 235: draw_ellipse_segments, 250: draw_ellipse_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/operations.rs: 11: op_draw_line, 27: op_draw_rectangle, 45: op_draw_rounded_rect, 65: op_draw_ellipse, 83: op_draw_circle, 108: op_draw_polygon, 128: op_draw_arc, 150: op_draw_chord, 170: op_draw_pieslice, 190: op_draw_point
- pillow-rs/src/compute/pool_cpu/ops/draw/polygon.rs: 60: draw_polygon_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/primitives.rs: 6: draw_line_on_canvas, 29: draw_horizontal_span, 54: draw_vertical_segment_exclusive, 86: draw_rect_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/draw/rounded_rect.rs: 7: validate_rounded_rect, 26: draw_rounded_rect_on_canvas
- pillow-rs/src/compute/pool_cpu/ops/effects.rs: 13: apply_effect_rows, 43: blend_row, 74: next, 109: with_process_rng, 118: op_effect_spread, 211: effect_spread_mapping, 260: effect_spread_advance_identity_rng, 279: op_effect_noise, 295: effect_noise_values
- pillow-rs/src/compute/pool_cpu/ops/enhance.rs: 64: op_enhance_brightness, 206: op_enhance_contrast, 270: op_enhance_color_saturation, 443: op_enhance_sharpness
- pillow-rs/src/compute/pool_cpu/ops/eval.rs: 116: op_eval
- pillow-rs/src/compute/pool_cpu/ops/filter/blur.rs: 575: execute_gaussian_blur, 602: execute_box_blur, 610: execute_box_blur_xy_with_passes
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/box_rows.rs: 184: blur_box_radius_one_row_ring
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/line.rs: 6: blur_line_step, 87: blur_line, 160: blur_one_row, 189: blur_rows, 238: blur_columns, 267: blur_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one.rs: 36: blur_luma_radius_one_fractional_line, 127: blur_rgb_radius_one_fractional_line, 222: blur_rgba_radius_one_fractional_line, 325: blur_interleaved_rows_three_passes, 432: blur_la_radius_one_fractional_line, 515: blur_la_rows_three_passes_radius_one, 562: blur_luma_rows_three_passes
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/radius_one/vertical.rs: 6: blur_luma_vertical_radius_one_rows, 70: blur_interleaved_vertical_radius_one_fractional_rows, 125: transpose_interleaved_rows, 152: blur_columns_radius_one_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/convolution.rs: 5: clip8_filter, 23: pillow_kernel_row_3, 34: pillow_kernel_row_5
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3.rs: 119: uniform_i32_filter3x3_sample_is_identity, 139: uniform_i32_filter3x3_is_identity, 450: binomial3x3_parameters_match, 455: execute_filter3x3
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5.rs: 259: execute_filter5x5
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32.rs: 575: uniform_i32_filter5x5_is_identity, 678: filter_5x5_i32
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/bounded_binomial.rs: 11: i32_filter5x5_binomial_kernel_is_applicable, 27: i32_filter5x5_bounded_binomial_is_applicable, 56: i32_filter5x5_bounded_binomial_kernel_is_applicable, 80: filter_5x5_i32_bounded_binomial
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes.rs: 12: filter_5x5_byte_row, 91: filter_5x5_byte_rows, 180: filter_5x5_constant_bytes, 203: filter_5x5_uniform_rgba_identity, 232: uniform5x5_parameters_match, 237: rgb_blur_ring5x5_parameters_match, 245: la_single_outlier5x5_parameters_match, 266: filter_5x5_la_single_outlier_rows, 352: filter_5x5_uniform_luma_rows, 428: filter_5x5_uniform_rgba_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/native_bytes/binomial.rs: 1: binomial5x5_parameters_match, 17: filter_5x5_binomial_luma_parallel_rows, 109: filter_5x5_binomial_la_parallel_rows, 211: filter_5x5_binomial_luma_rows, 273: filter_5x5_binomial_la_rows
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/blur_line_steps.rs: 6: l_step, 32: la_step, 79: rgb_step, 138: interleaved_step, 190: run
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/i32_cross_smooth.rs: 5: matches, 39: apply
- pillow-rs/src/compute/pool_cpu/ops/filter/kernels/mod.rs: 4: matches, 8: apply
- pillow-rs/src/compute/pool_cpu/ops/filter/rank.rs: 693: execute_median_filter_with_mode, 702: execute_max_filter_with_mode, 711: execute_min_filter_with_mode, 720: execute_rank_filter_with_mode
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/float.rs: 3: native_uniform_f_mode_image, 19: rank_filter_f_requires_legacy_sort, 43: select_rank_filter_f_value, 184: rank_filter_f_impl
- pillow-rs/src/compute/pool_cpu/ops/filter/rgb_blur_ring.rs: 15: parameters_match, 23: apply
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_box.rs: 17: apply_if_uniform
- pillow-rs/src/compute/pool_cpu/ops/filter/uniform_image.rs: 12: native_small_uniform_byte_image
- pillow-rs/src/compute/pool_cpu/ops/grayscale.rs: 16: op_grayscale
- pillow-rs/src/compute/pool_cpu/ops/imageops/colorize.rs: 3: gather_native_channel, 79: colorize_lut, 136: op_colorize
- pillow-rs/src/compute/pool_cpu/ops/imageops/expand.rs: 122: op_expand
- pillow-rs/src/compute/pool_cpu/ops/imageops/geometry.rs: 5: op_contain, 48: op_cover, 93: op_fit, 174: op_crop_border, 242: op_scale
- pillow-rs/src/compute/pool_cpu/ops/imageops/histogram.rs: 38: native_luma_lut_tables, 70: apply_native_luma_lut_row, 90: autocontrast_lut, 383: equalize_lut
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad.rs: 19: op_pad
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/native.rs: 23: pad_native_rows, 350: pad_native_bytes
- pillow-rs/src/compute/pool_cpu/ops/imageops/pixel.rs: 53: op_autocontrast, 141: op_equalize, 146: op_equalize_with_mask, 173: op_invert, 290: op_flip, 347: op_mirror, 390: op_posterize, 435: op_solarize
- pillow-rs/src/compute/pool_cpu/ops/paste.rs: 667: op_paste
- pillow-rs/src/compute/pool_cpu/ops/paste/kernels.rs: 11: paste_native_rows, 85: paste_rgb_into_rgba_rows, 196: paste_native_masked_l_row, 229: paste_native_masked_pa_row, 301: paste_native_masked_two_channel_blend_byte, 321: paste_native_masked_two_channel_l_row, 360: paste_native_masked_cmyk_row, 404: paste_native_masked_hsv_l_row, 444: paste_native_masked_rgb_l_row, 484: paste_native_masked_rgba_l_row
- pillow-rs/src/compute/pool_cpu/ops/paste/layout.rs: 11: paste_native_channels, 38: value, 47: paste_mask_layout, 83: paste_luma16_storage_byte_shift, 102: paste_mask_pixels
- pillow-rs/src/compute/pool_cpu/ops/put.rs: 5: op_put_pixel, 68: op_put_pixel_batch, 107: op_put_data, 194: op_put_alpha, 276: op_put_alpha_data
- pillow-rs/src/compute/pool_cpu/ops/reduce.rs: 226: execute_reduce_rgb, 256: execute_reduce_rgb_with_sparse_rows, 367: execute_reduce_rgb_sparse_2x2_with_rows, 454: execute_reduce_rgb_sparse_2x2, 471: reduce_rgba_premultiplied_block_sums, 503: execute_reduce
- pillow-rs/src/compute/pool_cpu/ops/reduce/kernels/rgb.rs: 105: execute_reduce_rgb_general, 144: reduce_rgb_dense_even_2x2
- pillow-rs/src/compute/pool_cpu/ops/resize.rs: 25: execute_resize, 64: execute_resize_boxed
- pillow-rs/src/compute/pool_cpu/ops/resize/filter.rs: 7: f_kernel_box, 12: f_kernel_triangle, 18: f_kernel_catrom, 36: f_kernel_lanczos, 55: f_kernel_hamming, 76: f_kernel_lanczos3, 83: resample_kernel
- pillow-rs/src/compute/pool_cpu/ops/resize/float.rs: 32: f_resize_mul_add, 45: f_resize_accumulate, 65: f_resize_horizontal_eight_tap_sample, 116: f_resize_horizontal_sample, 159: f_resize_contiguous_eight_tap_coefficients, 180: f_resize_vertical_sample, 267: f_resize_horizontal_pass, 349: f_resize_vertical_pass, 411: resize_f_tall_order, 527: resize_f, 540: resize_f_with_fma
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize.rs: 15: accumulate_row, 39: vertical_pass_packed, 65: packed_vertical_output, 114: nearest
- pillow-rs/src/compute/pool_cpu/ops/resize/integer.rs: 20: resize_i_mul_add, 30: resize_i_sum_eight, 48: resize_i_sum_eight_contiguous, 69: resize_i_sum_general, 86: resize_i_round_up_to_i32, 93: resize_i_contiguous_eight_tap_weights, 115: resize_i_horizontal_row, 171: resize_i_streaming_vertical_pass, 229: resize_i_streaming_with_fma, 278: resize_i, 441: resize_i_with_fma, 530: i32_sample_bytes_from_native_storage, 548: i32_samples_from_native_storage, 562: resize_i_boxed
- pillow-rs/src/compute/pool_cpu/ops/rotate.rs: 26: rotate_90_non_expand, 85: execute_rotate
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine.rs: 21: affine_nearest_fixed, 403: rotate_arbitrary_generic
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine/coordinates.rs: 2: ceil_div_positive_denominator, 15: fixed_coordinate_in_bounds_span, 39: nearest_positive_step_in_bounds_span, 80: fill_rgb_pixels, 87: fill_rgba_pixels
- pillow-rs/src/compute/pool_cpu/ops/rotate/cubic.rs: 4: rotate_cubic_horizontal_f32, 16: rotate_cubic_horizontal_f64, 32: rotate_cubic_horizontal_i32, 60: rotate_cubic_vertical_f64_fma, 72: rotate_cubic_vertical_f64
- pillow-rs/src/compute/pool_cpu/ops/rotate/scalar.rs: 17: rotate_arbitrary_scalar
- pillow-rs/src/compute/pool_cpu/ops/thumbnail.rs: 40: execute_thumbnail, 241: rgb_putpixel_thumbnail_fusion_supported, 292: reduce_f_thumbnail_2x2_samples, 327: reduce_f_thumbnail, 463: resize_i_thumbnail_reduce2x2, 481: resize_i_thumbnail_reduce2x2_with_fma, 579: reduce_i_thumbnail
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/fusion.rs: 13: execute_rgb_putpixel_thumbnail_fusion
- pillow-rs/src/compute/pool_cpu/ops/transform.rs: 59: op_transform
- pillow-rs/src/compute/pool_cpu/ops/transform/affine.rs: 9: transform_affine_luma_nearest, 61: transform_affine_luma_alpha_nearest, 117: transform_affine_rgb_nearest, 361: transform_affine_generic
- pillow-rs/src/compute/pool_cpu/ops/transform/projective.rs: 177: transform_projective_generic, 250: transform_luma16, 349: transform_mesh, 494: transform_mesh_with_filter, 602: cubic_sample
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar.rs: 155: transform_affine_f32, 227: sample_transform_f32, 396: f32_sub_c, 420: f32_add_c, 444: f32_fma_c, 475: f64_from_f32_c, 493: f32_from_f64_c, 511: f64_neg_c, 531: f64_sub_c, 555: f64_add_c, 579: f64_fma_c, 616: affine_nearest_fixed
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar/projective.rs: 2: transform_projective_scalar32, 206: transform_mesh_scalar32
- pillow-rs/src/compute/pool_cpu/ops/transpose.rs: 25: should_tile_transpose, 37: transpose_bytes_tiled, 116: transpose_bytes_serial, 153: transpose_bytes_rows, 202: execute_transpose
- pillow-rs/src/compute/pool_cpu/ops/transpose/arch.rs: 7: try_rotate270_rgba_tiled
- pillow-rs/src/compute/pool_cpu/ops/transpose/kernels/transpose_rgba/x86_64.rs: 26: rotate270_tiled
- pillow-rs/src/compute/pool_cpu/ops/transpose/native.rs: 19: execute_flip, 62: execute_axis_swap

### SIMD (569 declarations across 150 files)

- pillow-rs/src/compute/pool_simd/batch_execution.rs: 7: execute_batch
- pillow-rs/src/compute/pool_simd/mod.rs: 38: compose_transpose_methods, 53: transpose_identity_can_reuse
- pillow-rs/src/compute/pool_simd/ops/adapters.rs: 191: simd_unsupported, 197: preserves_native_contract, 255: simd_in_place_supported, 298: simd_execute_in_place
- pillow-rs/src/compute/pool_simd/ops/adapters/alpha_composite.rs: 19: simd_alpha_composite_operands_supported, 47: simd_alpha_composite_shape_supported, 61: supports_for_image, 68: supports_for_shape, 455: simd_alpha_composite_mirror, 558: simd_alpha_composite
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur.rs: 28: simd_native_blur_channels, 33: supports_for_image, 63: supports_for_shape, 117: simd_pil_box_blur, 136: simd_pil_box_blur_xy, 606: simd_box_blur
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels.rs: 388: simd_blur_rows, 482: simd_transpose_interleaved_rows, 539: simd_blur_transpose_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/la.rs: 8: simd_la_blur_scalar_pixel, 38: simd_la_vertical_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/luma.rs: 129: simd_luma_horizontal_radius_one_three_passes_rows, 176: simd_luma_vertical_radius_one_rows, 247: simd_luma_box_blur_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgb.rs: 140: simd_rgb_horizontal_radius_one_rows, 185: simd_rgb_vertical_radius_one_rows, 326: simd_rgb_box_blur_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba.rs: 47: simd_rgba_horizontal_radius_one_rows, 114: simd_rgba_vertical_radius_one_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_avx2.rs: 15: blur_middle_block
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/kernels/rgba_neon.rs: 18: blur_middle_block, 96: blur_vertical_rows_radius_seven
- pillow-rs/src/compute/pool_simd/ops/adapters/chops.rs: 28: materialize_chops_operand, 38: supports_constant_for_image, 42: supports_constant_for_shape, 158: supports_for_image, 211: supports_for_shape, 273: supports_in_place, 295: execute_in_place, 466: simd_multiply, 479: simd_screen, 492: simd_overlay, 503: simd_hard_light, 514: simd_soft_light, 538: simd_add, 573: simd_subtract, 607: simd_fused_multiply_screen, 683: simd_constant
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels.rs: 100: apply_native_blend_rows_in_place, 157: native_chops_blend, 271: native_chops_bytewise_in_place, 396: native_chops_overlay, 404: native_chops_hard_light, 443: native_chops_soft_light, 531: native_chops_pair_channels, 548: native_chops_affine, 623: native_chops_affine_in_place, 744: native_logical_and_vector, 756: native_logical_or_vector, 768: native_logical_xor_vector
- pillow-rs/src/compute/pool_simd/ops/adapters/chops/kernels/fused_multiply_screen.rs: 7: simd_fused_multiply_screen_row
- pillow-rs/src/compute/pool_simd/ops/adapters/color3dlut.rs: 54: color3dlut_supported_for_image, 83: color3dlut_supported_for_shape, 322: simd_color3dlut
- pillow-rs/src/compute/pool_simd/ops/adapters/colorize.rs: 10: supports_for_image, 20: supports_for_shape, 141: simd_colorize
- pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs: 49: supports_lab_for_image, 57: supports_lab_for_shape, 235: native_convert_pixels_per_vector, 262: native_convert_supported_for_image, 343: native_convert_supported_for_shape, 542: simd_convert, 619: simd_convert_lab
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/alpha.rs: 33: supports_image, 60: supports_shape, 89: try_convert
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/cmyk.rs: 13: native_cmyk_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/color_space.rs: 23: native_convert_cmyk_bytes, 109: native_convert_ycbcr_bytes, 190: native_convert_hsv_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch.rs: 15: native_convert_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/layout.rs: 6: append_layout_pixels
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/luma.rs: 6: append_rgb_luma_pixels
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/dispatch/typed.rs: 6: append_integer_pixels, 57: append_float_pixels
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/layout.rs: 14: native_convert_layout_block, 142: native_luma_to_rgb_bytes, 179: native_luma_to_rgba_bytes, 232: native_luma_alpha_to_rgba_bytes, 268: native_rgba_to_la_bytes, 318: native_rgba_color_degenerate, 396: native_rgb_to_rgba_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/mod.rs: 27: native_rgba_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs: 10: native_rgba_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs: 9: native_rgba_to_rgb_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/crop.rs: 12: supports_for_image, 40: supports_for_shape, 234: simd_crop_border, 270: simd_crop
- pillow-rs/src/compute/pool_simd/ops/adapters/draw.rs: 24: native_draw_layout, 91: shape_draw_channels, 114: supports_for_image, 187: supports_for_shape, 261: preserves_native_contract, 282: simd_draw_line, 314: simd_draw_point, 340: simd_draw_rectangle, 401: simd_draw_polygon, 427: simd_draw_rounded_rect, 463: simd_draw_ellipse, 497: simd_draw_circle, 530: simd_draw_arc, 566: simd_draw_chord, 604: simd_draw_pieslice
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes.rs: 66: simd_draw_ellipse_native, 133: simd_draw_arc_native, 202: simd_draw_chord_native, 288: simd_draw_pieslice_native, 433: simd_draw_polygon_native
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/shapes/rounded_rect.rs: 223: simd_draw_rounded_rect_native
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans.rs: 69: draw_line_vector_block, 112: simd_draw_span, 286: simd_draw_points_native, 398: simd_draw_rectangle_native
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/kernels/spans/line.rs: 11: simd_draw_line_native
- pillow-rs/src/compute/pool_simd/ops/adapters/duplicate.rs: 15: supports_for_image, 19: supports_for_shape, 45: simd_copy_native_bytes, 71: simd_duplicate
- pillow-rs/src/compute/pool_simd/ops/adapters/effects.rs: 135: native_invert, 206: simd_invert, 214: simd_invert_chops, 223: output_layout, 228: output_mode, 232: supports_for_image, 252: supports_for_shape, 277: supports_in_place, 281: execute_invert_in_place, 294: simd_effect_noise, 489: simd_effect_spread
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance.rs: 23: supports_for_image, 47: supports_for_shape, 76: supports_in_place, 87: execute_in_place, 393: simd_brightness, 450: simd_contrast, 485: simd_color_saturation, 528: execute_brightness_in_place
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/point.rs: 11: supports_for_image, 18: supports_for_shape, 30: supports_in_place, 106: execute_in_place, 131: simd_solarize, 149: simd_posterize
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/sharpness.rs: 347: simd_sharpness
- pillow-rs/src/compute/pool_simd/ops/adapters/expand.rs: 11: supports_for_image, 20: supports_for_shape, 279: simd_expand
- pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs: 44: supports_for_image, 62: supports_for_shape, 101: simd_extract_band, 160: simd_extract_band_into
- pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs: 21: supports_for_image, 43: supports_for_shape, 80: use_native_byte_convolution_path, 90: use_native_i32_convolution_path, 109: native_filter_identity_supported_for_image, 133: shape_native_filter_byte_channels, 150: shape_native_i32_convolution_path, 161: shape_native_filter_identity_supported, 322: simd_filter_3x3, 470: uniform_rgb_5x5_identity, 506: rgb_blur_ring_5x5, 670: simd_filter_5x5
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/byte_layouts.rs: 112: native_filter_5x5_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/i32_three_by_three.rs: 73: uniform_i32_filter3x3_is_identity_simd, 131: native_filter_3x3_i32_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/l.rs: 9: native_filter_5x5_uniform_luma_rows, 95: native_filter_5x5_binomial_luma_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/la.rs: 54: native_filter_5x5_single_outlier_la_rows
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/mod.rs: 29: scan_uniform_rgb, 39: apply_rgb_blur_ring
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/rgb_blur_ring.rs: 114: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/three_by_three.rs: 16: native_filter_3x3_vector, 69: native_filter_3x3_pixel_block_integer, 118: native_filter_3x3_channel_block_integer, 167: native_filter_3x3_luma_block_integer, 214: native_filter_3x3_rgb_byte_block_integer, 264: native_filter_3x3_pixel_block_vector, 312: native_filter_3x3_la_binomial_rows, 392: native_filter_3x3_la_binomial_rows, 481: native_filter_3x3_rows, 570: native_filter_3x3_rgb_byte_stream
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/uniform_rgb.rs: 19: scan_uniform_rgb
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/unsharp.rs: 13: simd_unsharp_blend_rgb_150
- pillow-rs/src/compute/pool_simd/ops/adapters/flip.rs: 14: supports_for_image, 32: supports_for_shape, 275: simd_flip, 298: simd_mirror
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs: 16: gaussian_blur_radius, 34: supports_for_image, 45: supports_for_shape, 64: simd_la_horizontal_radius_one_three_passes_rows, 185: simd_pil_gaussian_blur_l_radius_one, 303: simd_gaussian_blur
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/la.rs: 10: horizontal_radius_one
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/kernels/rgba_uniform.rs: 21: prove_identity
- pillow-rs/src/compute/pool_simd/ops/adapters/gradient.rs: 14: simd_linear_gradient_generate, 91: simd_radial_gradient_generate
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs: 52: supports_image, 56: supports_shape, 83: simd_grayscale_with_evidence, 129: simd_grayscale
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/channels.rs: 180: native_grayscale_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/cmyk.rs: 147: native_cmyk_grayscale_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/rgb_avx2.rs: 10: grayscale_rgb_avx2
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs: 10: native_ycbcr_grayscale_bytes, 78: native_ycbcr_grayscale_path
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr_ssse3.rs: 10: ycbcr_luma_ssse3_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/histogram.rs: 42: supports_for_image, 64: supports_for_shape, 97: simd_autocontrast, 144: simd_equalize
- pillow-rs/src/compute/pool_simd/ops/adapters/layout.rs: 9: native_byte_layout, 28: native_filter_byte_layout, 44: native_typed_filter_layout, 67: native_expand_contract_for_image, 88: native_expand_contract_for_shape, 117: native_copy_layout, 140: native_luma16_transpose_layout, 154: has_valid_luma16_copy_bytes, 171: has_vectorized_byte_rows, 183: has_empty_native_bytes, 197: has_vectorized_offset_rows, 220: has_vectorized_luma16_offset, 237: has_vectorized_mirror_rows, 245: has_nonempty_byte_data, 259: has_valid_byte_data, 273: has_vectorized_native_identity_copy, 303: simd_shape, 319: shape_channels, 345: shape_native_byte_channels, 361: shape_native_typed_filter_channels, 368: shape_has_nonempty_byte_data, 378: shape_has_vectorized_native_identity_copy, 389: shape_native_copy_channels, 397: shape_luma16_transpose_layout, 402: shape_luma16_copy_layout, 406: shape_native_identity_copy_channels, 421: shape_has_empty_native_bytes, 430: shape_has_vector_rows, 438: shape_has_vectorized_offset_rows, 455: shape_has_vectorized_luma16_offset, 464: shape_has_vectorized_mirror_rows, 688: shape_after_simd_op
- pillow-rs/src/compute/pool_simd/ops/adapters/lut.rs: 38: supports_for_image, 52: supports_for_shape, 77: in_place_supported, 87: execute_in_place, 103: native_lut_tables, 116: native_lut_chunk, 220: native_lut_tables_for_channels, 261: native_lut_apply, 360: native_lut_map_rows, 622: native_lut_is_identity, 658: native_point_lut, 734: simd_eval, 756: simd_remap_palette
- pillow-rs/src/compute/pool_simd/ops/adapters/mandelbrot.rs: 12: simd_mandelbrot_generate
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter.rs: 21: supports_for_image, 70: supports_for_shape, 123: native_float_rank_supported_for_image, 145: shape_native_float_rank_supported, 212: median_of_nine_vectors, 218: order_statistic_row, 553: simd_order_statistic_filter
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/extrema.rs: 438: simd_max_filter, 458: simd_min_filter
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/float.rs: 178: simd_float_order_statistic_filter, 257: simd_median_filter, 272: simd_rank_filter
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/kernels.rs: 18: median_of_nine_vectors, 49: median_filter_rgb_3x3_row, 94: la_order_statistic_3x3_row
- pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs: 110: supports_image_contract, 120: supports_shape_contract, 130: native_merge_interleave, 196: native_merge_rgb_luma_owners, 236: native_merge_bytes, 291: simd_merge
- pillow-rs/src/compute/pool_simd/ops/adapters/module_blend.rs: 12: supports_for_image, 21: supports_for_shape, 172: native_module_blend_arithmetic_supported, 293: simd_blend_module
- pillow-rs/src/compute/pool_simd/ops/adapters/native_arithmetic.rs: 5: simd_div255, 16: simd_div255_u16x8, 23: simd_pack_u16x16
- pillow-rs/src/compute/pool_simd/ops/adapters/native_bytes.rs: 34: native_expand_fill_sample, 63: native_fill_row, 153: native_byte_transform_bytes, 187: native_all_channel_transform_bytes, 211: native_byte_transform_copy_bytes, 233: apply_native_rows, 272: reverse_pixel_block, 328: copy_native_bytes, 360: native_copy_image_bytes, 394: mirror_native_row, 456: native_luma16_image_from_bytes, 476: record_padded_native_row_work
- pillow-rs/src/compute/pool_simd/ops/adapters/neighborhood.rs: 16: native_zero_byte_image, 29: native_small_uniform_byte_image, 38: native_uniform_byte_image_up_to, 58: simd_blur_load_u8x16, 63: simd_blur_pack_u32x16_to_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/offset.rs: 20: supports_for_image, 28: supports_for_shape, 36: simd_offset
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/byte.rs: 13: circular_byte_block, 74: narrow_rotated_row, 189: native_offset
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/luma16.rs: 165: native_offset_luma16
- pillow-rs/src/compute/pool_simd/ops/adapters/pad.rs: 173: supports_for_image, 180: supports_for_shape, 651: simd_pad
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply.rs: 54: native_paste_apply, 75: native_paste_apply_with_source_layout
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/cmyk_l.rs: 6: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/generic.rs: 151: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/l.rs: 8: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/la.rs: 8: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/pa.rs: 8: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgb.rs: 28: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/apply/rgba.rs: 6: apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/blend.rs: 11: native_paste_mask_block, 36: native_paste_mask_block_active, 70: native_paste_blend_vector16, 85: native_paste_binary_copy_vector16, 97: native_paste_blend_vector8, 114: native_paste_preblend_vector16, 131: native_paste_preblend_vector8, 148: native_paste_rgb_l_mask_block16, 171: native_paste_rgb_l_mask_block8
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma.rs: 9: native_paste_l_masked_row, 107: native_paste_rgba_l_masked_row, 423: native_paste_la_masked_row
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/luma16.rs: 200: native_paste_luma16_apply
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/kernels/rgb_cmyk.rs: 12: native_paste_pa_masked_row, 185: native_paste_rgb_l_masked_row, 345: native_paste_cmyk_l_masked_row
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/mod.rs: 46: simd_composite_module, 205: simd_paste, 304: native_paste_in_place
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/plan.rs: 66: native_paste_layout, 150: native_paste_shape_layout, 272: native_paste_region, 401: native_paste_plan_for_image, 418: native_paste_plan_for_image_with_source_layout, 446: native_paste_plan_for_shape, 479: native_paste_rgba_to_rgb_supported, 500: native_paste_rgba_to_rgb_supported_for_shape, 524: native_paste_rgb_to_rgba_supported, 545: native_paste_rgb_to_rgba_supported_for_shape, 575: native_composite_plan_for_image, 620: native_composite_plan_for_shape, 663: native_paste_actual_layout, 683: native_paste_actual_mask_layout
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/rgb_to_rgba.rs: 119: simd_paste_rgb_to_rgba
- pillow-rs/src/compute/pool_simd/ops/adapters/pipeline.rs: 26: pixel_mode_name, 80: simd_initial_mode, 108: simd_mode_after_op, 133: first_unsupported_simd_op
- pillow-rs/src/compute/pool_simd/ops/adapters/projection.rs: 16: simd_cmyk_projection, 88: simd_cmyk_projection_from_positive_writes, 164: simd_rgb_projection
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha.rs: 20: supports_for_image, 36: supports_for_shape, 69: update_shape, 80: output_mode, 257: simd_put_alpha, 361: simd_put_alpha_data
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha/kernels.rs: 153: simd_put_alpha_data_bytes, 221: simd_put_alpha_rgb_row, 251: simd_put_alpha_rgb_mask_row, 284: simd_put_alpha_rgba_mask_row, 293: simd_put_alpha_rgba_constant_bytes, 320: simd_put_alpha_rgb_constant_bytes, 385: simd_put_alpha_constant_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/put_data.rs: 34: supports_for_image, 43: supports_for_shape, 55: simd_put_data
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel.rs: 11: simd_rgba_putpixel_batch_supported, 48: simd_rgba_putpixel_batch, 200: native_putpixel_layout, 208: native_putpixel_supported_for_image, 226: native_putpixel_supported_for_shape, 302: simd_put_pixel
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels.rs: 9: apply_rgba_batch
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel/kernels/rgba_batch.rs: 8: apply_rgba_batch
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce.rs: 19: native_reduce_layout, 35: shape_native_reduce_layout, 193: native_reduce_supported_for_image, 232: native_reduce_supported_for_shape, 294: native_reduce_sparse_rgb_2x2, 456: native_reduce_bytes, 562: simd_reduce
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/kernels.rs: 2: native_reduce_pixel_sums, 40: native_reduce_average, 46: native_reduce_scalar_pixel, 114: native_reduce_3byte_2x2_vector_block, 187: native_reduce_rgb_3x5_vector_block, 237: native_reduce_vector_block
- pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs: 90: supports_for_image, 122: supports_for_shape, 158: resize_native_channels_for_image, 171: native_resize_byte_layout_for_image, 187: native_resize_byte_layout_for_shape, 204: simd_resize, 438: simd_resize_impl
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/aspect.rs: 84: simd_scale, 111: simd_contain, 143: simd_cover, 170: simd_fit
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/coordinates.rs: 3: resize_nearest_indices, 22: resize_nearest_vectorizable, 52: boxed_nearest_indices, 82: boxed_nearest_axis_maps_identity
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32.rs: 35: simd_resize_f
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/f32/boxed.rs: 6: simd_resize_f_boxed, 325: write_f32_samples_le
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32.rs: 50: build_resize_i_horizontal_plan, 150: resize_i_horizontal_values, 168: simd_resize_i32, 537: simd_resize_i32_boxed
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs: 32: horizontal_block, 101: vertical_block
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs: 21: simd_resize_luma16
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8.rs: 23: simd_resize_convolution, 45: simd_resize_convolution_with_parallel_threshold, 70: simd_resize_convolution_with_zero_row_elision, 106: simd_resize_convolution_into
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_boxed_convolution.rs: 28: simd_resize_convolution_boxed
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_convolution.rs: 10: resize_fixed_point_to_u8, 27: resize_premultiply_u8, 50: resize_unpremultiply_u8, 55: resize_horizontal_scalar, 81: resize_horizontal_vector_row, 177: resize_horizontal_convolution_row, 221: resize_vertical_vector_row
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_cover_i32.rs: 315: resize_horizontal_hsv_i32_vector_row, 396: resize_vertical_hsv_i32_vector_row, 666: resize_horizontal_cover_i32_vector_row, 713: resize_vertical_cover_i32_vector_row
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_nearest.rs: 136: simd_resize_nearest, 202: simd_resize_nearest_boxed
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/u8_plan.rs: 7: resize_coeff_slice, 19: resize_vertical_row_touches_nonzero_source, 43: resize_u8_coefficients_fit_i32, 91: build_resize_horizontal_plan
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/aspect.rs: 14: native_pad_round_dimension, 36: native_pad_contained_dimensions, 71: native_cover_dimensions, 111: native_fit_box, 159: native_fit_dimensions_supported, 179: native_fit_layout_for_image, 198: native_fit_layout_for_shape, 213: native_fit_filter, 226: native_resize_boxed_bounds, 253: native_resize_boxed_layout_for_image, 269: native_resize_boxed_layout_for_shape, 285: native_resize_boxed_supported_for_dimensions, 327: native_resize_boxed_supported_for_image, 358: native_resize_boxed_supported_for_shape, 382: native_fit_float_supported_for_image, 416: native_fit_float_supported_for_shape, 437: native_fit_supported_for_image, 505: native_fit_supported_for_shape, 563: native_pad_channels_for_image, 582: native_pad_channels_for_shape, 597: native_contain_supported_for_image, 637: native_contain_supported_for_shape, 665: native_cover_supported_for_image, 697: native_cover_supported_for_shape
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/resize.rs: 11: native_scale_dimensions, 25: native_resize_supported_for_dimensions, 76: native_resize_supported_for_image, 131: native_resize_supported_for_shape
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate.rs: 198: rotate_discrete_fast_angle, 213: rotate_fill_sample, 285: supports_for_image, 359: supports_for_shape, 434: copy_nearest_affine_pixels, 512: copy_right_angle_pixels, 597: finish_native_rotation, 614: simd_rotate
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/interpolation.rs: 8: simd_rotate_sample_coordinates, 39: bilinear_rotate_scalar, 53: rotate_fused_mul_add, 80: bilinear_rotate_vector, 100: gather_rotate_neighborhood_vectors, 161: rotate_unpremultiply_vector
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native.rs: 22: native_identity_rotate, 36: simd_right_angle_rotate_native, 138: simd_empty_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/bilinear.rs: 157: simd_bilinear_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/nearest.rs: 27: simd_nearest_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/typed_bilinear.rs: 121: simd_typed_bilinear_rotate_native
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/nearest.rs: 14: copy_contiguous_rgb_runs
- pillow-rs/src/compute/pool_simd/ops/adapters/support.rs: 112: simd_supports_for_image, 330: simd_supports_for_shape
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs: 40: native_thumbnail_dimensions, 146: native_thumbnail_supported_for_image, 186: native_thumbnail_supported_for_shape, 232: simd_thumbnail
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs: 14: thumbnail_f_reduce_value, 184: simd_thumbnail_reduce_f
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs: 14: simd_rgb_putpixel_thumbnail_fusion_supported, 126: simd_rgb_putpixel_thumbnail_prepare, 177: simd_rgb_putpixel_thumbnail_resize
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs: 17: simd_thumbnail_reduce_i, 126: simd_thumbnail_i_reduce2x2_resample
- pillow-rs/src/compute/pool_simd/ops/adapters/transform.rs: 255: supports_for_image, 283: supports_for_shape, 314: simd_transform
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/affine.rs: 157: simd_affine_bilinear_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/bilinear/projective.rs: 106: simd_projective_bilinear_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/luma16.rs: 9: simd_affine_luma16_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/mesh.rs: 90: simd_mesh_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/affine.rs: 177: simd_affine_nearest_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/nearest/projective.rs: 63: simd_projective_nearest_transform_bytes
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose.rs: 34: supports_for_image, 43: supports_for_shape, 54: rgba_avx2, 286: native_transpose_bytes, 427: native_transpose, 469: simd_transpose
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels.rs: 22: transpose_rgba
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/portable.rs: 35: transpose_gathered_row, 232: transpose_native_tiled, 287: transpose_native_collect_admitted, 319: finish, 350: new, 367: block, 419: transpose_native_collect, 471: transpose_native_odd_collect_admitted, 507: finish, 542: new, 558: block, 613: transpose_native_odd_collect
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose/kernels/rgba_avx2.rs: 14: transpose_rgba
- pillow-rs/src/compute/pool_simd/ops/native_channel.rs: 47: gather_interleaved_channel, 66: gather_channel_into, 95: gather_channel, 115: extract_rgb_channel_bytes, 291: extract_channel_into
- pillow-rs/src/compute/pool_simd/ops/native_channel/cmyk_ssse3.rs: 32: deinterleave_cmyk_16_ssse3
- pillow-rs/src/compute/pool_simd/ops/native_filter.rs: 6: pack_u16x16, 15: native_filter_load_byte_block, 31: native_filter_5x5_binomial_la_rows, 236: native_filter_5x5_i32_rows
- pillow-rs/src/compute/pool_simd/ops/native_merge.rs: 67: rgb16_interleave_ssse3, 100: vector_block, 135: rgb_interleave_ssse3, 235: rgb_interleave_neon
- pillow-rs/src/compute/pool_simd/point_batch.rs: 60: fused
- pillow-rs/src/compute/pool_simd/put_pixel_batch.rs: 9: try_execute

## 9. DynamicImage variant consumers affected by the mode enum

The baseline has 262 Rust files with 2100 explicit references to one of the ten legacy DynamicImage variant names across the tracked repository. This is a source search count, not a count of exhaustive match arms: it includes constructors, accessors, tests and comments. Review every listed file when replacing the enum; files using wildcard or generic methods without an explicit variant are not included in this list. Keep tests' inputs and assertions unchanged while adapting only the new type shape.

### pillow-rs (262 files)

- pillow-rs/src/color/conversion.rs: 26 references
- pillow-rs/src/color/grayscale.rs: 7 references
- pillow-rs/src/color/tests.rs: 8 references
- pillow-rs/src/color_grayscale_allocation_tests.rs: 9 references
- pillow-rs/src/compute/execution.rs: 7 references
- pillow-rs/src/compute/mod.rs: 3 references
- pillow-rs/src/compute/pool_cpu/batch_execution.rs: 1 references
- pillow-rs/src/compute/pool_cpu/mod.rs: 16 references
- pillow-rs/src/compute/pool_cpu/ops/chops.rs: 13 references
- pillow-rs/src/compute/pool_cpu/ops/color.rs: 45 references
- pillow-rs/src/compute/pool_cpu/ops/color3dlut.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/color_convert_allocation_tests.rs: 12 references
- pillow-rs/src/compute/pool_cpu/ops/composite.rs: 30 references
- pillow-rs/src/compute/pool_cpu/ops/draw/canvas.rs: 10 references
- pillow-rs/src/compute/pool_cpu/ops/effects.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/enhance.rs: 36 references
- pillow-rs/src/compute/pool_cpu/ops/eval.rs: 10 references
- pillow-rs/src/compute/pool_cpu/ops/filter/blur.rs: 9 references
- pillow-rs/src/compute/pool_cpu/ops/filter/blur/tests.rs: 3 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3.rs: 8 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter3x3/tests.rs: 1 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/filter/filter5x5/int32/tests.rs: 5 references
- pillow-rs/src/compute/pool_cpu/ops/filter/rank.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/float.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/filter/rank/tests.rs: 1 references
- pillow-rs/src/compute/pool_cpu/ops/grayscale.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/colorize.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/expand.rs: 20 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/geometry.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/histogram.rs: 6 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad.rs: 12 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/native.rs: 20 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pad/tests.rs: 13 references
- pillow-rs/src/compute/pool_cpu/ops/imageops/pixel.rs: 43 references
- pillow-rs/src/compute/pool_cpu/ops/paste.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/paste/kernels.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/paste/layout.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/paste/tests.rs: 18 references
- pillow-rs/src/compute/pool_cpu/ops/put.rs: 48 references
- pillow-rs/src/compute/pool_cpu/ops/reduce.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/reduce/tests.rs: 6 references
- pillow-rs/src/compute/pool_cpu/ops/resize.rs: 5 references
- pillow-rs/src/compute/pool_cpu/ops/resize/float.rs: 6 references
- pillow-rs/src/compute/pool_cpu/ops/resize/float_resize.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/resize/integer.rs: 5 references
- pillow-rs/src/compute/pool_cpu/ops/resize/tests.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/rotate/affine.rs: 3 references
- pillow-rs/src/compute/pool_cpu/ops/rotate/tests.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/thumbnail.rs: 12 references
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/fusion.rs: 3 references
- pillow-rs/src/compute/pool_cpu/ops/thumbnail/tests.rs: 14 references
- pillow-rs/src/compute/pool_cpu/ops/transform.rs: 1 references
- pillow-rs/src/compute/pool_cpu/ops/transform/affine.rs: 10 references
- pillow-rs/src/compute/pool_cpu/ops/transform/projective.rs: 8 references
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar.rs: 2 references
- pillow-rs/src/compute/pool_cpu/ops/transform/scalar/projective.rs: 4 references
- pillow-rs/src/compute/pool_cpu/ops/transform/tests.rs: 13 references
- pillow-rs/src/compute/pool_cpu/ops/transpose/tests.rs: 5 references
- pillow-rs/src/compute/pool_gpu/auxiliary_images.rs: 9 references
- pillow-rs/src/compute/pool_gpu/batch_auxiliary_ranges.rs: 1 references
- pillow-rs/src/compute/pool_gpu/batch_execution/routing/execution.rs: 1 references
- pillow-rs/src/compute/pool_gpu/batch_execution/routing/execution/standard.rs: 2 references
- pillow-rs/src/compute/pool_gpu/batch_native_routes.rs: 5 references
- pillow-rs/src/compute/pool_gpu/batch_normalization.rs: 13 references
- pillow-rs/src/compute/pool_gpu/batch_preparation.rs: 1 references
- pillow-rs/src/compute/pool_gpu/batch_telemetry.rs: 2 references
- pillow-rs/src/compute/pool_gpu/batch_upload.rs: 5 references
- pillow-rs/src/compute/pool_gpu/batch_upload/packing.rs: 2 references
- pillow-rs/src/compute/pool_gpu/geometry/native_execution/flip.rs: 2 references
- pillow-rs/src/compute/pool_gpu/geometry/native_execution/transpose.rs: 5 references
- pillow-rs/src/compute/pool_gpu/layout_admission.rs: 9 references
- pillow-rs/src/compute/pool_gpu/logical_mode_admission.rs: 6 references
- pillow-rs/src/compute/pool_gpu/mode.rs: 10 references
- pillow-rs/src/compute/pool_gpu/native_dispatch.rs: 8 references
- pillow-rs/src/compute/pool_gpu/native_hsv_pad.rs: 2 references
- pillow-rs/src/compute/pool_gpu/native_input.rs: 21 references
- pillow-rs/src/compute/pool_gpu/native_input/channels.rs: 13 references
- pillow-rs/src/compute/pool_gpu/native_paste.rs: 35 references
- pillow-rs/src/compute/pool_gpu/native_paste/execution.rs: 7 references
- pillow-rs/src/compute/pool_gpu/native_paste/execution/rgb_to_rgba.rs: 3 references
- pillow-rs/src/compute/pool_gpu/native_paste/masked_execution.rs: 12 references
- pillow-rs/src/compute/pool_gpu/operation_admission.rs: 1 references
- pillow-rs/src/compute/pool_gpu/operation_admission/auxiliary_modes.rs: 10 references
- pillow-rs/src/compute/pool_gpu/operation_support.rs: 4 references
- pillow-rs/src/compute/pool_gpu/point_ops.rs: 40 references
- pillow-rs/src/compute/pool_gpu/point_ops/native_alpha.rs: 5 references
- pillow-rs/src/compute/pool_gpu/projection.rs: 4 references
- pillow-rs/src/compute/pool_gpu/readback.rs: 7 references
- pillow-rs/src/compute/pool_gpu/resize/compact_box.rs: 6 references
- pillow-rs/src/compute/pool_gpu/resize/float_exactness.rs: 4 references
- pillow-rs/src/compute/pool_gpu/resize/float_exactness/word_paths.rs: 1 references
- pillow-rs/src/compute/pool_gpu/resize/float_exactness_f64.rs: 2 references
- pillow-rs/src/compute/pool_gpu/resize/float_fast_paths.rs: 10 references
- pillow-rs/src/compute/pool_gpu/resize/integer.rs: 2 references
- pillow-rs/src/compute/pool_gpu/resize/luma16.rs: 1 references
- pillow-rs/src/compute/pool_gpu/resource_pool.rs: 2 references
- pillow-rs/src/compute/pool_gpu/rgb_merge.rs: 1 references
- pillow-rs/src/compute/pool_gpu/rgb_merge/execution.rs: 5 references
- pillow-rs/src/compute/pool_gpu/stream/encoding.rs: 4 references
- pillow-rs/src/compute/pool_gpu/tests/affine_geometry.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/mod.rs: 4 references
- pillow-rs/src/compute/pool_gpu/tests/native_cover.rs: 6 references
- pillow-rs/src/compute/pool_gpu/tests/native_effects.rs: 24 references
- pillow-rs/src/compute/pool_gpu/tests/native_filters.rs: 29 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma/mode_admission.rs: 18 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma/native_byte_filter.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma/order_statistic.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/native_luma_transfer.rs: 2 references
- pillow-rs/src/compute/pool_gpu/tests/native_padding.rs: 11 references
- pillow-rs/src/compute/pool_gpu/tests/native_paste_execution.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/native_paste_planning/masked_paste_execution.rs: 6 references
- pillow-rs/src/compute/pool_gpu/tests/native_paste_planning/packing_and_native_planners.rs: 6 references
- pillow-rs/src/compute/pool_gpu/tests/palette_projective/palette_alpha_proofs.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/pipeline_fusion.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/point_merge.rs: 11 references
- pillow-rs/src/compute/pool_gpu/tests/projective_filtered.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/projective_nearest_execution.rs: 4 references
- pillow-rs/src/compute/pool_gpu/tests/projective_nearest_proofs.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/readback.rs: 2 references
- pillow-rs/src/compute/pool_gpu/tests/resize_float_proofs/box_and_average_proofs.rs: 13 references
- pillow-rs/src/compute/pool_gpu/tests/resize_float_proofs/dyadic_and_f64_proofs.rs: 10 references
- pillow-rs/src/compute/pool_gpu/tests/resize_float_proofs/finite_words_and_integer_rounding.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/resize_integer.rs: 3 references
- pillow-rs/src/compute/pool_gpu/tests/resize_thumbnail.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/resource_pool.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/rgb_compact.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/transpose_execution.rs: 5 references
- pillow-rs/src/compute/pool_gpu/tests/transpose_planning.rs: 7 references
- pillow-rs/src/compute/pool_gpu/tests/typed_resize.rs: 1 references
- pillow-rs/src/compute/pool_gpu/tests/upload_reuse.rs: 1 references
- pillow-rs/src/compute/pool_gpu/thumbnail.rs: 2 references
- pillow-rs/src/compute/pool_gpu/transform/geometry.rs: 6 references
- pillow-rs/src/compute/pool_gpu/transform/projective_exactness.rs: 4 references
- pillow-rs/src/compute/pool_gpu/transform/safety.rs: 2 references
- pillow-rs/src/compute/pool_simd/batch_execution.rs: 1 references
- pillow-rs/src/compute/pool_simd/mod.rs: 14 references
- pillow-rs/src/compute/pool_simd/ops/adapters/alpha_composite.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/box_blur/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/chops.rs: 12 references
- pillow-rs/src/compute/pool_simd/ops/adapters/color3dlut.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/colorize.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert.rs: 12 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/alpha.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/cmyk.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/layout.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_neon.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/kernels/rgba_rgb_ssse3.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/convert/tests/cmyk_packing.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/draw.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/draw/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/effects.rs: 9 references
- pillow-rs/src/compute/pool_simd/ops/adapters/effects/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance.rs: 30 references
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/point.rs: 10 references
- pillow-rs/src/compute/pool_simd/ops/adapters/enhance/tests.rs: 11 references
- pillow-rs/src/compute/pool_simd/ops/adapters/expand/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/extract_band.rs: 13 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/l.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/kernels/la.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/filter/tests.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/flip.rs: 24 references
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/gaussian_blur/tests.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/cmyk.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/kernels/ycbcr.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/grayscale/tests.rs: 5 references
- pillow-rs/src/compute/pool_simd/ops/adapters/histogram.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/layout.rs: 25 references
- pillow-rs/src/compute/pool_simd/ops/adapters/lut.rs: 5 references
- pillow-rs/src/compute/pool_simd/ops/adapters/lut/tests.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/median_filter/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/merge.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/merge/tests.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/module_blend.rs: 8 references
- pillow-rs/src/compute/pool_simd/ops/adapters/native_bytes.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/offset/kernels/luma16.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/pad.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/pad/tests.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/mod.rs: 7 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/plan.rs: 21 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/rgb_to_rgba.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/paste/tests.rs: 16 references
- pillow-rs/src/compute/pool_simd/ops/adapters/pipeline.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/put_alpha.rs: 18 references
- pillow-rs/src/compute/pool_simd/ops/adapters/put_pixel.rs: 7 references
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/reduce/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize.rs: 3 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/aspect.rs: 5 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/i32/neon.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/kernels/luma16.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/aspect.rs: 12 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/support/resize.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/resize/tests.rs: 10 references
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/rotate/kernels/native/typed_bilinear.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail.rs: 11 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/float.rs: 1 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/fusion.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/thumbnail/kernels/integer.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transform.rs: 6 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/kernels/luma16.rs: 4 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transform/tests.rs: 2 references
- pillow-rs/src/compute/pool_simd/ops/adapters/transpose.rs: 6 references
- pillow-rs/src/compute/pool_simd/point_batch.rs: 4 references
- pillow-rs/src/compute/transpose_tests.rs: 1 references
- pillow-rs/src/draw/bitmap.rs: 17 references
- pillow-rs/src/draw/mod.rs: 2 references
- pillow-rs/src/draw/text/composition.rs: 10 references
- pillow-rs/src/font/pilfont.rs: 1 references
- pillow-rs/src/image/backend.rs: 5 references
- pillow-rs/src/image/channel_access.rs: 2 references
- pillow-rs/src/image/color_count/tests.rs: 1 references
- pillow-rs/src/image/construction.rs: 22 references
- pillow-rs/src/image/data_access/get.rs: 11 references
- pillow-rs/src/image/entropy.rs: 5 references
- pillow-rs/src/image/materialization.rs: 6 references
- pillow-rs/src/image/palette.rs: 5 references
- pillow-rs/src/image/pipeline_admission.rs: 1 references
- pillow-rs/src/image/pixel_access.rs: 1 references
- pillow-rs/src/image/projection.rs: 9 references
- pillow-rs/src/image/projection/getprojection_cmyk_tests.rs: 2 references
- pillow-rs/src/image/projection/getprojection_rgb_pipeline_tests.rs: 1 references
- pillow-rs/src/image/projection/putpixel_pipeline.rs: 4 references
- pillow-rs/src/image/statistics/colors.rs: 4 references
- pillow-rs/src/image_utils.rs: 4 references
- pillow-rs/src/lab.rs: 3 references
- pillow-rs/src/ops/analysis.rs: 4 references
- pillow-rs/src/ops/analysis/bbox.rs: 4 references
- pillow-rs/src/ops/analysis/histogram.rs: 6 references
- pillow-rs/src/ops/convert/binary.rs: 2 references
- pillow-rs/src/ops/convert/dispatch.rs: 5 references
- pillow-rs/src/ops/convert/luma16.rs: 3 references
- pillow-rs/src/ops/convert/nonstandard.rs: 10 references
- pillow-rs/src/ops/convert/operation.rs: 1 references
- pillow-rs/src/ops/convert/palette.rs: 4 references
- pillow-rs/src/ops/convert/support.rs: 3 references
- pillow-rs/src/ops/convert/tests.rs: 4 references
- pillow-rs/src/ops/enhance.rs: 6 references
- pillow-rs/src/ops/param_filters.rs: 2 references
- pillow-rs/src/ops/paste/operations.rs: 1 references
- pillow-rs/src/ops/pil_resize/alpha.rs: 8 references
- pillow-rs/src/ops/pil_resize/boxed.rs: 12 references
- pillow-rs/src/ops/pil_resize/boxed/boxed_f_resize_eight_tap_tests.rs: 1 references
- pillow-rs/src/ops/pil_resize/boxed/boxed_f_resize_fma_tests.rs: 2 references
- pillow-rs/src/ops/pil_resize/float.rs: 18 references
- pillow-rs/src/ops/pil_resize/integer_modes.rs: 4 references
- pillow-rs/src/ops/pil_resize/integer_modes/cmyk.rs: 1 references
- pillow-rs/src/ops/pil_resize/integer_modes_tests.rs: 11 references
- pillow-rs/src/ops/pil_resize/luma16.rs: 3 references
- pillow-rs/src/ops/pil_resize/resize.rs: 34 references
- pillow-rs/src/ops/quantize/operations.rs: 4 references
- pillow-rs/src/raster/dynamic.rs: 107 references
- pillow-rs/src/raster/dynamic/transform.rs: 140 references

### Effects-family source progress: incomplete and unvalidated

- Added the `effects` family with separate `EffectSpread` and `EffectNoise` contracts, exact-mode preparation, prepared-operation registry entries, and Default implementations under all 25 supported mode folders. `EffectSpread` preserves each mode's native typed samples and exact output variant. `EffectNoise` reads dimensions from the exact typed input and declares its fixed `ImageLuma8` output.
- Every internal high-depth route is marked `NO_PILLOW_SUPPORT`. `EffectSpread` also explicitly reports its native-preserving extension as core-only for `La`, `LAB`, the four `I;16` identities, and the five internal high-depth identities; the old path converted or did not define those routes. `EffectNoise` accepts any exact mode as a native dimension carrier and returns L; only the L dimension-carrier route corresponds to Pillow's size-based entry point, so the other 24 mode contracts are marked `NO_PILLOW_SUPPORT` and core-only.
- `EffectSpreadDistance` and `EffectNoiseSigma` are typed input contracts. Prepared mode strategies receive the injected `ExecutionContext`. The archived byte-vector Spread path is now under Default and Sse2/Neon/Wasm SIMD128 strategies for its 14 legacy byte layouts; other modes use native typed Default strategies. Noise's vector arithmetic is declared for every exact mode and returns the fixed L output. Both use request guards to select the specialization or the same mode's Default path.
- Moved the process-global Park–Miller sequence and shared spread/noise control-buffer generation to `compute/helpers/effects`, which is used by CPU and GPU. GPU auxiliary-image generation and identity-spread stream advancement now use that shared helper. `affine_nearest_fixed`, also used by CPU transform and GPU planning, moved to `compute/helpers/geometry` rather than remaining hidden behind the old effects facade.
- Reusable vector data planes live in `effects/helpers/{spread_vector,noise_vector}.rs`; each declared mode/architecture entry points to the shared kernel without converting or retagging its input. Native-preserving Spread routes remain explicitly `NO_PILLOW_SUPPORT` for `La`, `LAB`, the four `I;16` identities, and the five internal high-depth identities.
- The registry entries point to the prepared pipeline callbacks, but the maintained CPU pool/executor and remaining eager/compatibility routes are not yet wired to them. The former EffectNoise and EffectSpread registrations in the SIMD registry must be removed only after those callers route through the injected CPU context. See the actionable TODO in `pool_cpu/ops/effects/ops.rs`; do not treat this family or CPU backend as complete.
- No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Rotate-family source progress: incomplete and unvalidated

- The new `rotate` owner declares all 25 exact mode strategies and returns the exact corresponding `DynamicImage` identity. Its entry point owns request facts and operation support; each mode's `ops.rs` declares its support and binds a Default implementation. The five internal native high-depth strategies use typed samples and carry nearby `NO_PILLOW_SUPPORT` comments.
- Pillow-defined `1`/`P` retain forced nearest sampling, `LA`/`RGBA` retain straight-alpha premultiplication around filtered sampling, and `La`/`RGBa`/`PA`/`RGBX`/CMYK retain raw stored-channel semantics. I/F filtered Default paths and architecture-selected bilinear paths treat their four-byte carriers as typed scalar samples; nearest paths preserve the raw carrier bytes. I;16 variants retain their endian-specific native storage, and I;16 nearest sampling keeps its archived floating-point Geometry.c coordinate mapping.
- The serial byte nearest kernel restores the three-byte contiguous source-span copy and RGBA four-pixel unroll from the archived CPU implementation. Exact-angle and unexpanded rectangular right-angle routing follows the archived behavior. I;16 filtering retains the archived low-byte filter arithmetic without casting the raster mode to L.
- All byte-backed exact modes now declare AVX2, SSE2, NEON, and Wasm SIMD128 nearest strategies in their exact mode architecture folders, including `LA`, `La`, `PA`, `RGBA`, `RGBa`, `RGBX`, `CMYK`, `YCbCr`, `LAB`, `HSV`, and the raw-byte `I`/`F` carriers. Their request guards select these kernels only for finite, non-identity, non-discrete nearest transforms with exact native bytes; all other requests select the same-mode Default strategy. `One` and `P` retain their forced-nearest behavior, and each result keeps its exact mode identity. The vector kernel gathers native samples into 16-byte blocks and retains scalar tails; three-byte modes also retain contiguous source-run copies. The SSE2 build declaration is disabled when AVX2 is a compile-time target feature, so an SSE2 context cannot reach code built for a stronger ISA.
- The same mode/architecture owners declare the archived eight-lane bilinear kernel for eligible byte modes. Its interpolation and geometry helpers stay in Rotate helpers; `LA` and `RGBA` select the alpha-aware path, while premultiplied/indexed/padded/color-space modes preserve their stored channel bytes. `I`/`F` use their separate typed bilinear vector kernel and retain raw-byte nearest sampling.
- The five archived Rotate regression cases now live in `rotate/tests.rs`, rewritten to call the typed prepared owner and exact `DynamicImage` variants. They have not been run.
- No Rotate pipeline or eager caller has been reconciled with the new family owner, and the serial pool remains unfinished. The active TODO in `rotate/ops.rs` names this work; no build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.

### Registry request-support source progress: incomplete and unvalidated

- Split semantic `InputContract` assessment into `registry/support/request/contracts.rs` and materialized `StrategyGuard` checks into `registry/support/request/guards.rs`. The parent module now delegates to these focused helpers and stays below the 1,000-line file limit. Error classes, messages, unresolved-input results, and exact byte-layout conditions were carried over during source review.
- The support source review is not complete for the repository as a whole; remaining functions and files still need the planned size review after the implementation migration. No build, formatter, checkstyle, lint, test, parity, benchmark, or coverage command has run.
