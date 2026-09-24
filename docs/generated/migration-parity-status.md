# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 7a994c4c68188795170580a826faf45afa7c08ea3d75375a3f9fb72d81e57f5f
lane: parity
```

## Evidence state

- Compatible evidence IDs: none
- Operation outcomes: not_proven=209
- Stale/incompatible artifacts: 3

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 0 | 13253 | `not_proven` |
| `parity_outcome` | `python-simd` | 0 | 2380 | `not_proven` |
| `parity_outcome` | `python-gpu` | 0 | 2380 | `not_proven` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
