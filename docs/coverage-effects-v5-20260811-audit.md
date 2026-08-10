# Effects coverage audit — 2026-08-11

Baseline: CPU managed snapshot `77df4e0e-41a2-49a5-ad76-fc6e70d5930c` at
`4f0fe721c`.

For `pillow-rs/src/compute/pool_cpu/ops/effects.rs`, the authoritative CPU
metrics were:

| Metric | Covered / total | Rate |
| --- | ---: | ---: |
| Lines | 1,161 / 1,342 | 86.5127% |
| Branches | 136 / 196 | 69.3878% |
| Functions | 36 / 69 | 52.1739% |
| Regions | 2,361 / 2,680 | 88.0970% |

## Reachability findings

- `op_effect_spread` and `op_effect_noise` are CPU-only registry entries. The
  existing corpus already exercises their normal public paths; the full CPU
  run covers the noise body and the supported spread modes. The remaining
  spread gaps around lines 191–229 are the `I;16`/mask combinations excluded
  from this campaign and defensive fallback paths.
- `op_alpha_composite`, `op_blend_module`, and `op_composite_module` are
  public routes with CPU fallback implementations. Existing RGBA, LA, RGB,
  palette, mask, and ordinary blend cases exercise the behavior branches. The
  remaining gaps at lines 328–329 and 482–483 are size-mismatch errors; adding
  them would be negative/error coverage rather than a new successful behavior
  lane, and the relevant public error cases are already represented elsewhere
  in the corpus.
- `op_effect_mandelbrot` is registered through the GPU entry
  (`compute/registry.rs`, `EffectMandelbrot`). Its uncovered implementation
  body at lines 1686–1719 belongs to the excluded GPU lane; executing it here
  would misrepresent backend coverage.
- Lines 723–846 (`eval`/`point`), 849–1226 (transforms), and 1460–1586
  (`color3dlut`) are unrelated helpers co-located in this extracted source
  unit. Their missing branches are handled by their own public-operation
  coverage buckets, not by ImageOps/effects fixtures.
- Remaining shape-construction errors, unsupported-mode arms, invalid
  parameter exits, and `unreachable!` cases are defensive or contract-invalid
  paths. No valid parity input can reach them without changing the public
  contract.

## Candidate verification

I temporarily tested one observed `effect_noise` case. Its isolated parity
case passed (`1/1`), and the full temporary CPU run passed `3,101/3,101`, but
the authoritative effects-file metrics and global metrics were unchanged.
The candidate was therefore removed; no fixture or denominator change is
justified by that experiment.

Conclusion: this bucket has no measured reachable CPU coverage improvement to
commit. The remaining gap is classified as GPU-only, outside-bucket extracted
code, defensive/error paths, or excluded 16-bit TIFF behavior.
