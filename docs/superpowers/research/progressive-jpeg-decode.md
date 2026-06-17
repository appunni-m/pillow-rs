# Progressive JPEG Decoding — IJG libjpeg Algorithm Reference

**Source:** IJG `jdphuff.c` (Thomas G. Lane, 1995-1997)
**Status:** Research for pillow-rs-image JPEG decoder implementation

## Architecture

Progressive JPEG (SOF2) transmits DCT coefficients in multiple scans. Each scan sends
a subset of the 64 DCT coefficients for each 8×8 block, progressively refining the image.
Two mechanisms: Spectral Selection (Ss, Se bands) and Successive Approximation (Ah, Al bits).

## Key Routines

| Ah | Band | Routine | Description |
|----|------|---------|-------------|
| 0 | DC | `decode_mcu_DC_first` | First DC scan: DPCM diff → `coef[0] = val << Al` |
| 0 | AC | `decode_mcu_AC_first` | First AC scan: Huffman (run,size) → `coef[nat[k]] = val << Al` |
| >0 | DC | `decode_mcu_DC_refine` | DC refinement: 1 correction bit per DC |
| >0 | AC | `decode_mcu_AC_refine` | AC refinement: refine existing + new ±1 coeffs |

## AC First Scan Algorithm
```
k = Ss
while k <= Se:
    s = huff_decode(ac_table)
    r = s >> 4   # run length (0-15)
    v = s & 15   # size (0-10)
    if v != 0:
        k += r
        coef[natural_order[k]] = extend(read_bits(v)) << Al
        k++
    else:
        if r != 15: break  # EOBR — end of band
        k += 16  # ZRL — skip 16 zeros
```

## Special Symbols
- **ZRL**: (15,0) — 16 consecutive zero coefficients
- **EOBR**: (r,0) with r<15 — end of band, r remaining zeros

## Key Implementation Detail
Coefficients placed in `jpeg_natural_order[]` (dezigzagged), not zigzag order.
After all progressive scans, block is already in row-major order for IDCT.

## AC Refinement Bug
Our implementation: BitReader exhausts at MCU 57/64. Likely cause:
ZRL handling reads correction bits for existing coeffs but `k` counter
doesn't correctly track which positions have non-zero values.
