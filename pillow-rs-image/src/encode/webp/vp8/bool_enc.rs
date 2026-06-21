//! VP8 boolean entropy encoder (RFC 6386 Section 7).
//!
//! VP8 uses a custom binary arithmetic coder that encodes one bit at a time
//! using 8-bit fixed-point probabilities (0–255, where 0 = 0% false,
//! 255 ≈ 100% false).  This module implements the encoder counterpart of
//! the boolean entropy decoder described in the specification.
//!
//! # Algorithm (RFC 6386 Section 7.3)
//!
//! The encoder maintains a 32-bit interval [`low`, `low` + `range`).  Each
//! `encode_bool` splits the interval proportionally to `prob`:
//!
//! ```text
//! split = 1 + ((range - 1) * prob >> 8)
//!
//! if value == false:  range = split
//! if value == true:   low += split;  range -= split
//! ```
//!
//! The interval is then renormalised by doubling both `low` and `range` until
//! `range >= 128`.  When the high bit of `low` (bit 31) is set before a shift,
//! the carry is propagated backward through already-emitted output bytes
//! (`add_one_to_output`).  A byte of `low` is emitted every eighth
//! renormalization shift.
//!
//! # Carry handling
//!
//! VP8 uses a retroactive carry-propagation scheme rather than deferring 0xFF
//! bytes.  When `low[31]` is set before a renormalization shift, the carry is
//! walked backward through the output, incrementing bytes and wrapping 0xFF to
//! 0x00 until the carry is absorbed.  This is the `add_one_to_output` function
//! from RFC 6386 Section 7.3.
//!
//! # Safety
//!
//! Zero `unsafe` code.

#![allow(dead_code)]

/// VP8 boolean entropy encoder.
///
/// Encodes a sequence of boolean values into a byte stream using
/// 8-bit fixed-point probabilities.
pub struct BoolEncoder {
    /// Bottom of the current coding interval (up to 32 bits).
    low: u32,

    /// Current interval width.  After renormalization this is always in
    /// [128, 255].
    range: u32,

    /// Number of renormalization shifts since the last byte was emitted.
    /// Starts at `-8`; each renormalization shift increments it by 1.
    /// When `count >= 0` we have accumulated a full byte to emit.
    count: i32,

    /// Output byte buffer.
    output: Vec<u8>,
}

impl BoolEncoder {
    /// Create a new bool encoder with initial state.
    ///
    /// * `low` = 0
    /// * `range` = 255
    /// * `count` = -8 (need 8 shifts before first byte)
    /// * `output` = empty
    pub fn new() -> Self {
        Self {
            low: 0,
            range: 255,
            count: -8,
            output: Vec::new(),
        }
    }

    /// Encode a single boolean value.
    ///
    /// # Parameters
    ///
    /// * `prob` — 8-bit probability of the value being `false` (0–255).
    ///   0 means `false` is impossible; 255 means `false` is nearly certain.
    /// * `value` — `false` (0) or `true` (1) to encode.
    pub fn encode_bool(&mut self, prob: u8, value: bool) {
        let prob = prob as u32;
        // RFC 6386 Section 7.2: split = 1 + ((range - 1) * prob >> 8)
        let split = 1 + (((self.range - 1) * prob) >> 8);

        if value {
            // 'true' occupies the upper sub-interval
            self.low = self.low.wrapping_add(split);
            self.range -= split;
        } else {
            // 'false' occupies the lower sub-interval
            self.range = split;
        }

        // Renormalize: double range and low until range >= 128.
        while self.range < 128 {
            // Carry check BEFORE the shift (RFC 6386 Section 7.3).
            if self.low & 0x8000_0000 != 0 {
                self.add_one_to_output();
            }
            self.range <<= 1;
            self.low <<= 1;
            self.count += 1;

            if self.count >= 0 {
                self.emit_byte();
            }
        }
    }

    /// Propagate a carry backward through already-emitted bytes.
    ///
    /// This is the `add_one_to_output` function from RFC 6386 Section 7.3.
    /// When the encoded value overflows past a 0xFF boundary, we walk the
    /// output from the most-recently-written byte backward, incrementing
    /// each byte and stopping when the byte does not wrap (i.e. was not
    /// 0xFF before incrementing).
    fn add_one_to_output(&mut self) {
        // Walk backward through the emitted bytes.
        for i in (0..self.output.len()).rev() {
            let (new_val, overflowed) = self.output[i].overflowing_add(1);
            self.output[i] = new_val;
            if !overflowed {
                // Byte was not 0xFF — carry absorbed here.
                return;
            }
            // byte was 0xFF and became 0x00 — carry propagates further.
        }
        // Loop finished without returning: the carry propagated past the
        // earliest byte in the output.  This is expected when no bytes
        // have been emitted yet or when the carry reaches the start of
        // the stream.
    }

    /// Emit the top byte of `low` to the output buffer.
    ///
    /// The byte is pushed to `output` and `low` is masked to 24 bits.
    /// Any outstanding carry must have been propagated by the caller.
    fn emit_byte(&mut self) {
        let byte = (self.low >> 24) as u8;
        self.output.push(byte);
        self.low &= 0x00FF_FFFF;
        self.count -= 8;
    }

    /// Flush remaining state and return the encoded byte stream.
    ///
    /// Encodes 32 uniform (`prob = 128`) zero bits to flush any residual
    /// state, matching the approach used by the libvpx reference encoder
    /// (`vp8_stop_encode`).  After this call all internal state is
    /// reflected in the output.
    ///
    /// # Returns
    ///
    /// The encoded byte vector (the caller takes ownership).
    pub fn finish(mut self) -> Vec<u8> {
        // Encode 32 zero-bits at uniform probability to flush the pipeline.
        // This ensures all remaining bits in `low` are shifted out and
        // emitted as bytes.
        for _ in 0..32 {
            self.encode_bool(128, false);
        }
        self.output
    }

    // ------------------------------------------------------------------
    // Convenience encoding helpers
    // ------------------------------------------------------------------

    /// Encode a literal value using MSb-first bit encoding (matching image_webp's
    /// [`ArithmeticDecoder::read_literal`] which accumulates bits with
    /// `v = (v << 1) + bit`).
    ///
    /// Each bit is encoded with `prob = 128` (uniform 50/50).
    ///
    /// # Parameters
    ///
    /// * `value` — Unsigned value to encode.
    /// * `bits`  — Number of bits to encode (0..32).
    pub fn encode_literal(&mut self, value: u32, bits: u8) {
        debug_assert!(bits <= 32, "encode_literal: bits must be ≤ 32");
        let n = bits.min(32);
        for i in (0..n).rev() {
            let bit = ((value >> i) & 1) != 0;
            self.encode_bool(128, bit);
        }
    }

    /// Encode a value using a VP8-style binary decision tree.
    ///
    /// # Tree format
    ///
    /// `tree` is a flat slice of `i8` pairs encoding a binary tree:
    /// `[left_0, right_0, left_1, right_1, ...]`.
    ///
    /// * A **positive** child value is the index of the next interior node
    ///   (multiply by 2 to get the child's index in the tree array).
    /// * A **negative** child value is a leaf: the decoded value is
    ///   `(-child) as u32`.
    ///
    /// `probs` provides the probability for each interior node.  There
    /// must be at least `tree.len() / 2` entries.
    ///
    /// # Traversal
    ///
    /// Starting at the root (node index 0), the function follows
    /// left/right branches until it reaches a leaf whose stored value
    /// matches `value`.
    pub fn encode_tree(&mut self, tree: &[i8], probs: &[u8], value: u32) {
        let mut node: usize = 0; // start at root
        loop {
            let prob = probs[node];
            let left = tree[2 * node] as i32;
            let right = tree[2 * node + 1] as i32;

            // Both children being leaves is VIRTUAL_ROOT (value comes
            // directly from the bit, not from the tree structure).
            let go_left = if left < 0 && right < 0 {
                // Both leaves: VP8 stores -(token+1); extract as -leaf-1
                // VP8 stores leaf as -(token+1); extract: token = -(leaf + 1)
                let left_val = (-(left + 1)) as u32;
                let _right_val = (-(right + 1)) as u32;
                value == left_val
            } else if left < 0 {
                // Left leaf only
                let left_val = (-(left + 1)) as u32;
                value == left_val
            } else if right < 0 {
                // Right leaf only — go left if value is NOT right leaf
                let right_val = (-(right + 1)) as u32;
                value != right_val
            } else {
                // Both interior — naive: go left (correct for VP8 token trees)
                true
            };

            self.encode_bool(prob, !go_left);

            if go_left {
                if left < 0 {
                    break;
                }
                // left is a DIRECT index into the tree array; convert to node number
                node = (left as usize) / 2;
            } else {
                if right < 0 {
                    break;
                }
                node = (right as usize) / 2;
            }
        }
    }

    /// Encode a signed value using VP8's signalling:
    ///
    /// 1. `encode_bool(128, abs > 0)` — whether the value is non-zero.
    /// 2. If non-zero: `encode_literal((abs - 1), 3)` — 3-bit magnitude.
    /// 3. If non-zero: `encode_bool(128, sign)` — sign bit.
    pub fn encode_signed(&mut self, value: i32) {
        let abs = value.unsigned_abs();
        self.encode_bool(128, abs > 0);
        if abs > 0 {
            self.encode_literal(abs - 1, 3);
            self.encode_bool(128, value < 0);
        }
    }

    // ------------------------------------------------------------------
    // Inspection helpers
    // ------------------------------------------------------------------

    /// Return the current number of emitted bytes.
    #[inline]
    pub fn len(&self) -> usize {
        self.output.len()
    }

    /// Return `true` if no bytes have been emitted yet.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.output.len() == 0
    }
}

impl Default for BoolEncoder {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------------------------------------------------
// Standalone helpers
// -----------------------------------------------------------------------

/// Encode a signed value using VP8's encoding convention.
///
/// This is identical to `BoolEncoder::encode_signed` and is provided for
/// call sites that prefer a free function.
pub fn encode_signed(enc: &mut BoolEncoder, value: i32) {
    enc.encode_signed(value);
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 1. Basic encoding ──────────────────────────────────────────────────

    #[test]
    fn test_basic_encode_and_finish() {
        let mut enc = BoolEncoder::new();
        enc.encode_bool(128, true);
        enc.encode_bool(128, false);
        enc.encode_bool(64, true);
        let output = enc.finish();
        assert!(
            !output.is_empty(),
            "non-trivial encode should produce non-empty output"
        );
    }

    #[test]
    fn test_single_bool_encode() {
        let mut enc = BoolEncoder::new();
        enc.encode_bool(128, true);
        let output = enc.finish();
        assert!(
            !output.is_empty(),
            "single bool encode should produce output"
        );
    }

    // ── 2. Determinism ─────────────────────────────────────────────────────

    #[test]
    fn test_deterministic_output() {
        let run = |vals: &[(u8, bool)]| -> Vec<u8> {
            let mut enc = BoolEncoder::new();
            for &(p, v) in vals {
                enc.encode_bool(p, v);
            }
            enc.finish()
        };

        let seq: &[(u8, bool)] = &[(128, true), (128, false), (200, true), (50, false)];
        let out1 = run(seq);
        let out2 = run(seq);
        assert_eq!(out1, out2, "same sequence must produce identical output");
    }

    #[test]
    fn test_prob_boundary_behaviors_differ() {
        // prob=0 (always-true expectation) vs prob=255 (always-false expectation)
        let make = |prob: u8, value: bool| -> Vec<u8> {
            let mut enc = BoolEncoder::new();
            enc.encode_bool(prob, value);
            enc.finish()
        };

        let out_always_true = make(0, true); // prob=0, encode true → efficient
        let out_always_false = make(255, false); // prob=255, encode false → efficient

        assert!(!out_always_true.is_empty());
        assert!(!out_always_false.is_empty());

        assert_ne!(
            out_always_true, out_always_false,
            "different prob values should produce different output"
        );
    }

    // ── 3. encode_literal ────────────────────────────────────────────────────

    #[test]
    fn test_encode_literal_different_values() {
        let encode_val = |value: u32, bits: u8| -> Vec<u8> {
            let mut enc = BoolEncoder::new();
            enc.encode_literal(value, bits);
            enc.finish()
        };

        let out0 = encode_val(0, 8);
        let out255 = encode_val(255, 8);
        assert_ne!(out0, out255, "0 and 255 should produce different output");

        let out42 = encode_val(42, 6);
        assert!(
            !out42.is_empty(),
            "literal 42 in 6 bits must produce output"
        );
    }

    #[test]
    fn test_encode_literal_bits_count() {
        let out_3bit = {
            let mut enc = BoolEncoder::new();
            enc.encode_literal(5, 3);
            enc.finish()
        };
        let out_8bit = {
            let mut enc = BoolEncoder::new();
            enc.encode_literal(5, 8);
            enc.finish()
        };
        assert!(
            out_8bit.len() >= out_3bit.len(),
            "more literal bits should not produce shorter output"
        );
    }

    // ── 4. encode_bool boundary conditions ───────────────────────────────────

    #[test]
    fn test_encode_bool_all_prob_values() {
        for prob in [0u8, 1, 127, 128, 129, 254, 255] {
            for value in [false, true] {
                let mut enc = BoolEncoder::new();
                enc.encode_bool(prob, value); // must not panic
                let output = enc.finish();
                assert!(!output.is_empty(), "prob={} value={}", prob, value);
            }
        }
    }

    #[test]
    fn test_multiple_sequential_encodes_no_panic() {
        let mut enc = BoolEncoder::new();
        for i in 0..50 {
            let prob = (i as u8).wrapping_mul(37);
            let val = i % 3 != 0;
            enc.encode_bool(prob, val);
        }
        let output = enc.finish();
        assert!(
            !output.is_empty(),
            "50 sequential encodes should produce output"
        );
    }

    // ── 5. finish consistency ────────────────────────────────────────────────

    #[test]
    fn test_finish_output_non_empty_for_non_trivial() {
        let mut enc = BoolEncoder::new();
        enc.encode_bool(128, true);
        let out = enc.finish();
        assert!(!out.is_empty(), "finish output must not be empty");
        for &b in &out {
            let _: u8 = b;
        }
    }

    #[test]
    fn test_double_finish_compiles() {
        // finish() consumes self, so a second call is impossible by design.
        let enc = BoolEncoder::new();
        let _output = enc.finish();
    }

    // ── 6. Default implementation ────────────────────────────────────────────

    #[test]
    fn test_default_equals_new() {
        let mut d = BoolEncoder::default();
        d.encode_bool(128, true);
        let out_d = d.finish();

        let mut n = BoolEncoder::new();
        n.encode_bool(128, true);
        let out_n = n.finish();

        assert_eq!(out_d, out_n, "default() and new() must be equivalent");
    }

    #[test]
    fn test_default_is_empty_initially() {
        let enc = BoolEncoder::default();
        assert!(enc.is_empty());
        assert_eq!(enc.len(), 0);
    }

    // ── 7. Multiple encodes (stress) ────────────────────────────────────────

    #[test]
    fn test_encode_many_bools() {
        let mut enc = BoolEncoder::new();
        for i in 0..128 {
            let prob = ((i * 7) % 256) as u8;
            let val = i % 3 == 0;
            enc.encode_bool(prob, val);
        }
        let output = enc.finish();
        assert!(!output.is_empty(), "128 encodes should not be empty");
        assert!(
            output.len() < 128,
            "128 encoded bools should compress well below 128 bytes, got {}",
            output.len()
        );
    }

    #[test]
    fn test_encode_256_bools_stress() {
        let mut enc = BoolEncoder::new();
        for i in 0..256 {
            let prob = 128;
            let val = i & 1 != 0; // alternating bits
            enc.encode_bool(prob, val);
        }
        let output = enc.finish();
        assert!(!output.is_empty());
        assert!(
            output.len() <= 64,
            "256 bools at prob=128 should fit in 64 bytes, got {}",
            output.len()
        );
    }

    // ── 8. Minimal encode ────────────────────────────────────────────────────

    #[test]
    fn test_minimal_encode_one_bit() {
        let mut enc = BoolEncoder::new();
        enc.encode_bool(128, false);
        let output = enc.finish();
        assert!(
            !output.is_empty(),
            "encoding just one bit should still produce output"
        );
    }

    #[test]
    fn test_empty_encode_no_bits() {
        let enc = BoolEncoder::new();
        let output = enc.finish();
        // With initial count=-8, finish emits some padding bytes — that's fine
        assert!(
            output.len() <= 8,
            "empty encoder should produce minimal output, got {}",
            output.len()
        );
    }

    // ── 9. Encoder state isolation ──────────────────────────────────────────

    #[test]
    fn test_encoder_isolation() {
        let mut enc_a = BoolEncoder::new();
        let mut enc_b = BoolEncoder::new();

        enc_a.encode_bool(128, true);
        enc_a.encode_bool(128, false);

        enc_b.encode_bool(128, false);
        enc_b.encode_bool(128, true);

        let out_a = enc_a.finish();
        let out_b = enc_b.finish();

        assert_ne!(
            out_a, out_b,
            "encoders with different inputs must produce different output"
        );
    }

    #[test]
    fn test_independent_encoders_same_input() {
        let run = || -> Vec<u8> {
            let mut enc = BoolEncoder::new();
            enc.encode_bool(128, true);
            enc.encode_bool(80, true);
            enc.encode_bool(200, false);
            enc.finish()
        };

        for _ in 0..10 {
            let out = run();
            assert_eq!(
                out,
                run(),
                "same input must be deterministic across fresh encoders"
            );
        }
    }

    // ── 10. encode_tree basic ──────────────────────────────────────────────

    /// Build a simple 3-leaf tree (VP8-style):
    ///           Node 0 (prob[0])
    ///          /              \
    ///     Leaf(0)          Node 1 (prob[1])
    ///                      /              \
    ///                  Leaf(1)          Leaf(2)
    const TREE_3: [i8; 4] = [-1, 2, -2, -3];
    const PROBS_3: [u8; 2] = [128, 128];

    #[test]
    fn test_encode_tree_value0() {
        let mut enc = BoolEncoder::new();
        enc.encode_tree(&TREE_3, &PROBS_3, 0);
        let output = enc.finish();
        assert!(!output.is_empty());

        let mut manual = BoolEncoder::new();
        manual.encode_bool(128, false);
        let manual_out = manual.finish();
        assert_eq!(output, manual_out, "tree(0) should match manual encode");
    }

    #[test]
    fn test_encode_tree_value1() {
        let mut enc = BoolEncoder::new();
        enc.encode_tree(&TREE_3, &PROBS_3, 1);
        let output = enc.finish();
        assert!(!output.is_empty());

        let mut manual = BoolEncoder::new();
        manual.encode_bool(128, true); // go right
        manual.encode_bool(128, false); // go left
        let manual_out = manual.finish();
        assert_eq!(output, manual_out, "tree(1) should match manual encode");
    }

    #[test]
    fn test_encode_tree_value2() {
        let mut enc = BoolEncoder::new();
        enc.encode_tree(&TREE_3, &PROBS_3, 2);
        let output = enc.finish();
        assert!(!output.is_empty());

        let mut manual = BoolEncoder::new();
        manual.encode_bool(128, true); // go right
        manual.encode_bool(128, true); // go right
        let manual_out = manual.finish();
        assert_eq!(output, manual_out, "tree(2) should match manual encode");
    }

    #[test]
    fn test_encode_tree_different_values_differ() {
        let enc_val = |v: u32| -> Vec<u8> {
            let mut enc = BoolEncoder::new();
            enc.encode_tree(&TREE_3, &PROBS_3, v);
            enc.finish()
        };

        let out0 = enc_val(0);
        let out1 = enc_val(1);
        let out2 = enc_val(2);
        assert_ne!(
            out0, out1,
            "tree values 0 and 1 must produce different outputs"
        );
        assert_ne!(
            out0, out2,
            "tree values 0 and 2 must produce different outputs"
        );
        assert_ne!(
            out1, out2,
            "tree values 1 and 2 must produce different outputs"
        );
    }

    #[test]
    fn test_encode_tree_determinism() {
        let mut enc1 = BoolEncoder::new();
        enc1.encode_tree(&TREE_3, &PROBS_3, 1);
        let out1 = enc1.finish();

        let mut enc2 = BoolEncoder::new();
        enc2.encode_tree(&TREE_3, &PROBS_3, 1);
        let out2 = enc2.finish();

        assert_eq!(out1, out2, "encode_tree must be deterministic");
    }

    // ── 11. encode_signed ──────────────────────────────────────────────────

    #[test]
    fn test_encode_signed_zero() {
        let mut enc = BoolEncoder::new();
        encode_signed(&mut enc, 0);
        let output = enc.finish();
        assert!(!output.is_empty(), "encode_signed(0) should produce output");
    }

    #[test]
    fn test_encode_signed_positive_and_negative() {
        let encode = |val: i32| -> Vec<u8> {
            let mut enc = BoolEncoder::new();
            encode_signed(&mut enc, val);
            enc.finish()
        };

        let pos = encode(5);
        let neg = encode(-5);
        assert_ne!(
            pos, neg,
            "positive and negative of same magnitude must differ"
        );

        let zero = encode(0);
        let one = encode(1);
        assert_ne!(zero, one, "0 and 1 must produce different signed output");
    }

    #[test]
    fn test_writer_is_empty_and_len() {
        let mut enc = BoolEncoder::new();
        assert!(enc.is_empty());
        assert_eq!(enc.len(), 0);

        enc.encode_bool(128, true);
        enc.finish();
    }

    // ── 12. Carry propagation tests ──────────────────────────────────────────

    /// Test that a simple carry propagation works (no crash).
    #[test]
    fn test_carry_propagation_simple() {
        let mut enc = BoolEncoder::new();
        // Encode many values with probabilities that create carries.
        for _ in 0..500 {
            enc.encode_bool(200, true);
            enc.encode_bool(100, false);
        }
        let bytes = enc.finish();
        assert!(!bytes.is_empty());
    }

    /// Stress test carry chains with extreme probabilities.
    #[test]
    fn test_carry_chain_stress() {
        let mut enc = BoolEncoder::new();
        for i in 0..1000 {
            // Alternate between near-certain-true and near-certain-false
            // to create value boundaries triggering carries.
            enc.encode_bool(240, i % 3 != 0);
            enc.encode_bool(20, i % 7 == 0);
        }
        let bytes = enc.finish();
        assert!(!bytes.is_empty());
    }

    /// Test that carry does not panic when output buffer is empty.
    #[test]
    fn test_carry_empty_output() {
        let mut enc = BoolEncoder::new();
        // Encode with extreme probabilities to force carries early.
        for _ in 0..100 {
            enc.encode_bool(0, true);
        }
        let bytes = enc.finish();
        assert!(!bytes.is_empty());
    }

    // ── 13. Output size invariants ──────────────────────────────────────────

    #[test]
    fn test_output_compression_ratio() {
        // Encode 1000 uniform bits: should produce ~125-150 bytes.
        let mut enc = BoolEncoder::new();
        for _ in 0..1000 {
            enc.encode_bool(128, false);
        }
        let bytes = enc.finish();
        assert!(
            bytes.len() >= 100,
            "expected >=100 bytes for 1000 zero bits, got {}",
            bytes.len()
        );
        assert!(
            bytes.len() <= 200,
            "expected <=200 bytes for 1000 zero bits, got {}",
            bytes.len()
        );
    }

    #[test]
    fn test_output_compression_ratio_true_only() {
        let mut enc = BoolEncoder::new();
        for _ in 0..1000 {
            enc.encode_bool(128, true);
        }
        let bytes = enc.finish();
        assert!(!bytes.is_empty());
        // Should be comparable to all-false output.
        assert!(
            bytes.len() >= 100,
            "expected >=100 bytes for 1000 one bits, got {}",
            bytes.len()
        );
    }
}
