# GPU batch validation and timing

This contributor guide accompanies the [source API](IMAGE_BATCHING.md).

Build without replacing the Pillow oracle, then run the focused lanes:

```sh
RUSTC_WRAPPER= make build-parity PYTHON=.venv/bin/python
make gpu-batch-parity gpu-batch-contracts PYTHON=.venv/bin/python
make gpu-batch-benchmark PYTHON=.venv/bin/python
```

The [parity inputs and lifecycle tests](../scripts/test_gpu_batch.py) compare
mode, dimensions, and complete native terminal pixels with live Pillow in
isolated processes. They include mixed modes/sizes, composed and auxiliary
graphs, alpha ordering, odd widths, and 4096×4096 extraction; `--small` explicitly
omits the large cases. CPU output, resident/download output, and eager execution
are separate runs. Counters assert actual shader work and grouped submissions.
Contract tests cover bounded long streams, duplicate keys, IDs, limits,
unsupported inputs, slow/abandoned consumers, close, held leases, and retry.
They also verify that eager submit waits for its own job with older outputs
still undelivered and that nested metadata mutations cannot alter snapshots.
An early contract-test harness error used the `PIL.Image` module as an
`isinstance` type; it was corrected to `PIL.Image.Image`, retaining the assertion.

Benchmark timing includes fresh graph construction, native upload, execution,
CPU output materialization, and terminal byte export. Warmed queued/eager GPU
throughput is compared with normal sequential Pillow; no parallel Pillow
baseline or host thread pool is used. Results are printed, not saved as JSON
artifacts. The repository-wide performance targets remain separate.

## Focused results, 2026-10-05

The checkout built with `RUSTC_WRAPPER= make build-parity
PYTHON=.venv/bin/python`. The following checks ran against that extension on a
local Metal device with Python 3.12.13 and the live Pillow 12.2.0 oracle; these are
source-workspace results, not release or CI results.

| Command / lane | Result | Shader dispatches | Submissions | Largest submission |
| --- | --- | ---: | ---: | ---: |
| `make gpu-batch-parity`: queued CPU outputs | 252/252 exact Pillow comparisons | 631 | 33 | 8 jobs |
| Same target: queued resident outputs, downloaded for comparison | 252/252 exact Pillow comparisons | 631 | 33 | 8 jobs |
| Same target: eager CPU outputs | 252/252 exact Pillow comparisons | 631 | 252 | 1 job |
| `make gpu-batch-contracts` | 9/9 lifecycle/admission tests | — | — | — |

Each parity lane uploaded 101,031,490 native bytes. CPU-output lanes copied
33,764,416 terminal bytes; resident scheduling copied zero terminal image bytes
to the host. Its explicit downloads still transferred pixels for the oracle
comparison. Charged GPU/host bytes, live jobs, and in-flight work were zero after
each fully consumed lane. The two 4096×4096 channel cases ran in every lane.

`RUSTC_WRAPPER= cargo test -p pillow-rs --lib
compute::pool_gpu::stream::tests` passed three pure boundary/ownership/LUT tests.
`RUSTC_WRAPPER= cargo test -p pillow-rs --lib
native_byte_paste_planner_checks_l_la_rgb_and_adapter_boundaries` passed the
shared planner test. These unit checks supplement the live comparisons.
The Rust API example in `docs/design/gpu_batch_flow.rs` also typechecked against
the built core library. No coverage campaign, injected device-fault campaign,
repository-wide parity run, CI run, release, or push forms part of this evidence.

Documentation lint and the strict site build passed (`make docs-lint docs-build
PYTHON=.venv/bin/python`). `make docs-test PYTHON=.venv/bin/python` passed all 44
tests after correcting a stale workload-catalog assertion: it pinned 655
workloads / 193 individual operations, while the committed HEAD catalog already
contains 657 / 195 (both contain 462 pipelines). No workload, category, or parity
expectation was removed. This corrects a genuine stale test, not an
implementation mismatch.

## Completed-work throughput

Release preparation also exposed two stale telemetry test setups. Concurrent
workers enabled telemetry only in their parent, although
`Backend::set_pipeline_telemetry_enabled` is explicitly thread-local; each
worker now enables it and retains every receipt, byte, ownership, and backend
assertion. The readback test expected RGBA-sized transfers for RGB duplication
even though the existing native byte route transfers aligned three-byte input
and output. For its 5×3 input, both counters now assert exactly 48 bytes rather
than 60. Other operation expectations, pixel equality, ownership, and
no-fallback assertions remain intact.

`make gpu-batch-benchmark PYTHON=.venv/bin/python` measured invert → median(3)
with 32 fresh images per window, two warmup windows, and the median of five
windows. GPU settings were `max_jobs=16`, `max_in_flight=2`; all native outputs
matched the isolated Pillow baseline. Rates include CPU output and byte export.

| Mode / size | Pillow images/s | Eager GPU images/s | Queued GPU images/s | Queued / Pillow | Queued / eager |
| --- | ---: | ---: | ---: | ---: | ---: |
| L 256² | 452.3 | 466.3 | 3878.6 | 8.58× | 8.32× |
| L 1024² | 40.5 | 394.9 | 1304.9 | 32.22× | 3.30× |
| RGB 256² | 165.1 | 528.3 | 2975.9 | 18.03× | 5.63× |
| RGB 1024² | 9.8 | 333.7 | 592.3 | 60.52× | 1.77× |

These are the repeated final-run values. The first final run was noisy,
especially for L 256²: Pillow/eager/queued rates were 93.7/181.3/1768.5 images/s.
Its L 1024² rates were 41.1/504.1/1236.3, RGB 256² were
160.0/536.6/3024.5, and RGB 1024² were 9.8/279.9/589.2. Both runs checked all
outputs and showed higher queued throughput than eager execution, but the
small-image timing variability prevents a general speed guarantee.

Earlier eager-versus-queued timings are superseded: review found that eager
submit could return before its own job completed when an older result was
ready. A dedicated regression now asserts completion after each submit without
draining older outputs. Grouped submission is not proof of concurrent kernel
execution, and the executor's defaults are not a universal optimum.

## Completed single-image latency

The same `benchmark_gpu_batch.isolated` helper measured one fresh image per
window, two warmups, and 15 samples; every terminal output matched Pillow.
The script's `--images 1 --samples 15` options reproduce the comparison
(its main table displays rates; invert those rates to obtain milliseconds).

| Mode / size | Pillow ms | Eager GPU ms | Queued GPU ms |
| --- | ---: | ---: | ---: |
| L 256² | 1.690 | 1.714 | 1.724 |
| L 1024² | 25.421 | 3.400 | 3.448 |
| RGB 256² | 5.798 | 1.739 | 1.738 |
| RGB 1024² | 90.014 | 4.026 | 2.592 |

Small L latency was slightly slower than Pillow. These local diagnostics do not
establish the all-operation CPU/SIMD/GPU performance goals or GPU/SIMD latency
parity. They measure only this pipeline, these modes/sizes, warm shader caches,
and this machine; explicit executor overhead is included.

## Alpha.6 release preparation gate, 2026-10-05

As of this evidence date, the source candidate was `12.2.0-alpha.6`, while
alpha.5 remained the latest accepted release. Alpha.6 was partially published
and failed a packaged-crate font check; see the
[release evidence](REGISTRY_RELEASE_MATRIX.md#alpha6-partial-publication-2026-10-05).
The later stable 12.2.1 release and current registry state are recorded in the
[current release entry](REGISTRY_RELEASE_MATRIX.md#current-accepted-release-1221-2026-10-07).
The full Python CPU campaign passed **17,105/17,105**. The first full SIMD
campaign passed **17,086/17,105**, with **19 failures** and zero infrastructure
errors. These failures initially blocked push, tagging, and publication. They all concern `Image.convert` from `1`, `L`, `LA`, or `I;16N` into
typed `I`/`F` output. No assertion, threshold, or selected case was removed.

Both campaigns used the isolated extension built by `make build-parity` and
these commands (replace `BACKEND` with `cpu` or `simd`):

```sh
MIGRATION_PARITY_BATCH_SIZE=256 MIGRATION_PARITY_SERIAL=1 \
make migration-parity-test PYTHON=.venv/bin/python \
MIGRATION_TARGET_BACKEND=BACKEND \
MIGRATION_PARITY_OUTPUT=/private/tmp/pillow-release-parity-BACKEND.json.gz
```

Initially failing case IDs, each prefixed by `PIL.Image.Image.convert.nuanced.`:

```text
coverage-batch-convert-pattern-l-f-7
coverage-batch-convert-pattern-l-i-6
coverage-batch-convert-pattern-la-f-17
coverage-batch-convert-pattern-la-i-16
mode-audit-1-to-F-17x3
mode-audit-1-to-I-17x3
mode-audit-I;16N-to-F-17x3
mode-audit-I;16N-to-F-typed-boundaries
mode-audit-I;16N-to-I-17x3
mode-audit-I;16N-to-I-typed-boundaries
mode-audit-L-to-F-17x3
mode-audit-L-to-I-17x3
mode-audit-LA-to-F-17x3
mode-audit-LA-to-I-17x3
one-to-f
simd-cov-f07-c04
simd-cov-f07-c05
simd-cov-f10-c04
simd-cov-f10-c05
```

The final GPU-batch extension passed the 756 focused comparisons and nine
contracts above. These results do not substitute for full ordinary GPU,
Parallel CPU, Node, or browser parity. The completed local gates below now
cover those profiles and source packaging.
The tag-triggered workflow later failed after partial publication. The source
passes below do not establish parity for the normalized registry crate.
No coverage was run locally.

`RUSTC_WRAPPER= cargo test -p pillow-rs --lib -- --test-threads=1` passed
507/507 core tests on Metal, including the native packed-L geometry regression.
The final endian-aware RGB transfer assertion also passed its focused rerun.
`RUSTC_WRAPPER= make fmt clippy PYTHON=.venv/bin/python` passed. Rust formatting
and public API boundary checks passed; no lint policy was weakened.

Documentation tests (44), release-tool tests (12), strict site build, generated
input reproducibility, version synchronization, workflow lint, Cargo audit,
and Cargo deny passed. The x86_64 Windows compile check and optimized WASM
build passed. Clippy passed its existing policy with substantial pre-existing
warning debt; the core Rustdoc build passed without documentation warnings.

The earlier main commit's [CI run](https://github.com/appunni-m/pillow-rs/actions/runs/37249018097)
failed its documentation/input checks. The candidate restored the omitted
font-style generator steps without changing committed parity cases, corrected
the corresponding benchmark chain length from two to three, and fixed the
stale dashboard counts described above. The exact candidate commit
`a0c7ffc45996ddd8a5e287fb78125d2a25fd2630` subsequently passed all nine jobs in
[main CI](https://github.com/appunni-m/pillow-rs/actions/runs/37297480408).
The separate release run failed after partial registry publication; the
packaged Rust font contract also fails, as recorded in the release evidence.

The two native RGBA expansion shortcuts incorrectly also admitted four-byte
`I`/`F` scalar targets. Excluding typed destinations from those shortcuts
restores their existing integer/float widening path without conversion through
RGBA color samples. `1` and `I;16N` use the same L conversion step required by
the reference behavior, so this fixes all 19 cases. The full SIMD rerun passed
**17,105/17,105**, with zero failures/infrastructure errors. The command above
used `/private/tmp/pillow-release-parity-simd-fixed.json.gz` for that rerun.

The new `native_luma_to_typed_samples_does_not_use_rgba_expansion` unit regression
checks L/LA→I/F exact scalar bytes across six vector/tail widths and varied
alpha. All seven `cargo test -p pillow-rs --lib native_luma_` tests passed. Four
focused strict-SIMD public parity cases (L/LA→I/F, 17×3) also passed using
`make migration-parity-test MIGRATION_TARGET_BACKEND=simd
MIGRATION_STRICT_TARGET_BACKEND=1` with their four `--case-id` arguments.
The pre-fix 19 comparisons and complete diffs were preserved in a 3,175-byte
temporary gzip file; both large initial campaign artifacts were removed.

`make release-python-sdist release-sdist-test PYTHON=.venv/bin/python
RELEASE_WHEEL_DIR=dist/release-check` passed: a fresh isolated environment built
the sdist, imported the installed replacement, and executed the README/Python
examples. Registry preflight confirmed image-slash-star 0.1.2 and fontdone
2.14.3-alpha.12 on crates.io, and fontdone 2.14.3-alpha.12 on npm.

## Final local release gates

All six full live-oracle profiles passed 17,105/17,105, with zero failures,
infrastructure errors, or not-run cases. They share the public Pillow corpus;
these are six comparisons of the same 17,105 cases, not 102,630 unique cases.

| Profile | Result | Configuration |
| --- | --- | --- |
| Serial CPU | 17,105/17,105 | Standard build; Rayon disabled |
| SIMD | 17,105/17,105 | Standard build; typed conversion fixed |
| GPU | 17,105/17,105 | Standard build; local Metal; normal contextual fallback policy |
| Parallel CPU | 17,105/17,105 | Cargo `parallel` explicitly enabled; normal Pillow oracle |
| Node WASM | 17,105/17,105 | Complete shared corpus |
| Browser WASM | 17,105/17,105 | Complete shared corpus |

The ordinary GPU and SIMD profile totals include their documented contextual
CPU fallback; they do not prove every operation has an exclusive GPU/SIMD
implementation. The separate batch comparisons require actual GPU work. The
four typed conversion cases additionally passed strict-SIMD execution.

```sh
MIGRATION_PARITY_BATCH_SIZE=256 MIGRATION_PARITY_SERIAL=1 \
make migration-parity-test PYTHON=.venv/bin/python \
MIGRATION_TARGET_BACKEND=gpu \
MIGRATION_PARITY_OUTPUT=/private/tmp/pillow-release-parity-gpu.json.gz

make test-wasm PYTHON=.venv/bin/python \
MIGRATION_JS_PARITY_OUTPUT=/private/tmp/pillow-release-node-parity.json \
MIGRATION_BROWSER_PARITY_OUTPUT=/private/tmp/pillow-release-browser-parity.json

MATURIN_DEVELOP_FLAGS=--skip-install \
make build-parity-parallel-cpu PYTHON=.venv/bin/python
MIGRATION_PARITY_BATCH_SIZE=256 MIGRATION_PARITY_SERIAL=1 \
MIGRATION_TARGET_PROFILE=parallel-cpu \
make migration-parity-test PYTHON=.venv/bin/python \
MIGRATION_TARGET_BACKEND=cpu \
MIGRATION_PARITY_OUTPUT=/private/tmp/pillow-release-parity-parallel-cpu.json.gz

make build-parity PYTHON=.venv/bin/python
```

The final command restores the standard extension. The Pillow 12.2.0 oracle
remains installed separately in `.venv`; none of these builds installed the
replacement `PIL` over it. Execution/shader-coverage sidecars were not requested.

These maintained release commands passed from the clean source candidate:

```sh
MATURIN_DEVELOP_FLAGS=--skip-install \
make release-check PYTHON=.venv/bin/python
make release-wheel-test release-parallel-wheel-test PYTHON=.venv/bin/python \
RELEASE_WHEEL_DIR=dist/release-check
make release-python-sdist release-sdist-test PYTHON=.venv/bin/python \
RELEASE_WHEEL_DIR=dist/release-check
make release-crate-package release-npm-pack PYTHON=.venv/bin/python
cargo package --list --locked -p pillow-rs
```

The standard wheel, companion wheel, `pillow-rs[parallel]` installation, sdist
build/installed examples, Rust package verification against registry dependencies,
and npm consumer/license checks passed. The core crate contains 234 packaged
files; the npm artifact contains ten. The unshipped ImageBatch prototype was not
part of these package artifacts. Older wheel artifacts were preserved separately.

The Windows x86_64 type check, documentation tests/site build, and fmt/Clippy
checks were repeated after the typed conversion change and passed. At the time,
source version declarations agreed on alpha.6 and published installation blocks
remained alpha.5 until the registry jobs and GitHub release succeeded. The
stable 12.2.1 release subsequently completed; its exact-commit CI and registry
evidence are listed in the [release matrix](REGISTRY_RELEASE_MATRIX.md).

## Grayscale completed-work throughput diagnostic, 2026-10-09

`scripts/benchmark_gpu_batch.py --operation grayscale --json` measures the
explicit singleton `ImageOps.grayscale` graph in Pillow, locked CPU, locked
single-thread SIMD, eager GPU, and queued GPU profiles. Each timed window builds
fresh source images and graphs, executes them, materializes outputs, and exports
all output bytes. It compares every output byte string and records backend
receipts, dispatches, uploads, readbacks, and queued-submission counts. This is
an across-image sustained-throughput diagnostic; it does not estimate
per-image latency by dividing a batch window. One command submission is not
proof that kernels run concurrently.

On the local macOS 15.7.7 arm64 Metal host, the 1024×768, 16-image windows
passed exact byte comparison in L and RGB. There were two warmup windows and
five measured windows (112 total jobs per subject). With the default
`max_in_flight=2`, queued GPU throughput versus SIMD/Pillow was:

| Mode | Pillow images/s | CPU images/s | SIMD images/s | Eager GPU images/s | Queued GPU images/s | Queued / SIMD | Queued / Pillow |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L | 10,765 | 18,360 | 17,999 | 498 | 1,484 | 0.082× | 0.138× |
| RGB | 1,954 | 4,873 | 7,254 | 426 | 1,169 | 0.161× | 0.598× |

Queueing improved throughput over eager GPU execution by about 3.0× for L and
2.7× for RGB. It still missed the SIMD throughput goal and the Pillow baseline.
Each grayscale graph dispatched one kernel. Across all warmup and timed jobs,
L uploaded/read back 88,080,384 bytes; RGB uploaded 264,241,152 bytes and read
back 88,080,384 bytes. All jobs completed and the GPU and host live-byte
counters returned to zero. The 256×256, eight-image windows also passed exact
comparison; queued GPU reached 3,209 images/s for L and 3,050 for RGB, well
below SIMD at 120,075 and 70,771 images/s.

A scale follow-up used 64 images per 1024×768 window, three measured windows,
and two warmups (320 total jobs per subject):

| Mode | Pillow images/s | CPU images/s | SIMD images/s | Eager GPU images/s | Queued GPU images/s | Queued / SIMD | Queued / Pillow |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| L | 12,744 | 18,471 | 16,632 | 512 | 1,775 | 0.107× | 0.139× |
| RGB | 2,316 | 4,515 | 7,305 | 401 | 1,057 | 0.145× | 0.457× |

Queueing improved over eager GPU by 3.5× for L and 2.6× for RGB, but remained
well below SIMD and Pillow. At `max_in_flight=2`, the L queue submitted 32 jobs
at once; RGB was limited to 30. Across all 320 jobs, L uploaded/read back
251,658,240 bytes each; RGB uploaded 754,974,720 bytes and read back
251,658,240 bytes. The matched `max_in_flight=1` run retained exact parity but
did not improve throughput: L queued GPU fell from 1,775 to 1,512 images/s as
the largest submission grew from 32 to 64; RGB fell from 1,057 to 1,024 while
the largest submission stayed at 30. Contemporaneous CPU/SIMD/Pillow controls
were mostly stable for RGB, so these results give no reason to change the
default. A separate 16-image `max_in_flight=1` run also failed to establish a
gain under larger host-rate shifts.

Grayscale GPU remains open because queued batching has not met its latency or
sustained-throughput targets. Per-call GPU latency remains separate in the
operation benchmark matrix.

The JSON report initially assumed backend receipts exposed
`actual_backend_counts` and `terminal_complete`. They do not: receipts prove
requested/actual backend and fallback state, while the explicit executor
reports submitted jobs and live-job state. The diagnostic now derives CPU/SIMD
counts from deterministic window counts under per-image backend locks, takes
GPU counts from executor counters, and establishes completion from exported
outputs plus zero live jobs. This was a genuine diagnostic reporting defect;
it did not indicate a parity or backend execution failure. Per-run data is
recorded in
[`performance-optimization-local-runs.csv`](evidence/performance-optimization-local-runs.csv).
No coverage was run.

### Grayscale 256-thread workgroup follow-up, 2026-10-09

The native grayscale shader and checked dispatch planner were changed together
from 64 to 256 invocations per workgroup. The completed-window diagnostic
passed exact output-byte comparisons for L and RGB at 1024×768, 257×129, and
1×1. The executor reported actual GPU execution, one dispatch per graph,
complete byte export, and no fallback. Eight 64-image runs measured median
queued throughput of 2,007 images/s for L and 1,255 for RGB, still below SIMD
at 16,856 and 6,972 images/s. A separate four-run 64-thread reference cohort
measured 1,759 and 1,153 images/s, but its L SIMD control was only 8,937
images/s; the cross-cohort shift makes the raw throughput difference
inconclusive as a causal kernel gain. Keep the candidate exploratory and the
operation open. The per-operation call-plus-materialization run measured GPU
at 771.041 µs versus SIMD at 104.938 µs, with the terminal phase dominated by
readback and synchronization. Full benchmark profiles are in the local evidence
ledger and ignored `build/performance-optimization/` artifacts. No coverage was
run.

### RGB grayscale per-call follow-up, 2026-10-09

Four additional parity-gated runs measured the full 1024×768 RGB grayscale
call and result materialization. The GPU receipt was actual GPU execution with
one dispatch, complete output, and no fallback. GPU medians ranged from
595.021 to 677.855 µs while SIMD ranged from 92.083 to 117.000 µs, leaving GPU
latency at 0.148× to 0.173× of SIMD. Each call uploaded 2,359,296 bytes and
read back 786,432 bytes. These controls do not demonstrate a GPU improvement;
the per-call target and the separate sustained-throughput target remain open.
Per-run reports are in the local evidence ledger and ignored benchmark
artifacts. No coverage was run.

### Mapped-input RGB grayscale follow-up, 2026-10-09

On supported shared-memory Metal adapters, singleton native RGB/YCbCr
grayscale now writes into a mapped storage buffer before dispatch. A strict GPU
parity case for a 3×1 RGB image passed, exercising the padded nine-byte input.
Four 1024×768 call-plus-materialization runs passed live Pillow parity and
reported actual GPU, one dispatch, complete output, and no fallback. GPU
medians were 353.021, 344.521, 339.688, and 339.229 µs against SIMD at 99.521,
92.042, 92.083, and 91.958 µs. Upload/readback volumes stayed 2,359,296 and
786,432 bytes. The GPU median across these runs was 342.104 µs, 43.9% below the
previous four-run upload cohort's 609.844 µs median; GPU still reached only
0.267×–0.282× SIMD latency.

A completed-window run with 64 images/window and exact output-byte comparison
measured queued RGB GPU throughput at 1,203.9 images/s against SIMD at 6,436.3;
for L it measured 1,976.2 against 15,904.4. GPU queued receipts showed 320
actual dispatches, 40 submissions, largest submission eight, complete exports,
zero fallbacks, and zero live jobs. The latency improvement did not meet the
sustained-throughput target. Keep the input mapping limited to supported Metal
and the operation open. Per-profile data is in the local ledger and ignored
benchmark artifacts. No coverage was run.

### Deeper grayscale queue diagnostic, 2026-10-09

A 64-image follow-up raised `max_in_flight` from 2 to 8 and `max_jobs` from 16
to 32. Both L and RGB passed exact output-byte comparisons; GPU requested and
actual backends matched, all 320 operations dispatched, outputs were exported,
and no live jobs or fallbacks remained. RGB queued throughput measured 786.5
images/s versus SIMD at 1,666.6; L measured 1,215.0 versus 10,490.3. The
same-run CPU/SIMD controls shifted sharply from the preceding `max_in_flight=2`
cohort, and the report explicitly does not prove concurrent kernel execution.
Do not infer that the deeper queue regressed or improved throughput from this
cross-cohort comparison. At that point, leave queue defaults unchanged; the GPU throughput
target remains unmet. The complete report is in the local ledger and ignored
benchmark artifacts. No coverage was run.

### YCbCr and CMYK grayscale per-call controls, 2026-10-09

Two additional parity-gated 1024×768 call-plus-materialization workloads
completed on the actual GPU with one dispatch and no fallback. Native YCbCr
measured GPU at 384.625 µs versus SIMD at 52.896 µs; CMYK measured GPU at
752.250 µs versus SIMD at 270.917 µs. The YCbCr and CMYK profiles remain
separate from RGB because they use different source byte layouts and conversion
paths. Neither meets the GPU latency target. Full per-profile rows are recorded
in the local evidence ledger and ignored benchmark artifacts. No coverage was
run.

### Grayscale queue default follow-up, 2026-10-10

After matching every subject to the pinned Pillow 12.2.0 distribution, compare
`max_in_flight=2` with explicit `max_in_flight=4` at `max_jobs=64`. Two four-
flight runs averaged 1,233.0/971.1/726.0 queued images/s for L/RGB/CMYK,
versus 1,150.1/811.0/706.6 for two flights. The rebuilt public default of four
then measured 1,329.3/1,028.1/786.7 images/s across 1024×1024 workloads.
Retain the four-flight default based on those repeated completed-window gains.

Each window included construction, submission, execution, materialization, and
byte export for 64 graphs; five measured windows followed two warmups. Exact
output bytes passed for every subject, requested CPU/SIMD/GPU backends executed
with no fallback, and queued GPU completed 448 jobs per workload. The final
queued/SIMD throughput ratios were 0.181×/0.278×/0.353× for L/RGB/CMYK.
Submission and completed-window throughput still do not prove concurrent
kernels or long-duration sustained performance; the GPU target remains unmet.
The runner now records the installed Pillow distribution separately from the
replacement `PIL` facade version so an implementation package version is not
mistaken for its oracle version. Reports are in ignored
`build/migration-parity/`, with eligible and ineligible rows distinguished in
the local ledger. No coverage was run.

A matched eight-flight trial was mixed: L fell to 1,310.9 images/s from
1,329.3, while RGB and CMYK reached 1,055.7 and 824.9 from 1,028.1 and 786.7.
It also doubled the submission count for L and raised RGB/CMYK submissions
from 49 to 69/70 as the largest group dropped from 16 to 8 jobs. Keep four
flights as the default; the single eight-flight cohort does not show a
consistent gain and remains below SIMD in every mode.

A direct mapped-input upload trial for batch grayscale passed parity and used
the actual GPU, but queued RGB throughput fell from 1,028.1 to 466.6 images/s.
After restoring the original queue upload path, RGB returned to 1,016.4; L and
CMYK remained close to their earlier rates. Reject the mapped path for this
queued workload because synchronous mapping does not pay off here. The raw
candidate and post-revert reports remain in `build/migration-parity/`.

## Native F Resize graphs

`scripts/test_gpu_batch.py --small --output gpu` includes two small filtered F
Resize graphs and a 2048×1536 to 1024×768 Bicubic case. The latter exercises
ordered-f64 coefficient arenas larger than the generic 64 KiB stage reserve.
The stream planner computes and charges the exact coefficient and parameter
storage before upload; eager GPU execution remains a separate route.

On 2026-10-10, queued and eager GPU each matched live Pillow for all 253 selected
graphs. A completed-window comparison used 16 graphs/window at 256×768 and
1024×768, then six graphs/window at 2048×1536; each cohort used two warmups and
seven measured windows. Every subject produced exact output bytes. At 256×768,
queued GPU throughput was 1,343.5 images/s versus 2,693.5 for SIMD. At
1024×768, it was 538.6 versus 646.5. At 2048×1536, it was 145.8 versus 150.9.
The large workload uploaded 12,582,912 bytes and read back 3,145,728 bytes per
image, used two actual GPU dispatches per Resize, and grouped up to six graphs
per submission. These short repeated windows do not establish long-duration
sustained throughput. GPU latency and throughput targets remain unmet; the
F Resize issue stays open. Receipts are in the local run ledger and
`build/migration-parity/f-resize-batch-final-*.json`. No coverage was run.
