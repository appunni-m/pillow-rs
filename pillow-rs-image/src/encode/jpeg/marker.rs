// ── JPEG marker writing (libjpeg-turbo 3.1.4.1 jcmarker.c) ───────────────

/// Helper: write a big-endian u16.
#[inline]
pub(crate) fn w16(out: &mut Vec<u8>, v: u16) {
    out.push((v >> 8) as u8);
    out.push(v as u8);
}

/// SOI (Start Of Image).
pub(crate) fn write_soi(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xFF, 0xD8]);
}

/// EOI (End Of Image).
pub(crate) fn write_eoi(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xFF, 0xD9]);
}

/// JPEG zigzag scan order (natural index → zigzag position).
const JPEG_NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// DQT (Define Quantization Table).  `slot` 0..3, `precision` 0 (8-bit) or 1 (16-bit).
/// `table` is 64 entries in NATURAL order; emitted in zigzag order per jcmarker.c.
pub(crate) fn write_dqt(out: &mut Vec<u8>, slot: u8, precision: u8, table: &[u16; 64]) {
    // Determine precision by max value (libjpeg emit_dqt picks 16-bit if any >255).
    let prec = if precision == 1 || table.iter().any(|&v| v > 255) {
        1u8
    } else {
        0u8
    };
    let sample_size = if prec == 1 { 2 } else { 1 };
    let length = 2u16 + 1 + 64 * sample_size as u16;
    out.extend_from_slice(&[0xFF, 0xDB]);
    w16(out, length);
    out.push((prec << 4) | (slot & 0x0F));
    for i in 0..64 {
        let v = table[JPEG_NATURAL_ORDER[i]];
        if prec == 1 {
            w16(out, v);
        } else {
            out.push(v as u8);
        }
    }
}

/// DHT (Define Huffman Table).
/// `class` 0=DC, 1=AC.  `slot` 0..3.  `bits` is the 16-entry BITS array;
/// `huffval` is the symbol list (length = sum of bits).
pub(crate) fn write_dht(out: &mut Vec<u8>, class: u8, slot: u8, bits: &[u8; 16], huffval: &[u8]) {
    let length = 2u16 + 1 + 16 + huffval.len() as u16;
    out.extend_from_slice(&[0xFF, 0xC4]);
    w16(out, length);
    out.push((class << 4) | (slot & 0x0F));
    out.extend_from_slice(bits);
    out.extend_from_slice(huffval);
}

/// SOF (Start Of Frame).  `marker` 0xC0 (baseline) or 0xC2 (progressive).
pub(crate) fn write_sof(
    out: &mut Vec<u8>,
    marker: u8,
    width: u16,
    height: u16,
    components: &[(u8, u8, u8, u8)], // (id, h_samp, v_samp, quant_slot)
) {
    let ncomp = components.len() as u16;
    let length = 2u16 + 1 + 2 + 2 + 1 + 3 * ncomp;
    out.extend_from_slice(&[0xFF, marker]);
    w16(out, length);
    out.push(8); // data precision (8 bit)
    w16(out, height);
    w16(out, width);
    out.push(ncomp as u8);
    for &(id, h, v, q) in components {
        out.push(id);
        out.push((h << 4) | v);
        out.push(q);
    }
}

/// SOS (Start Of Scan).
pub(crate) fn write_sos(
    out: &mut Vec<u8>,
    components: &[(u8, u8, u8)], // (id, dc_tbl, ac_tbl)
    ss: u8,
    se: u8,
    ah: u8,
    al: u8,
) {
    let ncomp = components.len() as u16;
    let length = 2u16 + 1 + 2 * ncomp + 3;
    out.extend_from_slice(&[0xFF, 0xDA]);
    w16(out, length);
    out.push(ncomp as u8);
    for &(id, dc, ac) in components {
        out.push(id);
        out.push((dc << 4) | ac);
    }
    out.push(ss);
    out.push(se);
    out.push((ah << 4) | al);
}

/// DRI (Define Restart Interval).
pub(crate) fn write_dri(out: &mut Vec<u8>, interval: u16) {
    out.extend_from_slice(&[0xFF, 0xDD]);
    w16(out, 4);
    w16(out, interval);
}

/// Write a RSTn marker (n in 0..7).
pub(crate) fn write_rst(out: &mut Vec<u8>, n: u8) {
    out.extend_from_slice(&[0xFF, 0xD0 + (n & 7)]);
}
