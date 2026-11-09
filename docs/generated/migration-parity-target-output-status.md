# Migration target-output status

This is a generated record of Rust-only output and error contracts.
These cases are checked against recorded expectations and are kept
separate from Pillow parity and injected-fault results.

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
| `python-cpu` | pass | 20 | 20 | 0 | `migration-parity-929d200d41e24d748fef68c0235026dd` |
| `python-simd` | not_proven | 20 | 0 | 0 | `not_proven` |
| `python-gpu` | not_proven | 20 | 0 | 0 | `not_proven` |
| `python-parallel-cpu` | not_proven | 20 | 0 | 0 | `not_proven` |
| `python-gpu-fault-contract` | not_proven | 20 | 0 | 0 | `not_proven` |

## Recorded expectations

| Case ID | Operation | Mode | Size | Recorded target result |
| --- | --- | --- | --- | --- |
| `PIL.Image.frombytes.expected-output.core-only-luma-a16` | `PIL.Image.frombytes` | `ImageLumaA16` | `[1, 1]` | `{"bytes":"NBJ4Vg==","format":null,"info":{},"kind":"image","mode":"ImageLumaA16","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-luma-a16.empty-height` | `PIL.Image.frombytes` | `ImageLumaA16` | `[1, 0]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageLumaA16","palette":null,"size":[1,0]}` |
| `PIL.Image.frombytes.expected-output.core-only-luma-a16.empty-width` | `PIL.Image.frombytes` | `ImageLumaA16` | `[0, 1]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageLumaA16","palette":null,"size":[0,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-luma-a16.short-data` | `PIL.Image.frombytes` | `ImageLumaA16` | `[1, 1]` | `{"error":{"class":"ValueError","code":null,"kind":"invalid_argument","message":"not enough image data","stage":"call"},"kind":"expected_error","mode":"ImageLumaA16","size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb16` | `PIL.Image.frombytes` | `ImageRgb16` | `[1, 1]` | `{"bytes":"NBJ4Vrya","format":null,"info":{},"kind":"image","mode":"ImageRgb16","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb16.empty-height` | `PIL.Image.frombytes` | `ImageRgb16` | `[1, 0]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgb16","palette":null,"size":[1,0]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb16.empty-width` | `PIL.Image.frombytes` | `ImageRgb16` | `[0, 1]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgb16","palette":null,"size":[0,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb16.short-data` | `PIL.Image.frombytes` | `ImageRgb16` | `[1, 1]` | `{"error":{"class":"ValueError","code":null,"kind":"invalid_argument","message":"not enough image data","stage":"call"},"kind":"expected_error","mode":"ImageRgb16","size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb32f` | `PIL.Image.frombytes` | `ImageRgb32F` | `[1, 1]` | `{"bytes":"AACgPwAAAL8AAEBA","format":null,"info":{},"kind":"image","mode":"ImageRgb32F","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb32f.empty-height` | `PIL.Image.frombytes` | `ImageRgb32F` | `[1, 0]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgb32F","palette":null,"size":[1,0]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb32f.empty-width` | `PIL.Image.frombytes` | `ImageRgb32F` | `[0, 1]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgb32F","palette":null,"size":[0,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgb32f.short-data` | `PIL.Image.frombytes` | `ImageRgb32F` | `[1, 1]` | `{"error":{"class":"ValueError","code":null,"kind":"invalid_argument","message":"not enough image data","stage":"call"},"kind":"expected_error","mode":"ImageRgb32F","size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba16` | `PIL.Image.frombytes` | `ImageRgba16` | `[1, 1]` | `{"bytes":"NBJ4Vrya8N4=","format":null,"info":{},"kind":"image","mode":"ImageRgba16","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba16.empty-height` | `PIL.Image.frombytes` | `ImageRgba16` | `[1, 0]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgba16","palette":null,"size":[1,0]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba16.empty-width` | `PIL.Image.frombytes` | `ImageRgba16` | `[0, 1]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgba16","palette":null,"size":[0,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba16.short-data` | `PIL.Image.frombytes` | `ImageRgba16` | `[1, 1]` | `{"error":{"class":"ValueError","code":null,"kind":"invalid_argument","message":"not enough image data","stage":"call"},"kind":"expected_error","mode":"ImageRgba16","size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba32f` | `PIL.Image.frombytes` | `ImageRgba32F` | `[1, 1]` | `{"bytes":"AACgPwAAAL8AAEBAAABAPw==","format":null,"info":{},"kind":"image","mode":"ImageRgba32F","palette":null,"size":[1,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba32f.empty-height` | `PIL.Image.frombytes` | `ImageRgba32F` | `[1, 0]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgba32F","palette":null,"size":[1,0]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba32f.empty-width` | `PIL.Image.frombytes` | `ImageRgba32F` | `[0, 1]` | `{"bytes":"","format":null,"info":{},"kind":"image","mode":"ImageRgba32F","palette":null,"size":[0,1]}` |
| `PIL.Image.frombytes.expected-output.core-only-rgba32f.short-data` | `PIL.Image.frombytes` | `ImageRgba32F` | `[1, 1]` | `{"error":{"class":"ValueError","code":null,"kind":"invalid_argument","message":"not enough image data","stage":"call"},"kind":"expected_error","mode":"ImageRgba32F","size":[1,1]}` |
