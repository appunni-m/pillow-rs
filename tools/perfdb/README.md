# pillow-perfdb

`pillow-perfdb` is a standalone Rust CLI and DuckDB store for the benchmark
evidence produced by pillow-rs. It imports public benchmark snapshots, the
Pillow-SIMD cohort, generated per-case and per-operation matrices, local run
ledgers, and raw benchmark-result JSON receipts. Each source artifact is
identified by its SHA-256; imported rows retain their source row, runner,
revision, workload, policy, parity, backend, fallback, and latency fields.

This is a separate Cargo workspace. It does not add DuckDB or CLI dependencies
to the image library workspace.

## Build

```sh
cargo build --release --manifest-path tools/perfdb/Cargo.toml
```

The bundled DuckDB library is built into this CLI. `Cargo.lock` pins the
complete standalone dependency graph.

## Seed the current evidence set

From the pillow-rs repository root:

```sh
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- \
  --db tools/perfdb/performance-results.duckdb \
  seed --evidence-dir docs/evidence
```

`seed` imports all top-level `.json` and `.csv` files in the evidence directory
in a stable order (JSON before CSV so matrix rows can use snapshot machine
metadata). Re-importing an identical file is idempotent. Files with unfamiliar
schemas are retained as raw artifact records instead of being discarded.

The default database is `tools/perfdb/performance-results.duckdb`; pass
`--db PATH` to keep a different database. DuckDB files and WALs are local state
and are ignored by Git. Commit source evidence snapshots and the generated
matrix CSVs as the durable, reviewable input history; the database can be
recreated by importing those files.

## Import more evidence

```sh
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- \
  --db tools/perfdb/performance-results.duckdb \
  import docs/evidence/benchmark.json build/migration-parity/benchmark-result.json
```

`import` accepts JSON/CSV files or directories. Directory imports are
recursive. A changed artifact is appended as a new immutable source; rows are
never overwritten by a later run.

## Inspect and compare

```sh
# Artifact, source-category, and row counts
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- status

# Verified CPU cases slower than Pillow, grouped by operation
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- rank \
  --profile python-cpu --baseline Pillow --below 1.0

# SIMD cases not yet 2x Pillow-SIMD
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- rank \
  --profile python-simd --baseline Pillow-SIMD --below 2.0

# Full timeline for a workload/profile; use --source-kind to select one feed
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- history \
  --workload pil-image-image.getchannel.materialized.rgb-1024x768 \
  --profile python-cpu

# Pair case rows from two artifact SHA prefixes, requiring a matching workload,
# policy/context, and verified runner signature
cargo run --release --manifest-path tools/perfdb/Cargo.toml -- compare \
  --workload pil-image-image.getchannel.materialized.rgb-1024x768 \
  --profile python-cpu --source-kind case-matrix \
  --from <old-artifact-sha-prefix> --to <new-artifact-sha-prefix>
```

`rank` only accepts `parity_backend_verified` case-matrix rows with complete
terminal materialization, the expected backend, and no fallback. Supported
baselines are `Pillow`, `Pillow-SIMD`, and `SIMD` (the GPU latency comparison).
The source matrix's verified ratios are used as published; observed-only rows
are not upgraded into verified evidence. A comparison reports an unpaired or
unverified-runner result instead of inventing a ratio when workload policy,
context, or exact runner identity cannot be matched.

`status` reports counts by source category, pipeline/operation scope, target
profile, and evidence state. It also shows whether the case matrix contains
GPU sustained-throughput measurements or only a not-measured status.

For snapshots that do not carry a per-workload fixture hash, the comparison
signature can verify workload ID, mode/size, policy/context, and machine
metadata, but cannot prove that the underlying fixture bytes were unchanged.
That limitation remains visible in the source record; it is not treated as
stronger evidence than the source format supplies.

## Data model

- `artifacts` records the original file hash, source path, format/schema,
  revision, measured time, metadata, and number of imported rows.
- `observations` stores normalized query columns alongside each exact source
  row as JSON. The observation ID is content-addressed.
- `artifact_rows` preserves each artifact's row ordinal and links repeated
  observations back to every artifact in which they appeared.

This keeps source categories distinct while allowing per-workload history and
strictly paired comparisons. The database is a derived index; source artifacts
remain the authority.
