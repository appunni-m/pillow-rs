# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 739363bdef369386eadd04d7c653c5f4c3275d24af41b6315c77d7a7db3244e5
lane: parity
```

## Evidence state

- Compatible evidence IDs: `migration-parity-1a33c060f0b84385bb0fc568342c75f0`
- Operation outcomes: pass=204
- Stale/incompatible artifacts: 1

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 1231 | 1231 | `migration-parity-1a33c060f0b84385bb0fc568342c75f0` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
