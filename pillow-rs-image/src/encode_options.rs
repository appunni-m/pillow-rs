//! Encode options — passed from Python save(**options) or JS encode options.
//! Each format picks the params it cares about; others are ignored.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct EncodeOptions {
    /// Quality 1-100 (JPEG, WebP lossy)
    pub quality: Option<u8>,
    /// Compression level 0-9 (PNG), 0=none, 9=max
    pub compression: Option<u8>,
    /// Progressive encoding (JPEG, PNG)
    pub progressive: Option<bool>,
    /// Optimize Huffman tables (JPEG)
    pub optimize: Option<bool>,
    /// Chroma subsampling: "444", "422", "420" (JPEG)
    pub subsampling: Option<String>,
    /// Lossless mode (WebP)
    pub lossless: Option<bool>,
    /// Interlaced (PNG Adam7, GIF)
    pub interlace: Option<bool>,
    /// Catch-all for future params
    pub extra: HashMap<String, String>,
}

impl EncodeOptions {
    pub fn none() -> Self {
        Self::default()
    }
}
