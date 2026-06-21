//! VP8 intra-frame encoder modules (RFC 6386).
//!
//! These modules implement the building blocks for a lossy VP8 keyframe encoder:
//!
//! * `dct` — 4×4 forward DCT + Walsh-Hadamard Transform
//! * `quant` — Quantization tables, quality mapping, RGB→YUV conversion
//! * `predict` — Intra prediction modes (DC, V, H, TM, B_PRED)
//! * `tokenize` — DCT coefficient tokenization + probability tables
//! * `bool_enc` — VP8 boolean entropy encoder (range coder)
//! * `loopfilter` — Deblocking loop filter
//! * `segmentation` — Macroblock segment feature data

#![allow(dead_code)]

pub mod bool_enc;
pub mod dct;
pub mod encoder;
pub mod loopfilter;
pub mod predict;
pub mod quant;
pub mod segmentation;
pub mod tokenize;
