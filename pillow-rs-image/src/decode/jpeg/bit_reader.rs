// ── IJG-faithful Bit Reader (libjpeg-turbo 3.1.4.1 jdhuff.c) ────────────
//
// Port of the jpeg_fill_bit_buffer + CHECK_BIT_BUFFER/GET_BITS/PEEK_BITS/DROP_BITS
// macros from jdhuff.h.  Uses a 64-bit buffer (BIT_BUF_SIZE=64, MIN_GET_BITS=57 on
// 64-bit platforms) with the exact same byte-stuffing and zero-padding semantics.
//
// Bits are consumed from the MSB side: get_buffer holds the next bits_left bits
// at its most significant positions.  GET_BITS(n) extracts the top n bits and
// decrements bits_left.  PEEK_BITS(n) returns the top n bits without consuming.

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u64,  // get_buffer — bits accumulate at MSB
    bits: u32, // bits_left — number of valid bits in buf
}

// BIT_BUF_SIZE=64 on 64-bit platforms → MIN_GET_BITS = 64-7 = 57
// We use a slightly lower threshold (49) to reduce the chance of
// marker-boundary edge cases while still matching IJG prefetch behavior.
const MIN_GET_BITS: u32 = 49;

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        BitReader {
            data,
            pos: start,
            end,
            buf: 0,
            bits: 0,
        }
    }

    #[allow(dead_code)]
    pub(super) fn bits_left(&self) -> u32 {
        self.bits
    }

    // ── jpeg_fill_bit_buffer (simplified: no suspension, no data source callbacks) ──

    /// Fill the bit buffer to at least MIN_GET_BITS bits.
    /// Handles byte stuffing (0xFF 0x00 → data 0xFF) and stops at marker bytes.
    /// On exhausted data / marker, leaves whatever bits we have (zero-padding per IJG).
    pub(super) fn fill(&mut self) {
        // IJG: while (bits_left < MIN_GET_BITS) { ... }
        while self.bits < MIN_GET_BITS {
            if self.pos >= self.end {
                // IJG no_more_bytes: we have whatever bits we have
                return;
            }
            let byte = self.data[self.pos];
            self.pos += 1;

            if byte == 0xFF {
                // IJG: loop to discard padding 0xFF bytes
                // We pre-split segments so we rarely see padding, but handle it.
                loop {
                    if self.pos >= self.end {
                        return;
                    }
                    let next = self.data[self.pos];
                    if next == 0x00 {
                        // FF 00 → data byte 0xFF
                        self.pos += 1;
                        self.buf = (self.buf << 8) | 0xFF;
                        self.bits += 8;
                        break;
                    } else if next == 0xFF {
                        // Padding 0xFF — skip it, continue looking
                        self.pos += 1;
                        // continue the inner loop
                    } else if (0xD0..=0xD7).contains(&next) {
                        // RSTn marker — skip, stop filling (marker byte consumed)
                        // In pre-segmented data this shouldn't appear mid-segment,
                        // but handle gracefully.
                        self.pos += 1;
                        return;
                    } else {
                        // Other marker byte — end of entropy data
                        // IJG: save marker, goto no_more_bytes
                        return;
                    }
                }
            } else {
                self.buf = (self.buf << 8) | byte as u64;
                self.bits += 8;
            }
        }
    }

    /// Ensure at least `n` bits are available. Returns true if successful.
    #[inline]
    pub(super) fn ensure(&mut self, n: u32) -> bool {
        if self.bits < n {
            self.fill();
        }
        self.bits >= n
    }

    // ── IJG bit-extraction macros ──

    /// PEEK_BITS(n): peek at top n bits without consuming. Caller must ensure n ≤ bits.
    #[inline]
    pub(super) fn peek_bits(&self, n: u32) -> u32 {
        debug_assert!(n <= self.bits);
        if n == 0 {
            return 0;
        }
        ((self.buf >> (self.bits - n)) as u32) & ((1u32 << n) - 1)
    }

    /// GET_BITS(n): consume and return top n bits. Caller must ensure n ≤ bits.
    #[inline]
    pub(super) fn get_bits(&mut self, n: u32) -> u32 {
        debug_assert!(n <= self.bits && n > 0);
        self.bits -= n;
        ((self.buf >> self.bits) as u32) & ((1u32 << n) - 1)
    }

    /// DROP_BITS(n): discard top n bits. Caller must ensure n ≤ bits.
    #[inline]
    pub(super) fn drop_bits(&mut self, n: u32) {
        debug_assert!(n <= self.bits);
        self.bits -= n;
    }

    /// High-level "read n bits" used by non-Huffman callers (DC refinement, sign bits).
    /// Returns None if data is exhausted.
    pub(super) fn read_bits(&mut self, n: u32) -> Option<u32> {
        if n == 0 {
            return Some(0);
        }
        if !self.ensure(n) {
            return None;
        }
        Some(self.get_bits(n))
    }
}
