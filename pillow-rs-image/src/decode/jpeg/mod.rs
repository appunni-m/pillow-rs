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

pub(crate) mod bit_reader;
pub(crate) mod decode;
pub(crate) mod huffman;
pub(crate) mod idct;
pub(crate) mod parser;
pub(crate) mod progressive;
pub(crate) mod upsample;

// Re-export the public entry point so `crate::decode::jpeg::decode` still works.
pub use decode::decode;
