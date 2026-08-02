# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: e9793ed33f529f1a1eec6f858df78d545f461fd585a1548ad85e016e90cf3234
lane: parity
```

## Evidence state

- Compatible evidence IDs: `migration-parity-ee2e0d5307c244f8ad02b0756a82f08e`
- Operation outcomes: pass=205
- Stale/incompatible artifacts: 2

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 1935 | 1935 | `migration-parity-ee2e0d5307c244f8ad02b0756a82f08e` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
