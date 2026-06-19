//! Huffman table matching IJG jpeg_make_d_derived_tbl + jpeg_huff_decode.
//! See: /tmp/libjpeg_turbo/jdhuff.c:155 (make_derived_tbl), :449 (huff_decode)

use super::bit_reader::BitReader;

#[derive(Debug, Clone)]
pub(super) struct HuffTable {
    pub(super) values: Vec<u8>,     // IJG: huffval[]
    maxcode: [i32; 18],             // IJG: maxcode[0..17], maxcode[17]=sentinel
    valoffset: [i32; 18],           // IJG: valoffset[0..17]
    /// Lookup table for codes ≤ 8 bits: entry = (code_len << 8) | symbol
    #[allow(dead_code)]
    lookup: [u16; 256],             // IJG: lookup[1<<HUFF_LOOKAHEAD]
}

impl HuffTable {
    /// Build from BITS and HUFFVAL arrays. Matches IJG jpeg_make_d_derived_tbl.
    pub(super) fn build(counts: &[u8; 16], values: &[u8]) -> Self {
        // Figure C.1: make table of Huffman code length for each symbol
        let mut huffsize = [0u8; 257];
        let mut p = 0usize;
        for l in 1..=16 {
            let cnt = counts[l - 1] as usize;
            for _ in 0..cnt { huffsize[p] = l as u8; p += 1; }
        }
        let numsymbols = p;

        // Figure C.2: generate the codes themselves
        let mut huffcode = [0u32; 257];
        let mut code: u32 = 0;
        let mut si = huffsize[0] as usize;
        p = 0;
        while p < numsymbols {
            while p < numsymbols && huffsize[p] as usize == si {
                huffcode[p] = code; code += 1; p += 1;
            }
            code <<= 1; si += 1;
        }

        // Figure F.15: generate decoding tables
        let mut maxcode = [-1i32; 18];
        let mut valoffset = [0i32; 18];
        p = 0;
        for l in 1..=16 {
            let cnt = counts[l - 1] as usize;
            if cnt > 0 {
                valoffset[l] = p as i32 - huffcode[p] as i32;
                p += cnt;
                maxcode[l] = huffcode[p - 1] as i32;
            }
        }
        maxcode[17] = 0x7FFFFFFFi32; // IJG: ensures jpeg_huff_decode terminates

        // Compute lookahead table with IJG sentinel init
        // Sentinel = (HUFF_LOOKAHEAD+1) << HUFF_LOOKAHEAD = 9 << 8 = 2304
        let mut lookup = [0u16; 256];
        for i in 0..256 { lookup[i] = 9u16 << 8; }
        p = 0;
        for l in 1..=8u32 {
            for _ in 0..counts[l as usize - 1] as usize {
                let lookbits = (huffcode[p] << (8 - l)) as usize;
                let entry = ((l as u16) << 8) | values[p] as u16;
                for ctr in 0..(1u32 << (8 - l)) as usize {
                    lookup[lookbits + ctr] = entry;
                }
                p += 1;
            }
        }

        HuffTable { values: values.to_vec(), maxcode, valoffset, lookup }
    }

    /// Decode one Huffman symbol. Matches IJG HUFF_DECODE + jpeg_huff_decode.
    pub(super) fn decode(&self, br: &mut BitReader) -> Option<u8> {
        // IJG fast path with 8-bit lookahead
        if let Some(look) = br.peek_bits(8) {
            let entry = self.lookup[look as usize];
            let len = (entry >> 8) as u32;
            if len <= 8 {
                br.drop_bits(len);
                return Some(entry as u8);
            }
        }
        // IJG slow path: jpeg_huff_decode with l=1
        let mut code = br.read_bits(1)? as i32;
        let mut l = 1i32;
        while code > self.maxcode[l as usize] {
            l += 1;
            if l > 16 { return None; }
            code = (code << 1) | (br.read_bits(1)? as i32);
        }

        let idx = (code + self.valoffset[l as usize]) as usize;
        if idx >= self.values.len() { return None; }
        Some(self.values[idx])
    }
}
