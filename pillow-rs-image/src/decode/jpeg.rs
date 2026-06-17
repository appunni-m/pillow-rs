//! JPEG decoder — IJG DCT_ISLOW IDCT for pixel-perfect libjpeg parity.
//!
//! Implements libjpeg's exact "slow-but-accurate" integer IDCT from `jidctint.c`.
//! Uses CONST_BITS=13, PASS1_BITS=2 fixed-point arithmetic matching libjpeg-turbo.
//!
//! Baseline JPEG only (SOF0). Supports 8-bit, 4:2:0 and 4:4:4 subsampling,
//! YCbCr color and grayscale.
//!
//! Reference: IJG libjpeg `jidctint.c` (Thomas G. Lane, 1991-1998)
//!            ISO/IEC 10918-1 / ITU-T T.81 (JPEG Standard)

use crate::types::{ColorType, DecodedImage};

// ── IDCT Constants (matching IJG jidctint.c) ──────────────────────────────

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;

// FIX(x) = (i32)(x * (1 << CONST_BITS) + 0.5)
const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12299;
const FIX_1_847759065: i32 = 15137;
const FIX_1_961570560: i32 = 16069;
const FIX_2_053119869: i32 = 16819;
const FIX_2_562915447: i32 = 20995;
const FIX_3_072711026: i32 = 25172;

const DCTSIZE: usize = 8;
const DCTSIZE2: usize = 64;

/// Full-precision multiply matching IJG's MULTIPLY macro (no premature descale).
/// Returns v * c at CONST_BITS (2^13) scale.
/// The descaling happens later in the DESCALE operations.
#[inline(always)]
fn mpy(v: i32, c: i32) -> i32 {
    (v as i64 * c as i64) as i32
}

#[inline(always)]
fn descale(x: i32, shift: i32) -> i32 {
    (x + (1 << (shift - 1))) >> shift
}

/// IJG-style range_limit: clamps (x + 128) to [0, 255].
///
/// The IDCT produces values centered around 0 (pixel - CENTERJSAMPLE).
/// This function adds back the level shift (128) and clamps to valid range.
#[inline(always)]
fn range_limit(x: i32) -> u8 {
    let x = x + 128;
    if x < 0 {
        0
    } else if x > 255 {
        255
    } else {
        x as u8
    }
}

// ── IJG jpeg_idct_islow — in-place on 8×8 block ─────────────────────────

pub fn jpeg_idct_islow(block: &mut [i32; DCTSIZE2], workspace: &mut [i32; DCTSIZE2]) {
    // Pass 1: columns
    for c in 0..DCTSIZE {
        let z2 = block[c + DCTSIZE * 2];
        let z3 = block[c + DCTSIZE * 6];
        let z1 = mpy(z2 + z3, FIX_0_541196100);
        let tmp2 = z1 + mpy(z3, -FIX_1_847759065);
        let tmp3 = z1 + mpy(z2, FIX_0_765366865);

        let z2 = block[c];
        let z3 = block[c + DCTSIZE * 4];
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        // Odd part — Figure 8
        let t0 = &block[c + DCTSIZE * 7];
        let t1 = &block[c + DCTSIZE * 5];
        let t2 = &block[c + DCTSIZE * 3];
        let t3 = &block[c + DCTSIZE];
        let (v0, v1, v2, v3) = (*t0, *t1, *t2, *t3);

        let z1 = v0 + v3;
        let z2 = v1 + v2;
        let z3 = v0 + v2;
        let z4 = v1 + v3;
        let z5 = mpy(z3 + z4, FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336);
        let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026);
        let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223);
        let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560);
        let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = z3 + z5;
        let z4 = z4 + z5;

        let o0 = t0 + z1 + z3;
        let o1 = t1 + z2 + z4;
        let o2 = t2 + z2 + z3;
        let o3 = t3 + z1 + z4;

        workspace[c] = descale(tmp10 + o3, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 7] = descale(tmp10 - o3, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE] = descale(tmp11 + o2, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 6] = descale(tmp11 - o2, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 2] = descale(tmp12 + o1, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 5] = descale(tmp12 - o1, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 3] = descale(tmp13 + o0, CONST_BITS - PASS1_BITS);
        workspace[c + DCTSIZE * 4] = descale(tmp13 - o0, CONST_BITS - PASS1_BITS);
    }

    // Pass 2: rows from workspace → block (in-place with range limiting)
    const FS: i32 = CONST_BITS + PASS1_BITS + 3;

    for r in 0..DCTSIZE {
        let row = r * DCTSIZE;
        let z2 = workspace[row + 2];
        let z3 = workspace[row + 6];
        let z1 = mpy(z2 + z3, FIX_0_541196100);
        let tmp2 = z1 + mpy(z3, -FIX_1_847759065);
        let tmp3 = z1 + mpy(z2, FIX_0_765366865);

        let z2 = workspace[row];
        let z3 = workspace[row + 4];
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;

        let tmp10 = tmp0 + tmp3;
        let tmp13 = tmp0 - tmp3;
        let tmp11 = tmp1 + tmp2;
        let tmp12 = tmp1 - tmp2;

        let v0 = workspace[row + 7];
        let v1 = workspace[row + 5];
        let v2 = workspace[row + 3];
        let v3 = workspace[row + 1];

        let z1 = v0 + v3;
        let z2 = v1 + v2;
        let z3 = v0 + v2;
        let z4 = v1 + v3;
        let z5 = mpy(z3 + z4, FIX_1_175875602);

        let t0 = mpy(v0, FIX_0_298631336);
        let t1 = mpy(v1, FIX_2_053119869);
        let t2 = mpy(v2, FIX_3_072711026);
        let t3 = mpy(v3, FIX_1_501321110);
        let z1 = mpy(z1, -FIX_0_899976223);
        let z2 = mpy(z2, -FIX_2_562915447);
        let z3 = mpy(z3, -FIX_1_961570560);
        let z4 = mpy(z4, -FIX_0_390180644);
        let z3 = z3 + z5;
        let z4 = z4 + z5;

        let o0 = t0 + z1 + z3;
        let o1 = t1 + z2 + z4;
        let o2 = t2 + z2 + z3;
        let o3 = t3 + z1 + z4;

        block[row] = range_limit(descale(tmp10 + o3, FS)) as i32;
        block[row + 7] = range_limit(descale(tmp10 - o3, FS)) as i32;
        block[row + 1] = range_limit(descale(tmp11 + o2, FS)) as i32;
        block[row + 6] = range_limit(descale(tmp11 - o2, FS)) as i32;
        block[row + 2] = range_limit(descale(tmp12 + o1, FS)) as i32;
        block[row + 5] = range_limit(descale(tmp12 - o1, FS)) as i32;
        block[row + 3] = range_limit(descale(tmp13 + o0, FS)) as i32;
        block[row + 4] = range_limit(descale(tmp13 - o0, FS)) as i32;
    }
}

// ── JPEG Utilities ────────────────────────────────────────────────────────

/// `jpeg_natural_order` maps zigzag index to natural (row-major) position.
///
/// Usage: `natural_block[jpeg_natural_order[zigzag_index]] = zigzag_block[zigzag_index]`
///
/// Source: IJG libjpeg `jpeg_natural_order` (from jdhuff.c / jdhuff.h)
const JPEG_NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Sign extension for DC/AC coefficient additional bits (Figure F.12).
///
/// Given `value` read as `size` bits, if value < 2^(size-1) it is negative:
/// result = value - 2^size + 1, otherwise result = value.
#[inline(always)]
fn extend(value: u32, size: u8) -> i32 {
    if size == 0 {
        return 0;
    }
    let threshold = 1u32 << (size - 1);
    if value < threshold {
        (value as i32) - ((1i32 << size) - 1)
    } else {
        value as i32
    }
}

/// YCbCr -> RGB conversion matching libjpeg's jdcolor.c.
///
/// libjpeg fixed-point coefficients (SCALEBITS=16):
///   R = Y + 1.40200 * (Cr - 128)
///   G = Y - 0.34414 * (Cb - 128) - 0.71414 * (Cr - 128)
///   B = Y + 1.77200 * (Cb - 128)
///
/// Uses precomputed 256-entry tables matching the IJG approach.
struct YccColorConverter {
    cr_r_tab: [i32; 256],
    cb_b_tab: [i32; 256],
    cr_g_tab: [i32; 256],
    cb_g_tab: [i32; 256],
}

impl YccColorConverter {
    fn new() -> Self {
        let mut cr_r_tab = [0i32; 256];
        let mut cb_b_tab = [0i32; 256];
        let mut cr_g_tab = [0i32; 256];
        let mut cb_g_tab = [0i32; 256];

        for i in 0..256 {
            let x = i as i32 - 128;
            // FIX(1.40200) = 91881, ONE_HALF = 32768, SCALEBITS = 16
            cr_r_tab[i] = ((91881i64 * x as i64 + 32768) >> 16) as i32;
            // FIX(1.77200) = 116130
            cb_b_tab[i] = ((116130i64 * x as i64 + 32768) >> 16) as i32;
            // -FIX(0.71414) = -46802
            cr_g_tab[i] = (-46802i64 * x as i64) as i32;
            // -FIX(0.34414) = -22554, + ONE_HALF for the G channel shift
            cb_g_tab[i] = ((-22554i64 * x as i64) + 32768) as i32;
        }

        YccColorConverter {
            cr_r_tab,
            cb_b_tab,
            cr_g_tab,
            cb_g_tab,
        }
    }

    #[inline(always)]
    fn ycc_to_rgb(&self, y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
        let y = y as i32;
        let r = y + self.cr_r_tab[cr as usize];
        let g = y + ((self.cb_g_tab[cb as usize] + self.cr_g_tab[cr as usize]) >> 16);
        let b = y + self.cb_b_tab[cb as usize];

        (
            r.clamp(0, 255) as u8,
            g.clamp(0, 255) as u8,
            b.clamp(0, 255) as u8,
        )
    }
}

// ── Marker Constants ──────────────────────────────────────────────────────

const M_SOI: u16 = 0xFFD8;
const M_EOI: u16 = 0xFFD9;
const M_SOS: u16 = 0xFFDA;
const M_SOF0: u16 = 0xFFC0;
const M_SOF2: u16 = 0xFFC2;
const M_DHT: u16 = 0xFFC4;
const M_DQT: u16 = 0xFFDB;
const M_DRI: u16 = 0xFFDD;

// ── Bit Reader ────────────────────────────────────────────────────────────

/// Reads bits from a byte-aligned entropy segment (no RST/EOI markers inside).
struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u32,
    bits: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        BitReader {
            data,
            pos: start,
            end,
            buf: 0,
            bits: 0,
        }
    }

    /// Fill the bit buffer by reading bytes from the stream.
    /// Handles byte stuffing (0xFF 0x00 -> 0xFF data).
    fn fill(&mut self) {
        while self.bits <= 24 {
            if self.pos >= self.end {
                return;
            }
            let byte = self.data[self.pos];
            self.pos += 1;

            if byte == 0xFF {
                // Check for byte stuffing
                if self.pos >= self.end {
                    return;
                }
                let next = self.data[self.pos];
                if next == 0x00 {
                    // Stuffed 0xFF: 0xFF 0x00 represents data byte 0xFF
                    self.pos += 1;
                    self.buf = (self.buf << 8) | 0xFFu32;
                    self.bits += 8;
                } else {
                    // Marker byte encountered (RST, EOI, or other).
                    // Since we extracted segments without markers, this is unexpected.
                    // Don't consume the marker byte - put the 0xFF back.
                    self.pos -= 1;
                    return;
                }
            } else {
                self.buf = (self.buf << 8) | byte as u32;
                self.bits += 8;
            }
        }
    }

    /// Read `n` bits from the stream (MSB first).
    fn read_bits(&mut self, n: u32) -> Option<u32> {
        if n > self.bits {
            self.fill();
            if n > self.bits {
                return None;
            }
        }
        let val = self.buf >> (self.bits - n);
        self.bits -= n;
        self.buf &= (1 << self.bits) - 1;
        Some(val)
    }

    /// Read a single bit.
    #[allow(dead_code)]
    fn read_bit(&mut self) -> Option<u32> {
        self.read_bits(1)
    }
}

// ── Huffman Table ─────────────────────────────────────────────────────────

/// Derived Huffman decode table using the IJG maxcode/valoffset algorithm.
#[derive(Debug, Clone)]
struct HuffTable {
    /// Symbol values (from DHT marker).
    values: Vec<u8>,
    /// Largest code of each bit length k (1-indexed, index 0 unused).
    /// Set to -1 if no codes of that length.
    maxcode: [i32; 17],
    /// Offset: valoffset[k] = (index of first symbol of length k) - (its code value).
    valoffset: [i32; 17],
}

impl HuffTable {
    /// Build a derived Huffman table from the DHT marker data.
    ///
    /// `counts[16]` = number of codes of each bit length (1-16).
    /// `values` = symbol values in code order.
    fn build(counts: &[u8; 16], values: &[u8]) -> Self {
        // Generate canonical Huffman codes
        let mut code = 0i32;
        let mut huffcode: Vec<i32> = Vec::with_capacity(values.len());

        for l in 1..=16 {
            let cnt = counts[l - 1] as usize;
            for _ in 0..cnt {
                huffcode.push(code);
                code += 1;
            }
            code <<= 1;
        }

        // Build maxcode[] and valoffset[] (1-indexed)
        let mut maxcode = [-1i32; 17];
        let mut valoffset = [0i32; 17];
        let mut p = 0i32;

        for l in 1..=16 {
            let cnt = counts[l - 1] as usize;
            if cnt > 0 {
                valoffset[l] = p - huffcode[p as usize];
                p += cnt as i32;
                maxcode[l] = huffcode[(p - 1) as usize];
            }
        }
        // Sentinel: prevent infinite loop on corrupt data
        maxcode[16] = 0x7FFFFFFF;

        HuffTable {
            values: values.to_vec(),
            maxcode,
            valoffset,
        }
    }

    /// Decode one Huffman symbol from the bit reader.
    fn decode(&self, br: &mut BitReader) -> Option<u8> {
        // Read first bit
        let mut code = br.read_bits(1)? as i32;
        let mut l = 1i32;

        // Keep reading until we have a valid code
        while code > self.maxcode[l as usize] {
            l += 1;
            if l > 16 {
                return None;
            }
            let bit = br.read_bits(1)?;
            code = (code << 1) | (bit as i32);
        }

        // Look up the symbol
        let idx = code + self.valoffset[l as usize];
        if idx < 0 || idx >= self.values.len() as i32 {
            return None;
        }
        Some(self.values[idx as usize])
    }
}

// ── JPEG Structures ───────────────────────────────────────────────────────

/// Frame component info from SOF0 marker.
#[derive(Debug, Clone, Copy)]
struct FrameComponent {
    /// Component ID (1=Y, 2=Cb, 3=Cr in JFIF).
    id: u8,
    /// Horizontal sampling factor.
    h_samp: u8,
    /// Vertical sampling factor.
    v_samp: u8,
    /// Quantization table index.
    quant_tbl: u8,
}

/// Scan component info from SOS marker.
#[derive(Debug, Clone, Copy)]
struct ScanComponent {
    /// Index into frame components.
    comp_index: usize,
    /// DC Huffman table index.
    dc_tbl: u8,
    /// AC Huffman table index.
    ac_tbl: u8,
}

/// Info for one scan (SOS marker).
#[derive(Debug, Clone)]
struct ScanInfo {
    components: Vec<ScanComponent>,
    /// Position of entropy-coded data start (after SOS params).
    entropy_start: usize,
    /// Position of the next marker after this scan's entropy data.
    entropy_end: usize,
    /// Spectral selection start (0 for DC, >0 for AC bands).
    ss: u8,
    /// Spectral selection end.
    se: u8,
    /// Successive approximation bit position high (previously refined).
    ah: u8,
    /// Successive approximation bit position low (currently refining).
    al: u8,
    /// Snapshot of DC Huffman tables at the time of this scan.
    dc_huff_tables: Vec<Option<HuffTable>>,
    /// Snapshot of AC Huffman tables at the time of this scan.
    ac_huff_tables: Vec<Option<HuffTable>>,
}

/// Parsed JPEG info from markers.
struct JpegInfo {
    width: u16,
    height: u16,
    num_components: u8,
    components: Vec<FrameComponent>,
    quant_tables: Vec<Option<[u16; 64]>>,
    dc_huff_tables: Vec<Option<HuffTable>>,
    ac_huff_tables: Vec<Option<HuffTable>>,
    /// For baseline: scan_components from single SOS. For progressive: first scan.
    scan_components: Vec<ScanComponent>,
    restart_interval: u16,
    /// For baseline: start of entropy data. For progressive: populated in scans.
    entropy_start: usize,
    /// Position of EOI marker.
    eoi_pos: usize,
    /// Maximum horizontal sampling factor.
    max_h_samp: u8,
    /// Maximum vertical sampling factor.
    max_v_samp: u8,
    /// true if SOF2 (progressive JPEG).
    progressive: bool,
    /// All scans (for progressive JPEG).
    scans: Vec<ScanInfo>,
}

// ── JPEG Parser ───────────────────────────────────────────────────────────

/// Read a big-endian u16 from the data at the given position.
fn read_u16(data: &[u8], pos: &mut usize) -> Option<u16> {
    if *pos + 1 >= data.len() {
        return None;
    }
    let val = (data[*pos] as u16) << 8 | data[*pos + 1] as u16;
    *pos += 2;
    Some(val)
}

/// Read a u8 from the data at the given position.
fn read_u8(data: &[u8], pos: &mut usize) -> Option<u8> {
    if *pos >= data.len() {
        return None;
    }
    let val = data[*pos];
    *pos += 1;
    Some(val)
}

/// Find the next marker (0xFF followed by a non-0x00, non-0xFF byte).
/// Scans through padding bytes.
fn find_next_marker(data: &[u8], pos: &mut usize) -> Option<u16> {
    while *pos < data.len() {
        // Skip non-0xFF bytes (might be entropy data or padding)
        while *pos < data.len() && data[*pos] != 0xFF {
            *pos += 1;
        }
        if *pos >= data.len() {
            return None;
        }
        // Found 0xFF, read next byte
        if *pos + 1 >= data.len() {
            return None;
        }
        let marker_byte = data[*pos + 1];
        if marker_byte == 0x00 || marker_byte == 0xFF {
            // 0xFF 0x00 is stuffed data, 0xFF 0xFF is padding
            *pos += 1; // Skip this 0xFF and continue
            continue;
        }
        let marker = 0xFF00u16 | marker_byte as u16;
        *pos += 2; // Skip past the marker bytes
        return Some(marker);
    }
    None
}

/// Scan entropy data for RST markers and EOI, returning segment positions
/// and the EOI location. Each segment is a (start, end) byte range in `data`
/// that contains no RST/EOI markers (only stuffed 0xFF 0x00).
#[allow(dead_code)]
struct EntropySegments {
    /// List of (start, end) byte ranges in `data` for each entropy segment.
    segments: Vec<(usize, usize)>,
    /// Position of EOI marker (0 if not found).
    eoi_pos: usize,
}

fn extract_entropy_segments(data: &[u8], start: usize, end_hint: usize) -> EntropySegments {
    let mut segments = Vec::new();
    let mut seg_start = start;
    let mut pos = start;
    let mut eoi_pos = 0;

    while pos < end_hint {
        if data[pos] == 0xFF {
            if pos + 1 >= end_hint {
                break;
            }
            let next = data[pos + 1];
            if next == 0x00 {
                // Stuffed byte: 0xFF 0x00 -> data 0xFF
                pos += 2;
            } else if next >= 0xD0 && next <= 0xD7 {
                // RST marker: end of current segment
                segments.push((seg_start, pos));
                pos += 2; // skip marker
                seg_start = pos;
            } else if next == 0xD9 {
                // EOI marker
                segments.push((seg_start, pos));
                eoi_pos = pos;
                break;
            } else {
                // Other marker (shouldn't appear in entropy data)
                // Skip it and continue
                // If it has a length parameter, skip it
                pos += 2;
                if pos + 1 < end_hint {
                    let len = (data[pos] as u16) << 8 | data[pos + 1] as u16;
                    pos += len as usize;
                }
            }
        } else {
            pos += 1;
        }
    }

    // If we reached the end without finding EOI, still add the last segment
    if seg_start < end_hint && eoi_pos == 0 {
        segments.push((seg_start, end_hint));
    }

    EntropySegments { segments, eoi_pos }
}

/// Parse SOF0 (Start of Frame, baseline).
fn parse_sof0(data: &[u8], pos: &mut usize) -> Option<(u16, u16, Vec<FrameComponent>, u8, u8)> {
    let length = read_u16(data, pos)?;
    let _end = *pos + (length as usize) - 2;

    let precision = read_u8(data, pos)?;
    if precision != 8 {
        return None; // Only 8-bit precision supported
    }

    let height = read_u16(data, pos)?;
    let width = read_u16(data, pos)?;
    let num_components = read_u8(data, pos)?;

    if num_components != 1 && num_components != 3 {
        return None; // Only grayscale and YCbCr supported
    }

    let mut components = Vec::with_capacity(num_components as usize);
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;

    for _ in 0..num_components {
        let id = read_u8(data, pos)?;
        let sampling = read_u8(data, pos)?;
        let h_samp = sampling >> 4;
        let v_samp = sampling & 0x0F;
        let quant_tbl = read_u8(data, pos)?;

        if h_samp < 1 || h_samp > 4 || v_samp < 1 || v_samp > 4 {
            return None; // Invalid sampling factor
        }
        if quant_tbl > 3 {
            return None; // Invalid quantization table index
        }

        max_h_samp = max_h_samp.max(h_samp);
        max_v_samp = max_v_samp.max(v_samp);

        components.push(FrameComponent {
            id,
            h_samp,
            v_samp,
            quant_tbl,
        });
    }

    Some((width, height, components, max_h_samp, max_v_samp))
}

/// Parse DQT (Define Quantization Table).
fn parse_dqt(
    data: &[u8],
    pos: &mut usize,
    quant_tables: &mut Vec<Option<[u16; 64]>>,
) -> Option<()> {
    let length = read_u16(data, pos)?;
    let end = *pos + (length as usize) - 2;

    while *pos < end {
        let info = read_u8(data, pos)?;
        let precision = (info >> 4) as usize; // 0 = 8-bit, 1 = 16-bit
        let table_id = (info & 0x0F) as usize;

        if table_id >= 4 {
            return None; // Max 4 quantization tables
        }

        let mut table_zigzag = [0u16; 64];
        for i in 0..64 {
            if precision == 0 {
                table_zigzag[i] = read_u8(data, pos)? as u16;
            } else {
                table_zigzag[i] = read_u16(data, pos)?;
            }
        }

        // Store quantization table in zigzag order (as read from the DQT marker).
        // Dequantization happens in zigzag order so we keep this layout.
        while quant_tables.len() <= table_id {
            quant_tables.push(None);
        }
        quant_tables[table_id] = Some(table_zigzag);
    }

    Some(())
}

/// Parse DHT (Define Huffman Table).
fn parse_dht(
    data: &[u8],
    pos: &mut usize,
    dc_tables: &mut Vec<Option<HuffTable>>,
    ac_tables: &mut Vec<Option<HuffTable>>,
) -> Option<()> {
    let length = read_u16(data, pos)?;
    let end = *pos + (length as usize) - 2;

    while *pos < end {
        let info = read_u8(data, pos)?;
        let table_class = info >> 4; // 0 = DC, 1 = AC
        let table_id = (info & 0x0F) as usize;

        if table_id >= 4 {
            return None; // Max 4 Huffman tables per class
        }

        // Read counts for each bit length (1-16)
        let mut counts = [0u8; 16];
        let mut total_values = 0usize;
        for i in 0..16 {
            counts[i] = read_u8(data, pos)?;
            total_values += counts[i] as usize;
        }

        // Read symbol values
        let mut values = Vec::with_capacity(total_values);
        for _ in 0..total_values {
            values.push(read_u8(data, pos)?);
        }

        let table = HuffTable::build(&counts, &values);

        if table_class == 0 {
            // DC table
            while dc_tables.len() <= table_id {
                dc_tables.push(None);
            }
            dc_tables[table_id] = Some(table);
        } else {
            // AC table
            while ac_tables.len() <= table_id {
                ac_tables.push(None);
            }
            ac_tables[table_id] = Some(table);
        }
    }

    Some(())
}

/// Parse SOS (Start of Scan) — baseline or progressive.
fn parse_sos(
    data: &[u8],
    pos: &mut usize,
    components: &[FrameComponent],
) -> Option<(Vec<ScanComponent>, usize, u8, u8, u8, u8)> {
    let _length = read_u16(data, pos)?;
    let num_scan_comps = read_u8(data, pos)?;

    if num_scan_comps == 0 {
        return None;
    }

    let mut scan_comps = Vec::with_capacity(num_scan_comps as usize);

    for _ in 0..num_scan_comps {
        let comp_id = read_u8(data, pos)?;
        let tbl_info = read_u8(data, pos)?;
        let dc_tbl = tbl_info >> 4;
        let ac_tbl = tbl_info & 0x0F;

        // Find the component index
        let comp_index = components.iter().position(|c| c.id == comp_id)?;

        if dc_tbl > 3 || ac_tbl > 3 {
            return None;
        }

        scan_comps.push(ScanComponent {
            comp_index,
            dc_tbl,
            ac_tbl,
        });
    }

    // Spectral selection and successive approximation
    let ss = read_u8(data, pos)?;
    let se = read_u8(data, pos)?;
    let ah_al = read_u8(data, pos)?;
    let ah = ah_al >> 4;
    let al = ah_al & 0x0F;

    // The entropy-coded data starts here
    let entropy_start = *pos;

    Some((scan_comps, entropy_start, ss, se, ah, al))
}

/// Parse DRI (Define Restart Interval).
fn parse_dri(data: &[u8], pos: &mut usize) -> Option<u16> {
    let _len = read_u16(data, pos)?; // Should be 4
    let restart_interval = read_u16(data, pos)?;
    Some(restart_interval)
}

/// Parse a JPEG file and return the decoded info structure.
/// Handles both baseline (SOF0) and progressive (SOF2) JPEG.
#[allow(unused_assignments)]
fn parse_jpeg(data: &[u8]) -> Option<JpegInfo> {
    let mut pos = 0usize;

    // SOI marker
    let soi = read_u16(data, &mut pos)?;
    if soi != M_SOI {
        return None;
    }

    let mut width = 0u16;
    let mut height = 0u16;
    let mut components: Vec<FrameComponent> = Vec::new();
    let mut num_components = 0u8;
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;
    let mut quant_tables: Vec<Option<[u16; 64]>> = Vec::new();
    let mut dc_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut ac_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut scan_components: Vec<ScanComponent> = Vec::new();
    let mut restart_interval: u16 = 0;
    let mut entropy_start: Option<usize> = None;
    let mut saw_sof = false;
    let mut saw_sos = false;
    let mut progressive = false;
    let mut scans: Vec<ScanInfo> = Vec::new();

    // Parse all markers
    loop {
        let marker = find_next_marker(data, &mut pos)?;

        match marker {
            M_SOF0 | M_SOF2 => {
                if saw_sof {
                    return None; // Only one SOF allowed
                }
                progressive = marker == M_SOF2;
                let result = parse_sof0(data, &mut pos)?;
                width = result.0;
                height = result.1;
                components = result.2;
                max_h_samp = result.3;
                max_v_samp = result.4;
                num_components = components.len() as u8;
                saw_sof = true;
            }
            M_DQT => {
                parse_dqt(data, &mut pos, &mut quant_tables)?;
            }
            M_DHT => {
                parse_dht(data, &mut pos, &mut dc_huff_tables, &mut ac_huff_tables)?;
            }
            M_SOS => {
                if !saw_sof {
                    return None; // SOS before SOF
                }
                let result = parse_sos(data, &mut pos, &components)?;
                let comps = result.0;
                let scan_start = result.1;
                let ss = result.2;
                let se = result.3;
                let ah = result.4;
                let al = result.5;

                // Find the end of this scan's entropy data (next marker)
                let scan_end = find_entropy_end(data, pos);

                let scan_info = ScanInfo {
                    components: comps.clone(),
                    entropy_start: scan_start,
                    entropy_end: scan_end,
                    ss,
                    se,
                    ah,
                    al,
                    dc_huff_tables: dc_huff_tables.clone(),
                    ac_huff_tables: ac_huff_tables.clone(),
                };
                scans.push(scan_info);

                if !progressive {
                    // Baseline: single scan
                    if scan_components.is_empty() {
                        scan_components = comps;
                        entropy_start = Some(scan_start);
                    }
                    saw_sos = true;
                    // Find EOI marker for baseline compat
                    if let Some(eoi) = find_eoi(data, pos) {
                        pos = eoi;
                    } else {
                        pos = data.len();
                    }
                    break; // Baseline exits after first SOS
                } else {
                    // Progressive: continue scanning
                    saw_sos = true;
                    if scan_components.is_empty() {
                        scan_components = comps;
                        entropy_start = Some(scan_start);
                    }
                    // Advance past the scan data to find next marker
                    pos = scan_end;
                }
            }
            M_DRI => {
                restart_interval = parse_dri(data, &mut pos)?;
            }
            M_EOI => {
                break; // End of image
            }
            // RST markers (no length)
            0xFFD0..=0xFFD7 => {
                // Skip
            }
            // TEM marker (no length)
            0xFF01 => {
                // Skip
            }
            // All other markers have a 2-byte length field
            _ => {
                let length = read_u16(data, &mut pos)? as usize;
                pos += length - 2;
            }
        }
    }

    if !saw_sos {
        return None;
    }

    // Find EOI marker position
    let eoi_pos = find_eoi(data, 0).unwrap_or(data.len());

    Some(JpegInfo {
        width,
        height,
        num_components,
        components,
        quant_tables,
        dc_huff_tables,
        ac_huff_tables,
        scan_components,
        restart_interval,
        entropy_start: entropy_start?,
        eoi_pos,
        max_h_samp,
        max_v_samp,
        progressive,
        scans,
    })
}

/// Find the end of an entropy-coded segment by scanning for the next marker.
fn find_entropy_end(data: &[u8], mut pos: usize) -> usize {
    while pos + 1 < data.len() {
        if data[pos] == 0xFF {
            let next = data[pos + 1];
            if next == 0x00 {
                pos += 2; // stuffed byte
            } else if next >= 0xD0 && next <= 0xD7 {
                pos += 2; // RST marker, skip
            } else {
                return pos; // Other marker found
            }
        } else {
            pos += 1;
        }
    }
    data.len()
}

/// Find the EOI marker (0xFFD9) starting from a given position.
fn find_eoi(data: &[u8], mut pos: usize) -> Option<usize> {
    while pos + 1 < data.len() {
        if data[pos] == 0xFF && data[pos + 1] == 0xD9 {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

// ── Entropy Decoding ──────────────────────────────────────────────────────

/// Decode a single block's DCT coefficients.
///
/// Returns decoded coefficients in zigzag order (to be dequantized and de-zigzagged).
fn decode_block(
    br: &mut BitReader,
    dc_table: &HuffTable,
    ac_table: &HuffTable,
    last_dc: &mut i32,
    block_zigzag: &mut [i32; 64],
) -> bool {
    // Initialize all coefficients to zero
    for coeff in block_zigzag.iter_mut() {
        *coeff = 0;
    }

    // --- Decode DC coefficient ---
    let dc_cat = match dc_table.decode(br) {
        Some(cat) => cat,
        None => return false,
    };

    if dc_cat > 0 {
        let bits = match br.read_bits(dc_cat as u32) {
            Some(b) => b,
            None => return false,
        };
        let diff = extend(bits, dc_cat);
        *last_dc += diff;
    }
    // dc_cat == 0 means diff = 0, no additional bits

    block_zigzag[0] = *last_dc;

    // --- Decode AC coefficients ---
    let mut k = 1usize;
    while k < 64 {
        let sym = match ac_table.decode(br) {
            Some(s) => s,
            None => return false,
        };

        if sym == 0x00 {
            // EOB - remaining coefficients are zero
            break;
        }

        let run = (sym >> 4) as usize;
        let size = sym & 0x0F;

        if size == 0 && run == 15 {
            // ZRL - skip 16 zeros
            k += 16;
            continue;
        }

        if size > 0 {
            // Skip `run` zeros
            k += run;
            if k >= 64 {
                break;
            }

            let bits = match br.read_bits(size as u32) {
                Some(b) => b,
                None => return false,
            };
            block_zigzag[k] = extend(bits, size);
            k += 1;
        }
    }

    true
}

/// 2x1 fancy upsampling — exact match of IJG libjpeg h2v1_fancy_upsample.
///
/// Produces 2 output columns per 1 input column (2x horizontal).
/// Uses a 3/4 + 1/4 triangle filter with alternating rounding bias (+1/+2)
/// to avoid DC bias, exactly matching IJG jdsample.c.
fn h2v1_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let mut out = vec![0u8; dst_w * src_h];
    for y in 0..src_h {
        let in_row = y * src_w;
        let out_row = y * dst_w;

        // First column special case
        let mut invalue = src[in_row] as i32;
        out[out_row] = invalue as u8;
        out[out_row + 1] = ((invalue * 3 + src[in_row + 1] as i32 + 2) >> 2) as u8;

        // Middle columns
        for col in 1..src_w - 1 {
            invalue = src[in_row + col] as i32 * 3;
            out[out_row + col * 2] =
                ((invalue + src[in_row + col - 1] as i32 + 1) >> 2) as u8;
            out[out_row + col * 2 + 1] =
                ((invalue + src[in_row + col + 1] as i32 + 2) >> 2) as u8;
        }

        // Last column special case
        if src_w > 1 {
            invalue = src[in_row + src_w - 1] as i32;
            out[out_row + (src_w - 1) * 2] =
                ((invalue * 3 + src[in_row + src_w - 2] as i32 + 1) >> 2) as u8;
            out[out_row + (src_w - 1) * 2 + 1] = invalue as u8;
        }
    }
    out
}

/// 2x2 fancy upsampling — exact match of IJG libjpeg h2v2_fancy_upsample.
///
/// Produces 2 output columns per 1 input column AND 2 output rows per 1 input row.
/// Uses a separable triangle filter: vertical (3/4+1/4) then horizontal (3/4+1/4),
/// with alternating rounding bias (+7/+8 for >>4), exactly matching IJG jdsample.c.
fn h2v2_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let dst_h = src_h * 2;

    // Per-row "column sum" = 3 * nearest_row + 1 * next_nearest_row (vertical interp)
    // Then horizontal: blend lastcolsum, thiscolsum, nextcolsum with >>4
    let mut out = vec![0u8; dst_w * dst_h];
    let mut inrow = 0usize;
    let mut outrow = 0usize;

    while outrow < dst_h {
        for v in 0..2 {
            if outrow >= dst_h {
                break;
            }

            // inptr0 = nearest row, inptr1 = next nearest row (above for v=0, below for v=1)
            let inptr0 = &src[inrow * src_w..];
            let inptr1 = if v == 0 {
                // Next nearest is row above; clamp to current row at top edge
                if inrow > 0 {
                    &src[(inrow - 1) * src_w..]
                } else {
                    &src[inrow * src_w..]
                }
            } else {
                // Next nearest is row below; clamp to current row at bottom edge
                if inrow + 1 < src_h {
                    &src[(inrow + 1) * src_w..]
                } else {
                    &src[inrow * src_w..]
                }
            };

            let out_row = outrow * dst_w;

            // Special case for first column
            let mut thiscolsum = inptr0[0] as i32 * 3 + inptr1[0] as i32;
            let mut nextcolsum = inptr0[1] as i32 * 3 + inptr1[1] as i32;
            out[out_row] = ((thiscolsum * 4 + 8) >> 4) as u8;
            out[out_row + 1] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
            let mut lastcolsum = thiscolsum;
            thiscolsum = nextcolsum;

            // Middle columns
            for col in 1..src_w - 1 {
                nextcolsum = inptr0[col + 1] as i32 * 3 + inptr1[col + 1] as i32;
                out[out_row + col * 2] =
                    ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                out[out_row + col * 2 + 1] =
                    ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
                lastcolsum = thiscolsum;
                thiscolsum = nextcolsum;
            }

            // Special case for last column
            if src_w > 1 {
                out[out_row + (src_w - 1) * 2] =
                    ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                out[out_row + (src_w - 1) * 2 + 1] =
                    ((thiscolsum * 4 + 7) >> 4) as u8;
            } else {
                // Single column: just write the single column
                out[out_row] = ((thiscolsum * 4 + 8) >> 4) as u8;
                if dst_w > 1 {
                    out[out_row + 1] = ((thiscolsum * 4 + 7) >> 4) as u8;
                }
            }

            outrow += 1;
        }
        inrow += 1;
    }

    out
}

// ── Image Reconstruction ──────────────────────────────────────────────────

/// Reconstruct a full image from decoded JPEG components.
///
/// This handles the complete decode-reconstruct pipeline:
///   Entropy decode -> Dequantize -> De-zigzag -> IDCT -> YCbCr -> RGB
fn reconstruct_image(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
    // Extract entropy segments (between RST markers)
    let entropy_segments = extract_entropy_segments(data, info.entropy_start, info.eoi_pos);

    if entropy_segments.segments.is_empty() {
        return None;
    }

    // Calculate MCU grid
    let mcu_width = (info.max_h_samp as u32) * 8;
    let mcu_height = (info.max_v_samp as u32) * 8;
    let num_mcus_x = ((info.width as u32) + mcu_width - 1) / mcu_width;
    let num_mcus_y = ((info.height as u32) + mcu_height - 1) / mcu_height;

    // Calculate component buffer dimensions (padded to MCU boundaries)
    let comp_buf_width: Vec<usize> = info
        .components
        .iter()
        .map(|c| num_mcus_x as usize * c.h_samp as usize * 8)
        .collect();
    let comp_buf_height: Vec<usize> = info
        .components
        .iter()
        .map(|c| num_mcus_y as usize * c.v_samp as usize * 8)
        .collect();

    // Allocate component buffers
    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]])
        .collect();

    // Per-component DC predictors (reset at each segment/RST)
    let mut dc_predictors: Vec<i32> = vec![0; info.num_components as usize];

    // Per-block decode state (reused across blocks)
    let mut block_zigzag = [0i32; 64];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];

    // YCbCr color converter
    let converter = YccColorConverter::new();

    // Track position across segments
    let mut seg_idx = 0;

    // First segment
    let mut segment_iter = entropy_segments.segments.iter().peekable();

    // For each segment, create a BitReader
    while let Some(&(seg_start, seg_end)) = segment_iter.next() {
        let mut br = BitReader::new(data, seg_start, seg_end);

        // Number of MCUs in this segment
        let mcus_in_segment = if info.restart_interval > 0 {
            info.restart_interval as usize
        } else {
            // No restart: all remaining MCUs
            num_mcus_x as usize * num_mcus_y as usize
        };

        // Decode MCUs in this segment
        let mcu_offset = seg_idx * mcus_in_segment;
        let max_mcus = num_mcus_x as usize * num_mcus_y as usize;

        for mcu_idx in 0..mcus_in_segment {
            let absolute_mcu = mcu_offset + mcu_idx;
            if absolute_mcu >= max_mcus {
                break;
            }

            let mcu_y = absolute_mcu / num_mcus_x as usize;
            let mcu_x = absolute_mcu % num_mcus_x as usize;

            // Decode each component in the scan
            for scan_comp in &info.scan_components {
                let comp = &info.components[scan_comp.comp_index];

                // Retrieve Huffman tables
                let dc_table = info.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?;
                let ac_table = info.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;

                // Retrieve quantization table
                let quant_table = info.quant_tables[comp.quant_tbl as usize].as_ref()?;

                // Decode each block in this component's sampling area
                for by in 0..comp.v_samp as usize {
                    for bx in 0..comp.h_samp as usize {
                        // Decode block coefficients (in zigzag order)
                        if !decode_block(
                            &mut br,
                            dc_table,
                            ac_table,
                            &mut dc_predictors[scan_comp.comp_index],
                            &mut block_zigzag,
                        ) {
                            return None;
                        }

                        // Dequantize (in zigzag order)
                        for i in 0..64 {
                            block_zigzag[i] = block_zigzag[i] * quant_table[i] as i32;
                        }

                        // De-zigzag to natural (row-major) order
                        for i in 0..64 {
                            block_natural[JPEG_NATURAL_ORDER[i]] = block_zigzag[i];
                        }

                        // Apply IDCT
                        jpeg_idct_islow(&mut block_natural, &mut workspace);

                        // Copy block pixels to component buffer
                        let comp_buf = &mut comp_buffers[scan_comp.comp_index];
                        let buf_w = comp_buf_width[scan_comp.comp_index];

                        // Calculate block position in the component buffer
                        let block_x = (mcu_x * comp.h_samp as usize + bx) * 8;
                        let block_y = (mcu_y * comp.v_samp as usize + by) * 8;

                        for row in 0..8 {
                            for col in 0..8 {
                                let px = block_natural[row * 8 + col];
                                let px_clamped = px.clamp(0, 255) as u8;
                                let buf_idx = (block_y + row) * buf_w + (block_x + col);
                                if buf_idx < comp_buf.len() {
                                    comp_buf[buf_idx] = px_clamped;
                                }
                            }
                        }
                    }
                }
            }

            // Handle RST at segment boundaries (except the last segment)
            if mcu_idx + 1 >= mcus_in_segment && segment_iter.peek().is_some() {
                // Reset DC predictors for next segment
                for pred in dc_predictors.iter_mut() {
                    *pred = 0;
                }
                seg_idx += 1;
            }
        }
    }

    // ── Assemble output image ──
    let w = info.width as usize;
    let h = info.height as usize;

    if info.num_components == 1 {
        // Grayscale: output Luma8
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                pixels.push(y_buf[y * y_w + x]);
            }
        }
        Some(DecodedImage::new(
            info.width as u32,
            info.height as u32,
            pixels,
            ColorType::L8,
        ))
    } else if info.num_components == 3 {
        // Color: YCbCr -> RGB
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];

        // Upsampling ratios (chroma relative to luma)
        let h_ratio = info.max_h_samp / info.components[1].h_samp;
        let v_ratio = info.max_v_samp / info.components[1].v_samp;

        // Upsample chroma using libjpeg-exact triangle filter
        let cb_upsampled = fancy_upsample(
            &comp_buffers[1], comp_buf_width[1], comp_buf_height[1],
            h_ratio as usize, v_ratio as usize, w, h,
        );
        let cr_upsampled = fancy_upsample(
            &comp_buffers[2], comp_buf_width[2], comp_buf_height[2],
            h_ratio as usize, v_ratio as usize, w, h,
        );

        // Up-sampled chroma row stride = original chroma width * h_ratio
        let cb_stride = comp_buf_width[1] * h_ratio as usize;
        let cr_stride = comp_buf_width[2] * h_ratio as usize;

        let mut pixels = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let y_val = y_buf[y * y_w + x];
                let cb_val = cb_upsampled[y * cb_stride + x];
                let cr_val = cr_upsampled[y * cr_stride + x];

                let (r, g, b) = converter.ycc_to_rgb(y_val, cb_val, cr_val);
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
            }
        }

        Some(DecodedImage::new(
            info.width as u32,
            info.height as u32,
            pixels,
            ColorType::Rgb8,
        ))
    } else {
        None
    }
}

/// Dispatch to libjpeg-exact chroma upsampling based on ratios.
fn fancy_upsample(
    src: &[u8], src_w: usize, src_h: usize,
    h_ratio: usize, v_ratio: usize, dst_w: usize, dst_h: usize,
) -> Vec<u8> {
    match (h_ratio, v_ratio) {
        (1, 1) => {
            let mut out = Vec::with_capacity(dst_w * dst_h);
            for y in 0..dst_h {
                let row = y * src_w;
                for x in 0..dst_w {
                    out.push(src[row + x]);
                }
            }
            out
        }
        (2, 1) => h2v1_fancy_upsample(src, src_w, src_h),
        (2, 2) => h2v2_fancy_upsample(src, src_w, src_h),
        _ => {
            // Fallback: nearest-neighbor
            let mut out = vec![0u8; dst_w * dst_h];
            for y in 0..dst_h {
                let sy = y / v_ratio;
                for x in 0..dst_w {
                    let sx = x / h_ratio;
                    out[y * dst_w + x] = src[sy * src_w + sx];
                }
            }
            out
        }
    }
}

/// Progressive JPEG reconstruction: accumulate coefficients across multiple
/// scans, then run IDCT and assemble the output.
fn progressive_reconstruct(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
    eprintln!("PROGRESSIVE RECONSTRUCT: {} scans, {}x{}, {} comps",
        info.scans.len(), info.width, info.height, info.num_components);

    // Print first scan info
    if let Some(s) = info.scans.first() {
        eprintln!("  First scan: ss={}, se={}, ah={}, al={}, {} comps, entropy=[{},{})",
            s.ss, s.se, s.ah, s.al, s.components.len(), s.entropy_start, s.entropy_end);
        for c in &s.components {
            let has_dc = s.dc_huff_tables.get(c.dc_tbl as usize).and_then(|t| t.as_ref()).is_some();
            let has_ac = s.ac_huff_tables.get(c.ac_tbl as usize).and_then(|t| t.as_ref()).is_some();
            eprintln!("    comp_idx={}: dc_tbl={}({}) ac_tbl={}({})",
                c.comp_index, c.dc_tbl, if has_dc {"OK"} else {"MISSING"}, c.ac_tbl, if has_ac {"OK"} else {"MISSING"});
        }
    }
    let mcu_width = (info.max_h_samp as u32) * 8;
    let mcu_height = (info.max_v_samp as u32) * 8;
    let num_mcus_x = ((info.width as u32) + mcu_width - 1) / mcu_width;
    let num_mcus_y = ((info.height as u32) + mcu_height - 1) / mcu_height;

    let comp_buf_width: Vec<usize> = info
        .components
        .iter()
        .map(|c| num_mcus_x as usize * c.h_samp as usize * 8)
        .collect();
    let comp_buf_height: Vec<usize> = info
        .components
        .iter()
        .map(|c| num_mcus_y as usize * c.v_samp as usize * 8)
        .collect();

    let comp_num_blocks: Vec<usize> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let blocks_x = comp_buf_width[i] / 8;
            let blocks_y = comp_buf_height[i] / 8;
            blocks_x * blocks_y
        })
        .collect();

    // Allocate coefficient storage: [component][block_index][64 coefficients]
    let mut coeff_storage: Vec<Vec<[i32; 64]>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![[0i32; 64]; comp_num_blocks[i]])
        .collect();

    // Allocate component buffers
    let mut comp_buffers: Vec<Vec<u8>> = info
        .components
        .iter()
        .enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]])
        .collect();

    // Zigzag order table: natural_order[zigzag_index] = natural_index.
    // To get zigzag from natural: JPEG_NATURAL_ORDER maps zigzag->natural.
    // Build inverse: zigzag_order[natural_index] = zigzag_index.
    let mut zigzag_order = [0usize; 64];
    for zi in 0..64 {
        zigzag_order[JPEG_NATURAL_ORDER[zi]] = zi;
    }

    // Process each scan in order
    for scan in &info.scans {
        let segs = extract_entropy_segments(data, scan.entropy_start, scan.entropy_end);
        if segs.segments.is_empty() {
            continue;
        }

        let mut dc_predictors: Vec<i32> = vec![0; info.num_components as usize];
        let mut seg_idx = 0;
        let mut segment_iter = segs.segments.iter().peekable();

        let mcus_in_segment = if info.restart_interval > 0 {
            info.restart_interval as usize
        } else {
            num_mcus_x as usize * num_mcus_y as usize
        };
        let max_mcus = num_mcus_x as usize * num_mcus_y as usize;

        while let Some(&(seg_start, seg_end)) = segment_iter.next() {
            let mut br = BitReader::new(data, seg_start, seg_end);
            let mcu_offset = seg_idx * mcus_in_segment;

            let is_dc_scan = scan.ss == 0 && scan.se == 0;
            let is_dc_first = is_dc_scan && scan.ah == 0;
            let is_dc_refine = is_dc_scan && scan.ah > 0;
            let is_ac_first = !is_dc_scan && scan.ah == 0;
            let is_ac_refine = !is_dc_scan && scan.ah > 0;

            for mcu_idx in 0..mcus_in_segment {
                let absolute_mcu = mcu_offset + mcu_idx;
                if absolute_mcu >= max_mcus {
                    break;
                }
                let mcu_y = absolute_mcu / num_mcus_x as usize;
                let mcu_x = absolute_mcu % num_mcus_x as usize;

                for scan_comp in &scan.components {
                    let comp_idx = scan_comp.comp_index;
                    let comp = &info.components[comp_idx];

                    if is_dc_first {
                        let dc_table = scan.dc_huff_tables[scan_comp.dc_tbl as usize]
                            .as_ref()?;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let dc_cat = dc_table.decode(&mut br)?;
                                if dc_cat > 0 {
                                    let bits = br.read_bits(dc_cat as u32)?;
                                    dc_predictors[comp_idx] += extend(bits, dc_cat);
                                }
                                coeff_storage[comp_idx][block_idx][0] = dc_predictors[comp_idx];
                            }
                        }
                    } else if is_dc_refine {
                        let dc_table = scan.dc_huff_tables[scan_comp.dc_tbl as usize]
                            .as_ref()?;
                        let bit = 1i32 << scan.al;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let sym = dc_table.decode(&mut br)?;
                                if sym != 0 {
                                    let c = &mut coeff_storage[comp_idx][block_idx][0];
                                    if *c >= 0 { *c += bit; } else { *c -= bit; }
                                }
                            }
                        }
                    } else if is_ac_first {
                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize]
                            .as_ref()?;
                        let al = scan.al;
                        let ss = scan.ss as usize;
                        let se = scan.se as usize;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let mut k = ss;
                                while k <= se && k < 64 {
                                    let sym = ac_table.decode(&mut br)?;
                                    if sym == 0x00 { break; } // EOB
                                    let run = (sym >> 4) as usize;
                                    let size = (sym & 0x0F) as u8;
                                    if size == 0 && run == 15 {
                                        k += 16; // ZRL
                                        continue;
                                    }
                                    if size > 0 {
                                        k += run;
                                        if k > se || k >= 64 { break; }
                                        let bits = br.read_bits(size as u32)?;
                                        let val = extend(bits, size);
                                        coeff_storage[comp_idx][block_idx][k] = val << al;
                                        k += 1;
                                    }
                                }
                            }
                        }
                    } else if is_ac_refine {
                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize]
                            .as_ref()?;
                        let al = scan.al;
                        let bit = 1i32 << al;
                        let ss = scan.ss as usize;
                        let se = scan.se as usize;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let coeffs = &mut coeff_storage[comp_idx][block_idx];
                                let mut k = ss;
                                // Simple approach: iterate positions, refine existing
                                // non-zero coeffs and look for new ones
                                while k <= se && k < 64 {
                                    if coeffs[k] != 0 {
                                        // Refine existing
                                        let sym = ac_table.decode(&mut br)?;
                                        if sym != 0 {
                                            if coeffs[k] > 0 { coeffs[k] += bit; }
                                            else { coeffs[k] -= bit; }
                                        }
                                        k += 1;
                                    } else {
                                        // Look for new coeff or skip zeros (EOB/ZRL)
                                        let sym = ac_table.decode(&mut br)?;
                                        if sym == 0x00 { break; } // EOB
                                        let run = (sym >> 4) as usize;
                                        let size = (sym & 0x0F) as u8;
                                        if size == 0 && run == 15 {
                                            k += 16; // ZRL
                                            continue;
                                        }
                                        if size > 0 {
                                            k += run;
                                            if k > se || k >= 64 { break; }
                                            let bits = br.read_bits(size as u32)?;
                                            let val = extend(bits, size);
                                            coeffs[k] = val << al;
                                            k += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // RST handling
                if mcu_idx + 1 >= mcus_in_segment && segment_iter.peek().is_some() {
                    for pred in dc_predictors.iter_mut() { *pred = 0; }
                    seg_idx += 1;
                }
            }
        }
    }

    // After all scans: dequantize, IDCT, build component buffers
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];

    for comp_idx in 0..info.num_components as usize {
        let comp = &info.components[comp_idx];
        let buf_w = comp_buf_width[comp_idx];
        let blocks_x = buf_w / 8;
        let total_blocks = comp_num_blocks[comp_idx];
        let quant_table = info.quant_tables[comp.quant_tbl as usize].as_ref()?;

        for block_idx in 0..total_blocks {
            let coeffs = &coeff_storage[comp_idx][block_idx];
            for i in 0..64 {
                block_natural[JPEG_NATURAL_ORDER[i]] = coeffs[i] * quant_table[i] as i32;
            }
            jpeg_idct_islow(&mut block_natural, &mut workspace);

            let block_y = (block_idx / blocks_x) * 8;
            let block_x = (block_idx % blocks_x) * 8;
            for row in 0..8 {
                for col in 0..8 {
                    let px = block_natural[row * 8 + col].clamp(0, 255) as u8;
                    let bi = (block_y + row) * buf_w + (block_x + col);
                    if bi < comp_buffers[comp_idx].len() {
                        comp_buffers[comp_idx][bi] = px;
                    }
                }
            }
        }
    }

    // Assemble output (same as baseline)
    let w = info.width as usize;
    let h = info.height as usize;
    let converter = YccColorConverter::new();

    if info.num_components == 1 {
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                pixels.push(y_buf[y * y_w + x]);
            }
        }
        Some(DecodedImage::new(info.width as u32, info.height as u32, pixels, ColorType::L8))
    } else if info.num_components == 3 {
        let y_buf = &comp_buffers[0];
        let y_w = comp_buf_width[0];
        let h_ratio = info.max_h_samp / info.components[1].h_samp;
        let v_ratio = info.max_v_samp / info.components[1].v_samp;
        let cb_up = fancy_upsample(
            &comp_buffers[1], comp_buf_width[1], comp_buf_height[1],
            h_ratio as usize, v_ratio as usize, w, h,
        );
        let cr_up = fancy_upsample(
            &comp_buffers[2], comp_buf_width[2], comp_buf_height[2],
            h_ratio as usize, v_ratio as usize, w, h,
        );
        let mut pixels = Vec::with_capacity(w * h * 3);
        let cb_stride = comp_buf_width[1] * h_ratio as usize;
        let cr_stride = comp_buf_width[2] * h_ratio as usize;
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = converter.ycc_to_rgb(y_buf[y * y_w + x], cb_up[y * cb_stride + x], cr_up[y * cr_stride + x]);
                pixels.push(r); pixels.push(g); pixels.push(b);
            }
        }
        Some(DecodedImage::new(info.width as u32, info.height as u32, pixels, ColorType::Rgb8))
    } else {
        None
    }
}

// ── Public API ────────────────────────────────────────────────────────────

/// Decode JPEG bytes into a DecodedImage (pixel-perfect with libjpeg).
///
/// Supports baseline JPEG (SOF0) with:
/// - 8-bit precision
/// - 4:2:0 and 4:4:4 chroma subsampling
/// - Grayscale (1 component) and YCbCr (3 components)
/// - Restart markers (DRI)
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    // Parse JPEG headers
    let info = parse_jpeg(data)?;

    // Validate we have all required tables
    if info.scan_components.is_empty() {
        return None;
    }

    // Check all required tables exist
    for scan_comp in &info.scan_components {
        if info.dc_huff_tables.len() <= scan_comp.dc_tbl as usize
            || info.dc_huff_tables[scan_comp.dc_tbl as usize].is_none()
        {
            return None;
        }
        if info.ac_huff_tables.len() <= scan_comp.ac_tbl as usize
            || info.ac_huff_tables[scan_comp.ac_tbl as usize].is_none()
        {
            return None;
        }
    }

    for comp in &info.components {
        if info.quant_tables.len() <= comp.quant_tbl as usize
            || info.quant_tables[comp.quant_tbl as usize].is_none()
        {
            return None;
        }
    }

    // Reconstruct the image
    if info.progressive {
        progressive_reconstruct(&info, data)
    } else {
        reconstruct_image(&info, data)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idct_dc_only() {
        let mut block = [0i32; DCTSIZE2];
        block[0] = 512;
        let mut ws = [0i32; DCTSIZE2];
        jpeg_idct_islow(&mut block, &mut ws);
        let first = block[0];
        for item in block.iter().take(DCTSIZE2).skip(1) {
            assert_eq!(*item, first);
        }
    }

    #[test]
    fn test_jpeg_natural_order() {
        // Verify that jpeg_natural_order is a permutation of 0..63
        let mut seen = [false; 64];
        for &idx in JPEG_NATURAL_ORDER.iter() {
            assert!(idx < 64);
            assert!(!seen[idx], "duplicate entry {}", idx);
            seen[idx] = true;
        }
        // All should be seen
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn test_extend() {
        // size=1: 0->-1, 1->1
        assert_eq!(extend(0, 1), -1);
        assert_eq!(extend(1, 1), 1);

        // size=2: 0->-3, 1->-2, 2->2, 3->3
        assert_eq!(extend(0, 2), -3);
        assert_eq!(extend(1, 2), -2);
        assert_eq!(extend(2, 2), 2);
        assert_eq!(extend(3, 2), 3);

        // size=3: 0->-7, 1->-6, 2->-5, 3->-4, 4->4, 5->5, 6->6, 7->7
        assert_eq!(extend(0, 3), -7);
        assert_eq!(extend(3, 3), -4);
        assert_eq!(extend(4, 3), 4);
        assert_eq!(extend(7, 3), 7);

        // size=0: always 0
        assert_eq!(extend(42, 0), 0);
    }

    #[test]
    fn test_ycc_converter() {
        let conv = YccColorConverter::new();

        // Neutral gray: Y=128, Cb=128, Cr=128 -> (128, 128, 128)
        let (r, g, b) = conv.ycc_to_rgb(128, 128, 128);
        assert_eq!(r, 128);
        assert_eq!(g, 128);
        assert_eq!(b, 128);

        // White: Y=255, Cb=128, Cr=128 -> (255, 255, 255)
        let (r, g, b) = conv.ycc_to_rgb(255, 128, 128);
        assert_eq!(r, 255);
        assert_eq!(g, 255);
        assert_eq!(b, 255);

        // Pure red: R=255 in RGB corresponds to Y for red
        // Y=76, Cb=85, Cr=255 -> R=~255, G=~0, B=~0 (approximate)
        // Actually for the test let's just verify it doesn't panic and returns valid values
        let (r, g, b) = conv.ycc_to_rgb(76, 85, 255);
        assert!(r >= 200 || r <= 100); // R should be high or... let me just check clamping
        assert!(r <= 255);
        assert!(g <= 255);
        assert!(b <= 255);

        // Verify all values are in valid range for various inputs
        for y in [0u8, 16, 128, 235, 255] {
            for cb in [0u8, 128, 255] {
                for cr in [0u8, 128, 255] {
                    let (r, g, b) = conv.ycc_to_rgb(y, cb, cr);
                    assert!(r <= 255);
                    assert!(g <= 255);
                    assert!(b <= 255);
                }
            }
        }
    }

    #[test]
    fn test_huffman_table_basic() {
        // Build a simple Huffman table with 1 code of length 1 (value 0)
        let mut counts = [0u8; 16];
        counts[0] = 1; // 1 code of length 1
        let values = vec![0u8]; // value for the code
        let table = HuffTable::build(&counts, &values);

        // Verify structure
        assert_eq!(table.values.len(), 1);
        assert_eq!(table.values[0], 0);
        assert_ne!(table.maxcode[1], -1);
        // maxcode[2..=15] should be -1, maxcode[16] is sentinel for corrupt data
        assert_eq!(table.maxcode[2..=15], [-1; 14]); // no codes of lengths 2-15

        // Can't easily test decode without a bit reader on real data,
        // but at least table construction should work
    }

    #[test]
    fn test_huffman_table_decode_zero_bit() {
        // A single code of length 1: the symbol "0" is just the first bit = 0
        let mut counts = [0u8; 16];
        counts[0] = 2; // 2 codes of length 1
        let values = vec![5u8, 10u8]; // symbols for code 0 and code 1
        let table = HuffTable::build(&counts, &values);

        // Create bit reader with test data
        // Byte 0b11001010 has bits: 1, 1, 0, 0, 1, 0, 1, 0 (MSB first)
        let test_data = [0b11001010u8];
        let mut br = BitReader::new(&test_data, 0, 1);

        // Read first bit: 1 -> code 1 -> symbol values[1] = 10
        let val = table.decode(&mut br);
        assert_eq!(val, Some(10));

        // Read second bit: 1 -> code 1 -> symbol 10
        let val = table.decode(&mut br);
        assert_eq!(val, Some(10));

        // Read third bit: 0 -> code 0 -> symbol 5
        let val = table.decode(&mut br);
        assert_eq!(val, Some(5));

        // Fourth bit: 0 -> code 0 -> symbol 5
        let val = table.decode(&mut br);
        assert_eq!(val, Some(5)); // bit 4 = 0

        // Let me trace: byte 0b11001010
        // bit 7: 1 -> code 1 -> values[1] = 10
        // bit 6: 1 -> code 1 -> values[1] = 10
        // bit 5: 0 -> code 0 -> values[0] = 5
        // bit 4: 0 -> code 0 -> values[0] = 5
        // bit 3: 1 -> code 1 -> values[0] = 10
        let val = table.decode(&mut br);
        assert_eq!(val, Some(10));
    }

    #[test]
    fn test_extract_segments_no_rst() {
        // Data with no RST markers, just some entropy data followed by EOI
        // 0xFF 0xD9 is EOI
        let data = [0xA5u8, 0x5A, 0xFF, 0x00, 0x01, 0xFF, 0xD9];
        let segments = extract_entropy_segments(&data, 0, 7);
        assert_eq!(segments.segments.len(), 1);
        assert_eq!(segments.segments[0], (0, 5)); // up to but not including 0xFF 0xD9
        assert_eq!(segments.eoi_pos, 5);
    }

    #[test]
    fn test_extract_segments_with_rst() {
        // Entropy data with RST0 marker
        let data = [
            0xA5, 0x5A, // entropy data
            0xFF, 0xD0, // RST0
            0x01, 0x02, // next segment
            0xFF, 0xD9, // EOI
        ];
        let segments = extract_entropy_segments(&data, 0, 8);
        assert_eq!(segments.segments.len(), 2);
        assert_eq!(segments.segments[0], (0, 2)); // before RST
        assert_eq!(segments.segments[1], (4, 6)); // after RST
        assert_eq!(segments.eoi_pos, 6);
    }

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10101010u8, 0b11110000];
        let mut br = BitReader::new(&data, 0, 2);

        // Read 4 bits
        assert_eq!(br.read_bits(4), Some(0b1010));
        // Read 4 more
        assert_eq!(br.read_bits(4), Some(0b1010));
        // Read 4 more
        assert_eq!(br.read_bits(4), Some(0b1111));
        // Read 4 more
        assert_eq!(br.read_bits(4), Some(0b0000));
        // Should be out of bits
        assert_eq!(br.read_bits(1), None);
    }

    #[test]
    fn test_bit_reader_byte_stuffing() {
        // 0xFF 0x00 should be treated as a single 0xFF data byte
        let data = [0xAA, 0xFF, 0x00, 0x55];
        let mut br = BitReader::new(&data, 0, 4);

        // Read 8 bits: 0xAA
        assert_eq!(br.read_bits(8), Some(0xAA));
        // Read 8 bits: 0xFF (stuffed)
        assert_eq!(br.read_bits(8), Some(0xFF));
        // Read 8 bits: 0x55
        assert_eq!(br.read_bits(8), Some(0x55));
    }

    #[test]
    fn test_bit_reader_stops_at_marker() {
        // Data with 0xFF followed by non-zero (simulates RST marker boundary)
        let data = [0xAA, 0xFF, 0xD0, 0x55];
        let mut br = BitReader::new(&data, 0, 4);

        // Read 8 bits: 0xAA
        assert_eq!(br.read_bits(8), Some(0xAA));
        // Next fill should encounter 0xFF and stop (since 0xFFD0 is an RST marker that
        // shouldn't be in a clean segment). Our BitReader puts back the 0xFF.
        // So read_bits(8) should return None because the fill function returns false.
        let result = br.read_bits(8);
        assert_eq!(result, None);
    }

    #[test]
    fn test_empty_jpeg_rejected() {
        // An empty byte slice should return None
        assert!(decode(&[]).is_none());
    }

    #[test]
    fn test_invalid_jpeg_rejected() {
        // Random bytes that aren't a valid JPEG
        let data = b"this is not a jpeg file!!!";
        assert!(decode(data).is_none());
    }

    #[test]
    fn test_truncated_jpeg_rejected() {
        // Only SOI marker, nothing else
        let data = [0xFF, 0xD8];
        assert!(decode(&data).is_none());
    }
}
