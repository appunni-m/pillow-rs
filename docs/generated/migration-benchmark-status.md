# Migration benchmark status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@3
manifest_sha256: 4f615d2cf2e203e385ec44ae8869388ee6bd69afb45dedcf736b3b60a619defb
lane: benchmark
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=220
- Stale/incompatible artifacts: 3

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `benchmark_input_mapping` | `python-cpu` | 209 | 211 | `not_proven` |
| `benchmark_input_mapping` | `python-simd` | 5 | 5 | `not_proven` |
| `benchmark_input_mapping` | `python-gpu` | 4 | 4 | `not_proven` |
| `benchmark_input_mapping` | `python-parallel-cpu` | 0 | 0 | `not_proven` |
| `benchmark_input_mapping` | `python-gpu-fault-contract` | 0 | 0 | `not_proven` |
| `benchmark_budget_outcome` | `python-cpu` | 0 | 211 | `not_proven` |
| `benchmark_budget_outcome` | `python-simd` | 0 | 5 | `not_proven` |
| `benchmark_budget_outcome` | `python-gpu` | 0 | 4 | `not_proven` |
| `benchmark_budget_outcome` | `python-parallel-cpu` | 0 | 0 | `not_proven` |
| `benchmark_budget_outcome` | `python-gpu-fault-contract` | 0 | 0 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
