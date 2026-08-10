# CPU effects coverage classification — baseline `bc6cfbdfb`

This bounded worker audited the public Pillow 12.2.0 `effect_*` inputs at
`bc6cfbdfb994f3564bfb6803284d680c290e8a21`. It does not add runtime code,
synthetic inputs, expected outputs, thresholds, GPU cases, crash cases,
pending-TIFF cases, or fontdone changes.

## Provenance

| Item | Value |
| --- | --- |
| Worktree | `/Users/lazytrot/work/pillow-rs/.worktrees/coverage-batch-cpu-effects-2-20260811` |
| Branch | `codex/coverage-batch-cpu-effects-2-20260811` |
| HEAD | `bc6cfbdfb994f3564bfb6803284d680c290e8a21` |
| Active CPU baseline supplied for this worker | `23,466/26,611` lines |
| Closest managed CPU file snapshot | `c90216d5-3bf8-4fbf-af6a-6948c0ba6c32`, suite `migration-parity-rust` |
| Effects file snapshot metrics | `1,161/1,342` lines; `136/196` branches; `36/69` functions; `2,361/2,680` regions |

The closest stored CPU snapshot is at commit `1dff439093cbfcf7cd2355572dfaf883a6850e8e`.
There is no diff in `effects.rs`, `ops/module_fns.rs`, or the relevant registry
code between that snapshot commit and the requested baseline, so the
reachability classification below remains applicable at `bc6cfbdfb`.

## Public effect reachability

### `effect_spread`

`op_effect_spread` at `effects.rs:49-144` is covered by existing public
`PIL.Image.Image.effect_spread` cases. The managed report records hits through
the zero-distance return, L/LA/RGB/RGBA mode dispatch, in-bounds and
out-of-bounds spread branches, and all reconstructed output modes. No new
valid public input was justified.

### `effect_noise`

`op_effect_noise` at `effects.rs:765-807` is covered by existing public
`PIL.Image.effect_noise` cases. The report records both the rejection loop and
both `CLIP8` outcomes (`0` at lines 798-799 and `255` at lines 800-801), as
well as the ordinary cast path. The valid `sigma=1_000_000.0` case already
provides the required clamp evidence.

### `effect_mandelbrot`

`op_effect_mandelbrot` at `effects.rs:1686-1719` has zero managed hits: 23
executable lines and four branch arms remain uncovered. No valid public
`PIL.Image.effect_mandelbrot` input can reach this function in the current
architecture.

The public path is:

```text
PIL.Image.effect_mandelbrot
  -> _core.image_effect_mandelbrot
  -> ops::module_fns::effect_mandelbrot
  -> Image::frombytes
```

`ops::module_fns::effect_mandelbrot` owns the public implementation and its
validation. Although `PipelineOp::EffectMandelbrot` and a registry closure
exist, no public constructor pushes that pipeline operation. The existing
manifest cases therefore exercise `module_fns.rs`, not the dead helper in
`effects.rs`. Adding a parity fixture would only duplicate already-covered
public behavior and could not change this file's coverage.

Covering these lines would require a deliberate runtime refactor to route the
public operation through the registry, or a direct internal Rust test. Neither
is a valid input-only parity change for this worker. The current public
contract also rejects negative extents and qualities below two before the
pipeline, so malformed/error stimuli are excluded.

## Verification

The maintained public CPU batch contained 22 valid effect cases:

```text
22 selected / 22 executed / 22 passed / 0 failed / 0 infrastructure errors
```

`make migration-parity-inputs-check` was also run. It reached the repository's
existing duplicate-accounting test but failed because the checkout has no
legacy fixture directories (`AssertionError: 0 != 1592`). No effect input or
fixture was changed by this worker.

## Result

No parity input can honestly improve this bounded file at the requested
baseline. The remaining effect-specific gap is the unreachable
`op_effect_mandelbrot` helper described above; the other uncovered lines in
`effects.rs` belong to unrelated operations and are outside this batch.
