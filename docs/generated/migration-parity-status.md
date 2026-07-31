# Migration parity status

This is a generated evidence view. It never changes the manifest or
lane inputs, and it does not turn missing evidence into a pass.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: dd171a9c7823b663abcfb46c9953694a67e888448de5027682b84b963347517f
lane: parity
```

## Evidence state

- Compatible evidence IDs: `migration-parity-db7fb82474a44a9ab737d803a1c793ab`
- Operation outcomes: pass=204
- Stale/incompatible artifacts: 1

| Dimension | Target profile | Covered | Total | Evidence ID |
| --- | --- | ---: | ---: | --- |
| `parity_outcome` | `python-cpu` | 1190 | 1190 | `migration-parity-db7fb82474a44a9ab737d803a1c793ab` |

## Interpretation

- `pass` and measured counts are evidence from a compatible run.
- `not_proven` means the specification exists but the required fresh
  evidence is absent, stale, dirty, or not ingested.
- Static operation support is not a substitute for live parity.
