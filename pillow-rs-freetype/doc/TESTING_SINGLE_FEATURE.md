# Testing a Single Feature

The full parity suite runs **3,080,658 cases across 85 subjects** and takes
**~20 minutes** (cold: oracle cache build ~15 min + comparison ~5 min; warm:
~5 min with existing cache).  When debugging a specific failure bucket, you
can filter to a single operation in under a minute.

## Runtime Estimates

| Target                          | Cases        | Cold cache | Warm cache |
|---------------------------------|-------------|-----------|-----------|
| `make test` (full parity)       | 3,080,658   | ~20 min   | ~5 min    |
| `make test-op OP=load_glyph`    | 1,992,206   | ~10 min   | ~5 min    |
| `make test-op OP=render_glyph`  | 336,945     | ~2 min    | ~1 min    |
| `make test-op OP=...` (most ops)| <50,000     | <30 s     | <10 s     |
| `make test-list` (selection)    | —           | ~10 s     | ~10 s     |

The oracle cache (`tests/fixtures/outputs/unified_oracle_cache/`) is ~1.4 GB
full-suite.  It is only invalidated when the oracle binary (`gen_unified_oracle`)
or linked `libfreetype.so` is rebuilt, or when `FONTDONE_UNIFIED_ORACLE_REFRESH=1`
is set.

## Quick Start

```bash
# Run only ftadvanc.get_advance cases (refreshes oracle each run — no cache)
make -C pillow-rs-freetype test-op OP=ftadvanc.get_advance

# Run only load_glyph cases (2M cases — use VARIABILITY_LIMIT to speed up)
FONTDONE_UNIFIED_VARIABILITY_LIMIT=2 make -C pillow-rs-freetype test-op OP=load_glyph

# Run only cases whose case_id/subject/case matches a substring
make -C pillow-rs-freetype test-case CASE=freetype.set_transform

# Diagnostic: print selection summary without executing
make -C pillow-rs-freetype test-list

# Diagnostic: print operation bucket counts without executing
make -C pillow-rs-freetype test-list-ops
```

## Cache Behaviour

### Full-suite runs (`make test`)

The full `make test` (or `make test-unified-fixtures`) creates an oracle cache file
under `tests/fixtures/outputs/unified_oracle_cache/*.jsonl`.  The cache key is a
SHA-256 of:

1. the canonical JSON serialisation of **all selected cases**,
2. the C oracle binary hash + linked `libfreetype.so` hash, and
3. the serialised oracle argv batch.

On the next full-suite run, if the oracle binary hasn't changed and
`FONTDONE_UNIFIED_ORACLE_REFRESH` is **not** set, the cached output is reused,
skipping the C oracle invocation entirely (~15 min saved).

### Filtered runs (`make test-op`, `make test-case`)

When you filter with `FONTDONE_UNIFIED_OPERATION_FILTER` or
`FONTDONE_UNIFIED_CASE_FILTER`, `select_runtime_cases` prunes the case list
**before** `ensure_oracle_cache` is called.  The cache key is computed from
the filtered case set, so filtered runs and full-suite runs produce
**different cache keys** and write to **different cache files**.

| What changes               | Cache behaviour                                 |
|---------------------------|--------------------------------------------------|
| Same filter, same binary   | Cache hit — uses previous filtered cache         |
| Different filter           | New cache file — never overwrites other filters  |
| Oracle binary rebuilt      | Cache miss for all filters — fresh keys          |
| `FONTDONE_UNIFIED_ORACLE_REFRESH=1` | Always bypasses cache (forced by `make test-op`) |

The `make test-op` and `make test-case` targets **always set
`FONTDONE_UNIFIED_ORACLE_REFRESH=1`**, so they re-run the C oracle every time.
This is intentional for local debugging — you want the freshest possible
comparison against the oracle.

## Environment Variable Reference

All env vars are read at runtime by `tests/unified_fixture_parity.rs`.

| Variable                              | Effect                                               |
|--------------------------------------|------------------------------------------------------|
| `FONTDONE_UNIFIED_OPERATION_FILTER`  | Substring match on `case.operation`                  |
| `FONTDONE_UNIFIED_CASE_FILTER`       | Substring match on `case_id`, `subject`, or `case`   |
| `FONTDONE_UNIFIED_CASE_LIMIT`        | Stop after N expanded cases                          |
| `FONTDONE_UNIFIED_VARIABILITY_LIMIT` | Truncate each variability axis to N values           |
| `FONTDONE_UNIFIED_ORACLE_REFRESH`    | If set, skip oracle cache; re-run C oracle           |
| `FONTDONE_UNIFIED_SELECTION_ONLY`    | Print case selection summary, don't run comparisons  |
| `FONTDONE_UNIFIED_PROFILE`           | Print per-stage timing (ns/ms)                       |
| `FONTDONE_UNIFIED_ORACLE`            | Override path to the C oracle binary                 |

## Example Workflow: Debugging a Failure Bucket

1. **Identify the failing operation.**  Run `make test-list-ops` to see all
   operation buckets with pending/rejected counts.

2. **Filter to the target operation.**  Use `make test-op OP=...` to run only
   those cases.

3. **Narrow further with case filter.**  If you know a specific font or
   parameter pattern, combine filters with a raw cargo invocation:
   ```bash
   FONTDONE_UNIFIED_OPERATION_FILTER="load_glyph" \
   FONTDONE_UNIFIED_CASE_FILTER="DejaVuSans" \
   FONTDONE_UNIFIED_ORACLE_REFRESH=1 \
   cargo test --test unified_fixture_parity --locked unified_fixture_parity -- --nocapture
   ```

4. **Reduce variability explosion.**  If the test case expands into too
   many sub-cases, limit the cardinality:
   ```bash
   FONTDONE_UNIFIED_VARIABILITY_LIMIT=2 \
   make test-op OP=load_glyph
   ```

5. **Profile slow cases.**  Enable profiling to find bottlenecks:
   ```bash
   FONTDONE_UNIFIED_PROFILE=1 \
   make test-op OP=render_glyph
   ```

## Cache Files on Disk

```text
tests/fixtures/outputs/unified_oracle_cache/
├── 44c6...1c5a3.jsonl   # cache for full-suite run (~1.4 GB, 2.88M lines)
├── 56b1...8c68ca.jsonl   # cache for OP=load_glyph
├── 91a9...55e53.jsonl   # cache for OP=ftadvanc.get_advance
└── ...
```

Each `.jsonl` file contains one JSON line per unique case.  You can inspect
them directly with `jq` or any JSONL tool.

## Relationship to `make -C pillow-rs-freetype test`

The full-suite `make -C pillow-rs-freetype test` target does **not** set
`FONTDONE_UNIFIED_ORACLE_REFRESH`, so it uses and updates the cache for the
full case set.  Filtered runs (`test-op`, `test-case`) never touch the
full-suite cache file because they produce a different cache key.
