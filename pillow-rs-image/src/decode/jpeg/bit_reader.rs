//! Bit reader matching IJG jpeg_fill_bit_buffer + GET_BITS semantics.
//! 64-bit buffer (like IJG on 64-bit), MIN_GET_BITS=15 fill threshold,
//! zero-padding on exhausted data (matching IJG no_more_bytes).
//! See: /tmp/libjpeg_turbo/jdhuff.c:299 (jpeg_fill_bit_buffer)

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u64,
    bits: u32,
}

const MIN_GET_BITS: u32 = 15;  // IJG default

impl<'a> BitReader<'a> {
    pub(super) fn new(data: &'a [u8], start: usize, end: usize) -> Self {
        BitReader { data, pos: start, end, buf: 0, bits: 0 }
    }

    /// Fill to at least MIN_GET_BITS. Matches IJG jpeg_fill_bit_buffer.
    /// Returns false if data is exhausted and we had to pad with zeros.
    fn fill(&mut self) -> bool {
        while self.bits < MIN_GET_BITS {
            if self.pos >= self.end {
                // IJG no_more_bytes: pad with zero bits
                self.buf <<= MIN_GET_BITS - self.bits;
                self.bits = MIN_GET_BITS;
                return false;
            }
            let byte = self.data[self.pos];
            self.pos += 1;

            if byte == 0xFF {
                // IJG: consume ALL consecutive 0xFF bytes
                loop {
                    if self.pos >= self.end {
                        // End of data after 0xFF — treat as marker, pad
                        self.buf <<= MIN_GET_BITS - self.bits;
                        self.bits = MIN_GET_BITS;
                        return false;
                    }
                    let next = self.data[self.pos];
                    self.pos += 1;
                    if next == 0x00 {
                        // FF 00 → data byte 0xFF
                        self.buf = (self.buf << 8) | 0xFFu64;
                        self.bits += 8;
                        break;
                    } else if (0xD0..=0xD7).contains(&next) {
                        // RSTn — skip, continue filling
                        break;
                    } else if next == 0xFF {
                        // Padding FF — continue loop
                        continue;
                    } else {
                        // Marker byte — IJG: pad, save marker, continue
                        self.buf <<= MIN_GET_BITS - self.bits;
                        self.bits = MIN_GET_BITS;
                        return false;
                    }
                }
            } else {
                self.buf = (self.buf << 8) | byte as u64;
                self.bits += 8;
            }
        }
        true
    }

    /// Read n bits. Matches IJG GET_BITS macro: extracts from TOP of buffer.
    pub(super) fn read_bits(&mut self, n: u32) -> Option<u32> {
        if n > self.bits && !self.fill() && n > self.bits {
            return None;
        }
        // GET_BITS: ((get_buffer >> (bits_left -= nbits)) & mask)
        self.bits -= n;
        let val = (self.buf >> self.bits) as u32 & ((1u32 << n) - 1);
        Some(val)
    }

    /// Peek ahead n bits without consuming. Matches IJG PEEK_BITS.
    #[allow(dead_code)]
    pub(super) fn peek_bits(&mut self, n: u32) -> Option<u32> {
        if n > self.bits && !self.fill() && n > self.bits {
            return None;
        }
        Some((self.buf >> (self.bits - n)) as u32 & ((1u32 << n) - 1))
    }

    /// Drop n bits. Matches IJG DROP_BITS.
    #[allow(dead_code)]
    pub(super) fn drop_bits(&mut self, n: u32) {
        self.bits = self.bits.saturating_sub(n);
    }
}
