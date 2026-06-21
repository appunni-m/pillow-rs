//! pillow-rs-font -- Pure-Rust FreeType 2.6.x compatible font renderer.
//!
//! Produces pixel-identical output to PIL's bundled FreeType.
//! Zero external font dependencies.

#![deny(missing_docs)]
#![deny(unsafe_code)]
// Allow during active development — several fields are API-surface for future use
#![allow(dead_code)]

pub mod error;
mod parser;
mod scaler;
mod raster;
mod metrics;
mod tables;

use std::sync::Arc;

use parser::cmap::parse_cmap;
use parser::head::parse_head;
use parser::hhea::parse_hhea;
use parser::hmtx::parse_hmtx;
use parser::maxp::parse_maxp;
use parser::name::parse_name;
use parser::os2::parse_os2;
use parser::{parse_table_directory, find_table, tag};

use tables::FontData;

pub use error::FontError;
pub use metrics::GlyphMask;
pub use tables::Font;

impl Font {
    /// Load a TrueType/OpenType font from raw bytes at a given point size.
    ///
    /// Parses all required tables immediately and stores them in `Arc<FontData>`.
    pub fn truetype(data: &[u8], size_pt: f32) -> Result<Self, FontError> {
        let dir = parse_table_directory(data)?;

        let head_data = find_table(data, &dir, tag(b"head"))
            .ok_or_else(|| FontError::InvalidFont("missing 'head' table".into()))?;
        let head = parse_head(head_data)?;

        let maxp_data = find_table(data, &dir, tag(b"maxp"))
            .ok_or_else(|| FontError::InvalidFont("missing 'maxp' table".into()))?;
        let maxp = parse_maxp(maxp_data)?;

        let cmap_data = find_table(data, &dir, tag(b"cmap"))
            .ok_or_else(|| FontError::InvalidFont("missing 'cmap' table".into()))?;
        let cmap = parse_cmap(cmap_data)?;

        let hhea_data = find_table(data, &dir, tag(b"hhea"))
            .ok_or_else(|| FontError::InvalidFont("missing 'hhea' table".into()))?;
        let hhea = parse_hhea(hhea_data)?;

        let hmtx_data = find_table(data, &dir, tag(b"hmtx"))
            .ok_or_else(|| FontError::InvalidFont("missing 'hmtx' table".into()))?;
        let hmtx = parse_hmtx(hmtx_data, hhea.num_hmetrics, maxp.num_glyphs)?;

        let name_data = find_table(data, &dir, tag(b"name"));
        let name = match name_data {
            Some(d) => parse_name(d)?,
            None => parser::name::NameTable {
                family: "Unknown".into(),
                subfamily: "Regular".into(),
            },
        };

        let os2 = find_table(data, &dir, tag(b"OS/2")).and_then(parse_os2);

        let loca_data = find_table(data, &dir, tag(b"loca"))
            .map(|d| d.to_vec())
            .ok_or_else(|| FontError::InvalidFont("missing 'loca' table".into()))?;

        let glyf_data = find_table(data, &dir, tag(b"glyf"))
            .map(|d| d.to_vec())
            .ok_or_else(|| FontError::InvalidFont("missing 'glyf' table".into()))?;

        let loca_format = head.index_to_loc_format;

        // Parse cvt table (Control Value Table)
        let cvt_data = find_table(data, &dir, tag(b"cvt "));
        let cvt: Vec<i32> = cvt_data.map_or(Vec::new(), |d| {
            d.chunks_exact(2)
                .map(|c| i16::from_be_bytes([c[0], c[1]]) as i32 * 64)
                .collect()
        });
        let cvt_size = cvt.len() as u16;

        // Parse fpgm table (Font Program)
        let fpgm = find_table(data, &dir, tag(b"fpgm"))
            .map(|d| d.to_vec())
            .unwrap_or_default();

        // Parse prep table (CVT Program)
        let prep = find_table(data, &dir, tag(b"prep"))
            .map(|d| d.to_vec())
            .unwrap_or_default();

        Ok(Font {
            data: Arc::new(FontData {
                cmap,
                head,
                hhea,
                hmtx,
                maxp,
                name,
                os2,
                loca_data,
                glyf_data,
                loca_format,
                size_pt,
                cvt,
                fpgm,
                prep,
                cvt_size,
            }),
            size_pt,
        })
    }
}
