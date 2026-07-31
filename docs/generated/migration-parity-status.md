# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 58cdf3e032fae34a6a44abac37df42133aade3f1a3714aa9227cb6c881e7f290
lane: parity
```

## Evidence state

- Compatible evidence IDs: `migration-parity-48088e3a4cb64b7d9b95498ba05e7ee0`
- Operation outcomes: pass=204
- Stale/incompatible artifacts: 1

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 1181 | 1181 | `migration-parity-48088e3a4cb64b7d9b95498ba05e7ee0` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
