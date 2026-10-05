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

The source version is `12.2.0-alpha.6`; alpha.5 remains the latest accepted
release. Alpha.6 was partially published and failed a packaged-crate font check;
see the [release evidence](REGISTRY_RELEASE_MATRIX.md#alpha6-partial-publication-2026-10-05).
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
files; the npm artifact contains ten. The retired archive is outside each
package root/allowlist. Older wheel artifacts were preserved separately.

The Windows x86_64 type check, documentation tests/site build, and fmt/Clippy
checks were repeated after the typed conversion change and passed. Source
version declarations agree on alpha.6; published installation blocks remain
alpha.5 until all registry jobs and the GitHub release succeed. Local checks
do not substitute for the required exact-main-commit CI success before tagging.
