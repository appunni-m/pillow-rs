# Scalar I/F GetColors CPU receipt and rebaseline

This focused current-source Mac ARM64 cohort verifies the selected eager
GetColors step instead of borrowing the later GetProjection receipt. The core
now emits a CPU receipt only when the image was already materialized before
the call, so a first-time lazy pipeline materialization keeps its own backend
receipt.

## Workload and identity

- Benchmark workloads: `pipeline-chain.terminal-read.getcolors.scalar-i-1024x768`
  and `pipeline-chain.terminal-read.getcolors.scalar-f-1024x768`.
- Exact parity case: `PIL.Image.Image.getcolors.nuanced.analysis-scalar-if-terminal-1024x768`.
- Boundary: selected `i-getcolors` / `f-getcolors` call, warm cache, five
  warmups, 20 iterations × five samples, concurrency one.
- Target: dirty source revision `9230f5c55ddfe1eab865c1aa5e05ab7b87241c31`,
  default CPU features. The loaded `_core.abi3.so` SHA-256 and workload input
  identities are in `extension-identity.json`.
- CPU receipt: `GetColors/cpu`, completed, one operation, no fallback, zero
  vector blocks. This workload exercises serial CPU only.

## Results

| Workload | Pillow median | CPU median | CPU / Pillow | Exact parity |
| --- | ---: | ---: | ---: | --- |
| I, first receipt run | 0.8280 ms | 0.9804 ms | 0.844× | pass, 1/1 |
| I, focused repeat | 0.7902 ms | 0.9610 ms | 0.822× | pass, 1/1 |
| F, first receipt run | 2.2656 ms | 0.9862 ms | 2.297× | pass, 1/1 |

The two current I-mode runs reproduce a host-local CPU miss. The earlier
unproven run measured Pillow at 1.9747 ms and CPU at 1.1227 ms; it is retained
only to explain why the focused repeat was run and is not used as performance
credit. The I-mode algorithm remains under investigation in Kata `pillow-rs#zw3x`.
These dirty local measurements do not replace the published clean matrix.

## Files

- `getcolors-scalar-if-cpu-receipt-20261016.json` — both selected workloads.
- `benchmark-parity.json.gz` — exact one-case CPU parity gate for that run.
- `getcolors-scalar-i-cpu-repeat-20261016.json` — focused repeat of I only.
- `benchmark-repeat-parity.json.gz` — exact one-case CPU parity gate for repeat.
- `extension-identity.json` — source, binary, host, manifest, and input identity.

The benchmark target's focused selection contracts passed 38/38. No full test
or parity campaign, coverage, commit, or push ran; the commit/push batch remains
0/33.
