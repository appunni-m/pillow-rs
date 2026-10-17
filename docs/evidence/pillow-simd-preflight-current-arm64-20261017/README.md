# Focused Pillow-SIMD preflight recheck

Date: 2026-10-17. This evidence reruns only the two historical preflight case IDs
against ordinary Pillow 12.2.0 on the current dirty source revision
9230f5c55ddfe1eab865c1aa5e05ab7b87241c31.

## Results

- CPU parity: migration-parity-e0efb8d3a5464a7dae44937434289005, 2/2 passed.
- SIMD-profile parity: migration-parity-57690da8bdac4f1b881061561c84756b, 2/2 passed.
- Both runs name regular Pillow 12.2.0 as oracle and include the same two input
  identities. The source revision was dirty.
- The selected cases were:
  - PIL.Image.Image.filter.nuanced.l-mode-kernel-5x5-noise-1024x768
  - PIL.ImageOps.grayscale.nuanced.performance-large-rgb-noise-1024x768

The SIMD report is a parity result for the requested python-simd profile. It
does not itself prove vector-instruction execution or provide performance
evidence; benchmark receipts are required for those claims.

## Historical limit

This confirms the two cases against regular Pillow on current source and is
consistent with the old wrong-oracle diagnosis. It does not reconstruct the
expired byte-difference artifacts for GitHub run 38339047980, so that
historical run remains unclassified. The separate Pillow-versus-Pillow-SIMD
source-compatibility comparison was not rerun here.

The original CI run 38055331119 is independently attributable to the
wrong-oracle harness defect from its driver and workflow configuration. These
focused reports are not a substitute for the missing historical output.

## Files

cpu-parity.json and simd-parity.json are complete focused parity reports.
The adjacent attestation files and bundles bind those reports to their
producers and inputs. SHA256SUMS.txt covers all JSON artifacts in this
directory.
