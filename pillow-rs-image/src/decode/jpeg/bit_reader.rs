// ── Bit Reader ────────────────────────────────────────────────────────────

pub(super) struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    end: usize,
    buf: u32,
    bits: u32,
}

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

    /// Fill the bit buffer by reading bytes from the stream.
    /// Handles byte stuffing (0xFF 0x00 -> 0xFF data) and skips 0xFF padding.
    pub(super) fn fill(&mut self) {
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

    pub(super) fn read_bits(&mut self, n: u32) -> Option<u32> {
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
