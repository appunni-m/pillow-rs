// ── JPEG Quantization Tables (libjpeg-turbo 3.1.4.1 jcparam.c) ───────────
//
// Default luma/chroma tables and quality scaling.  At quality 75 libjpeg
// uses the standard IJG tables divided by 2 then clamped (the "scale_factor"
// path), producing the well-known q75 tables.

/// IJG std luminance quant table (jpeg_std_quant_table, val_base 8-bit).
pub(crate) const STD_LUMA_QT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 62, 99, 68, 56, 24, 35, 55, 64, 81, 104, 81,
    64, 49, 64, 78, 87, 103, 113, 92, 73, 72, 92, 95, 98, 112, 100, 103, 99,
];

/// IJG std chrominance quant table.
pub(crate) const STD_CHROMA_QT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

/// Per-component encode parameters.
pub(crate) struct EncodeParams {
    /// Quantization tables indexed by slot 0..3, in NATURAL order, as u16.
    /// (libjpeg stores quantval[] in natural order; emit_dqt reorders to
    /// zigzag at marker-write time.)
    pub quant_tables: Vec<[u16; 64]>,
    /// Quant slot assigned to each component (Y/Cb/Cr).
    pub comp_quant: Vec<u8>,
}

/// Build encode params for a given quality / subsampling / component count.
///
/// Mirrors jcparam.c jpeg_set_quality → jpeg_quality_scaling →
/// jpeg_add_quant_table.  Tables are kept in natural order (the order the
/// std tables are defined in and the order quantval[] is filled in).
pub(crate) fn build_params(quality: u8, subsampling: &str, num_components: usize) -> EncodeParams {
    let q = quality.clamp(1, 100) as i32;

    // IJG: if quality < 50, scale = 5000/quality; else scale = 200 - 2*quality.
    let scale_factor: i32 = if q < 50 { 5000 / q } else { 200 - 2 * q };

    // jpeg_add_quant_table: temp = (base * scale_factor + 50) / 100, clamped
    // 1..255 (force_baseline for 8-bit baseline).
    let scale = |base: u8| -> u16 {
        let v = ((base as i32) * scale_factor + 50) / 100;
        v.clamp(1, 255) as u16
    };

    let mut luma = [0u16; 64];
    let mut chroma = [0u16; 64];
    for i in 0..64 {
        luma[i] = scale(STD_LUMA_QT[i]);
        chroma[i] = scale(STD_CHROMA_QT[i]);
    }

    let mut quant_tables = vec![luma];
    if num_components >= 3 {
        // Cb and Cr share one chroma table (slot 1), as in libjpeg defaults.
        quant_tables.push(chroma);
    }

    // Component→quant slot. Y → 0; Cb,Cr → 1 (chroma table).
    let mut comp_quant = vec![0u8];
    if num_components >= 3 {
        comp_quant.push(1);
        comp_quant.push(1);
    }
    let _ = subsampling;

    EncodeParams {
        quant_tables,
        comp_quant,
    }
}
