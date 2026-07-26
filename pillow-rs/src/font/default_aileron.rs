//! Pillow's embedded Aileron Regular TrueType subset.
//!
//! The decoded payload is byte-for-byte identical to the font embedded in
//! Pillow 12.2.0 `src/PIL/ImageFont.py::load_default`. Pillow opens it with
//! `layout_engine=Layout.BASIC`; `Font::load_default` does the same through the
//! pure-Rust `fontdone` path.
//!
//! Decoded length: 12,676 bytes.
//! Decoded SHA-256: 69853909b940023570964e29cffe30da95aea8de3627736b5cd15ab30143169f.
//! Source: <https://github.com/python-pillow/Pillow/blob/12.2.0/src/PIL/ImageFont.py>
//! Font license: `default_aileron.LICENSE.txt`.

use crate::error::PilError;

const ENCODED_FONT: &[u8] = include_bytes!("default_aileron.b64");
const DECODED_LEN: usize = 12_676;

pub(super) fn decode() -> Result<Vec<u8>, PilError> {
    let mut decoded = Vec::with_capacity(DECODED_LEN);
    let mut quartet = [0u8; 4];
    let mut quartet_len = 0usize;
    let mut padding_seen = false;

    for &byte in ENCODED_FONT {
        if byte.is_ascii_whitespace() {
            continue;
        }
        if padding_seen {
            return Err(invalid_payload(
                "non-whitespace data follows base64 padding",
            ));
        }

        quartet[quartet_len] = if byte == b'=' {
            64
        } else {
            decode_digit(byte).ok_or_else(|| invalid_payload("invalid base64 digit"))?
        };
        quartet_len += 1;

        if quartet_len != 4 {
            continue;
        }
        if quartet[0] == 64 || quartet[1] == 64 {
            return Err(invalid_payload(
                "base64 padding appears before the final two digits",
            ));
        }

        decoded.push((quartet[0] << 2) | (quartet[1] >> 4));
        match (quartet[2], quartet[3]) {
            (64, 64) => padding_seen = true,
            (64, _) => return Err(invalid_payload("invalid base64 padding")),
            (third, 64) => {
                decoded.push((quartet[1] << 4) | (third >> 2));
                padding_seen = true;
            }
            (third, fourth) => {
                decoded.push((quartet[1] << 4) | (third >> 2));
                decoded.push((third << 6) | fourth);
            }
        }
        quartet_len = 0;
    }

    if quartet_len != 0 {
        return Err(invalid_payload("incomplete base64 quartet"));
    }
    if decoded.len() != DECODED_LEN {
        return Err(invalid_payload(
            "decoded font length does not match the pinned payload",
        ));
    }
    Ok(decoded)
}

fn invalid_payload(message: &str) -> PilError {
    PilError::InternalError(format!("embedded Aileron payload is invalid: {message}"))
}

fn decode_digit(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}
