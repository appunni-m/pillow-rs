# Migration benchmark status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@3
manifest_sha256: c961cb80ace865f0ca092c3648bb86c1417638402efb7480f872a6085b027cee
lane: benchmark
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=222
- Stale/incompatible artifacts: 0

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `benchmark_input_mapping` | `python-cpu` | 211 | 213 | `not_proven` |
| `benchmark_input_mapping` | `python-simd` | 5 | 5 | `not_proven` |
| `benchmark_input_mapping` | `python-gpu` | 4 | 4 | `not_proven` |
| `benchmark_input_mapping` | `python-parallel-cpu` | 0 | 0 | `not_proven` |
| `benchmark_input_mapping` | `python-gpu-fault-contract` | 0 | 0 | `not_proven` |
| `benchmark_budget_outcome` | `python-cpu` | 0 | 213 | `not_proven` |
| `benchmark_budget_outcome` | `python-simd` | 0 | 5 | `not_proven` |
| `benchmark_budget_outcome` | `python-gpu` | 0 | 4 | `not_proven` |
| `benchmark_budget_outcome` | `python-parallel-cpu` | 0 | 0 | `not_proven` |
| `benchmark_budget_outcome` | `python-gpu-fault-contract` | 0 | 0 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
