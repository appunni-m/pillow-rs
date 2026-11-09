# Migration target-output status

This is a generated record of successful Rust-only outputs. These cases
are checked against recorded expectations and are kept separate from
Pillow parity and injected-fault results.

```yaml
generator: scripts/generate_migration_parity_docs.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@3
manifest_sha256: c961cb80ace865f0ca092c3648bb86c1417638402efb7480f872a6085b027cee
verification: expected_output
```

## Backend target profiles

| Target profile | Outcome | Cases | Passed | Failed | Evidence ID |
| --- | --- | ---: | ---: | ---: | --- |
| `python-cpu` | pass | 5 | 5 | 0 | `migration-parity-95b10803e1814e1984181ec8b0e5866f` |
| `python-simd` | not_proven | 5 | 0 | 0 | `not_proven` |
| `python-gpu` | not_proven | 5 | 0 | 0 | `not_proven` |
| `python-parallel-cpu` | not_proven | 5 | 0 | 0 | `not_proven` |
| `python-gpu-fault-contract` | not_proven | 5 | 0 | 0 | `not_proven` |

## Recorded expectations

| Case ID | Operation | Mode | Size | Expected output record |
| --- | --- | --- | --- | --- |
| `PIL.Image.frombytes.expected-output.core-only-luma-a16` | `PIL.Image.frombytes` | `ImageLumaA16` | `[1, 1]` | `{"bytes":"NBJ4Vg==","format":null,"info":{},"kind":"image","mode":"ImageLumaA16","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb16` | `PIL.Image.frombytes` | `ImageRgb16` | `[1, 1]` | `{"bytes":"NBJ4Vrya","format":null,"info":{},"kind":"image","mode":"ImageRgb16","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb32f` | `PIL.Image.frombytes` | `ImageRgb32F` | `[1, 1]` | `{"bytes":"AACgPwAAAL8AAEBA","format":null,"info":{},"kind":"image","mode":"ImageRgb32F","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba16` | `PIL.Image.frombytes` | `ImageRgba16` | `[1, 1]` | `{"bytes":"NBJ4Vrya8N4=","format":null,"info":{},"kind":"image","mode":"ImageRgba16","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba32f` | `PIL.Image.frombytes` | `ImageRgba32F` | `[1, 1]` | `{"bytes":"AACgPwAAAL8AAEBAAABAPw==","format":null,"info":{},"kind":"image","mode":"ImageRgba32F","palette":null,"size":[1,1]}` |
