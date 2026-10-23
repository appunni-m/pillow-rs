# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: a113f249e059678178cdc13dec599b023c055d682a8794e1762aa8bc066548cc
lane: parity
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=217
- Stale/incompatible artifacts: 2

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 0 | 17312 | `not_proven` |
| `parity_outcome` | `python-simd` | 0 | 6214 | `not_proven` |
| `parity_outcome` | `python-gpu` | 0 | 6214 | `not_proven` |
| `parity_outcome` | `python-parallel-cpu` | 0 | 1 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
