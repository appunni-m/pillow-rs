use super::bit_reader::BitReader;
// ── Huffman Table ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub(super) struct HuffTable {
    values: Vec<u8>,
    maxcode: [i32; 17],
    valoffset: [i32; 17],
}

impl HuffTable {
    /// Build a derived Huffman table from the DHT marker data.
    pub(super) fn build(counts: &[u8; 16], values: &[u8]) -> Self {
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

        HuffTable {
            values: values.to_vec(),
            maxcode,
            valoffset,
        }
    }

    /// Decode one Huffman symbol from the bit reader.
    pub(super) fn decode(&self, br: &mut BitReader) -> Option<u8> {
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
