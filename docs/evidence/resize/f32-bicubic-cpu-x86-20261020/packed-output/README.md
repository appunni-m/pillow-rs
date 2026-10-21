# F-mode Bicubic Resize packed output trial — x86 CPU — 2026-10-20

## Frozen selection

- Workload: `pipeline-chain.resize-native-f32.bicubic-noise-1024x768`
- Exact Pillow parity case: `PIL.Image.Image.resize.nuanced.f-resize-bicubic-noise-1024x768`
- Input/output: F mode, 1024×768 to 512×384, Bicubic
- Timed boundary: `call;materialize`, warm cache, 5 warmups, 20 iterations × 5 samples, concurrency 1
- Selected backend: serial CPU only; ordinary Pillow oracle
- Host: native x86_64 AMD Ryzen 5 2600X, Linux 7.0.0-38, CPython 3.12.3, Rust 1.96.1
- Source revision: `418d5f0cde51ca64a97cbd007ea731da72d68abf`, dirty working tree
- Control `geometry.rs` SHA-256: `53472213dc0ec6bf8d71b518282f1a6b1272c826f4fad849e2f33026e4ce4d8a`
- Candidate `geometry.rs` SHA-256: `38eacc60cc981945f8d1dc2a1feee39d536cf45ed8eaf4e14926ad29971b9294`

## Result

The candidate writes the final vertical F32 samples directly to packed
little-endian pixel slots, then flattens the `Vec<[u8; 4]>` without a second
full-frame copy. The accumulator order, F64-to-F32 rounding boundary, and
parallel implementation remain unchanged. A focused Rust test compared the
old F32 materialization with the packed output byte-for-byte for finite data,
signed zero, infinities, and NaNs.

The sequence was candidate B1, control A1, control A2, candidate B2. Each run
passed its selected exact parity gate and reported a complete terminal CPU
receipt with requested/actual backend `cpu` and no fallback.

| Run | Core SHA-256 | Pillow median | CPU median | Pillow / CPU |
| --- | --- | ---: | ---: | ---: |
| B1 candidate | `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066` | 4.661849 ms | 4.426197 ms | 1.05324× |
| A1 control | `dee09f63e602025d70551cb8e960bb8cc0c9cc0a636e1e00b323d4de223c5d19` | 4.639393 ms | 4.588777 ms | 1.01103× |
| A2 control | `dee09f63e602025d70551cb8e960bb8cc0c9cc0a636e1e00b323d4de223c5d19` | 4.482637 ms | 4.471771 ms | 1.00243× |
| B2 candidate | `7b7433a32cd1a4c49dd4019d328b673f5cf960c16a6cef2133a6123ce118a066` | 4.478001 ms | 4.318801 ms | 1.03686× |

Pairwise, B1 was 3.54% lower latency than A1, and B2 was 3.42% lower latency
than A2. The mean of run medians was 4.372499 ms for the candidate and
4.530274 ms for the control (3.48% lower candidate latency). Both candidate
runs were faster than their same-run Pillow measurement. This supports keeping
the candidate for this one x86 F-mode CPU workload; it does not establish other
modes, sizes, architectures, profiles, or the full Resize operation target.

The measured boundary includes Python call and terminal materialization. The
benchmark's resource receipt is not a process-wide allocator trace, so the
allocation reduction is established by the source path, not by claiming a
measured allocation-count delta. The SIMD F-resize kernel is separately
implemented under `pool_simd` and was not changed or included in this CPU
trial. SIMD and GPU goals remain open.

## Run artifacts

- [B1 candidate report](resize-f32-bicubic-packed-cpu-20261020.json)
- [B1 raw samples](resize-f32-bicubic-packed-cpu-20261020-raw.json)
- [B1 execution and parity attestations](resize-f32-bicubic-packed-cpu-20261020-attestation.json), [parity attestation](resize-f32-bicubic-packed-cpu-20261020-parity-attestation.json)
- [A1 control report](resize-f32-bicubic-control-cpu-20261020.json)
- [A1 raw samples](resize-f32-bicubic-control-cpu-20261020-raw.json)
- [A1 execution and parity attestations](resize-f32-bicubic-control-cpu-20261020-attestation.json), [parity attestation](resize-f32-bicubic-control-cpu-20261020-parity-attestation.json)
- [A2 control report](resize-f32-bicubic-control-A2-cpu-20261020.json)
- [A2 raw samples](resize-f32-bicubic-control-A2-cpu-20261020-raw.json)
- [A2 execution attestation](resize-f32-bicubic-control-A2-cpu-20261020-attestation.json); parity was reused from A1 after the report runner verified the same core SHA.
- [B2 candidate report](resize-f32-bicubic-packed-B2-cpu-20261020.json)
- [B2 raw samples](resize-f32-bicubic-packed-B2-cpu-20261020-raw.json)
- [B2 execution and parity attestations](resize-f32-bicubic-packed-B2-cpu-20261020-attestation.json), [parity attestation](resize-f32-bicubic-packed-B2-cpu-20261020-parity-attestation.json)

The original A-B-B-A full parity payloads were left on the existing x86 runner;
only their compact reports, raw samples, and attestations were copied here. No
checkout or `target/` directory was copied, and nothing was deleted from the
runner. The local comparison ledger contains the eight Pillow/CPU subject rows
for these four reports.

## Stable-runner-ID PerfDB pairing probe

Two additional selected runs used the same host and workload with the explicit
runner ID `x86_64-ryzen-5-2600x-native-01`. A1 is the control; B1 is the packed
output candidate. Each run passed its exact selected Pillow parity gate (1/1)
and reported completed CPU execution with no fallback. They were collected as
a PerfDB identity/linkage probe, not as another causal performance sequence;
use the preceding B1/A1/A2/B2 results for the A-B-B-A performance conclusion.

The stable-ID reports and raw samples are retained beside this file. Their
4.1 MB parity reports were transferred in gzip form as
[`A1 parity`](resize-f32-bicubic-runnerid-A1-20261020-parity.json.gz) and
[`B1 parity`](resize-f32-bicubic-runnerid-B1-20261020-parity.json.gz), avoiding
uncompressed duplicate payloads. The reports, raw samples, execution
attestations, and parity attestations are also retained here.

The first stable-ID compare (A1 control `3a2e3afe48d2`, B1 candidate
`c3946531702c`) exposed a fixture coverage omission. The workload declared
`PIL.Image.Image.resize.performance.standard`, but the linked F-mode case
declared only `PIL.Image.Image.resize.behavior.default`. The public call and
materialization steps were identical to the benchmark input. The generator now
adds the missing performance coverage declaration to that one case; no steps,
assertions, workload selection, or thresholds changed.

The corrected-coverage rerun used the same runner ID and one selected workload:

| Run | PerfDB artifact | Pillow median | CPU median | Pillow / CPU |
| --- | --- | ---: | ---: | ---: |
| A2 control | `908ecf845ce8` | 4.540923 ms | 4.587948 ms | 0.98975× |
| B2 candidate | `67bd859ce35f` | 4.527096 ms | 4.339565 ms | 1.04321× |

The retained files are [A2 report](resize-f32-bicubic-runnerid-A2-20261020.json),
[A2 raw samples](resize-f32-bicubic-runnerid-A2-20261020-raw.json),
[A2 parity sidecar](resize-f32-bicubic-runnerid-A2-20261020-parity.json.gz),
[B2 report](resize-f32-bicubic-runnerid-B2-20261020.json),
[B2 raw samples](resize-f32-bicubic-runnerid-B2-20261020-raw.json), and
[B2 parity sidecar](resize-f32-bicubic-runnerid-B2-20261020-parity.json.gz).

Both passed the exact case (1/1), whose declared coverage now includes both
requirements, and reported terminal CPU execution with no fallback. These
single A/B runs are a proof-link probe, not a replacement for the earlier
A-B-B-A performance result.

All four stable-ID benchmark reports and their parity sidecars were imported
serially into the existing generated DuckDB in the separate `pillow-perfdb`
project. The exact runner parity-input artifact used for A2/B2 has SHA-256
`b138a899330e292b35365c2e0c9321168d2db6d3a92c1ee4dbc42b0ed1fd8743`; its
compressed copy is retained as
[`runner parity input`](runner-fixture-pil-image-image-parity-input-b138a899-20261020.json.gz).
The local dirty fixture has a different hash, so the existing runner input was
preserved and imported as gzip. Its 1.3 MB compressed copy avoids transferring
or retaining a second uncompressed 38.9 MB fixture in the checkout.

PerfDB now accepts the corrected parity coverage, but `compare` still withholds
the A2/B2 ratio because these direct reports lack a compatible, shared
`workflow-parity-link@2` source/producer/build context. A direct parity sidecar
proves each run's exact output, but does not prove that the two binaries have
the required cross-run build provenance. No baseline was promoted. The strict
gate remains in place; produce the required workflow/build witness before
retrying this selected comparison. Four Pillow/CPU rows each for A1/B1 and
A2/B2 were added to the local run ledger.
