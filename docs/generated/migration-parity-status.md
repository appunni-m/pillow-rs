# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 0bbb44ffa8992bb7a5772f65deabf1ebdee9611e59d672477088c25d6705c39e
lane: parity
```

## Evidence state

- Compatible evidence IDs: `migration-parity-5abfbb181b74408f8d6259aedc5c78dd`
- Operation outcomes: pass=204
- Stale/incompatible artifacts: 1

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 1310 | 1310 | `migration-parity-5abfbb181b74408f8d6259aedc5c78dd` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
