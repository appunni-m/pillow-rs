# Manifest Review Artifacts

This directory holds temporary reviewed manifest slices before they are merged
into `tests/manifest.yaml`.

Each slice file is named:

```text
manifest.<offset>.<limit>.yaml
```

The file contains replacement YAML entries for that exact zero-based manifest
slice only. Workers must not edit `tests/manifest.yaml` directly.

After review and merge, keep only artifacts that are still active or blocked.

