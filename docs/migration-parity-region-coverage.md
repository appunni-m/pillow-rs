# Migration parity region coverage

This is a generated coverage view. The metric is **region coverage**
(covered regions / total regions) from the maintained merged lane; it is
not parity proof and does not change the manifest or lane inputs.

```yaml
generator: scripts/report_migration_parity_region_coverage.py@1
manifest_path: pillow-rs/tests/fixtures/manifest.yaml
manifest_schema: migration-parity/manifest@2
manifest_sha256: 69af3ac55db9c2c6161d44c3fd5e4755e34d1941e66aee76af14bcc5096e0c28
coverage_run_id: migration-coverage-23ce833fdd3345b89cb93a58c81792d3
coverage_target_profile: python-cpu
metric: region
threshold: below 90%
```

Each operation's coverage is the region coverage of the files declared
by its coverage component(s); operations inside one component share the
component's measured coverage by design.

## PIL.Image.Image.getbbox

`PIL.Image.Image.getbbox -> region coverage 7988/8872 (90.0%)`

## Operations below 90% region coverage

3 of 205 coverage-required operations are below 90%.

| Operation | Component(s) | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `PIL.ImageSequence.Iterator` | `image-sequence` | 3562/4099 | 86.9% |
| `PIL.ImageSequence.Iterator.__iter__` | `image-sequence` | 3562/4099 | 86.9% |
| `PIL.ImageSequence.Iterator.__next__` | `image-sequence` | 3562/4099 | 86.9% |

## Per-file region coverage for involved components

| Component | File | Region coverage | Percent |
| --- | --- | ---: | ---: |
| `image-sequence` | `pillow-rs/src/image.rs` | 3562/4099 | 86.9% |
| `image-sequence` | `pillow-rs-py/python/pillow_rs/imagesequence.py` | 0/0 | n/a |

