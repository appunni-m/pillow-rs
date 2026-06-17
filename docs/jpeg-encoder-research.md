# IJG JPEG Encoder — Complete Implementation Reference

> Deep research compiled from IJG libjpeg-turbo source: `jcparam.c`, `jfdctint.c`, `jccolor.c`, `jchuff.c`, `jcphuff.c`, `structure.doc`, `filelist.doc`, `libjpeg.txt`.

## 1. Architecture Overview

The IJG JPEG library has a 4-layer architecture:

| Layer | Compressor | Decompressor | Role |
|-------|-----------|-------------|------|
| Public API | `jc*.c` | `jd*.c` | `jpeg_start_compress`, `jpeg_write_scanlines`, etc. |
| Pipeline controller | `jcmaster.c`, `jcmain.c`, `jcmarker.c` | `jdmaster.c`, `jdmain.c`, `jdmarker.c` | State machine, marker I/O |
| Processing | `jcdctmgr.c`, `jccolor.c`, `jchuff.c`/`jcphuff.c` | `jddctmgr.c`, `jdcolor.c`, `jdhuff.c`/`jdphuff.c` | DCT, color conversion, entropy coding |
| Support | `jmemmgr.c`, `jutils.c` | same | Memory pools, utility functions |

**Key insight from `structure.doc`**: Coefficient blocks are kept in **natural (raster) order everywhere**; only the entropy codec does zigzag/dezigzag. Progressive JPEG requires a **full-image DCT coefficient array**, not strip buffers.

## 2. Quality Scaling — `jcparam.c`

### `jpeg_quality_scaling()` formula

```
quality = clamp(quality, 1, 100)

if quality < 50:
    scale = 5000 / quality    // integer division!
else:
    scale = 200 - quality * 2
```

Examples:
| Quality | Scale | Description |
|---------|-------|-------------|
| 1 | 5000 | Maximum compression |
| 10 | 500 | Heavy compression |
| 25 | 200 | |
| 50 | 100 | Raw Annex K tables |
| 75 | 50 | |
| 85 | 30 | |
| 100 | 0 → clamps to 1 | All quant entries become 1 |

### Table scaling

```c
for (i = 0; i < 64; i++) {
    val = (basic_table[i] * scale_factor + 50) / 100;
    // Clamp to [1, 255] for baseline (force_baseline)
    // Clamp to [1, 32767] for 12-bit
    quant_table[i] = clamp(val, 1, 255);
}
```

**IMPORTANT**: At Q=100, scale=0, but the clamping ensures quant=1 for all entries.

### Progressive scan count

`jpeg_simple_progression()` produces exactly **10 scans** for YCbCr:
1. DC first (Al=1)
2-5. AC first (different spectral bands)
6. AC refine (Ah=2, Al=1)
7. DC refine (Ah=1, Al=0)
8-10. AC refine (Ah=1, Al=0) — 3 separate scans

Non-YCbCr: `2 + 4*N` scans where N is the number of components.

### Built-in Huffman tables

Standard Huffman tables from `jstdhuff.c` are only valid for **8-bit precision**. For 12-bit, `optimize_coding` is forced TRUE.

## 3. Forward DCT — `jfdctint.c`

### Algorithm

Two-pass (rows then columns), slow-but-accurate integer DCT using CONST_BITS=13, PASS1_BITS=2. This is the **exact inverse** of our IDCT in `decode/jpeg/idct.rs`.

**Row pass (Pass 1)**: Process 8 rows, each producing intermediate results at `CONST_BITS - PASS1_BITS` scale.
**Column pass (Pass 2)**: Process intermediate results through columns, descale by `CONST_BITS + PASS1_BITS + 3`.

### Input/Output convention

- **Input**: Level-shifted samples in `[-128, 127]` (pixel - 128)
- **Output**: DCT coefficients at full scale (before quantization)
- **Order**: Natural (row-major), not zigzag

### Critical detail — descaling

The FDCT uses the SAME descaling as the IDCT, ensuring roundtrip identity:
```c
#define DESCALE(x, n)  RIGHT_SHIFT((x) + (1 << ((n) - 1)), n)
```

The final descale uses `FSHIFT = CONST_BITS + PASS1_BITS + 3 = 18`, matching the IDCT's second pass.

## 4. Color Conversion — `jccolor.c`

### RGB → YCbCr formulas

Using 16-bit fixed-point fractions (matching IJG exactly):

```
Y  = ( 19595*R + 38470*G +  7471*B + 32768) >> 16
Cb = (-11056*R - 21712*G + 32768*B + 8421376) >> 16   // +128 bias
Cr = ( 32768*R - 27439*G -  5329*B + 8421376) >> 16   // +128 bias
```

Where:
- `19595 = round(0.29900 * 65536)`
- `38470 = round(0.58700 * 65536)`
- `7471 = round(0.11400 * 65536)`
- `32768 = 0.5 * 65536` (rounding bias)
- `8421376 = 128.5 * 65536` (128 bias + rounding)

Output range: Y ∈ [0, 255], Cb/Cr ∈ [0, 255] (centered at 128).

### Level shift

After RGB→YCbCr, the encoder subtracts 128 from each component before FDCT:
```
sample[y][x] = component[y][x] - 128
```

## 5. Huffman Encoding — `jchuff.c`

### Baseline encoder

Uses standard JPEG Annex K Huffman tables by default. Can optionally compute optimal tables via `jpeg_gen_optimal_table()`.

### DC coefficient encoding

```
diff = block[0] - prev_dc
prev_dc = block[0]

// Compute category (number of bits needed)
if diff == 0: cat = 0
else: cat = ceil(log2(|diff| + 1))

// Huffman-encode the category
emit_huffman(cat)  // from DC Huffman table

// Emit the amplitude bits
if diff > 0: emit(diff, cat)       // positive: direct binary
if diff < 0: emit(diff - 1, cat)   // negative: ones-complement-like
```

The category encoding follows JPEG Figure F.1/F.2:
| Category | Range |
|----------|-------|
| 0 | 0 |
| 1 | -1, 1 |
| 2 | -3,-2, 2,3 |
| 3 | -7..-4, 4..7 |
| n | -(2^n-1)..-2^(n-1), 2^(n-1)..2^n-1 |

### AC coefficient encoding

```
run = 0
for k = 1..63:
    if block[k] == 0:
        run++
        if k == 63: emit(EOB)  // 0x00
    else:
        while run >= 16:
            emit(ZRL)  // 0xF0
            run -= 16
        cat = ceil(log2(|block[k]| + 1))
        symbol = (run << 4) | cat
        emit_huffman(symbol)  // from AC Huffman table
        emit_amplitude(block[k], cat)
        run = 0
```

### Amplitude encoding for AC

Same as DC: positive values are direct binary, negative values are ones-complement.

### Zigzag ordering

Coefficients are accessed in zigzag order during entropy coding:
```c
// jchuff.c uses cinfo->natural_order[] to index
// block[k] in zigzag = block_natural[natural_order[k]]
```

The encoder stores coefficients in natural order but uses the natural_order→zigzag mapping when reading them for Huffman encoding.

## 6. Progressive Huffman Encoding — `jcphuff.c`

### State machine

Progressive encoding has two modes per scan type:
- **Ah == 0** (first scan): Encode new coefficients
- **Ah > 0** (refinement scan): Refine existing coefficients

### DC first scan (Ss=0, Se=0, Ah=0, Al>0)

Same as baseline DC encoding but the differential is point-transformed:
```
encoded_value = diff >> Al
// Emit category + amplitude bits at reduced precision
```

### DC refinement scan (Ss=0, Se=0, Ah>0)

One raw bit per block — the next most significant bit of the DC coefficient.

### AC first scan (Ah=0, Al>0)

Huffman-encode AC coefficients in band [Ss..Se]. Each coefficient is point-transformed:
```
encoded_value = coeff[k] >> Al
```
EOB marks the end of non-zero coefficients in the band.

### AC refinement scan (Ah>0)

This is the most complex mode. Two sub-modes controlled by EOBRUN:

**EOBRUN counter**: Accumulates from EOB symbols. Persists across blocks.
- Read: `EOBRUN = (1 << run) + extra_bits(run)`
- Decremented per zero-coefficient position across blocks

**Per-position loop** (for each k in Ss..Se):
1. If `coeff[k] != 0`: emit 1 refinement bit (the (Ah)th bit of coefficient)
2. If `coeff[k] == 0 && EOBRUN > 0`: EOBRUN-- (skip this position)
3. If `coeff[k] == 0 && EOBRUN == 0`: Huffman-decode next symbol
   - EOB: set new EOBRUN
   - ZRL (0xF0): skip 16 positions
   - Run+Size: new non-zero coefficient at position k+run

### Huffman symbol formula (jcphuff.c)

```c
int nbits = JPEG_NBITS_NONZERO(temp);
symbol = (nbits << 4);  // run=0 implicitly (not used for first-scans)
```

Actually for first scans:
```c
symbol = (run << 4) | nbits;
// Where nbits = ceil(log2(|temp| + 1))
```

For refinement: EOB is `0x00`, ZRL is `0xF0`.

### EOBRUN overflow

EOBRUN is a 16-bit counter that saturates at 0x7FFF per IJG spec. When EOBRUN overflows, a special escape sequence is emitted.

## 7. Marker Structure — `jcmarker.c`

### Baseline JPEG marker sequence

```
SOI (0xFFD8)
APP0/JFIF (optional, 0xFFE0) — JFIF header
DQT (0xFFDB) — quantization table(s)
SOF0 (0xFFC0) — frame header (baseline)
DHT (0xFFC4) — Huffman table(s)  
SOS (0xFFDA) — scan header
  [entropy-coded data with byte stuffing]
EOI (0xFFD9)
```

### DQT segment format

```
FF DB [Lh Ll] [Pq Tq] [Q0...Q63]
```
- L = 3 + 64*(precision_bytes) — includes 2-byte length field
- Pq = 0 (8-bit) or 1 (16-bit)
- Tq = table ID (0-3)
- Qk: 64 values in zigzag order

For 8-bit: L = 67 (2 + 1 + 64)

### DHT segment format

```
FF C4 [Lh Ll] [Tc Th] [L1...L16] [V1...Vn]
```
- L = 3 + 16 + n (where n = sum(Li))
- Tc = 0 (DC) or 1 (AC)
- Th = table ID (0-3)
- Li = count of codes of length i
- Vj = symbols in order of increasing code

For DC luminance (12 symbols): L = 3 + 16 + 12 = 31

### SOF0 segment format

```
FF C0 [Lh Ll] [precision] [Yh Yl] [Xh Xl] [Nf] [C1 H1V1 Tq1]...
```
- L = 8 + 3*Nf
- precision = 8 (baseline)
- Y = height, X = width
- Nf = number of components (1 or 3)
- For each component: ID, sampling(4:4=0x11), quant_table

### SOS segment format

```
FF DA [Lh Ll] [Ns] [Cs1 Td1Ta1]... [Ss Se Ah Al]
```
- L = 6 + 2*Ns
- Ns = number of scan components
- For each: component ID, DC table (4 bits) + AC table (4 bits)
- Ss = 0, Se = 63, Ah = 0, Al = 0 (baseline)

## 8. Bit Writer — Byte Stuffing

### Rules

During entropy-coded data (between SOS and EOI):
- After emitting a 0xFF byte, always follow with 0x00
- This prevents accidental marker detection
- Restart markers (0xFFD0-0xFFD7) appear in the stuffed stream naturally

### Implementation

```rust
struct BitWriter {
    buf: Vec<u8>,
    bits_buf: u32,    // accumulates bits MSB-first
    bits_count: u32,
}

fn write_bits(value: u16, n: u32) {
    bits_buf = (bits_buf << n) | (value & mask(n))
    bits_count += n
    while bits_count >= 8:
        emit_byte((bits_buf >> (bits_count - 8)) as u8)
        bits_count -= 8
}

fn flush() {
    if bits_count > 0:
        // Pad with 1-bits (JPEG convention)
        emit_byte((bits_buf << (8 - bits_count)) as u8)
}

fn emit_byte(byte: u8) {
    buf.push(byte)
    if byte == 0xFF:
        buf.push(0x00)  // byte stuffing
}
```

**Critical**: Byte stuffing applies ONLY to entropy-coded data. Marker bytes (SOI, DQT, SOF0, DHT, SOS, EOI) are NOT stuffed. The BitWriter should only be used for the entropy-coded segment.

## 9. MCU Assembly

### 4:4:4 (no subsampling)

Each MCU = one 8×8 block from each component.
For RGB→YCbCr input: 3 blocks per MCU (Y, Cb, Cr).

### Block processing order

For each MCU row (by), for each MCU column (bx):
```
for component in [Y, Cb, Cr]:
    load 8×8 block from component plane
    level shift: block[i] = pixel[i] - 128
    fdct(block)
    quantize(block, quant_table)
    zigzag: reorder to zigzag order
    huffman_encode(block, dc_table, ac_table)
```

### Edge padding

For images whose dimensions are not multiples of 8, pads with edge-replicated pixels.

## 10. Common Bugs / Gotchas

### Quantizer value >= 256
Baseline JPEG requires all quantizer values ≤ 255. At quality < ~25, the scaling can push values above 255. Must clamp.

### DQT stores zigzag, FDCT produces natural
The DQT marker stores tables in zigzag order. The FDCT produces coefficients in natural order. Must reorder before/after quantization.

### Byte stuffing count
If the final byte of entropy data is 0xFF and the next marker byte is also 0xFF, the stuffing byte (0x00) must come BEFORE the marker. The marker bytes themselves are never stuffed.

### DC prediction reset at each scan
Each scan in the JPEG file has its own DC predictor state. For multiple scans (progressive or restart-interval), the DC predictor resets to 0 at scan boundaries.

### Level shift direction
Encoder: pixel - 128 (level shift DOWN before FDCT)
Decoder: IDCT output + 128 (level shift UP after IDCT)

### Chrominance table
For 4:4:4 encoding, both Cb and Cr use the SAME chrominance quantization table (table ID 1). They share the same Huffman tables too (DC table 1, AC table 1).

## 11. Test Strategy

### Roundtrip verification
The only reliable way to verify JPEG encoder correctness:
1. Encode known pixel data
2. Decode with our IJG-exact decoder
3. Compare SHA-256 of decoded pixels against expected output

### PIL reference generation
For each encode parameter combination:
1. PIL: decode source → encode(params) → decode → pixels → SHA-256
2. Our encoder: decode source → encode(params) → decode → pixels → SHA-256  
3. Compare hashes

Lossy formats (JPEG) will not match byte-for-byte between different encoder implementations, but the roundtrip pixel output must match for the encode→decode pair to be correct.

### Quality table generation test
At quality 50, our quantization tables must exactly match Annex K. At quality 100, all entries must be 1. At quality 1, all entries are scaled to maximum.
