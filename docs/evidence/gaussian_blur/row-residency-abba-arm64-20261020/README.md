# GaussianBlur RGBA SIMD horizontal scanline residency — ARM64 — 2026-10-20

## Result

The candidate fused the three horizontal fixed-point passes per scanline and
kept their two intermediate rows in reusable buffers. Each pass still rounded
to the same byte output; the vertical implementation and NEON kernel were
unchanged. The intent was to avoid writing and rereading two full-frame
horizontal intermediates.

The candidate is a **no-go**. The selected full-call A–B–B–A run did not show a
stable enough gain to retain the change. It reduced the average of the two
candidate medians by 2.55% against the average of the control medians, but the
control itself moved down 2.84% from A1 to A2. B2 was only 0.78% faster than
the adjacent A2 control; per-run Pillow ratios likewise put B2 and A2 at
1.013× and 1.014×. This is not persuasive evidence of a candidate benefit on
this host.

| Leg | Pillow median | SIMD median | Pillow / SIMD | Pillow p95 | SIMD p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| A1 control | 6.130084 ms | 6.151917 ms | 0.996× | 6.213000 ms | 6.354917 ms |
| B1 candidate | 6.506458 ms | 5.889021 ms | 1.105× | 7.083125 ms | 5.973792 ms |
| B2 candidate | 6.008479 ms | 5.930541 ms | 1.013× | 6.108625 ms | 6.121084 ms |
| A2 control | 6.063479 ms | 5.977208 ms | 1.014× | 6.140833 ms | 6.032250 ms |

## Frozen workload and proof

- Workload: `pipeline-chain.blur-material.gaussian-rgba-noise-1024x768`
- Exact parity case: `PIL.Image.Image.filter.nuanced.performance-material-rgba-noise-1024x768-radius-7-5`
- Context: RGBA 1024×768, Pillow 12.2.0, CPython 3.12.13, macOS 15.7.7 ARM64
- Boundary: warm whole workflow through terminal materialization; one warmup,
  3 iterations × 2 samples, concurrency one
- Profile: default SIMD build; Parallel CPU disabled
- A1/A2 loaded extension SHA-256:
  `93e393bdcb9c3caa86bc188aa521a81b92b28729a1f8258a4af073062a6bd390`
- B1/B2 loaded extension SHA-256:
  `c9a731a8595d9a71e042669ed11f27e2f74c9ead43911f83d384ed0d38152989`

All four benchmark reports passed schema validation and the selected case
passed exact Pillow parity. B2 reused B1's report-bound parity because it used
the same candidate binary and workload. Every SIMD receipt says actual backend
`simd`, terminal complete, no fallback, `vector`, 589,824 vector blocks, and
zero scalar tail. The timing boundary includes allocation and materialization.

A2 parity reuse from A1 was rejected by the harness because the full target
runtime identity differed, even though the selected core binary hash matched.
The A2 control was therefore rerun with fresh parity. The failed attempt's
small producer bundle is retained under `A2/failed-parity-reuse/`; its copied
parity report was byte-identical to A1 and was removed after hash verification.

## Artifacts

Each leg contains its benchmark report, raw samples, execution attestation, and
producer attestation. `A1/parity.json.gz`, `B1/parity.json.gz`, and
`A2/parity.json.gz` preserve one compressed parity proof for each actually
measured binary. B2's byte-identical reused parity copy was removed; the B2
report retains its parity evidence ID. `SHA256SUMS` covers the retained files.

The candidate source was reverted after the no-go. The selected operation
remains open because ARM64 SIMD is still near Pillow and below the campaign's
SIMD target.
