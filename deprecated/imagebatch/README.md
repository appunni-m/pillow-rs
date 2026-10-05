# Deprecated ImageBatch experiment

Archived on 2026-10-05 from main `66ce7ba04` and its local changes.
This directory preserves the previous experiment. It is outside the Cargo
workspace and Python package roots; its code is not compiled, imported, or
published by the current packages. `PIL.ImageBatch`, the native batch bindings,
and the Rust batch exports have been removed from active source. There is no
compatibility shim. Published tags and packages have not been changed.

The replacement [explicit GPU executor](../../docs/IMAGE_BATCHING.md) is
implemented in current source, with a [working core API example](../../docs/design/gpu_batch_flow.rs)
and [separate validation evidence](../../docs/GPU_BATCH_VALIDATION.md). It is a
distinct API and is not included in the published alpha.5 artifacts.

## Preserved material

- `pillow-rs/src/batch.rs`: former Rust executor and operation wrappers.
- `pillow-rs-py/python/pillow_rs/imagebatch.py`: former Python API.
- `scripts/`: original parity assertions and benchmark workloads.
- `docs/IMAGE_BATCHING.md`: former API guide and measurement claims.
- `integration/python_bindings.rs`: batch-only bindings removed from the mixed
  native binding source.
- `integration/gpu_queue_helpers.rs`, `compute_group_limit.rs`,
  `compute_policy.patch`, and `image_metadata_probes.rs`: batch-only helpers,
  validation policies, and tests extracted from shared runtime files.
- `integration/Makefile` and `cargo_features.toml`: retired fault-test build hooks.
- `integration/COMMANDS.md`: previous command-reference snapshot.
- `integration/pre-archive.patch`: the complete local diff captured before
  archiving, including shared non-batch work. Use it for selective recovery;
  applying it wholesale can overwrite current work.

These are historical sources and integration fragments, not a standalone
buildable package. Their original import paths and Make commands are retired.
Original assertions and thresholds are preserved; old passing results are not
evidence for the replacement design.

The ordinary GPU pipeline planner, kernels, device-limit checks, and shared
image optimizations stay in active source. Internal GPU functions called
`execute_batch`, `prepare_batch`, and `encode_batch` process ordinary pipeline
segments and are not the retired public ImageBatch executor.
