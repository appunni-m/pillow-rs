# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: cb07746491b0906acb4eee0d6becbee202dfd70b5389883e7cd98a1a352f7ffc
lane: parity
```

## Evidence state

- Compatible evidence IDs: `migration-parity-7727653953554344a085234f2c60cfad`
- Operation outcomes: fail=102, pass=102
- Stale/incompatible artifacts: 0

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 725 | 1181 | `migration-parity-7727653953554344a085234f2c60cfad` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
