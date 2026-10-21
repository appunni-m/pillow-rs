# GetColors I direct list-builder candidate — ARM64

This selected experiment replaces repeated `list.append` with a lazy
`IntoPyObject` adapter consumed by `PyList::new`. The list constructor sees the
exact iterator length and writes each Python tuple into its final slot; the
binding does not first allocate a Rust `Vec` of Python tuples.

## Frozen scope and execution

- Workload: `pipeline-chain.terminal-read.getcolors.high-cardinality-signed-run-i-128x64`
- Exact parity case: `PIL.Image.Image.getcolors.nuanced.i-high-cardinality-signed-run-fallback-128x64`
- Timed step: `call-fallback-4097`, including all 4,097 returned Python rows
- Input: mode `I`, 128×64; 5 warmups, 20 iterations × 5 samples (100 observations), warm cache, concurrency 1
- Reference: Pillow 12.2.0; target: release `python-cpu`, source revision `418d5f0cde51ca64a97cbd007ea731da72d68abf` with dirty changes
- Runner: `lazys-macbook-pro-arm64`
- Exact parity passed 1/1. The receipt reports actual CPU, terminal completion, one operation, and no fallback.

## First measurement

| Subject | Median call | P95 | Throughput median | Pillow / subject |
| --- | ---: | ---: | ---: | ---: |
| Pillow | 173.750 µs | 178.583 µs | 5,755.40 ops/s | 1.00000× |
| Serial CPU | 175.167 µs | 373.000 µs | 5,708.86 ops/s | 0.99191× |

This first run was 0.82% slower than Pillow. Its 373.000 µs target P95 was
noisy, and the 6.8% median delta versus the earlier tracking point was not a
matched comparison. This run alone did not establish a win.

## Selected repeat and matched append control

The stable-runner repeat measured 168.729 µs against same-report Pillow at
175.021 µs (1.03729× Pillow/CPU). A selected mode-parity guard passed all 8
cases for modes `1`, `L`, `P`, `LA`, `RGBA`, varied RGB/I, and nonzero `F`.

The matched append control measured 183.354 µs against Pillow at 177.5625 µs
(0.96841×). The direct-list candidate measured 170.1045 µs against Pillow at
175.479 µs (1.03160×), a 7.23% lower CPU median than the append control. Both
reports used the same workload, stable runner, warm cache, and 100 observations;
the paired report also passed the exact I-mode case. Its receipt confirms actual
CPU execution, terminal completion, one operation, and no fallback. This
demonstrates the CPU target for this selected workload; it does not complete
GetColors across other modes, sizes, or backends.

## Rejected intermediate-list experiment

The first attempt collected `Bound<PyTuple>` values into an exactly reserved
Rust `Vec` and then copied that vector into `PyList::new`. It passed the same
selected parity case, but measured 189.896 µs against same-run Pillow at
175.250 µs (0.92287× Pillow/CPU), with a 381.584 µs target P95. That version
was rejected. The retained implementation feeds converted rows directly from
the core result iterator to `PyList::new`, avoiding both append growth and the
intermediate Rust vector.

## Artifacts and limits

- Benchmark report: `benchmark-result.json`, run `migration-benchmark-f553fcc201864ff6ad700ab417cf28b0`, SHA-256 `be7e9a4033e73a38dcf4362e5d1785d657167d3a94831e8cf081b70878142267`.
- Parity report: `parity-result.json`, run `migration-parity-benchmark-gate-f67e7bb2cfe44074a5fab0addda81b26`, SHA-256 `79d216017a60aaac33afb7b38e165cf724de1af1f754cf62b7e72e35c50cbe10`.
- Producer and runtime evidence: `producer-attestation.json`, `benchmark-attestation.json`, and `parity-attestation.json`.
- Raw observations: `raw-samples.json`.
- The report was imported into generated DuckDB history. No strict baseline was promoted because the benchmark extension SHA-256 (`35979d251e4e49d8cee6b1ac8bc6608fd84dce8c5dca4426345d30d8294b0c25`) differs from the newly captured build-witness artifact SHA-256 (`916a51ed984d8077859cfc52e9d805cf4d0afede6921b9ad4d78f8f10c63fba1`).

One parity-reuse attempt stopped before timing because the Pillow oracle
identity differed; it produced no benchmark observation and is excluded. The
later stable-runner repeat reused matching parity successfully. Only the
selected workload and focused parity cases ran; no full benchmark or parity
campaign ran. Other GetColors modes, sizes, and backends remain open.
