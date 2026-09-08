# Migration coverage status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 7fb0ed379ee29f57ed979dde9a20dbc9ab5c8aa957465210eab3f28836428597
lane: coverage
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=207
- Stale/incompatible artifacts: 3

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `coverage_input_mapping` | `python-cpu` | 1593 | 1593 | `not_proven` |
| `coverage_input_mapping` | `python-simd` | 0 | 0 | `not_proven` |
| `coverage_input_mapping` | `python-gpu` | 0 | 0 | `not_proven` |
| `function_coverage` | `python-cpu` | 0 | 0 | `not_proven` |
| `function_coverage` | `python-simd` | 0 | 0 | `not_proven` |
| `function_coverage` | `python-gpu` | 0 | 0 | `not_proven` |
| `line_coverage` | `python-cpu` | 0 | 0 | `not_proven` |
| `line_coverage` | `python-simd` | 0 | 0 | `not_proven` |
| `line_coverage` | `python-gpu` | 0 | 0 | `not_proven` |
| `branch_coverage` | `python-cpu` | 0 | 0 | `not_proven` |
| `branch_coverage` | `python-simd` | 0 | 0 | `not_proven` |
| `branch_coverage` | `python-gpu` | 0 | 0 | `not_proven` |
| `region_coverage` | `python-cpu` | 0 | 0 | `not_proven` |
| `region_coverage` | `python-simd` | 0 | 0 | `not_proven` |
| `region_coverage` | `python-gpu` | 0 | 0 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
