# SIMD filters and geometry batch 2

This batch audits the SIMD filter and geometry adapters from the managed
coverage baseline. It intentionally adds no synthetic invalid-operation or
invalid-shape input.

## Baseline

| item | value |
| --- | --- |
| commit | `bc6cfbdfb994f3564bfb6803284d680c290e8a21` |
| suite | `migration-parity-rust-simd` |
| managed snapshot | `17c0e46e-a7c0-4a50-a89b-fd2b0bedaa1f` |
| supplied active SIMD scope | `24,944/30,558` lines |
| `pool_simd/ops/adapters.rs` | `1,125/1,182` lines; `110/156` branches |
| `pool_simd/ops/scalar.rs` | `2,670/2,695` lines; `766/842` branches |

The snapshot reports `40,761/68,614` lines globally because the managed
project report also contains the broader workspace and fontdone source. The
`24,944/30,558` figure is the active SIMD project scope supplied for this
batch.

## Focused parity evidence

The selected public cases cover F-mode max/min/median/rank filters, I-mode
3x3 and 5x5 filters, F-mode resize/thumbnail fallback, and ordinary L/LA/RGBA
SIMD geometry paths (`contain`, `cover`, and `fit`). They were run through
the repository Make target after building the isolated worktree extension.

| backend | selected | executed | passed | failed | infrastructure errors | not run |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| CPU | 16 | 16 | 16 | 0 | 0 | 0 |
| SIMD | 16 | 16 | 16 | 0 | 0 | 0 |

No GPU, crash-quarantine, pending-TIFF, or fontdone lane was run.

## Gap classification

The valid F/I filter dispatch is already reached by the focused SIMD cases.
The remaining adapter entries at the filter dispatch headers and closing
lines (`497/503`, `515/521`, `533/539`, `551/557`, `569/580`, `592/603`, and
`615/617`) are the unselected side of `if let PipelineOp::<expected>`
dispatch. Reaching that side requires calling an adapter with a different
pipeline variant than the registry registered for it; no valid public input
can do that.

The remaining geometry adapter gaps (`812`, `856`, `888`, `920`, `950`, `994`,
`1018`, `1067`, `1084`, `1100`, `1123`, `1159`, and `1176`) are explicit
wrong-operation errors. Their valid operation bodies are exercised by the
focused geometry cases. F/I geometry is deliberately routed to the native
CPU geometry implementation because converting those sample domains through
packed RGBA8 would change values; the F-mode fallback cases verify that
public behavior without pretending it is SIMD packed-pixel coverage.

The scalar geometry gaps at `2041`, `2179`, `2231`, `2276`, `2279`, `2282`,
`2285`, `2373`, `2426-2427`, and `2431-2432` are zero-dimension, clamped
destination, or out-of-bounds safety branches. Some are valid edge contracts,
but the existing corpus already exercises the public zero-source and normal
geometry variants. The remaining branch arms require combinations that either
do not produce a distinct public workflow or would be an invalid-shape probe;
they are retained as visible gaps rather than faked with direct kernel tests.

The scalar unknown-transpose fallback at `1872` is an invalid method code and
is intentionally not added to the valid-input corpus.

## Input-check result

`check_migration_parity_inputs.py` confirmed that regenerated parity and crash
quarantine inputs reproduce exactly. The subsequent repository contract test
has one pre-existing failure because deprecated legacy fixture directories are
absent (`test_legacy_duplicate_accounting_is_explicit`: expected `1592`, found
`0`). This batch did not alter fixtures, thresholds, or denominators.

Conclusion: no new parity case is justified for this batch. The existing
public F/I filter and geometry cases are verified on both CPU and SIMD, while
the remaining reported regions are classified for future managed coverage
work.
