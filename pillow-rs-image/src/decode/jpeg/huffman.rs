// ── IJG-faithful Derived Huffman Table (libjpeg-turbo 3.1.4.1 jdhuff.c) ──
//
// Port of jpeg_make_d_derived_tbl + HUFF_DECODE macro + jpeg_huff_decode.
//
// Uses a lookahead table (HUFF_LOOKAHEAD=8, 256 entries) for fast-path decode
// of codes ≤8 bits, with a bit-by-bit fallback (jpeg_huff_decode) for longer codes.
//
// Table entry format (matching IJG d_derived_tbl.lookup):
//   (nb << 8) | symbol      for codes ≤ HUFF_LOOKAHEAD
//   (HUFF_LOOKAHEAD+1) << 8  for codes > HUFF_LOOKAHEAD  (= 0x0900 sentinel)

use super::bit_reader::BitReader;

const HUFF_LOOKAHEAD: u32 = 8;

#[derive(Debug, Clone)]
pub(super) struct HuffTable {
    /// Lookahead table: indexed by next 8 bits of input.
    /// Entry = (code_length << 8) | symbol, or 0x0900 if code > 8 bits.
    lookup: [u16; 256],
    /// Original Huffman symbol values (for slow-path index calculation).
    values: Vec<u8>,
    /// maxcode[l]: largest Huffman code of length l, or -1 if none.
    /// maxcode[17] is the sentinel (0x7FFFFFFF) ensuring termination.
    maxcode: [i32; 18],
    /// valoffset[l]: huffval[] index of 1st symbol of length l, minus the
    /// smallest code of length l.  Used in slow path: symbol = values[code + valoffset[l]].
    valoffset: [i32; 18],
}

impl HuffTable {
    // ── jpeg_make_d_derived_tbl ───────────────────────────────────────────

    /// Build a derived Huffman table from DHT marker data.
    /// `counts[l-1]` = number of codes of length l (1..16).
    /// `values` = symbol values in the order they appear in the DHT segment.
    pub(super) fn build(counts: &[u8; 16], values: &[u8]) -> Self {
        let numsymbols = values.len();

        // ── Generate Huffman codes (Figure F.15: code generation) ──
        let mut huffcode: Vec<i32> = vec![0; numsymbols];
        let mut code: i32 = 0;
        let mut p = 0usize;

        for l in 1..=16 {
            let cnt = counts[l - 1] as usize;
            for _ in 0..cnt {
                if p >= numsymbols {
                    break;
                }
                huffcode[p] = code;
                code += 1;
                p += 1;
            }
            code <<= 1;
        }

        // Validate codes: each code < 2^length
        p = 0;
        for l in 1..=16 {
            let cnt = counts[l - 1] as usize;
            for _ in 0..cnt {
                if p >= numsymbols {
                    break;
                }
                if huffcode[p] as i64 >= (1i64 << l) {
                    // Bad table — return a minimal valid table
                    return HuffTable::empty();
                }
                p += 1;
            }
        }

        // ── Build maxcode / valoffset (Figure F.15) ──
        let mut maxcode = [-1i32; 18];
        let mut valoffset = [0i32; 18];
        p = 0;
        for l in 1..=16 {
            if counts[l - 1] > 0 {
                valoffset[l] = p as i32 - huffcode[p];
                p += counts[l - 1] as usize;
                maxcode[l] = huffcode[p - 1];
            }
            // else: maxcode[l] stays -1 (no codes of this length)
        }
        valoffset[17] = 0;
        maxcode[17] = 0x7FFFFFi32; // IJG sentinel: 0xFFFFFL ensures termination

        // ── Build lookahead table ──
        // Initialize all entries to "too long" sentinel: (HUFF_LOOKAHEAD+1) << HUFF_LOOKAHEAD
        let mut lookup = [0x0900u16; 256]; // 9 << 8

        p = 0;
        for l in 1..=HUFF_LOOKAHEAD {
            let cnt = counts[(l - 1) as usize] as usize;
            for _ in 0..cnt {
                if p >= numsymbols {
                    break;
                }
                // Left-justify the code followed by all possible bit sequences
                let lookbits = (huffcode[p] << (HUFF_LOOKAHEAD - l)) as usize;
                let entry: u16 = ((l as u16) << 8) | values[p] as u16;
                let fill_count = 1usize << (HUFF_LOOKAHEAD - l);
                for ctr in 0..fill_count {
                    let idx = lookbits + ctr;
                    if idx < 256 {
                        lookup[idx] = entry;
                    }
                }
                p += 1;
            }
        }

        HuffTable {
            lookup,
            values: values.to_vec(),
            maxcode,
            valoffset,
        }
    }

    /// Return a minimal valid table (used when input table data is corrupt).
    fn empty() -> Self {
        let mut maxcode = [-1i32; 18];
        maxcode[17] = 0x7FFFFFi32;
        HuffTable {
            lookup: [0x0900u16; 256],
            values: vec![0],
            maxcode,
            valoffset: [0i32; 18],
        }
    }

    // ── HUFF_DECODE macro, inlined as a method ───────────────────────────

    /// Decode one Huffman symbol from the bit stream.
    /// Returns None if data is exhausted or corrupt.
    ///
    /// Implements the IJG HUFF_DECODE macro:
    ///   1. Fast path: PEEK_BITS(HUFF_LOOKAHEAD), index lookup table.
    ///   2. If code ≤ HUFF_LOOKAHEAD bits: DROP_BITS(nb), return symbol.
    ///   3. Slow path: jpeg_huff_decode — bit-by-bit traversal.
    pub(super) fn decode(&self, br: &mut BitReader) -> Option<u8> {
        // Ensure at least HUFF_LOOKAHEAD bits for the fast path
        if !br.ensure(HUFF_LOOKAHEAD) {
            // Not enough bits for lookahead — go directly to slow path
            return self.decode_slow(br, 1);
        }

        let look = br.peek_bits(HUFF_LOOKAHEAD) as usize;
        let entry = self.lookup[look];
        let nb = (entry >> 8) as u32; // code length, or HUFF_LOOKAHEAD+1 if too long

        if nb <= HUFF_LOOKAHEAD {
            // Fast path: code fits in lookahead
            br.drop_bits(nb);
            Some((entry & 0xFF) as u8)
        } else {
            // Slow path: code is > HUFF_LOOKAHEAD bits
            self.decode_slow(br, 1)
        }
    }

    // ── jpeg_huff_decode ─────────────────────────────────────────────────

    /// Slow-path Huffman decode: bit-by-bit traversal up to 16 bits.
    /// `min_bits` is the starting bit count (typically 1 after fast-path miss).
    fn decode_slow(&self, br: &mut BitReader, min_bits: u32) -> Option<u8> {
        // IJG: int l = min_bits; CHECK_BIT_BUFFER(*state, l, return -1);
        //      code = GET_BITS(l);
        let min = min_bits.max(1);
        if !br.ensure(min) {
            return None;
        }
        let mut code = br.get_bits(min) as i32;
        let mut l = min as usize;

        // IJG: while (code > htbl->maxcode[l]) {
        //        code <<= 1; CHECK_BIT_BUFFER(1); code |= GET_BITS(1); l++; }
        while code > self.maxcode[l] {
            l += 1;
            if l > 16 {
                // Corrupt data — reached sentinel at position 17
                return None;
            }
            if !br.ensure(1) {
                return None;
            }
            code = (code << 1) | (br.get_bits(1) as i32);
        }

        // IJG: return htbl->pub->huffval[code + htbl->valoffset[l]];
        let idx = (code + self.valoffset[l]) as usize;
        if idx < self.values.len() {
            Some(self.values[idx])
        } else {
            None
        }
    }
}
