# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 87c72ef021853c67e68f6f282040bdb1dac0b1332564041bebe0be7d7f70e4d3
lane: parity
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=209
- Stale/incompatible artifacts: 3

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 0 | 11272 | `not_proven` |
| `parity_outcome` | `python-simd` | 0 | 399 | `not_proven` |
| `parity_outcome` | `python-gpu` | 0 | 399 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
