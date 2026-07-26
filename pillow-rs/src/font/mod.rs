//! Pillow-compatible font loading and text rendering.
//!
//! TrueType loading, metadata, glyph rendering, advance, and kerning go
//! through the `_imagingft` adapter so this public module does not expose
//! FreeType-core helper APIs.

use crate::error::PilError;

mod default_aileron;
pub mod imagingft;
pub mod pilfont;

/// Pillow `FreeTypeFont`-compatible handle backed by the pure-Rust FreeType path.
pub struct Font {
    engine: imagingft::TrueTypeEngine,
}

impl Font {
    /// Load a TrueType/OpenType face from bytes at the requested Pillow point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        imagingft::load_truetype(data, size)
    }

    /// Loads the same embedded Aileron Regular subset as Pillow 12.2.0.
    ///
    /// Pillow opens this subset with the BASIC layout engine. The regular
    /// TrueType constructor is used here so default fonts and caller-supplied
    /// fonts share the same pure-Rust `fontdone` pipeline.
    pub fn load_default(size: f32) -> Result<Self, PilError> {
        Self::from_bytes(default_aileron::decode()?, size)
    }

    /// Return the requested Pillow point size for this FreeType font.
    pub fn font_size(&self) -> f32 {
        self.engine.size_pt
    }

    /// Return the non-negative text mask extent for Pillow-style text layout.
    pub fn text_bbox(&self, text: &str) -> (u32, u32) {
        let bbox = imagingft::getbbox(self, text);
        let w = (bbox.2 - bbox.0).max(0) as u32;
        let h = (bbox.3 - bbox.1).max(0) as u32;
        (w, h)
    }

    /// Return the Pillow-compatible grayscale text mask.
    pub fn getmask(&self, text: &str) -> (u32, u32, Vec<u8>) {
        imagingft::getmask(self, text)
    }
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Font::FreeType({}px)", self.font_size())
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    #[test]
    fn load_default_uses_pillow_aileron_through_truetype() {
        let font = Font::load_default(10.0).expect("the pinned Pillow Aileron subset must load");

        assert_eq!(imagingft::getname(&font), ("Aileron", "Regular"));
        assert_eq!(imagingft::getmetrics(&font), (10, 3));
        assert_eq!(imagingft::getbbox(&font, "Hello"), (0, 2, 25, 10));

        let (width, height, mask) = imagingft::getmask(&font, "Hello");
        let digest: [u8; 32] = Sha256::digest(&mask).into();
        assert_eq!((width, height), (25, 8));
        assert_eq!(
            digest,
            [
                0xeb, 0x99, 0x72, 0x55, 0x1d, 0xc0, 0xed, 0x86, 0xb0, 0x13, 0x22, 0x06, 0x71, 0x16,
                0x7a, 0xe2, 0x14, 0x44, 0x3b, 0xf3, 0xd0, 0x02, 0x3d, 0x71, 0x78, 0x2c, 0xd2, 0x0a,
                0x5e, 0x23, 0x31, 0xab,
            ]
        );

        let (width, height, rgba) =
            imagingft::render_text_binary(&font, "Hello", (255, 255, 255, 255), 0.0);
        let mask = rgba
            .chunks_exact(4)
            .map(|pixel| pixel[3])
            .collect::<Vec<_>>();
        let digest: [u8; 32] = Sha256::digest(&mask).into();
        assert_eq!(imagingft::getbbox_binary(&font, "Hello"), (0, 2, 28, 10));
        assert_eq!((width, height), (28, 8));
        assert_eq!(
            digest,
            [
                0xbb, 0xc5, 0x80, 0x38, 0x68, 0x77, 0x25, 0xe8, 0x8d, 0x83, 0x41, 0x9d, 0xd4, 0x07,
                0xca, 0x07, 0x37, 0xbc, 0x9f, 0xa8, 0xda, 0x5c, 0x68, 0x77, 0x23, 0x8b, 0xbc, 0x0d,
                0xb4, 0x79, 0xad, 0x44,
            ]
        );
    }
}
