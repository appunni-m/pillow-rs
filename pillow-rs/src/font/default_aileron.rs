//! Pillow's embedded Aileron Regular TrueType subset.
//!
//! The payload is byte-for-byte identical to the font embedded in
//! Pillow 12.2.0 `src/PIL/ImageFont.py::load_default`. Pillow opens it with
//! `layout_engine=Layout.BASIC`; `ImageFont::load_default` does the same through the
//! pure-Rust `fontdone` path.
//!
//! Length: 12,676 bytes.
//! SHA-256: 69853909b940023570964e29cffe30da95aea8de3627736b5cd15ab30143169f.
//! Source: <https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py>
//! ImageFont license: `default_aileron.LICENSE.txt`.

const FONT: &[u8; 12_676] = include_bytes!("default_aileron.ttf");

pub(super) fn decode() -> Vec<u8> {
    FONT.to_vec()
}
