# Testing a Single Feature

The full parity suite defines **6,314 explicit concrete cases**. The current
baseline has 6,302 runnable comparisons and 12 visible pending cases. After the
test binary is built, the normal full comparison takes about five seconds with
the current fixture set. When debugging a specific failure bucket, filter to a
single operation or case.

## Runtime Estimates

| Target | Scope | Expected comparison time after build |
|---|---|---:|
| `make test` | all 6,314 explicit cases | ~5 s |
| `make test-op OP=load_glyph` | explicit `load_glyph` cases only | <5 s |
| `make test-op OP=render_glyph` | explicit `render_glyph` cases only | <5 s |
| `make test-case CASE=...` | matching case/subject IDs only | <5 s |
| `make test-pending-case CASE=...` | one exact pending `case_id` promoted into comparison | <5 s |
| `make test-list` | selection diagnostic, no comparison | <1 s |

The oracle cache lives under `tests/fixtures/outputs/unified_oracle_cache/`. Its
key includes selected inputs, fixture content hashes, the oracle binary, and
linked FreeType. `FONTDONE_UNIFIED_ORACLE_REFRESH=1` bypasses it.

## Quick Start

```bash
# Run only ftadvanc.get_advance cases (refreshes oracle each run — no cache)
make -C pillow-rs-freetype test-op OP=ftadvanc.get_advance

# Run only explicit load_glyph cases
make -C pillow-rs-freetype test-op OP=load_glyph

# Run only cases whose case_id/subject/case matches a substring
make -C pillow-rs-freetype test-case CASE=freetype.set_transform

# Diagnostic: promote exactly one pending route into the runtime comparison.
# This is expected to fail until the implementation is fixed; use it instead
# of editing route-audit classification by hand.
make -C pillow-rs-freetype test-pending-case CASE=ftglyph.FT_Glyph_To_Bitmap.pending_stroked_mono_target_outline_to_bitmap

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
| `FONTDONE_UNIFIED_INCLUDE_PENDING_CASE` | Exact `case_id` of one pending route to compare anyway |
| `FONTDONE_UNIFIED_CASE_LIMIT`        | Stop after N selected concrete cases                  |
| `FONTDONE_UNIFIED_ORACLE_REFRESH`    | If set, skip oracle cache; re-run C oracle           |
| `FONTDONE_UNIFIED_SELECTION_ONLY`    | Print case selection summary, don't run comparisons  |
| `FONTDONE_UNIFIED_PROFILE`           | Print per-stage timing (ns/ms)                       |
| `FONTDONE_UNIFIED_ORACLE`            | Override path to the C oracle binary                 |

## Example Workflow: Debugging a Failure Bucket

1. **Identify the failing operation.**  Run `make test-list-ops` to see all
   operation buckets with pending/rejected counts.

2. **Filter to the target operation.**  Use `make test-op OP=...` to run only
   those cases.

3. **Narrow further with case filter.** If you know a specific case, subject,
   font name encoded in the case ID, or parameter-specific variant ID, use:
   ```bash
   make test-case CASE=recursive-composite
   ```

4. **Promote one pending route only when tracing a known blocker.** Use
   `make test-pending-case CASE=<exact-case-id>` when a route is intentionally
   pending but the oracle and runner can execute it. This keeps the normal
   route audit truthful while producing a real C-vs-Rust failure without
   editing `scripts/check_public_api_inputs.py`.

5. **Select one explicit case.** Use `make test-case CASE=...` rather than
   changing the input set or applying a hidden cardinality limit.

6. **Profile slow cases.** Enable profiling to find bottlenecks:
   ```bash
   FONTDONE_UNIFIED_PROFILE=1 \
   make test-op OP=render_glyph
   ```

## Cache Files on Disk

```text
tests/fixtures/outputs/unified_oracle_cache/
|-- 44c6...1c5a3.jsonl   # one selected full-suite input set
|-- 56b1...8c68ca.jsonl   # one OP=load_glyph selection
`-- 91a9...55e53.jsonl   # one ftadvanc.get_advance selection
```

Each `.jsonl` file contains one JSON line per unique oracle invocation.

## Relationship to `make -C pillow-rs-freetype test`

The full-suite `make -C pillow-rs-freetype test` target does **not** set
`FONTDONE_UNIFIED_ORACLE_REFRESH`, so it uses and updates the cache for the
full case set.  Filtered runs (`test-op`, `test-case`) never touch the
full-suite cache file because they produce a different cache key.
