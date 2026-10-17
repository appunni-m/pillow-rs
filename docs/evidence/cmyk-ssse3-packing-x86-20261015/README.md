# CMYK-to-RGB SSSE3 packing — native x86, 2026-10-15

## Decision

Keep the x86 SSSE3 full-block packing change staged as a partial candidate. The
1024×768 CMYK conversion-and-bytes boundary is **19.37% faster** across the
A–B–B–A bracket. Candidate medians are 4.640 and 4.570 ms, both below both
controls (5.880 and 5.543 ms). Same-run Pillow speedups improve from
1.483–1.582× to **1.797–1.799×**. This remains below the user's 2× milestone
and repository's 5× SIMD target. Convert remains open; operation batch is 0/33.

The helper replaces sixteen RGB triplet appends with one packed 48-byte append.
It reuses existing SSSE3 shuffle masks in the shared native layer. Arithmetic,
rounding, output allocation, partial-block handling, CPU, NEON, GPU, and existing
Merge paths are unchanged. Runtime SSSE3 admission happens before the loop.
Astra's read-only safety review found no defect; the native tests exercise the
new helper on a runner with SSSE3. The non-SSSE3 fallback was reviewed but could
not be forced on this runner.

## Frozen scope and results

Only two original workload IDs ran, with SIMD and regular Pillow 12.2.0,
release build, warm cache, five warmups, 20 iterations × five samples,
concurrency one. All 100 measured samples per subject and cohort are retained.
Image construction is outside both selected timers; output allocation and
conversion/export work are inside. No GPU, Parallel CPU, full image campaign,
or coverage ran. Throughput statistics are per-call reciprocals at concurrency
one, not a sustained GPU measurement.

| Boundary / subject (ms) | A1 control | B1 candidate | B2 candidate | A2 restored control |
| --- | ---: | ---: | ---: | ---: |
| Explicit convert + tobytes / Pillow | 9.300171 | 8.344968 | 8.211406 | 8.220940 |
| Explicit convert + tobytes / SIMD | 5.879945 | 4.639881 | 4.569821 | 5.542600 |
| Call-selected observed image / Pillow | 8.580579 | 8.176415 | 8.086119 | 8.176175 |
| Call-selected observed image / SIMD | 5.806239 | 4.624862 | 4.608881 | 5.706999 |

- `pipeline-op.convert-mode-cmyk-to-rgb-1024x768`: explicit `call;materialize`.
  Mean cohort median: 5.711272 → 4.604851 ms, −19.37%. Mean same-run
  Pillow speedup improves 17.31%.
- `pipeline-chain.convert.material-cmyk-to-rgb-1024x768`: `call` selected.
  The adapter implicitly calls `tobytes()` for the observed image before
  ending that timer. The later named `materialize` step is outside it.
  This is not an unmaterialized lazy-call measurement or whole-workflow timer.
  Mean cohort median: 5.756619 → 4.616872 ms, −19.80%; candidate speedups
  1.754–1.768× Pillow.

Pillow shifted between cohorts. The candidate still beats both controls in
absolute latency and same-run Pillow ratio for each boundary. The complete
16-row [results matrix](results-matrix.csv) is also appended to the local run
ledger (7,631 data rows). These dirty native trial rows do not replace the clean
published matrix or prove performance on every mode, size, architecture or path.

## Parity and actual execution

Each source variant passed exactly these SIMD cases:

- `PIL.Image.Image.convert.nuanced.material-cmyk-to-rgb-1024x768`
- `PIL.Image.Image.convert.nuanced.mode-audit-CMYK-to-RGB-17x3`

B2 reused the exact B1 proof after matching source, extension, inputs, producer,
oracle and profile identities. A2 refreshed only the full-size gate because the
versioned driver now explicitly pins and records both source interpreters.
The small restored-control guard reuses unchanged A1 evidence. Three full-size
parity preflights plus two small guards performed five fresh comparisons total.
All passed. Exact original compressed proofs are retained under `parity/`;
summary files record their uncompressed SHA-256 values.

Every boundary/cohort has an adjacent untimed execution proof: actual SIMD,
terminal complete, no fallback, `Convert` on `vector`, **49,152 vector blocks**,
zero scalar tail. These original events are retained in the JSONL files and
serialized into the recovered reports. Timing samples themselves were collected
without receipt tracing. Legacy resource counters are retained as reported;
zero allocation counters do not establish allocation-free execution.

Two focused Rust tests passed: channel order across output blocks and CMYK
rounding/length/vector-tail behavior at 0, 1, 15, 16, 17, 31, 32 and 33 pixels,
including component/K extremes and rejected layouts. Their first compile failed
because this sparse runner lacked two unrelated `include_bytes!` assets. Exactly
713 bytes of existing JPEG/WebP fixtures were copied, then the same two-test
filter passed (574 other tests filtered out). No assertion or threshold changed.

## Genuine benchmark reporting defect and recovery

The native producer grouped execution by timing policy but indexed final timing,
phase, execution, status, error and repeat-count data only by parity case ID.
These two workloads share a case with different boundaries: the second group
silently overwrote the first report row. For example, original A1 reported the
call-selected 8.580579/5.806239 ms values for both rows, although its retained
explicit-boundary raw values were 9.300171/5.879945 ms.

The four `*.original.json.txt` files preserve those reports byte-for-byte. The
text suffix keeps automatic evidence-directory imports from ranking the flawed
reports. Use **`*.recovered.json`** for this trial. `recovery.json` records the
original, raw and recovered hashes. `recover_reports.py` reconstructs rows from
the exact child command boundaries, discards exactly five warmups and uses the
existing aggregation functions. It binds each proof to the immediately preceding
target timing invocation. No image rerun or inferred sample was used.

Root fixed the current producer to retain all fields by workload identity and
deduplicate same-case/same-policy aliases while unioning requested child
profiles. Five regression tests live in the existing Make-enforced selection
test module; they cover capture on/off, independent boundaries, repeat counts,
late failures, aliases and mixed backend subjects. **34 selection tests and 11
sample tests passed.** Historical reports lacking raw samples remain suspect
where a case spans multiple policies; they cannot be repaired from medians.
This reporting defect is tracked in Kata `gkv8`.

The experiment preparation also found two metadata mistakes before execution:
removing CPU from declared case profiles violated the manifest requirement, and
pre-filtering benchmark subjects violated the old producer's expected contract.
The exact original declarations were restored; existing environment selectors
limited actual execution to SIMD. Failed preparation logs are retained. These
were experiment setup errors, not image parity failures or weakened contracts.

## Source identity and runner preservation

Runner: AMD Ryzen 5 2600X, Ubuntu 24.04.5, native x86_64, Rust 1.96.1,
Python 3.12.3, maturin 1.14.1; default features, parallel disabled. Noninteractive
PATH initially hid existing tools; explicit paths avoided any installation.
The isolated checkout is `/home/appunni/pillow-rs-resize-trial-20261015`, dirty
HEAD `f2b63f559e91022a85cb43f7264bdf8b586a448d`. Existing geometry changes were
held constant. It is not the complete current local HEAD; the two affected source
files matched the local control hashes exactly before applying this narrow patch.
Source-file inventories and oracle digests are in each `*.identity.json`.

Extension SHA-256:

- A1/A2 control: `b31e957d613d43a3ad4738fb33d9f10dfb1a43281c3049bd9895e1f0c630a031`
- B1/B2 candidate: `d2a55c732b52d7882c27612e8c2668143218f15a128b566ca29565a8b91168d1`

The runner is restored to the exact control source and extension. No remote
file was deleted, reset or cleaned. The unreferenced test file, patches, logs
and results remain there. Transfers to the runner were 7,386 bytes of experiment
inputs/scripts/patch plus 713 bytes of missing fixture assets. Retrieved evidence
was 130,483 bytes plus 7,470 bytes of exact compressed parity proofs. No target
directory or binary was transferred.

These inventories and build logs support this bounded dirty-snapshot comparison;
they are not strict in-process build/execution attestations. The local candidate remains staged and uncommitted. Kata trial `1yz4`, parent Convert `dj16`, and master `0h0b`
record the scope. No commit or push; broad pre-push verification remains pending.

## DuckDB admission

The separate pillow-perfdb project imported all four recovered reports (16 rows)
and five exact parity artifacts. Comparing explicit-boundary A1 artifact
`541f9fa9cf76` with B1 `81cb0b3a1dc6` returned **zero paired rows**: the legacy
report identifies its runner only with the label `machine`. The store calculated
no ratio. Its gate remains unchanged. The observed trial deltas above are bounded
manual comparisons with recorded SSH/source/build context, not strict store
admission or accepted operation completion. The candidate stays staged pending
runner/build/execution evidence from the current capture path. Historical native
producer timestamps can also reuse the parity preflight start time; they should
not be treated as exact timing-child launch timestamps.
