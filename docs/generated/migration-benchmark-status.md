# Migration benchmark status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: dd739ebe46ccc6cdd1ab01c610082db2fbdd7efd2d347efa4c3f8852a1f3d937
lane: benchmark
```

## Evidence state

- Compatible evidence IDs: `migration-benchmark-9bf4a5f25b774d479b1bb771481aabf7`
- Operation outcomes: not_proven=47, pass=156
- Stale/incompatible artifacts: 1

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `benchmark_input_mapping` | `python-cpu` | 203 | 203 | `not_proven` |
| `benchmark_budget_outcome` | `python-cpu` | 0 | 203 | `migration-benchmark-9bf4a5f25b774d479b1bb771481aabf7` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
