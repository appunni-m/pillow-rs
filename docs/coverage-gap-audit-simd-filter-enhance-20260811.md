# SIMD filter/enhance coverage audit

Date: 2026-08-11
Baseline commit: `6ebcd44736aa706e0c0661b223406b0b240c5086`
Managed SIMD snapshot: `35d7073a-48f8-4a80-848d-9261a06c1de4`
Worktree: `codex/coverage-gap-audit-registry-20260811`

## Result

No additional input batch is justified for these two CPU source files in the
SIMD lane. The public filter and enhancement operations have registered SIMD
adapters. The only intentional CPU-file path is GaussianBlur, whose shared
fallback is already covered. Adding more valid public cases would exercise the
SIMD adapter/scalar implementation, not the uncovered CPU functions.

This is an evidence-only audit. It does not change the generator, fixtures,
manifest, thresholds, or runtime code.

## Managed snapshot

The snapshot is the current managed `migration-parity-rust-simd` result on the
baseline commit. It is fresh, passed, and contains no infrastructure failure.
The queried files report:

| file | lines | branches | functions | regions |
| --- | ---: | ---: | ---: | ---: |
| `pillow-rs/src/compute/pool_cpu/ops/filter.rs` | 82/568 | 2/18 | 3/31 | 183/959 |
| `pillow-rs/src/compute/pool_cpu/ops/enhance.rs` | 0/216 | 0/18 | 0/14 | 0/398 |

The numerator is covered and the denominator is the managed report's
executable count. `coverage_query(view=file, line_ranges=...)` was used for
the exact records; no coverage `show` command was used.

## Exact gaps in `filter.rs`

The ranges below are physical source spans containing the snapshot's
uncovered executable lines; comments and non-counted signature/blank lines
inside a span are not additional coverage items.

| source span | implementation | classification |
| --- | --- | --- |
| 17-25 | `clip8_filter` | Backend-unreachable under SIMD; the SIMD convolution owns its equivalent helper in `pool_simd/ops/scalar.rs`. |
| 35-39, 46-52 | 3-tap and 5-tap kernel helpers | Backend-unreachable under SIMD; SIMD filter adapters call the SIMD scalar helpers. |
| 80 | Unsupported channel arm in `raw_bytes_to_image` | Defensive; public `DynamicImage` channel counts are 1-4. |
| 99-183 | `filter_3x3_i32` | Backend-unreachable under SIMD; `Filter3x3` with `I` uses `pool_simd/ops/scalar.rs`. |
| 189-291 | `filter_5x5_i32` | Backend-unreachable under SIMD; `Filter5x5` with `I` uses `pool_simd/ops/scalar.rs`. |
| 376-453 | `rank_filter_impl` | Backend-unreachable under SIMD; rank/max/min/median adapters use SIMD scalar routines, including the `F` branch. |
| 458-526 | `execute_filter3x3` | Backend-unreachable under SIMD; the registered `Filter3x3` SIMD adapter is selected first. |
| 529-641 | `execute_filter5x5` | Backend-unreachable under SIMD; the registered `Filter5x5` SIMD adapter is selected first. |
| 667-711 | `execute_box_blur` | Backend-unreachable under SIMD; `BoxBlur` uses `simd_box_blur`. |
| 714-748 | CPU rank/filter public wrappers | Backend-unreachable under SIMD because their registered SIMD adapters do not call these wrappers. |

The only covered implementation groups in this file are
`raw_bytes_to_image`'s valid 1-4 channel arms (57-78), `pil_box_blur`
(298-368), and `execute_gaussian_blur` (644-664). The eight partial branch
records are lines 18, 20, 169, 277, 389, 466, 537, and 669; each has 0 of 2
branches covered in this SIMD snapshot. They belong to the same unreachable
or defensive groups above.

## Exact gaps in `enhance.rs`

All counted executable lines are uncovered: `8-19` (`preserve_alpha_result`),
`21-57` (`op_enhance_brightness`), `59-125`
(`op_enhance_contrast`), `127-196` (`op_enhance_color_saturation`), and
`198-261` (`op_enhance_sharpness`). The nine partial branch records are lines
27, 65, 72, 96, 135, 211, 212, 244, and 248; each has 0 of 2 branches
covered.

These are backend-unreachable, not unsupported public behavior. Under SIMD,
the valid ImageEnhance operations dispatch to `simd_brightness`,
`simd_contrast`, `simd_color_saturation`, and `simd_sharpness`, which call
`pool_simd/ops/scalar.rs`. Thus valid L, LA, RGB, RGBA, and CMYK cases cannot
increase coverage in this CPU file without changing backend routing.

## Public case mapping

The following current generated case IDs are representative evidence cases,
not proposed fixture additions:

| public operation/path | current case IDs | SIMD route and target-file impact |
| --- | --- | --- |
| `PIL.Image.Image.filter` / GaussianBlur | `PIL.ImageFilter.GaussianBlur.behavior.default`, `PIL.ImageFilter.GaussianBlur.parameter.radius`, `PIL.ImageFilter.GaussianBlur.mode.l`, `PIL.ImageFilter.GaussianBlur.mode.la`, `PIL.ImageFilter.GaussianBlur.mode.rgba` | `simd_gaussian_blur` delegates to `execute_gaussian_blur`; already covers `pil_box_blur` 298-368 and Gaussian 644-664. |
| `PIL.Image.Image.filter` / convolution | `PIL.ImageFilter.BLUR.behavior.default`, `PIL.ImageFilter.SHARPEN.mode.i`, `PIL.ImageFilter.Kernel.nuanced.five-by-five` | `simd_filter_3x3`/`simd_filter_5x5` and SIMD scalar I-mode; no new CPU `filter.rs` lines. |
| `PIL.Image.Image.filter` / window filters | `PIL.ImageFilter.BoxBlur.mode.l`, `PIL.ImageFilter.MaxFilter.mode.l`, `PIL.ImageFilter.MedianFilter.mode.l`, `PIL.ImageFilter.MinFilter.mode.l`, `PIL.ImageFilter.RankFilter.mode.l`, `PIL.Image.Image.filter.nuanced.f-mode-max-filter` | `simd_box_blur` or SIMD rank/filter adapters; no new CPU `filter.rs` lines. `ModeFilter` is implemented synchronously in `ops/param_filters.rs`, not this file. |
| `PIL.ImageEnhance.Brightness` | `PIL.ImageEnhance.Brightness.mode.l`, `PIL.ImageEnhance.Brightness.mode.la`, `PIL.ImageEnhance.Brightness.mode.rgba`, `PIL.ImageEnhance.Brightness.mode.cmyk` | `simd_brightness`; no `enhance.rs` lines. |
| `PIL.ImageEnhance.Color` / `Contrast` / `Sharpness` | `PIL.ImageEnhance.Color.mode.la`, `PIL.ImageEnhance.Contrast.mode.cmyk`, `PIL.ImageEnhance.Sharpness.mode.rgba` | `simd_color_saturation`, `simd_contrast`, and `simd_sharpness`; no `enhance.rs` lines. |

`UnsharpMask` is a separate valid filter case family. Its core path materializes
a `PipelineOp::GaussianBlur` and then performs its own blend in
`ops/param_filters.rs`; it does not call `pool_cpu/ops/enhance.rs`.

## Candidate decision and expected impact

Candidate input batch for increasing the two requested CPU files under SIMD:
**none**. The existing valid cases already cover the only CPU fallback, and
the remaining public cases are routed to code outside these files. Expected
impact from adding more valid input-only cases is therefore **0 lines, 0
branches, 0 functions, and 0 regions** in these files.

The next meaningful improvement would be an explicitly reviewed backend
design change (for example, deliberately routing a SIMD operation through the
CPU implementation), or a separate non-parity unit-test policy for direct
CPU helper coverage. Neither is an input-generation fix and neither is
included in this audit.

## Verification commands

The managed baseline verification was the safe SIMD coverage command:

```text
MIGRATION_TARGET_BACKEND=simd make migration-parity-coverage-rust
```

The exact snapshot queried was
`35d7073a-48f8-4a80-848d-9261a06c1de4` on commit `6ebcd4473`. A future focused
parity confirmation, if needed, should use only the existing case IDs above:

```text
MIGRATION_TARGET_BACKEND=simd \
MIGRATION_PARITY_ARGS='--case-id PIL.ImageFilter.GaussianBlur.behavior.default --case-id PIL.ImageFilter.BLUR.behavior.default --case-id PIL.ImageFilter.Kernel.nuanced.five-by-five --case-id PIL.ImageFilter.BoxBlur.mode.l --case-id PIL.ImageFilter.MaxFilter.mode.l --case-id PIL.ImageFilter.MedianFilter.mode.l --case-id PIL.ImageFilter.MinFilter.mode.l --case-id PIL.ImageFilter.RankFilter.mode.l --case-id PIL.ImageEnhance.Brightness.mode.l --case-id PIL.ImageEnhance.Color.mode.la --case-id PIL.ImageEnhance.Contrast.mode.cmyk --case-id PIL.ImageEnhance.Sharpness.mode.rgba' \
make migration-parity-test
```

That command is a confirmation batch, not a proposed coverage fix. It must be
run through the managed registered-command ledger if executed.
