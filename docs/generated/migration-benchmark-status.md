# Migration benchmark status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: cb07746491b0906acb4eee0d6becbee202dfd70b5389883e7cd98a1a352f7ffc
lane: benchmark
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=203
- Stale/incompatible artifacts: 1

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `benchmark_input_mapping` | `python-cpu` | 203 | 203 | `not_proven` |
| `benchmark_budget_outcome` | `python-cpu` | 0 | 203 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
