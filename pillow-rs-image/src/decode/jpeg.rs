//! JPEG decoder — IJG DCT_ISLOW IDCT for pixel-perfect libjpeg parity.
//!
//! Implements libjpeg's exact "slow-but-accurate" integer IDCT from `jidctint.c`.
//! Uses CONST_BITS=13, PASS1_BITS=2 fixed-point arithmetic matching libjpeg-turbo.
//!
//! Supports:
//!   - Baseline JPEG (SOF0): 8-bit, 4:2:0/4:2:2/4:4:4/4:1:1 subsampling,
//!     grayscale/YCbCr, restart markers
//!   - Progressive JPEG (SOF2): DC-first, DC-refine, AC-first, AC-refine scans
//!
//! Reference: IJG libjpeg `jidctint.c`, `jdphuff.c`, `jdsample.c`
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
#[inline(always)]
fn mpy(v: i32, c: i32) -> i32 {
    (v as i64 * c as i64) as i32
}

#[inline(always)]
fn descale(x: i32, shift: i32) -> i32 {
    (x + (1 << (shift - 1))) >> shift
}

/// IJG-style range_limit: clamps (x + 128) to [0, 255].
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
        let v0 = block[c + DCTSIZE * 7];
        let v1 = block[c + DCTSIZE * 5];
        let v2 = block[c + DCTSIZE * 3];
        let v3 = block[c + DCTSIZE];
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
const JPEG_NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// Sign extension for DC/AC coefficient additional bits (Figure F.12).
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
            cr_r_tab[i] = ((91881i64 * x as i64 + 32768) >> 16) as i32;
            cb_b_tab[i] = ((116130i64 * x as i64 + 32768) >> 16) as i32;
            cr_g_tab[i] = (-46802i64 * x as i64) as i32;
            cb_g_tab[i] = ((-22554i64 * x as i64) + 32768) as i32;
        }

        YccColorConverter { cr_r_tab, cb_b_tab, cr_g_tab, cb_g_tab }
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

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u32,
    bits: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        BitReader { data, pos: start, end, buf: 0, bits: 0 }
    }

    /// Fill the bit buffer by reading bytes from the stream.
    /// Handles byte stuffing (0xFF 0x00 -> 0xFF data) and skips 0xFF padding.
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
                } else if (0xD0..=0xD7).contains(&next) {
                    // RST marker — skip both marker bytes, continue filling
                    self.pos += 1;
                } else {
                    // Other marker byte encountered — put back the 0xFF
                    self.pos -= 1;
                    return;
                }
            } else {
                self.buf = (self.buf << 8) | byte as u32;
                self.bits += 8;
            }
        }
    }

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
}

// ── Huffman Table ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct HuffTable {
    values: Vec<u8>,
    maxcode: [i32; 17],
    valoffset: [i32; 17],
}

impl HuffTable {
    /// Build a derived Huffman table from the DHT marker data.
    fn build(counts: &[u8; 16], values: &[u8]) -> Self {
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
        maxcode[16] = 0x7FFFFFFF;

        HuffTable { values: values.to_vec(), maxcode, valoffset }
    }

    /// Decode one Huffman symbol from the bit reader.
    fn decode(&self, br: &mut BitReader) -> Option<u8> {
        let mut code = br.read_bits(1)? as i32;
        let mut l = 1i32;

        while code > self.maxcode[l as usize] {
            l += 1;
            if l > 16 {
                return None;
            }
            let bit = br.read_bits(1)?;
            code = (code << 1) | (bit as i32);
        }

        let idx = code + self.valoffset[l as usize];
        if idx < 0 || idx >= self.values.len() as i32 {
            return None;
        }
        Some(self.values[idx as usize])
    }
}

// ── JPEG Structures ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
struct FrameComponent {
    id: u8,
    h_samp: u8,
    v_samp: u8,
    quant_tbl: u8,
}

#[derive(Debug, Clone, Copy)]
struct ScanComponent {
    comp_index: usize,
    dc_tbl: u8,
    ac_tbl: u8,
}

#[derive(Debug, Clone)]
struct ScanInfo {
    components: Vec<ScanComponent>,
    entropy_start: usize,
    entropy_end: usize,
    ss: u8,
    se: u8,
    ah: u8,
    al: u8,
    dc_huff_tables: Vec<Option<HuffTable>>,
    ac_huff_tables: Vec<Option<HuffTable>>,
}

struct JpegInfo {
    width: u16,
    height: u16,
    num_components: u8,
    components: Vec<FrameComponent>,
    quant_tables: Vec<Option<[u16; 64]>>,
    dc_huff_tables: Vec<Option<HuffTable>>,
    ac_huff_tables: Vec<Option<HuffTable>>,
    scan_components: Vec<ScanComponent>,
    restart_interval: u16,
    entropy_start: usize,
    eoi_pos: usize,
    max_h_samp: u8,
    max_v_samp: u8,
    progressive: bool,
    scans: Vec<ScanInfo>,
}

// ── JPEG Parser ───────────────────────────────────────────────────────────

fn read_u16(data: &[u8], pos: &mut usize) -> Option<u16> {
    if *pos + 1 >= data.len() { return None; }
    let val = (data[*pos] as u16) << 8 | data[*pos + 1] as u16;
    *pos += 2;
    Some(val)
}

fn read_u8(data: &[u8], pos: &mut usize) -> Option<u8> {
    if *pos >= data.len() { return None; }
    let val = data[*pos];
    *pos += 1;
    Some(val)
}

fn find_next_marker(data: &[u8], pos: &mut usize) -> Option<u16> {
    while *pos < data.len() {
        while *pos < data.len() && data[*pos] != 0xFF {
            *pos += 1;
        }
        if *pos >= data.len() { return None; }
        if *pos + 1 >= data.len() { return None; }
        let marker_byte = data[*pos + 1];
        if marker_byte == 0x00 || marker_byte == 0xFF {
            *pos += 1;
            continue;
        }
        let marker = 0xFF00u16 | marker_byte as u16;
        *pos += 2;
        return Some(marker);
    }
    None
}

fn find_entropy_end(data: &[u8], mut pos: usize) -> usize {
    while pos + 1 < data.len() {
        if data[pos] == 0xFF {
            let next = data[pos + 1];
            if next == 0x00 {
                pos += 2;
            } else if next >= 0xD0 && next <= 0xD7 {
                pos += 2;
            } else {
                return pos;
            }
        } else {
            pos += 1;
        }
    }
    data.len()
}

fn find_eoi(data: &[u8], mut pos: usize) -> Option<usize> {
    while pos + 1 < data.len() {
        if data[pos] == 0xFF && data[pos + 1] == 0xD9 {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

fn parse_sof0(data: &[u8], pos: &mut usize) -> Option<(u16, u16, Vec<FrameComponent>, u8, u8)> {
    let length = read_u16(data, pos)?;
    let precision = read_u8(data, pos)?;
    if precision != 8 { return None; }
    let height = read_u16(data, pos)?;
    let width = read_u16(data, pos)?;
    let num_components = read_u8(data, pos)?;
    if num_components != 1 && num_components != 3 { return None; }

    let mut components = Vec::with_capacity(num_components as usize);
    let mut max_h_samp = 0u8;
    let mut max_v_samp = 0u8;

    for _ in 0..num_components {
        let id = read_u8(data, pos)?;
        let sampling = read_u8(data, pos)?;
        let h_samp = sampling >> 4;
        let v_samp = sampling & 0x0F;
        let quant_tbl = read_u8(data, pos)?;
        if h_samp < 1 || h_samp > 4 || v_samp < 1 || v_samp > 4 { return None; }
        if quant_tbl > 3 { return None; }
        max_h_samp = max_h_samp.max(h_samp);
        max_v_samp = max_v_samp.max(v_samp);
        components.push(FrameComponent { id, h_samp, v_samp, quant_tbl });
    }

    Some((width, height, components, max_h_samp, max_v_samp))
}

fn parse_dqt(data: &[u8], pos: &mut usize, quant_tables: &mut Vec<Option<[u16; 64]>>) -> Option<()> {
    let length = read_u16(data, pos)?;
    let end = *pos + (length as usize) - 2;

    while *pos < end {
        let info = read_u8(data, pos)?;
        let precision = (info >> 4) as usize;
        let table_id = (info & 0x0F) as usize;
        if table_id >= 4 { return None; }

        let mut table_zigzag = [0u16; 64];
        for i in 0..64 {
            table_zigzag[i] = if precision == 0 {
                read_u8(data, pos)? as u16
            } else {
                read_u16(data, pos)?
            };
        }
        while quant_tables.len() <= table_id { quant_tables.push(None); }
        quant_tables[table_id] = Some(table_zigzag);
    }
    Some(())
}

fn parse_dht(
    data: &[u8], pos: &mut usize,
    dc_tables: &mut Vec<Option<HuffTable>>,
    ac_tables: &mut Vec<Option<HuffTable>>,
) -> Option<()> {
    let length = read_u16(data, pos)?;
    let end = *pos + (length as usize) - 2;

    while *pos < end {
        let info = read_u8(data, pos)?;
        let table_class = info >> 4;
        let table_id = (info & 0x0F) as usize;
        if table_id >= 4 { return None; }

        let mut counts = [0u8; 16];
        let mut total_values = 0usize;
        for i in 0..16 {
            counts[i] = read_u8(data, pos)?;
            total_values += counts[i] as usize;
        }

        let mut values = Vec::with_capacity(total_values);
        for _ in 0..total_values {
            values.push(read_u8(data, pos)?);
        }

        let table = HuffTable::build(&counts, &values);
        if table_class == 0 {
            while dc_tables.len() <= table_id { dc_tables.push(None); }
            dc_tables[table_id] = Some(table);
        } else {
            while ac_tables.len() <= table_id { ac_tables.push(None); }
            ac_tables[table_id] = Some(table);
        }
    }
    Some(())
}

fn parse_sos(data: &[u8], pos: &mut usize, components: &[FrameComponent]) -> Option<(Vec<ScanComponent>, usize, u8, u8, u8, u8)> {
    let _length = read_u16(data, pos)?;
    let num_scan_comps = read_u8(data, pos)?;
    if num_scan_comps == 0 { return None; }

    let mut scan_comps = Vec::with_capacity(num_scan_comps as usize);
    for _ in 0..num_scan_comps {
        let comp_id = read_u8(data, pos)?;
        let tbl_info = read_u8(data, pos)?;
        let dc_tbl = tbl_info >> 4;
        let ac_tbl = tbl_info & 0x0F;
        let comp_index = components.iter().position(|c| c.id == comp_id)?;
        if dc_tbl > 3 || ac_tbl > 3 { return None; }
        scan_comps.push(ScanComponent { comp_index, dc_tbl, ac_tbl });
    }

    let ss = read_u8(data, pos)?;
    let se = read_u8(data, pos)?;
    let ah_al = read_u8(data, pos)?;
    let ah = ah_al >> 4;
    let al = ah_al & 0x0F;
    let entropy_start = *pos;

    Some((scan_comps, entropy_start, ss, se, ah, al))
}

fn parse_dri(data: &[u8], pos: &mut usize) -> Option<u16> {
    let _len = read_u16(data, pos)?;
    let restart_interval = read_u16(data, pos)?;
    Some(restart_interval)
}

fn parse_jpeg(data: &[u8]) -> Option<JpegInfo> {
    let mut pos = 0usize;

    let soi = read_u16(data, &mut pos)?;
    if soi != M_SOI { return None; }

    let mut width = 0u16; let mut height = 0u16;
    let mut components: Vec<FrameComponent> = Vec::new();
    let mut num_components = 0u8;
    let mut max_h_samp = 0u8; let mut max_v_samp = 0u8;
    let mut quant_tables: Vec<Option<[u16; 64]>> = Vec::new();
    let mut dc_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut ac_huff_tables: Vec<Option<HuffTable>> = Vec::new();
    let mut scan_components: Vec<ScanComponent> = Vec::new();
    let mut restart_interval: u16 = 0;
    let mut entropy_start: Option<usize> = None;
    let mut saw_sof = false; let mut saw_sos = false;
    let mut progressive = false;
    let mut scans: Vec<ScanInfo> = Vec::new();

    loop {
        let marker = find_next_marker(data, &mut pos)?;

        match marker {
            M_SOF0 | M_SOF2 => {
                if saw_sof { return None; }
                progressive = marker == M_SOF2;
                let result = parse_sof0(data, &mut pos)?;
                width = result.0; height = result.1;
                components = result.2;
                max_h_samp = result.3; max_v_samp = result.4;
                num_components = components.len() as u8;
                saw_sof = true;
            }
            M_DQT => { parse_dqt(data, &mut pos, &mut quant_tables)?; }
            M_DHT => { parse_dht(data, &mut pos, &mut dc_huff_tables, &mut ac_huff_tables)?; }
            M_SOS => {
                if !saw_sof { return None; }
                let result = parse_sos(data, &mut pos, &components)?;
                let comps = result.0; let scan_start = result.1;
                let ss = result.2; let se = result.3; let ah = result.4; let al = result.5;
                let scan_end = find_entropy_end(data, pos);

                let scan_info = ScanInfo {
                    components: comps.clone(),
                    entropy_start: scan_start,
                    entropy_end: scan_end,
                    ss, se, ah, al,
                    dc_huff_tables: dc_huff_tables.clone(),
                    ac_huff_tables: ac_huff_tables.clone(),
                };
                scans.push(scan_info);

                if !progressive {
                    if scan_components.is_empty() { scan_components = comps; entropy_start = Some(scan_start); }
                    saw_sos = true;
                    if let Some(eoi) = find_eoi(data, pos) { pos = eoi; } else { pos = data.len(); }
                    break;
                } else {
                    saw_sos = true;
                    if scan_components.is_empty() { scan_components = comps; entropy_start = Some(scan_start); }
                    pos = scan_end;
                }
            }
            M_DRI => { restart_interval = parse_dri(data, &mut pos)?; }
            M_EOI => { break; }
            0xFFD0..=0xFFD7 => {}
            0xFF01 => {}
            _ => { let length = read_u16(data, &mut pos)? as usize; pos += length - 2; }
        }
    }

    if !saw_sos { return None; }
    let eoi_pos = find_eoi(data, 0).unwrap_or(data.len());

    Some(JpegInfo {
        width, height, num_components, components, quant_tables,
        dc_huff_tables, ac_huff_tables, scan_components, restart_interval,
        entropy_start: entropy_start?, eoi_pos, max_h_samp, max_v_samp,
        progressive, scans,
    })
}

// ── Entropy Decoding ──────────────────────────────────────────────────────

fn decode_block(
    br: &mut BitReader, dc_table: &HuffTable, ac_table: &HuffTable,
    last_dc: &mut i32, block_zigzag: &mut [i32; 64],
) -> bool {
    for coeff in block_zigzag.iter_mut() { *coeff = 0; }

    let dc_cat = match dc_table.decode(br) { Some(cat) => cat, None => return false };
    if dc_cat > 0 {
        let bits = match br.read_bits(dc_cat as u32) { Some(b) => b, None => return false };
        *last_dc += extend(bits, dc_cat);
    }
    block_zigzag[0] = *last_dc;

    let mut k = 1usize;
    while k < 64 {
        let sym = match ac_table.decode(br) { Some(s) => s, None => return false };
        if sym == 0x00 { break; }
        let run = (sym >> 4) as usize;
        let size = sym & 0x0F;
        if size == 0 && run == 15 {
            k += 16; continue;
        }
        if size > 0 {
            k += run;
            if k >= 64 { break; }
            let bits = match br.read_bits(size as u32) { Some(b) => b, None => return false };
            block_zigzag[k] = extend(bits, size);
            k += 1;
        }
    }
    true
}

// ── Chroma Upsampling (libjpeg-exact triangle filter) ─────────────────────

/// 2x1 fancy upsampling — exact match of IJG libjpeg h2v1_fancy_upsample.
fn h2v1_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let mut out = vec![0u8; dst_w * src_h];
    for y in 0..src_h {
        let in_row = y * src_w;
        let out_row = y * dst_w;

        let mut invalue = src[in_row] as i32;
        out[out_row] = invalue as u8;
        if src_w > 1 {
            out[out_row + 1] = ((invalue * 3 + src[in_row + 1] as i32 + 2) >> 2) as u8;
        } else {
            out[out_row + 1] = invalue as u8;
        }

        for col in 1..src_w - 1 {
            invalue = src[in_row + col] as i32 * 3;
            out[out_row + col * 2] = ((invalue + src[in_row + col - 1] as i32 + 1) >> 2) as u8;
            out[out_row + col * 2 + 1] = ((invalue + src[in_row + col + 1] as i32 + 2) >> 2) as u8;
        }

        if src_w > 1 {
            invalue = src[in_row + src_w - 1] as i32;
            out[out_row + (src_w - 1) * 2] = ((invalue * 3 + src[in_row + src_w - 2] as i32 + 1) >> 2) as u8;
            out[out_row + (src_w - 1) * 2 + 1] = invalue as u8;
        }
    }
    out
}

/// 2x2 fancy upsampling — exact match of IJG libjpeg h2v2_fancy_upsample.
fn h2v2_fancy_upsample(src: &[u8], src_w: usize, src_h: usize) -> Vec<u8> {
    let dst_w = src_w * 2;
    let dst_h = src_h * 2;
    let mut out = vec![0u8; dst_w * dst_h];
    let mut inrow = 0usize;
    let mut outrow = 0usize;

    while outrow < dst_h {
        for v in 0..2 {
            if outrow >= dst_h { break; }

            let inptr0 = &src[inrow * src_w..];
            let inptr1 = if v == 0 {
                if inrow > 0 { &src[(inrow - 1) * src_w..] } else { &src[inrow * src_w..] }
            } else {
                if inrow + 1 < src_h { &src[(inrow + 1) * src_w..] } else { &src[inrow * src_w..] }
            };

            let out_row = outrow * dst_w;

            let mut thiscolsum = inptr0[0] as i32 * 3 + inptr1[0] as i32;
            let mut nextcolsum = if src_w > 1 {
                inptr0[1] as i32 * 3 + inptr1[1] as i32
            } else {
                thiscolsum
            };
            out[out_row] = ((thiscolsum * 4 + 8) >> 4) as u8;
            out[out_row + 1] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
            let mut lastcolsum = thiscolsum;
            thiscolsum = nextcolsum;

            for col in 1..src_w - 1 {
                nextcolsum = inptr0[col + 1] as i32 * 3 + inptr1[col + 1] as i32;
                out[out_row + col * 2] = ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                out[out_row + col * 2 + 1] = ((thiscolsum * 3 + nextcolsum + 7) >> 4) as u8;
                lastcolsum = thiscolsum;
                thiscolsum = nextcolsum;
            }

            if src_w > 1 {
                out[out_row + (src_w - 1) * 2] = ((thiscolsum * 3 + lastcolsum + 8) >> 4) as u8;
                out[out_row + (src_w - 1) * 2 + 1] = ((thiscolsum * 4 + 7) >> 4) as u8;
            } else {
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

/// Crop a component buffer to the valid image-derived dimensions.
///
/// The component buffer is padded to MCU-aligned boundaries. Chroma data
/// beyond the actual image area must not be fed into the upsampler,
/// or the triangle filter blends garbage padding values at image edges.
fn crop_component(buf: &[u8], buf_w: usize, _buf_h: usize, crop_w: usize, crop_h: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(crop_w * crop_h);
    for y in 0..crop_h {
        let src_off = y * buf_w;
        out.extend_from_slice(&buf[src_off..src_off + crop_w]);
    }
    out
}

/// Dispatch to libjpeg-exact chroma upsampling based on ratios.
fn fancy_upsample(
    src: &[u8], src_w: usize, src_h: usize,
    h_ratio: usize, v_ratio: usize, _dst_w: usize, _dst_h: usize,
) -> Vec<u8> {
    match (h_ratio, v_ratio) {
        (1, 1) => {
            let mut out = Vec::with_capacity(src_w * src_h);
            for y in 0..src_h {
                let row = y * src_w;
                for x in 0..src_w {
                    out.push(src[row + x]);
                }
            }
            out
        }
        (2, 1) => h2v1_fancy_upsample(src, src_w, src_h),
        (2, 2) => h2v2_fancy_upsample(src, src_w, src_h),
        _ => {
            // Integer-only nearest-neighbor fallback for other ratios
            let out_w = src_w * h_ratio;
            let out_h = src_h * v_ratio;
            let mut out = vec![0u8; out_w * out_h];
            for y in 0..out_h {
                let sy = y / v_ratio;
                for x in 0..out_w {
                    let sx = x / h_ratio;
                    out[y * out_w + x] = src[sy * src_w + sx];
                }
            }
            out
        }
    }
}

// ── Image Reconstruction (baseline) ───────────────────────────────────────

fn reconstruct_image(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
    let mcu_width = (info.max_h_samp as u32) * 8;
    let mcu_height = (info.max_v_samp as u32) * 8;
    let num_mcus_x = ((info.width as u32) + mcu_width - 1) / mcu_width;
    let num_mcus_y = ((info.height as u32) + mcu_height - 1) / mcu_height;

    let comp_buf_width: Vec<usize> = info.components.iter()
        .map(|c| num_mcus_x as usize * c.h_samp as usize * 8).collect();
    let comp_buf_height: Vec<usize> = info.components.iter()
        .map(|c| num_mcus_y as usize * c.v_samp as usize * 8).collect();

    let mut comp_buffers: Vec<Vec<u8>> = info.components.iter().enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]]).collect();

    let mut dc_predictors: Vec<i32> = vec![0; info.num_components as usize];
    let mut block_zigzag = [0i32; 64];
    let mut block_natural = [0i32; 64];
    let mut workspace = [0i32; 64];
    let converter = YccColorConverter::new();

    // Extract entropy segments (between RST markers)
    let entropy_segments = extract_entropy_segments(data, info.entropy_start, info.eoi_pos);
    if entropy_segments.segments.is_empty() { return None; }

    let total_mcus = (num_mcus_x * num_mcus_y) as usize;
    let mut segment_iter = entropy_segments.segments.iter().peekable();
    let mut seg_idx = 0;
    let mcus_per_seg = if info.restart_interval > 0 {
        info.restart_interval as usize
    } else {
        total_mcus
    };

    while let Some(&(seg_start, seg_end)) = segment_iter.next() {
        let mut br = BitReader::new(data, seg_start, seg_end);
        let mcu_offset = seg_idx * mcus_per_seg;

        for mcu_idx in 0..mcus_per_seg {
            let absolute_mcu = mcu_offset + mcu_idx;
            if absolute_mcu >= total_mcus { break; }
            let mcu_y = absolute_mcu / num_mcus_x as usize;
            let mcu_x = absolute_mcu % num_mcus_x as usize;

            for scan_comp in &info.scan_components {
                let comp = &info.components[scan_comp.comp_index];
                let dc_table = info.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?;
                let ac_table = info.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                let quant_table = info.quant_tables[comp.quant_tbl as usize].as_ref()?;

                for by in 0..comp.v_samp as usize {
                    for bx in 0..comp.h_samp as usize {
                        if !decode_block(&mut br, dc_table, ac_table,
                                        &mut dc_predictors[scan_comp.comp_index], &mut block_zigzag) {
                            return None;
                        }
                        // Dequantize and IDCT
                        for i in 0..64 { block_zigzag[i] *= quant_table[i] as i32; }
                        for i in 0..64 { block_natural[JPEG_NATURAL_ORDER[i]] = block_zigzag[i]; }
                        jpeg_idct_islow(&mut block_natural, &mut workspace);

                        let buf_w = comp_buf_width[scan_comp.comp_index];
                        let block_x = (mcu_x * comp.h_samp as usize + bx) * 8;
                        let block_y = (mcu_y * comp.v_samp as usize + by) * 8;
                        for row in 0..8 {
                            for col in 0..8 {
                                let px = block_natural[row * 8 + col].clamp(0, 255) as u8;
                                let bi = (block_y + row) * buf_w + (block_x + col);
                                if bi < comp_buffers[scan_comp.comp_index].len() {
                                    comp_buffers[scan_comp.comp_index][bi] = px;
                                }
                            }
                        }
                    }
                }
            }

            // Handle RST at segment boundaries (except the last segment)
            if mcu_idx + 1 >= mcus_per_seg && segment_iter.peek().is_some() {
                for pred in dc_predictors.iter_mut() { *pred = 0; }
                seg_idx += 1;
            }
        }
    }

    // ── Assemble output image ──
    let w = info.width as usize;
    let h = info.height as usize;

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
        let h_ratio_us = h_ratio as usize;
        let v_ratio_us = v_ratio as usize;

        // Image-derived chroma dimensions (not MCU-padded)
        let chroma_src_w = (w + h_ratio_us - 1) / h_ratio_us;
        let chroma_src_h = (h + v_ratio_us - 1) / v_ratio_us;

        // Crop then upsample
        let cb_cropped = crop_component(
            &comp_buffers[1], comp_buf_width[1], comp_buf_height[1],
            chroma_src_w, chroma_src_h);
        let cr_cropped = crop_component(
            &comp_buffers[2], comp_buf_width[2], comp_buf_height[2],
            chroma_src_w, chroma_src_h);
        let cb_upsampled = fancy_upsample(
            &cb_cropped, chroma_src_w, chroma_src_h, h_ratio_us, v_ratio_us, w, h);
        let cr_upsampled = fancy_upsample(
            &cr_cropped, chroma_src_w, chroma_src_h, h_ratio_us, v_ratio_us, w, h);

        let chroma_stride = chroma_src_w * h_ratio_us;
        let mut pixels = Vec::with_capacity(w * h * 3);
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = converter.ycc_to_rgb(
                    y_buf[y * y_w + x],
                    cb_upsampled[y * chroma_stride + x],
                    cr_upsampled[y * chroma_stride + x]);
                pixels.push(r); pixels.push(g); pixels.push(b);
            }
        }
        Some(DecodedImage::new(info.width as u32, info.height as u32, pixels, ColorType::Rgb8))
    } else {
        None
    }
}

// ── Progressive JPEG Reconstruction ───────────────────────────────────────

fn progressive_reconstruct(info: &JpegInfo, data: &[u8]) -> Option<DecodedImage> {
    let mcu_width = (info.max_h_samp as u32) * 8;
    let mcu_height = (info.max_v_samp as u32) * 8;
    let num_mcus_x = ((info.width as u32) + mcu_width - 1) / mcu_width;
    let num_mcus_y = ((info.height as u32) + mcu_height - 1) / mcu_height;

    let comp_buf_width: Vec<usize> = info.components.iter()
        .map(|c| num_mcus_x as usize * c.h_samp as usize * 8).collect();
    let comp_buf_height: Vec<usize> = info.components.iter()
        .map(|c| num_mcus_y as usize * c.v_samp as usize * 8).collect();
    let comp_num_blocks: Vec<usize> = info.components.iter().enumerate()
        .map(|(i, _)| (comp_buf_width[i] / 8) * (comp_buf_height[i] / 8)).collect();

    // Allocate coefficient storage: [component][block_index][64 coefficients]
    // Coefficients are stored in ZIGZAG order (as decoded from Huffman symbols).
    // The final dequantize step converts zigzag → natural via JPEG_NATURAL_ORDER.
    let mut coeff_storage: Vec<Vec<[i32; 64]>> = info.components.iter().enumerate()
        .map(|(i, _)| vec![[0i32; 64]; comp_num_blocks[i]]).collect();
    // Component pixel buffers (padded to MCU boundaries, filled during final IDCT pass)
    let mut comp_buffers: Vec<Vec<u8>> = info.components.iter().enumerate()
        .map(|(i, _)| vec![128u8; comp_buf_width[i] * comp_buf_height[i]]).collect();

    // Process each scan in order
    for (scan_idx, scan) in info.scans.iter().enumerate() {
        // Extract entropy segments (split at RST markers within the scan data)
        let segs = extract_entropy_segments(data, scan.entropy_start, scan.entropy_end);
        if segs.segments.is_empty() { continue; }

        let is_dc_scan = scan.ss == 0 && scan.se == 0;
        let is_dc_first = is_dc_scan && scan.ah == 0;
        let is_dc_refine = is_dc_scan && scan.ah > 0;
        let is_ac_first = !is_dc_scan && scan.ah == 0;
        let is_ac_refine = !is_dc_scan && scan.ah > 0;

        let mcus_in_segment = if info.restart_interval > 0 {
            info.restart_interval as usize
        } else {
            (num_mcus_x * num_mcus_y) as usize
        };
        let max_mcus = (num_mcus_x * num_mcus_y) as usize;

        // Per-component EOBRUN state: persists across ALL blocks in this scan segment.
        // Follows IJG savable_state.EOBRUN — NOT reset per-MCU or per-block.
        let mut ac_refine_eobrun: u32 = 0;
        // DC predictors (reset at each RST segment boundary)
        let mut dc_predictors: Vec<i32> = vec![0; info.num_components as usize];

        for seg_idx in 0..segs.segments.len() {
            let (seg_start, seg_end) = segs.segments[seg_idx];
            let mut br = BitReader::new(data, seg_start, seg_end);
            let mcu_offset = seg_idx * mcus_in_segment;

            for mcu_idx in 0..mcus_in_segment {
                let absolute_mcu = mcu_offset + mcu_idx;
                if absolute_mcu >= max_mcus { break; }
                let mcu_y = absolute_mcu / num_mcus_x as usize;
                let mcu_x = absolute_mcu % num_mcus_x as usize;

                for scan_comp in &scan.components {
                    let comp_idx = scan_comp.comp_index;
                    let comp = &info.components[comp_idx];

                    if is_dc_first {
                        // ── DC first scan (Huffman-coded DC values) ──
                        let dc_table = scan.dc_huff_tables[scan_comp.dc_tbl as usize].as_ref()?;
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
                                coeff_storage[comp_idx][block_idx][0] = dc_predictors[comp_idx] << scan.al;
                            }
                        }
                    } else if is_dc_refine {
                        // ── DC refinement scan (1 raw bit per block) ──
                        let bit = 1i32 << scan.al;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let raw_bit = br.read_bits(1)?;
                                if raw_bit != 0 {
                                    coeff_storage[comp_idx][block_idx][0] |= bit;
                                }
                            }
                        }
                    } else if is_ac_first {
                        // ── AC first scan (Huffman-coded AC run-length) ──
                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let mut k = scan.ss as usize;
                                let se = scan.se as usize;
                                while k <= se && k < 64 {
                                    let sym = ac_table.decode(&mut br)?;
                                    if sym == 0x00 { break; } // EOB
                                    let run = (sym >> 4) as usize;
                                    let size = (sym & 0x0F) as u8;
                                    if size == 0 && run == 15 {
                                        k += 16; continue; // ZRL
                                    }
                                    if size > 0 {
                                        k += run;
                                        if k > se || k >= 64 { break; }
                                        let bits = br.read_bits(size as u32)?;
                                        let val = extend(bits, size);
                                        coeff_storage[comp_idx][block_idx][k] = val << scan.al;
                                        k += 1;
                                    }
                                }
                            }
                        }
                    } else if is_ac_refine {
                        // ── AC refinement scan (1-bit refinement + Huffman for new coeffs) ──
                        // Algorithm: IJG libjpeg-turbo `jdphuff.c` `decode_mcu_AC_refine`
                        //
                        // Key points:
                        //  - EOBRUN persists across ALL blocks in this scan segment (saved state)
                        //  - When eobrun > 0: skip all zero positions, refine non-zero ones, then eobrun--
                        //  - When eobrun == 0: decode Huffman at zero positions
                        //  - IJG has no inner block loop (blocks_in_MCU = 1 for AC refine);
                        //    our inline block loop is equivalent because eobrun crosses iterations

                        let ac_table = scan.ac_huff_tables[scan_comp.ac_tbl as usize].as_ref()?;
                        let p1 = 1i32 << scan.al;
                        let m1 = (-1i32) << scan.al;
                        let ss = scan.ss as usize;
                        let se = scan.se as usize;
                        for by in 0..comp.v_samp as usize {
                            for bx in 0..comp.h_samp as usize {
                                let block_idx = (mcu_y * comp.v_samp as usize + by)
                                    * (comp_buf_width[comp_idx] / 8)
                                    + (mcu_x * comp.h_samp as usize + bx);
                                let coeffs = &mut coeff_storage[comp_idx][block_idx];
                                let mut k = ss;

                                // Phase 1: Normal decode (only when eobrun == 0)
                                if ac_refine_eobrun == 0 {
                                    while k <= se && k < 64 {
                                        if coeffs[k] != 0 {
                                            // Refinement bit for existing non-zero coefficient
                                            let bit = br.read_bits(1)?;
                                            if bit != 0 {
                                                if coeffs[k] >= 0 { coeffs[k] += p1; }
                                                else { coeffs[k] += m1; }
                                            }
                                            k += 1;
                                        } else {
                                            let sym = ac_table.decode(&mut br)?;
                                            let run = (sym >> 4) as usize;
                                            let s_val = (sym & 0x0F) as u8;
                                            if s_val != 0 {
                                                // New non-zero coefficient
                                                let bit = br.read_bits(1)?;
                                                let val = if bit != 0 { p1 } else { m1 };
                                                // Skip run zeros (refine non-zero coeffs on the way)
                                                let mut r = run;
                                                loop {
                                                    if k > se || k >= 64 { break; }
                                                    if coeffs[k] != 0 {
                                                        let bit = br.read_bits(1)?;
                                                        if bit != 0 {
                                                            if coeffs[k] >= 0 { coeffs[k] += p1; }
                                                            else { coeffs[k] += m1; }
                                                        }
                                                    } else {
                                                        if r == 0 { break; }
                                                        r -= 1;
                                                    }
                                                    k += 1;
                                                }
                                                if k > se || k >= 64 { break; }
                                                coeffs[k] = val;
                                                k += 1;
                                            } else if run == 15 {
                                                // ZRL: skip 16 positions (refine non-zero on the way)
                                                let end = (k + 16).min(se + 1).min(64);
                                                while k < end {
                                                    if coeffs[k] != 0 {
                                                        let bit = br.read_bits(1)?;
                                                        if bit != 0 {
                                                            if coeffs[k] >= 0 { coeffs[k] += p1; }
                                                            else { coeffs[k] += m1; }
                                                        }
                                                    }
                                                    k += 1;
                                                }
                                            } else {
                                                // EOBRUN: remaining band is zero (across blocks)
                                                ac_refine_eobrun = 1u32 << run;
                                                if run > 0 {
                                                    ac_refine_eobrun |= br.read_bits(run as u32)?;
                                                }
                                                break; // → Phase 2
                                            }
                                        }
                                    }
                                }

                                // Phase 2: EOBRUN handler / band-end processing
                                while k <= se && k < 64 {
                                    if coeffs[k] != 0 {
                                        let bit = br.read_bits(1)?;
                                        if bit != 0 {
                                            if coeffs[k] >= 0 { coeffs[k] += p1; }
                                            else { coeffs[k] += m1; }
                                        }
                                    }
                                    k += 1;
                                }
                                if ac_refine_eobrun > 0 {
                                    ac_refine_eobrun -= 1;
                                }
                            }
                        }
                    }
                }

                // RST handling at segment boundaries
                if mcu_idx + 1 >= mcus_in_segment && seg_idx + 1 < segs.segments.len() {
                    for pred in dc_predictors.iter_mut() { *pred = 0; }
                }
            }
        }
    }

    // ── Final pass: dequantize, IDCT, build component buffers ──
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

    // ── Assemble output image ──
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
        let h_ratio_us = h_ratio as usize;
        let v_ratio_us = v_ratio as usize;
        let chroma_src_w = (w + h_ratio_us - 1) / h_ratio_us;
        let chroma_src_h = (h + v_ratio_us - 1) / v_ratio_us;
        let cb_cropped = crop_component(
            &comp_buffers[1], comp_buf_width[1], comp_buf_height[1],
            chroma_src_w, chroma_src_h);
        let cr_cropped = crop_component(
            &comp_buffers[2], comp_buf_width[2], comp_buf_height[2],
            chroma_src_w, chroma_src_h);
        let cb_up = fancy_upsample(
            &cb_cropped, chroma_src_w, chroma_src_h, h_ratio_us, v_ratio_us, w, h);
        let cr_up = fancy_upsample(
            &cr_cropped, chroma_src_w, chroma_src_h, h_ratio_us, v_ratio_us, w, h);
        let mut pixels = Vec::with_capacity(w * h * 3);
        let chroma_stride = chroma_src_w * h_ratio_us;
        for y in 0..h {
            for x in 0..w {
                let (r, g, b) = converter.ycc_to_rgb(y_buf[y * y_w + x], cb_up[y * chroma_stride + x], cr_up[y * chroma_stride + x]);
                pixels.push(r); pixels.push(g); pixels.push(b);
            }
        }
        Some(DecodedImage::new(info.width as u32, info.height as u32, pixels, ColorType::Rgb8))
    } else {
        None
    }
}

/// Split entropy data at RST markers into clean segments (no markers).
fn extract_entropy_segments(data: &[u8], start: usize, end_hint: usize) -> EntropySegments {
    let mut segments = Vec::new();
    let mut seg_start = start;
    let mut pos = start;
    let mut eoi_pos = 0;

    while pos < end_hint {
        if data[pos] == 0xFF {
            if pos + 1 >= end_hint { break; }
            match data[pos + 1] {
                0x00 => { pos += 2; }
                0xD0..=0xD7 => {
                    segments.push((seg_start, pos));
                    pos += 2;
                    seg_start = pos;
                }
                0xD9 => {
                    segments.push((seg_start, pos));
                    eoi_pos = pos;
                    break;
                }
                _ => {
                    pos += 2;
                    if pos + 1 < end_hint {
                        let len = (data[pos] as u16) << 8 | data[pos + 1] as u16;
                        pos += len as usize;
                    }
                }
            }
        } else {
            pos += 1;
        }
    }

    if seg_start < end_hint && eoi_pos == 0 {
        segments.push((seg_start, end_hint));
    }

    EntropySegments { segments, eoi_pos }
}

/// Entropy segment information (between RST/EOI markers).
struct EntropySegments {
    segments: Vec<(usize, usize)>,
    #[allow(dead_code)]
    eoi_pos: usize,
}

// ── Public API ────────────────────────────────────────────────────────────

/// Decode JPEG bytes into a DecodedImage (pixel-perfect with libjpeg).
///
/// Supports baseline JPEG (SOF0) and progressive JPEG (SOF2) with:
/// - 8-bit precision
/// - 4:2:0, 4:2:2, 4:4:4 and 4:1:1 chroma subsampling
/// - Grayscale (1 component) and YCbCr (3 components)
/// - Restart markers (DRI)
/// - Progressive: DC first, DC refine, AC first, AC refine scans
pub fn decode(data: &[u8]) -> Option<DecodedImage> {
    let info = parse_jpeg(data)?;

    if info.scan_components.is_empty() { return None; }

    for comp in &info.components {
        if info.quant_tables.len() <= comp.quant_tbl as usize
            || info.quant_tables[comp.quant_tbl as usize].is_none()
        { return None; }
    }

    if info.progressive {
        progressive_reconstruct(&info, data)
    } else {
        for scan_comp in &info.scan_components {
            if info.dc_huff_tables.len() <= scan_comp.dc_tbl as usize
                || info.dc_huff_tables[scan_comp.dc_tbl as usize].is_none()
            { return None; }
            if info.ac_huff_tables.len() <= scan_comp.ac_tbl as usize
                || info.ac_huff_tables[scan_comp.ac_tbl as usize].is_none()
            { return None; }
        }
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
        let mut seen = [false; 64];
        for &idx in JPEG_NATURAL_ORDER.iter() {
            assert!(idx < 64);
            assert!(!seen[idx], "duplicate entry {}", idx);
            seen[idx] = true;
        }
        assert!(seen.iter().all(|&s| s));
    }

    #[test]
    fn test_extend() {
        assert_eq!(extend(0, 1), -1);
        assert_eq!(extend(1, 1), 1);
        assert_eq!(extend(0, 2), -3);
        assert_eq!(extend(1, 2), -2);
        assert_eq!(extend(2, 2), 2);
        assert_eq!(extend(3, 2), 3);
        assert_eq!(extend(0, 3), -7);
        assert_eq!(extend(3, 3), -4);
        assert_eq!(extend(4, 3), 4);
        assert_eq!(extend(7, 3), 7);
        assert_eq!(extend(42, 0), 0);
    }

    #[test]
    fn test_ycc_converter() {
        let conv = YccColorConverter::new();
        let (r, g, b) = conv.ycc_to_rgb(128, 128, 128);
        assert_eq!(r, 128); assert_eq!(g, 128); assert_eq!(b, 128);
        let (r, g, b) = conv.ycc_to_rgb(255, 128, 128);
        assert_eq!(r, 255); assert_eq!(g, 255); assert_eq!(b, 255);
        let (r, g, b) = conv.ycc_to_rgb(76, 85, 255);
        assert!(r <= 255); assert!(g <= 255); assert!(b <= 255);
        for y in [0u8, 16, 128, 235, 255] {
            for cb in [0u8, 128, 255] {
                for cr in [0u8, 128, 255] {
                    let (r, g, b) = conv.ycc_to_rgb(y, cb, cr);
                    assert!(r <= 255); assert!(g <= 255); assert!(b <= 255);
                }
            }
        }
    }

    #[test]
    fn test_huffman_table_basic() {
        let mut counts = [0u8; 16];
        counts[0] = 1;
        let values = vec![0u8];
        let table = HuffTable::build(&counts, &values);
        assert_eq!(table.values.len(), 1);
        assert_eq!(table.values[0], 0);
        assert_ne!(table.maxcode[1], -1);
        assert_eq!(table.maxcode[2..=15], [-1; 14]);
    }

    #[test]
    fn test_huffman_table_decode_zero_bit() {
        let mut counts = [0u8; 16];
        counts[0] = 2;
        let values = vec![5u8, 10u8];
        let table = HuffTable::build(&counts, &values);
        let test_data = [0b11001010u8];
        let mut br = BitReader::new(&test_data, 0, 1);
        assert_eq!(table.decode(&mut br), Some(10));
        assert_eq!(table.decode(&mut br), Some(10));
        assert_eq!(table.decode(&mut br), Some(5));
        assert_eq!(table.decode(&mut br), Some(5));
        assert_eq!(table.decode(&mut br), Some(10));
    }

    #[test]
    fn test_extract_segments_no_rst() {
        let data = [0xA5u8, 0x5A, 0xFF, 0x00, 0x01, 0xFF, 0xD9];
        let segments = extract_entropy_segments(&data, 0, 7);
        assert_eq!(segments.segments.len(), 1);
        assert_eq!(segments.segments[0], (0, 5));
        assert_eq!(segments.eoi_pos, 5);
    }

    #[test]
    fn test_extract_segments_with_rst() {
        let data = [0xA5, 0x5A, 0xFF, 0xD0, 0x01, 0x02, 0xFF, 0xD9];
        let segments = extract_entropy_segments(&data, 0, 8);
        assert_eq!(segments.segments.len(), 2);
        assert_eq!(segments.segments[0], (0, 2));
        assert_eq!(segments.segments[1], (4, 6));
        assert_eq!(segments.eoi_pos, 6);
    }

    #[test]
    fn test_bit_reader_basic() {
        let data = [0b10101010u8, 0b11110000];
        let mut br = BitReader::new(&data, 0, 2);
        assert_eq!(br.read_bits(4), Some(0b1010));
        assert_eq!(br.read_bits(4), Some(0b1010));
        assert_eq!(br.read_bits(4), Some(0b1111));
        assert_eq!(br.read_bits(4), Some(0b0000));
        assert_eq!(br.read_bits(1), None);
    }

    #[test]
    fn test_bit_reader_byte_stuffing() {
        let data = [0xAA, 0xFF, 0x00, 0x55];
        let mut br = BitReader::new(&data, 0, 4);
        assert_eq!(br.read_bits(8), Some(0xAA));
        assert_eq!(br.read_bits(8), Some(0xFF));
        assert_eq!(br.read_bits(8), Some(0x55));
    }

    #[test]
    fn test_bit_reader_stops_at_marker() {
        let data = [0xAA, 0xFF, 0xD0, 0x55];
        let mut br = BitReader::new(&data, 0, 4);
        assert_eq!(br.read_bits(8), Some(0xAA));
        let result = br.read_bits(8);
        assert_eq!(result, None);
    }

    #[test]
    fn test_empty_jpeg_rejected() {
        assert!(decode(&[]).is_none());
    }

    #[test]
    fn test_invalid_jpeg_rejected() {
        let data = b"this is not a jpeg file!!!";
        assert!(decode(data).is_none());
    }

    #[test]
    fn test_truncated_jpeg_rejected() {
        let data = [0xFF, 0xD8];
        assert!(decode(&data).is_none());
    }
}
