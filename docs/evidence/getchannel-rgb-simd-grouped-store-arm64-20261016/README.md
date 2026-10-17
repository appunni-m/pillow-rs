# GetChannel grouped NEON store A–B–B–A

**Disposition: reject the candidate.** Replacing four separate 16-byte NEON
stores with one grouped 64-byte store passed the focused parity gate and static
code-generation inspection, but did not improve the full Python call.

## Frozen scope

- Workload: `pil-image-image.getchannel.materialized.rgb-1024x768`
- Parity case: `PIL.Image.Image.getchannel.nuanced.performance-rgb-1024x768`
- Input: RGB 1024×768; boundary: `call;observe-result`
- Profiles: Pillow, serial CPU, single-thread SIMD; warm cache; 5 warmups, 20
  iterations × 5 samples, concurrency 1
- Host: Apple ARM64, macOS 15.7.7; Pillow 12.2.0
- Candidate changed only the RGB NEON stores. Loads, output bytes, mode
  specialization, and tail handling were unchanged.

## Results

The experiment order was A1 control, B1 candidate, B2 candidate, A2 restored
control. Values below are each report's median latency in microseconds; the
summary compares the median of the two control medians with the median of the
two candidate medians.

| Profile | A1 | B1 | B2 | A2 | Control median | Candidate median | Change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Pillow | 177.376 | 187.187 | 187.146 | 185.646 | 181.511 | 187.167 | +3.1% |
| CPU | 46.083 | 48.416 | 49.042 | 48.459 | 47.271 | 48.729 | +3.1% |
| SIMD | 83.771 | 86.730 | 99.500 | 86.417 | 85.094 | 93.115 | +9.4% |

Exact parity passed for the selected CPU and SIMD profiles in B1 and A2; B2
reused B1's parity only after identity validation. The SIMD execution receipt
reported `neon-deinterleave`, 49,152 vector blocks, zero scalar tail, terminal
completion, and no fallback. Both paths allocate one 786,432-byte output;
reported peak live host memory was 3,145,728 bytes.

The static AArch64 probe showed the candidate lowering to grouped vector stores
without spills, but that instruction change did not translate to public-call
latency. Reject the candidate and leave the kernel implementation unchanged.
The scoped boundary test remains to protect 63/64/65 and 127/128/129-pixel
behavior across all three channels. No test defect was found.

## Evidence files

- `A1-control-result.json` remains in
  `../getchannel-rgb-simd-current-core-rebaseline-arm64-20261016/`.
- B1, B2, and A2 benchmark/parity reports, attestations, producer receipts, and
  raw samples are stored beside this README.
- `neon-store-codegen-probe.rs` and `.s` preserve the isolated compiler probe.
- The complete per-run rows are appended to
  `../performance-optimization-local-runs.csv`.
