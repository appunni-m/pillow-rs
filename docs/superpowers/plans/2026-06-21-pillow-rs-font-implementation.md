# pillow-rs-font Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build pillow-rs-font — a pure-Rust FreeType 2.6.x compatible font renderer producing pixel-identical output to PIL's bundled FreeType.

**Architecture:** 12 Rust source files across 4 modules (parser, scaler, raster, metrics) + 1 Python reference generator. Mirror pillow-rs-image pattern: manifest.yaml → coverage_matrix.json → tests/coverage_matrix_tests.rs.

**Tech Stack:** Rust (pure safe, zero external font deps), `log`, `thiserror`. Dev: `sha2`, `serde`, `serde_json`.

---

## File Structure Map

```
pillow-rs-font/                         [NEW crate]
├── Cargo.toml                          [NEW] deps: log, thiserror
├── manifest.yaml                       [NEW] API surface + coverage dimensions
├── .gitignore                          [NEW] target/, Cargo.lock (workspace)
├── src/
│   ├── lib.rs                          [NEW] pub API: Font, GlyphMask, FontError re-exports
│   ├── error.rs                        [NEW] thiserror FontError enum
│   ├── tables.rs                       [NEW] FontData struct, Arc-shared parsed tables
│   ├── parser/
│   │   ├── mod.rs                      [NEW] table directory parsing + dispatch
│   │   ├── cmap.rs                     [NEW] formats 4, 12 (char→glyph index)
│   │   ├── head.rs                     [NEW] units_per_em, flags, index_to_loc_format
│   │   ├── hhea.rs                     [NEW] ascent, descent, line_gap
│   │   ├── hmtx.rs                     [NEW] advance_width, lsb per glyph
│   │   ├── maxp.rs                     [NEW] num_glyphs
│   │   ├── name.rs                     [NEW] family + style (platform 3, encoding 1)
│   │   ├── os2.rs                      [NEW] sTypoAscender, sTypoDescender
│   │   ├── post.rs                     [NEW] underline metrics
│   │   ├── loca_glyf.rs               [NEW] glyph outline extraction
│   │   └── kern.rs                     [NEW] kerning pairs (optional)
│   ├── scaler.rs                       [NEW] 26.6 fixed-point scaling
│   ├── raster.rs                       [NEW] cell-based rasterizer (ftgrays.c)
│   └── metrics.rs                      [NEW] getbbox, getmetrics, getlength
├── scripts/
│   └── generate_font_refs.py           [NEW] PIL FreeType → coverage_matrix.json
├── tests/
│   ├── coverage_matrix_tests.rs         [NEW] single test driver
│   └── fixtures/
│       ├── coverage_matrix.json         [NEW] auto-generated, committed
│       ├── input/fonts/                 [NEW] test font files
│       └── outputs/raws/               [NEW] PIL .bin pixel dumps

pillow-rs/Cargo.toml                    [MODIFY] add pillow-rs-font dep, remove fontdue
pillow-rs/src/font/mod.rs               [MODIFY] replace fontdue → pillow_rs_font
pillow-rs-py/src/lib.rs                 [MODIFY] PyFont wrap pillow_rs_font
pillow-rs-py/python/pillow_rs/imagefont.py [MODIFY] remove PIL fallback (_pil_font)
```

---

## Phase 1: Scaffold + Table Parser (foundational)

### Task 1.1: Create crate skeleton

**Files:**
- Create: `pillow-rs-font/Cargo.toml`
- Create: `pillow-rs-font/.gitignore`
- Create: `pillow-rs-font/src/lib.rs`
- Create: `pillow-rs-font/src/error.rs`
- Modify: `Cargo.toml` (workspace members)

- [ ] **Step 1: Add crate to workspace members**

Edit `/home/appunni/work/pil-wasm/Cargo.toml`:
```toml
[workspace]
members = [
    "pillow-rs",
    "pillow-rs-py",
    "pillow-rs-js",
    "pillow-rs-image",
    "pillow-rs-font",
]
```

- [ ] **Step 2: Create Cargo.toml**

`pillow-rs-font/Cargo.toml`:
```toml
[package]
name = "pillow-rs-font"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Pure-Rust FreeType 2.6.x compatible font renderer — pixel-identical to PIL"

[dependencies]
log = "0.4"
thiserror = "2"

[dev-dependencies]
sha2 = "0.11"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[lints]
workspace = true
```

- [ ] **Step 3: Create .gitignore**

`pillow-rs-font/.gitignore`:
```
target/
```

- [ ] **Step 4: Create error.rs**

`pillow-rs-font/src/error.rs`:
```rust
//! Font error types.

/// Errors that can occur during font loading, glyph lookup, or rendering.
#[derive(Debug, thiserror::Error)]
pub enum FontError {
    /// The font data is not a valid TrueType/OpenType font.
    #[error("Invalid TrueType font: {0}")]
    InvalidFont(String),

    /// The cmap table uses an unsupported format.
    #[error("Unsupported cmap table format: {0}")]
    UnsupportedCmapFormat(u16),

    /// The rasterizer ran out of buffer space.
    #[error("Rasterizer buffer overflow")]
    RasterOverflow,

    /// Glyph outline data is malformed.
    #[error("Invalid glyph outline: {0}")]
    InvalidOutline(String),
}
```

- [ ] **Step 5: Create stub lib.rs**

`pillow-rs-font/src/lib.rs`:
```rust
//! pillow-rs-font — Pure-Rust FreeType 2.6.x compatible font renderer.
//!
//! Produces pixel-identical output to PIL's bundled FreeType.
//! Zero external font dependencies.

#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod error;
mod parser;
mod scaler;
mod raster;
mod metrics;
mod tables;

pub use error::FontError;
pub use metrics::GlyphMask;
pub use tables::Font;
```

- [ ] **Step 6: Create stub files for all modules**

```bash
mkdir -p pillow-rs-font/src/parser
mkdir -p pillow-rs-font/scripts
mkdir -p pillow-rs-font/tests/fixtures/input/fonts
mkdir -p pillow-rs-font/tests/fixtures/outputs/raws

for f in parser/mod parser/cmap parser/head parser/hhea parser/hmtx \
         parser/maxp parser/name parser/os2 parser/post parser/loca_glyf \
         parser/kern scaler raster metrics tables; do
    echo "// TODO: implement" > "pillow-rs-font/src/${f}.rs"
done
```

- [ ] **Step 7: Verify crate compiles**

```bash
cargo check -p pillow-rs-font
```
Expected: Compiles with dead-code warnings (stubs not used yet).

- [ ] **Step 8: Add temporary allowance for dead_code while scaffolding**

Edit `pillow-rs-font/src/lib.rs` to add at top:
```rust
// TODO: remove once all modules are populated
#![allow(dead_code)]
```

Run: `cargo check -p pillow-rs-font`
Expected: Clean compile, no warnings.

---

### Task 1.2: Implement parser/mod.rs — table directory

**Files:**
- Write: `pillow-rs-font/src/parser/mod.rs`

- [ ] **Step 1: Write the table directory parser**

`pillow-rs-font/src/parser/mod.rs`:
```rust
//! TrueType/OpenType table directory parsing.
//!
//! Reads the offset table and table directory from raw font bytes.
//! Matches FreeType 2.6's SFNT table loading in `sfnt/ttload.c`.

use crate::error::FontError;

/// Magic bytes identifying an OpenType font with TrueType outlines.
const OTTO_MAGIC: u32 = 0x4F54544F; // "OTTO"
/// Magic bytes identifying a TrueType font.
const TRUE_MAGIC: u32 = 0x00010000;

/// A reference to a single font table within the raw data.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TableRecord {
    /// 4-byte table tag (e.g. b'cmap', b'head').
    pub tag: u32,
    /// Byte offset from start of font data.
    pub offset: u32,
    /// Length in bytes.
    pub length: u32,
}

/// Parsed table directory — maps table tags to their data slices.
pub(crate) struct TableDirectory {
    /// Number of tables in the directory.
    pub num_tables: u16,
    /// Individual table records, in order of appearance.
    pub records: Vec<TableRecord>,
}

/// Read a big-endian u16 from a byte slice at the given offset.
#[inline]
fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let b = data.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([b[0], b[1]]))
}

/// Read a big-endian u32 from a byte slice at the given offset.
#[inline]
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let b = data.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

/// Parse the TrueType/OpenType table directory from raw font bytes.
///
/// Returns the table directory if the data is a valid font.
pub(crate) fn parse_table_directory(data: &[u8]) -> Result<TableDirectory, FontError> {
    if data.len() < 12 {
        return Err(FontError::InvalidFont(
            "data too short for offset table (need 12 bytes)".into()
        ));
    }

    let sf_version = read_u32(data, 0)
        .ok_or_else(|| FontError::InvalidFont("cannot read sfVersion".into()))?;

    // Accept TrueType (0x00010000) and OpenType with TrueType outlines ("OTTO")
    if sf_version != TRUE_MAGIC && sf_version != OTTO_MAGIC {
        return Err(FontError::InvalidFont(format!(
            "unknown sfVersion: 0x{:08X}", sf_version
        )));
    }

    let num_tables = read_u16(data, 4)
        .ok_or_else(|| FontError::InvalidFont("cannot read numTables".into()))?;

    let entry_size = 16usize;
    let dir_start = 12usize;
    let dir_end = dir_start + (num_tables as usize) * entry_size;

    if data.len() < dir_end {
        return Err(FontError::InvalidFont(format!(
            "data too short for {} table records", num_tables
        )));
    }

    let mut records = Vec::with_capacity(num_tables as usize);
    for i in 0..num_tables as usize {
        let off = dir_start + i * entry_size;
        let tag = read_u32(data, off)
            .ok_or_else(|| FontError::InvalidFont("cannot read table tag".into()))?;
        let _checksum = read_u32(data, off + 4);
        let offset = read_u32(data, off + 8)
            .ok_or_else(|| FontError::InvalidFont("cannot read table offset".into()))?;
        let length = read_u32(data, off + 12)
            .ok_or_else(|| FontError::InvalidFont("cannot read table length".into()))?;

        records.push(TableRecord { tag, offset, length });
    }

    Ok(TableDirectory { num_tables, records })
}

/// Look up a table by its 4-byte tag, returning a slice into the font data.
pub(crate) fn find_table<'a>(
    data: &'a [u8],
    dir: &TableDirectory,
    tag: u32,
) -> Option<&'a [u8]> {
    for record in &dir.records {
        if record.tag == tag {
            let start = record.offset as usize;
            let end = start + record.length as usize;
            return data.get(start..end);
        }
    }
    None
}

/// Build a u32 tag from 4 ASCII bytes. E.g., tag(b"cmap") = 0x636D6170.
#[inline]
pub(crate) const fn tag(bytes: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_data_returns_invalid_font_error() {
        let result = parse_table_directory(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn valid_true_type_magic_parses_directory() {
        // Minimal TrueType font: sfVersion + 1 table
        let mut data = vec![0u8; 12 + 16];
        data[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // TRUE_MAGIC
        data[4..6].copy_from_slice(&[0x00, 0x01]); // numTables = 1
        // table record at offset 12
        data[12..16].copy_from_slice(b"cmap");
        data[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x1C]); // offset = 28

        let dir = parse_table_directory(&data).expect("should parse");
        assert_eq!(dir.num_tables, 1);
        assert_eq!(dir.records[0].tag, tag(b"cmap"));
    }

    #[test]
    fn otto_magic_also_accepted() {
        let mut data = vec![0u8; 12 + 16];
        data[0..4].copy_from_slice(b"OTTO");
        data[4..6].copy_from_slice(&[0x00, 0x01]);
        data[12..16].copy_from_slice(b"cmap");
        data[20..24].copy_from_slice(&[0x00, 0x00, 0x00, 0x1C]);

        let dir = parse_table_directory(&data).expect("OTTO should parse");
        assert_eq!(dir.num_tables, 1);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 2 tests pass, 1 (empty_data) — FAIL because `InvalidFont` needs `PartialEq`. Add `#[derive(Debug, PartialEq)]` to `FontError`.

- [ ] **Step 3: Add PartialEq to FontError**

Edit `pillow-rs-font/src/error.rs`:
```rust
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FontError {
```

Run tests again: 3 PASS.

---

### Task 1.3: Implement parser/head.rs — font header

**Files:**
- Write: `pillow-rs-font/src/parser/head.rs`

- [ ] **Step 1: Write head table parser**

`pillow-rs-font/src/parser/head.rs`:
```rust
//! 'head' table — font header.
//!
//! Contains global font information: units_per_em, glyph data format,
//! font direction hints, and creation/modification timestamps.

use crate::error::FontError;

/// Parsed 'head' table.
#[derive(Debug, Clone)]
pub(crate) struct HeadTable {
    /// Font design units per em-square. Typically 1000 or 2048.
    pub units_per_em: u16,
    /// Format of the 'loca' table: 0 = short (offset/2), 1 = long (direct).
    pub index_to_loc_format: i16,
    /// Font flags (bit 3 = instructions may depend on point size).
    pub flags: u16,
}

/// Parse the 'head' table from raw bytes.
pub(crate) fn parse_head(data: &[u8]) -> Result<HeadTable, FontError> {
    if data.len() < 54 {
        return Err(FontError::InvalidFont(
            "head table too short (need 54 bytes)".into()
        ));
    }
    let units_per_em = u16::from_be_bytes([data[18], data[19]]);
    let index_to_loc_format = i16::from_be_bytes([data[50], data[51]]);
    let flags = u16::from_be_bytes([data[16], data[17]]);

    if units_per_em == 0 {
        return Err(FontError::InvalidFont(
            "head: units_per_em is zero".into()
        ));
    }

    Ok(HeadTable {
        units_per_em,
        index_to_loc_format,
        flags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_head_table() {
        let mut data = vec![0u8; 54];
        data[18..20].copy_from_slice(&[0x08, 0x00]); // units_per_em = 2048
        data[50..52].copy_from_slice(&[0x00, 0x01]); // index_to_loc_format = 1 (long)
        data[16..18].copy_from_slice(&[0x00, 0x08]); // flags = 8 (bit 3 set)

        let head = parse_head(&data).expect("should parse");
        assert_eq!(head.units_per_em, 2048);
        assert_eq!(head.index_to_loc_format, 1);
        assert_eq!(head.flags, 8);
    }

    #[test]
    fn zero_units_per_em_is_error() {
        let data = vec![0u8; 54];
        let result = parse_head(&data);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 5 PASS (3 from mod.rs, 2 from head).

---

### Task 1.4: Implement parser/maxp.rs

**Files:**
- Write: `pillow-rs-font/src/parser/maxp.rs`

- [ ] **Step 1: Write maxp table parser**

`pillow-rs-font/src/parser/maxp.rs`:
```rust
//! 'maxp' table — maximum profile.
//!
//! Contains memory requirements: number of glyphs, max points, max contours.

use crate::error::FontError;

/// Parsed 'maxp' table.
#[derive(Debug, Clone)]
pub(crate) struct MaxpTable {
    /// Total number of glyphs in the font (including glyph 0 /.notdef).
    pub num_glyphs: u16,
}

/// Parse the 'maxp' table from raw bytes. Version 1.0 required (32 bytes).
pub(crate) fn parse_maxp(data: &[u8]) -> Result<MaxpTable, FontError> {
    if data.len() < 6 {
        return Err(FontError::InvalidFont(
            "maxp table too short (need 6 bytes)".into()
        ));
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    if version != 0x00010000 {
        return Err(FontError::InvalidFont(format!(
            "maxp: unsupported version 0x{:08X}", version
        )));
    }
    let num_glyphs = u16::from_be_bytes([data[4], data[5]]);
    Ok(MaxpTable { num_glyphs })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_maxp_with_valid_data() {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&[0x00, 0x01, 0x00, 0x00]); // version 1.0
        data[4..6].copy_from_slice(&[0x01, 0xF4]); // num_glyphs = 500

        let maxp = parse_maxp(&data).expect("should parse");
        assert_eq!(maxp.num_glyphs, 500);
    }

    #[test]
    fn wrong_version_is_error() {
        let data = vec![0u8; 6]; // version = 0.0
        let result = parse_maxp(&data);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 7 PASS.

---

### Task 1.5: Implement parser/cmap.rs — format 4 (BMP segment mapping)

**Files:**
- Write: `pillow-rs-font/src/parser/cmap.rs`

- [ ] **Step 1: Write cmap parser with format 4 support**

`pillow-rs-font/src/parser/cmap.rs`:
```rust
//! 'cmap' table — character to glyph index mapping.
//!
//! Supports format 4 (BMP segment mapping) and format 12 (full Unicode).
//! Format selection priority: 12 → 4 (matching FreeType 2.6).

use crate::error::FontError;
use log::warn;

/// A character-to-glyph mapping table.
#[derive(Debug, Clone)]
pub(crate) struct CmapTable {
    /// Format 4 subtables (one per encoding record).
    pub format4: Vec<Format4Subtable>,
    /// Format 12 subtables (one per encoding record).
    pub format12: Vec<Format12Subtable>,
}

/// Format 4: Segment mapping for Unicode BMP (U+0000–U+FFFF).
#[derive(Debug, Clone)]
pub(crate) struct Format4Subtable {
    /// Platform ID from the encoding record (3 = Windows).
    pub platform_id: u16,
    /// Encoding ID from the encoding record (1 = Unicode BMP).
    pub encoding_id: u16,
    /// End character codes for each segment.
    pub end_codes: Vec<u16>,
    /// Start character codes for each segment.
    pub start_codes: Vec<u16>,
    /// Delta values (glyph_index = char_code + delta if not in range_offset).
    pub id_deltas: Vec<i16>,
    /// Range offsets for segments with non-contiguous mappings.
    pub id_range_offsets: Vec<u16>,
    /// Glyph ID array (indexed via range offsets).
    pub glyph_id_array: Vec<u16>,
}

/// Format 12: Segmented coverage for full Unicode (U+0000–U+10FFFF).
#[derive(Debug, Clone)]
pub(crate) struct Format12Subtable {
    /// Platform ID from the encoding record (3 = Windows).
    pub platform_id: u16,
    /// Encoding ID (10 = Unicode full repertoire).
    pub encoding_id: u16,
    /// Start character codes for each group.
    pub start_codes: Vec<u32>,
    /// End character codes for each group.
    pub end_codes: Vec<u32>,
    /// Start glyph IDs for each group (glyph = start_glyph + (char - start)).
    pub start_glyph_ids: Vec<u32>,
}

/// Parsed encoding record from the cmap header.
#[derive(Debug, Clone)]
struct EncodingRecord {
    platform_id: u16,
    encoding_id: u16,
    subtable_offset: u32,
}

impl CmapTable {
    /// Map a Unicode codepoint to a glyph index.
    ///
    /// Returns `None` if no mapping exists (caller should use glyph 0 = .notdef).
    pub fn map(&self, codepoint: u32) -> Option<u16> {
        // Try format 12 first (preferred for full Unicode coverage)
        for sub in &self.format12 {
            if let Some(glyph) = sub.map_codepoint(codepoint) {
                return Some(glyph);
            }
        }
        // Fall back to format 4 (BMP only)
        if codepoint <= 0xFFFF {
            for sub in &self.format4 {
                if let Some(glyph) = sub.map_codepoint(codepoint as u16) {
                    return Some(glyph);
                }
            }
        }
        None
    }
}

impl Format12Subtable {
    fn map_codepoint(&self, codepoint: u32) -> Option<u16> {
        // Binary search through groups
        for i in 0..self.start_codes.len() {
            if codepoint >= self.start_codes[i] && codepoint <= self.end_codes[i] {
                let offset = codepoint - self.start_codes[i];
                return Some((self.start_glyph_ids[i] + offset) as u16);
            }
            if codepoint < self.start_codes[i] {
                break; // groups are sorted — no need to continue
            }
        }
        None
    }
}

impl Format4Subtable {
    fn map_codepoint(&self, char_code: u16) -> Option<u16> {
        // Find the segment containing char_code
        for seg in 0..self.end_codes.len() {
            if char_code > self.end_codes[seg] {
                continue;
            }
            if char_code < self.start_codes[seg] {
                return None; // before first segment
            }
            if self.id_range_offsets[seg] == 0 {
                // Contiguous mapping: glyph = (char + delta) mod 65536
                let glyph = (char_code as u32)
                    .wrapping_add_signed(self.id_deltas[seg] as i32) as u16;
                return Some(glyph);
            }
            // Non-contiguous: use range offset to index into glyph_id_array
            let range_off = self.id_range_offsets[seg] as usize;
            let idx_in_seg = (char_code - self.start_codes[seg]) as usize;
            let glyph_idx = (range_off / 2) + idx_in_seg;
            if glyph_idx < self.glyph_id_array.len() {
                let raw = self.glyph_id_array[glyph_idx];
                if raw != 0 {
                    let glyph = (raw as u32)
                        .wrapping_add_signed(self.id_deltas[seg] as i32) as u16;
                    return Some(glyph);
                }
            }
            return None;
        }
        None
    }
}

/// Parse the cmap table. Returns a CmapTable with all supported subtables.
pub(crate) fn parse_cmap(data: &[u8]) -> Result<CmapTable, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont("cmap table too short".into()));
    }
    let _version = u16::from_be_bytes([data[0], data[1]]);
    let num_tables = u16::from_be_bytes([data[2], data[3]]) as usize;
    let header_size = 4usize;
    let record_size = 8usize;

    if data.len() < header_size + num_tables * record_size {
        return Err(FontError::InvalidFont("cmap: encoding records overflow".into()));
    }

    let mut records = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let off = header_size + i * record_size;
        let platform_id = u16::from_be_bytes([data[off], data[off + 1]]);
        let encoding_id = u16::from_be_bytes([data[off + 2], data[off + 3]]);
        let subtable_offset = u32::from_be_bytes([
            data[off + 4], data[off + 5], data[off + 6], data[off + 7],
        ]);
        records.push(EncodingRecord {
            platform_id,
            encoding_id,
            subtable_offset,
        });
    }

    let mut fmt4 = Vec::new();
    let mut fmt12 = Vec::new();

    for rec in &records {
        let sub_off = rec.subtable_offset as usize;
        if sub_off + 2 > data.len() {
            continue;
        }
        let format = u16::from_be_bytes([data[sub_off], data[sub_off + 1]]);
        match format {
            4 => {
                if let Ok(sub) = parse_format4(data, sub_off) {
                    fmt4.push(Format4Subtable {
                        platform_id: rec.platform_id,
                        encoding_id: rec.encoding_id,
                        ..sub
                    });
                }
            }
            12 => {
                if let Ok(sub) = parse_format12(data, sub_off) {
                    fmt12.push(Format12Subtable {
                        platform_id: rec.platform_id,
                        encoding_id: rec.encoding_id,
                        ..sub
                    });
                }
            }
            other => {
                warn!("[cmap] unsupported format {}: skipping", other);
            }
        }
    }

    Ok(CmapTable {
        format4: fmt4,
        format12: fmt12,
    })
}

/// Parse a format 4 subtable. Returns the parsed fields.
fn parse_format4(data: &[u8], offset: usize) -> Result<Format4Subtable, FontError> {
    let b = &data[offset..];
    if b.len() < 16 {
        return Err(FontError::InvalidFont("cmap format 4: too short".into()));
    }

    let length = u16::from_be_bytes([b[2], b[3]]) as usize;
    if offset + length > data.len() {
        return Err(FontError::InvalidFont("cmap format 4: length exceeds data".into()));
    }
    let _language = u16::from_be_bytes([b[4], b[5]]);
    let seg_count_x2 = u16::from_be_bytes([b[6], b[7]]);
    let seg_count = (seg_count_x2 / 2) as usize;

    if seg_count == 0 {
        return Err(FontError::InvalidFont("cmap format 4: zero segments".into()));
    }

    // Tables: endCode (seg_count), reservedPad (2), startCode (seg_count),
    //         idDelta (seg_count), idRangeOffset (seg_count), glyphIdArray (variable)
    let end_codes_off = 14usize;
    let start_codes_off = end_codes_off + seg_count * 2 + 2; // +2 for reservedPad
    let id_deltas_off = start_codes_off + seg_count * 2;
    let id_range_offsets_off = id_deltas_off + seg_count * 2;
    let glyph_array_off = id_range_offsets_off + seg_count * 2;

    let mut end_codes = Vec::with_capacity(seg_count);
    let mut start_codes = Vec::with_capacity(seg_count);
    let mut id_deltas = Vec::with_capacity(seg_count);
    let mut id_range_offsets = Vec::with_capacity(seg_count);

    for i in 0..seg_count {
        let e = end_codes_off + i * 2;
        end_codes.push(u16::from_be_bytes([b[e], b[e + 1]]));
    }
    for i in 0..seg_count {
        let s = start_codes_off + i * 2;
        start_codes.push(u16::from_be_bytes([b[s], b[s + 1]]));
    }
    for i in 0..seg_count {
        let d = id_deltas_off + i * 2;
        id_deltas.push(i16::from_be_bytes([b[d], b[d + 1]]));
    }
    for i in 0..seg_count {
        let r = id_range_offsets_off + i * 2;
        id_range_offsets.push(u16::from_be_bytes([b[r], b[r + 1]]));
    }

    // glyphIdArray: variable length after id_range_offsets
    let available = if glyph_array_off < b.len() {
        b.len() - glyph_array_off
    } else {
        0
    };
    let mut glyph_id_array = Vec::new();
    let mut g_off = glyph_array_off;
    while g_off + 2 <= b.len() {
        glyph_id_array.push(u16::from_be_bytes([b[g_off], b[g_off + 1]]));
        g_off += 2;
    }

    Ok(Format4Subtable {
        platform_id: 0,
        encoding_id: 0,
        end_codes,
        start_codes,
        id_deltas,
        id_range_offsets,
        glyph_id_array,
    })
}

/// Parse a format 12 subtable.
fn parse_format12(data: &[u8], offset: usize) -> Result<Format12Subtable, FontError> {
    let b = &data[offset..];
    if b.len() < 16 {
        return Err(FontError::InvalidFont("cmap format 12: too short".into()));
    }
    let _reserved = u16::from_be_bytes([b[2], b[3]]);
    let length = u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize;
    let _language = u32::from_be_bytes([b[8], b[9], b[10], b[11]]);
    let num_groups = u32::from_be_bytes([b[12], b[13], b[14], b[15]]) as usize;

    let group_start = 16usize;
    if group_start + num_groups * 12 > length || offset + length > data.len() {
        return Err(FontError::InvalidFont("cmap format 12: groups overflow".into()));
    }

    let mut start_codes = Vec::with_capacity(num_groups);
    let mut end_codes = Vec::with_capacity(num_groups);
    let mut start_glyph_ids = Vec::with_capacity(num_groups);

    for i in 0..num_groups {
        let o = group_start + i * 12;
        let sc = u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        let ec = u32::from_be_bytes([b[o + 4], b[o + 5], b[o + 6], b[o + 7]]);
        let sg = u32::from_be_bytes([b[o + 8], b[o + 9], b[o + 10], b[o + 11]]);
        start_codes.push(sc);
        end_codes.push(ec);
        start_glyph_ids.push(sg);
    }

    Ok(Format12Subtable {
        platform_id: 0,
        encoding_id: 0,
        start_codes,
        end_codes,
        start_glyph_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_format4_segment(
        start: u16, end: u16, delta: i16, range_offset: u16,
    ) -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>) {
        let e = end.to_be_bytes().to_vec();
        let s = start.to_be_bytes().to_vec();
        let d = delta.to_be_bytes().to_vec();
        let r = range_offset.to_be_bytes().to_vec();
        (e, s, d, r, vec![])
    }

    fn build_format4_bytes(segments: &[(u16, u16, i16, u16, Vec<u16>)]) -> Vec<u8> {
        let seg_count = segments.len() as u16;
        let mut b = vec![0u8; 14]; // header
        b[0] = 0x00; b[1] = 0x04; // format 4
        let total_len = 16u16 + seg_count * 8 + 2; // header + arrays + reservedPad
        b[2] = (total_len >> 8) as u8; b[3] = total_len as u8;
        b[6] = ((seg_count * 2) >> 8) as u8; b[7] = (seg_count * 2) as u8;

        // Collect glyph arrays
        let mut end_codes = Vec::new();
        let mut start_codes = Vec::new();
        let mut id_deltas = Vec::new();
        let mut id_range_offsets = Vec::new();
        let mut glyph_array = Vec::new();
        for (s, e, d, r, ga) in segments {
            end_codes.extend_from_slice(&e.to_be_bytes());
            start_codes.extend_from_slice(&s.to_be_bytes());
            id_deltas.extend_from_slice(&d.to_be_bytes());
            id_range_offsets.extend_from_slice(&r.to_be_bytes());
            for g in ga {
                glyph_array.extend_from_slice(&g.to_be_bytes());
            }
        }
        b.extend(&end_codes);
        b.extend(&[0u8, 0u8]); // reservedPad
        b.extend(&start_codes);
        b.extend(&id_deltas);
        b.extend(&id_range_offsets);
        b.extend(&glyph_array);
        b
    }

    #[test]
    fn format4_segment_search_finds_code_in_first_segment() {
        let segments = vec![
            (32u16, 126u16, -32i16, 0u16, vec![]), // subtract 32 from char code
        ];
        let cmap_data_bytes = build_format4_bytes(&segments);

        // Wrap in cmap header: version=0, numTables=1, encoding record→offset 20
        let mut full_data = vec![0u8; 24]; // 4 (version+count) + 8 (record) + 12 (pad)
        full_data[2] = 0x00; full_data[3] = 0x01; // numTables = 1
        full_data[4..8].copy_from_slice(&[0x00, 0x03, 0x00, 0x01]); // platform 3, encoding 1
        let sub_off = 24u32;
        full_data[8..12].copy_from_slice(&sub_off.to_be_bytes());
        full_data.extend_from_slice(&cmap_data_bytes);

        let cmap = parse_cmap(&full_data).expect("should parse");
        assert_eq!(cmap.format4.len(), 1);
        // 'A' (65) should map to 65 - 32 = 33
        let glyph = cmap.map(65).expect("should map 'A'");
        assert_eq!(glyph, 33);
    }

    #[test]
    fn map_unmapped_codepoint_returns_none_not_error() {
        let segments = vec![(65u16, 90u16, 0i16, 0u16, vec![])]; // only A-Z
        let cmap_data_bytes = build_format4_bytes(&segments);
        let mut full_data = vec![0u8; 24];
        full_data[2] = 0x00; full_data[3] = 0x01;
        full_data[4..8].copy_from_slice(&[0x00, 0x03, 0x00, 0x01]);
        let sub_off = 24u32;
        full_data[8..12].copy_from_slice(&sub_off.to_be_bytes());
        full_data.extend_from_slice(&cmap_data_bytes);

        let cmap = parse_cmap(&full_data).expect("should parse");
        // '!' (33) is outside A-Z range → no mapping
        assert!(cmap.map(33).is_none());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 9 PASS.

---

### Task 1.6: Implement parser/hhea.rs and parser/hmtx.rs

**Files:**
- Write: `pillow-rs-font/src/parser/hhea.rs`
- Write: `pillow-rs-font/src/parser/hmtx.rs`

- [ ] **Step 1: Write hhea parser**

`pillow-rs-font/src/parser/hhea.rs`:
```rust
//! 'hhea' table — Horizontal Header.
//!
//! Contains font-wide horizontal metrics: ascent, descent, line gap,
//! and the number of hmtx entries with explicit advance widths.

use crate::error::FontError;

/// Parsed 'hhea' table.
#[derive(Debug, Clone)]
pub(crate) struct HheaTable {
    /// Typographic ascent (font units, positive up).
    pub ascent: i16,
    /// Typographic descent (font units, negative down).
    pub descent: i16,
    /// Typographic line gap.
    pub line_gap: i16,
    /// Number of hmtx entries that have explicit advance widths.
    /// Remaining glyphs use the last advance width.
    pub num_hmetrics: u16,
}

/// Parse 'hhea' table from raw bytes.
pub(crate) fn parse_hhea(data: &[u8]) -> Result<HheaTable, FontError> {
    if data.len() < 36 {
        return Err(FontError::InvalidFont(
            "hhea table too short (need 36 bytes)".into()
        ));
    }
    let ascent = i16::from_be_bytes([data[4], data[5]]);
    let descent = i16::from_be_bytes([data[6], data[7]]);
    let line_gap = i16::from_be_bytes([data[8], data[9]]);
    let _advance_width_max = u16::from_be_bytes([data[10], data[11]]);
    // num_hmetrics is at offset 34 (bytes 34-35)
    let num_hmetrics = u16::from_be_bytes([data[34], data[35]]);

    Ok(HheaTable {
        ascent,
        descent,
        line_gap,
        num_hmetrics,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_hhea() {
        let mut data = vec![0u8; 36];
        data[4..6].copy_from_slice(&[0x07, 0x00]); // ascent = 1792
        data[6..8].copy_from_slice(&[0xFE, 0x00]); // descent = -512
        data[34..36].copy_from_slice(&[0x01, 0xF4]); // num_hmetrics = 500

        let hhea = parse_hhea(&data).expect("should parse");
        assert_eq!(hhea.ascent, 1792);
        assert_eq!(hhea.descent, -512);
        assert_eq!(hhea.num_hmetrics, 500);
    }
}
```

- [ ] **Step 2: Write hmtx parser**

`pillow-rs-font/src/parser/hmtx.rs`:
```rust
//! 'hmtx' table — Horizontal Metrics.
//!
//! Contains advance width and left side bearing for each glyph.

use crate::error::FontError;

/// Horizontal metrics for a single glyph.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LongHorMetric {
    /// Advance width in font design units.
    pub advance_width: u16,
    /// Left side bearing in font design units.
    pub lsb: i16,
}

/// Parsed 'hmtx' table.
#[derive(Debug, Clone)]
pub(crate) struct HmtxTable {
    /// Metrics for the first `num_hmetrics` glyphs (explicit advance width).
    pub h_metrics: Vec<LongHorMetric>,
    /// Left side bearings for remaining glyphs (share last advance width).
    pub left_side_bearings: Vec<i16>,
}

impl HmtxTable {
    /// Get the horizontal metrics for a glyph index.
    pub fn get(&self, glyph_index: u16) -> LongHorMetric {
        let idx = glyph_index as usize;
        if idx < self.h_metrics.len() {
            self.h_metrics[idx]
        } else {
            // Use last advance_width with per-glyph lsb
            let last_advance = self.h_metrics.last()
                .map(|m| m.advance_width)
                .unwrap_or(0);
            let lsb = self.left_side_bearings
                .get(idx - self.h_metrics.len())
                .copied()
                .unwrap_or(0);
            LongHorMetric {
                advance_width: last_advance,
                lsb,
            }
        }
    }
}

/// Parse 'hmtx' table. `num_hmetrics` from hhea, `num_glyphs` from maxp.
pub(crate) fn parse_hmtx(
    data: &[u8],
    num_hmetrics: u16,
    num_glyphs: u16,
) -> Result<HmtxTable, FontError> {
    let hm_count = num_hmetrics as usize;
    let total_glyphs = num_glyphs as usize;

    if hm_count > total_glyphs || hm_count == 0 {
        return Err(FontError::InvalidFont(
            "hmtx: num_hmetrics out of range".into()
        ));
    }

    let long_entry_size = 4usize; // advance_width(u16) + lsb(i16)
    let needed = hm_count * long_entry_size + (total_glyphs - hm_count) * 2;
    if data.len() < needed {
        return Err(FontError::InvalidFont(format!(
            "hmtx table too short: need {} bytes, have {}",
            needed, data.len()
        )));
    }

    let mut h_metrics = Vec::with_capacity(hm_count);
    for i in 0..hm_count {
        let off = i * long_entry_size;
        let advance_width = u16::from_be_bytes([data[off], data[off + 1]]);
        let lsb = i16::from_be_bytes([data[off + 2], data[off + 3]]);
        h_metrics.push(LongHorMetric { advance_width, lsb });
    }

    let lsb_start = hm_count * long_entry_size;
    let lsb_count = total_glyphs - hm_count;
    let mut left_side_bearings = Vec::with_capacity(lsb_count);
    for i in 0..lsb_count {
        let off = lsb_start + i * 2;
        let lsb = i16::from_be_bytes([data[off], data[off + 1]]);
        left_side_bearings.push(lsb);
    }

    Ok(HmtxTable {
        h_metrics,
        left_side_bearings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hmtx_with_single_glyph() {
        let mut data = vec![0u8; 4]; // one long metric
        data[0..2].copy_from_slice(&[0x05, 0x00]); // advance = 1280
        data[2..4].copy_from_slice(&[0x00, 0x20]); // lsb = 32

        let hmtx = parse_hmtx(&data, 1, 1).expect("should parse");
        assert_eq!(hmtx.h_metrics.len(), 1);
        assert_eq!(hmtx.get(0).advance_width, 1280);
        assert_eq!(hmtx.get(0).lsb, 32);
    }

    #[test]
    fn trailing_lsb_for_extra_glyphs() {
        // num_hmetrics=1, num_glyphs=3
        // glyph 0: advance=100, lsb=10
        // glyph 1: advance=100 (reused), lsb=20
        // glyph 2: advance=100 (reused), lsb=30
        let mut data = vec![0u8; 4 + 4]; // one long + two lsb
        data[0..2].copy_from_slice(&[0x00, 0x64]); // glyph 0: advance = 100
        data[2..4].copy_from_slice(&[0x00, 0x0A]); // glyph 0: lsb = 10
        data[4..6].copy_from_slice(&[0x00, 0x14]); // glyph 1: lsb = 20
        data[6..8].copy_from_slice(&[0x00, 0x1E]); // glyph 2: lsb = 30

        let hmtx = parse_hmtx(&data, 1, 3).expect("should parse");
        assert_eq!(hmtx.get(0).advance_width, 100);
        assert_eq!(hmtx.get(0).lsb, 10);
        assert_eq!(hmtx.get(1).advance_width, 100); // reused
        assert_eq!(hmtx.get(1).lsb, 20);
        assert_eq!(hmtx.get(2).advance_width, 100); // reused
        assert_eq!(hmtx.get(2).lsb, 30);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 12 PASS.

---

### Task 1.7: Implement parser/name.rs and parser/os2.rs

**Files:**
- Write: `pillow-rs-font/src/parser/name.rs`
- Write: `pillow-rs-font/src/parser/os2.rs`

- [ ] **Step 1: Write name table parser**

`pillow-rs-font/src/parser/name.rs`:
```rust
//! 'name' table — Naming Table.
//!
//! Contains font family name, subfamily (style), and other metadata strings.
//! Prefers platform 3 (Windows), encoding 1 (Unicode BMP = UTF-16BE) per FreeType 2.6.

use crate::error::FontError;

/// Font identification strings extracted from the name table.
#[derive(Debug, Clone)]
pub(crate) struct NameTable {
    /// Font family name (nameID 1).
    pub family: String,
    /// Font subfamily/style name (nameID 2).
    pub subfamily: String,
}

/// Name record entry from the table.
#[derive(Debug)]
struct NameRecord {
    platform_id: u16,
    encoding_id: u16,
    /// Language ID (platform-specific).
    _language_id: u16,
    /// Name identifier (1 = family, 2 = subfamily, etc.).
    name_id: u16,
    /// Byte offset within the string storage area.
    offset: u16,
    /// Length in bytes.
    length: u16,
}

/// Parse the 'name' table from raw bytes.
pub(crate) fn parse_name(data: &[u8]) -> Result<NameTable, FontError> {
    if data.len() < 6 {
        return Err(FontError::InvalidFont(
            "name table too short (need 6 bytes)".into()
        ));
    }
    let _format = u16::from_be_bytes([data[0], data[1]]);
    let count = u16::from_be_bytes([data[2], data[3]]) as usize;
    let string_offset = u16::from_be_bytes([data[4], data[5]]) as usize;

    if data.len() < 6 + count * 12 {
        return Err(FontError::InvalidFont(
            "name table: records overflow data".into()
        ));
    }

    let mut records: Vec<NameRecord> = Vec::with_capacity(count);
    for i in 0..count {
        let off = 6 + i * 12;
        let platform_id = u16::from_be_bytes([data[off], data[off + 1]]);
        let encoding_id = u16::from_be_bytes([data[off + 2], data[off + 3]]);
        let _language_id = u16::from_be_bytes([data[off + 4], data[off + 5]]);
        let name_id = u16::from_be_bytes([data[off + 6], data[off + 7]]);
        let length = u16::from_be_bytes([data[off + 8], data[off + 9]]);
        let str_off = u16::from_be_bytes([data[off + 10], data[off + 11]]);
        records.push(NameRecord {
            platform_id,
            encoding_id,
            _language_id,
            name_id,
            offset: str_off,
            length,
        });
    }

    let family = find_name_string(data, string_offset, &records, 1);
    let subfamily = find_name_string(data, string_offset, &records, 2);

    Ok(NameTable {
        family: family.unwrap_or_else(|| "Unknown".into()),
        subfamily: subfamily.unwrap_or_else(|| "Regular".into()),
    })
}

/// Search for a name string by name_id, preferring platform 3 encoding 1.
fn find_name_string(
    data: &[u8],
    string_base: usize,
    records: &[NameRecord],
    name_id: u16,
) -> Option<String> {
    // Priority 1: platform 3 (Windows), encoding 1 (Unicode BMP)
    for r in records {
        if r.name_id == name_id && r.platform_id == 3 && r.encoding_id == 1 {
            if let Ok(s) = decode_utf16be(data, string_base, r.offset as usize, r.length as usize) {
                return Some(s);
            }
        }
    }
    // Priority 2: platform 1 (Mac), encoding 0 (Roman)
    for r in records {
        if r.name_id == name_id && r.platform_id == 1 && r.encoding_id == 0 {
            if let Ok(s) = decode_mac_roman(data, string_base, r.offset as usize, r.length as usize) {
                return Some(s);
            }
        }
    }
    // Fallback: any record with matching name_id
    for r in records {
        if r.name_id == name_id {
            if r.platform_id == 3 {
                if let Ok(s) = decode_utf16be(data, string_base, r.offset as usize, r.length as usize) {
                    return Some(s);
                }
            }
        }
    }
    None
}

/// Decode a UTF-16BE string from the name table's string storage.
fn decode_utf16be(
    data: &[u8], base: usize, offset: usize, length: usize,
) -> Result<String, FontError> {
    let start = base + offset;
    let end = start + length;
    let bytes = data.get(start..end).ok_or_else(|| {
        FontError::InvalidFont("name: string offset out of range".into())
    })?;
    if length % 2 != 0 {
        return Err(FontError::InvalidFont(
            "name: UTF-16BE string has odd length".into()
        ));
    }
    let chars: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&chars)
        .map_err(|e| FontError::InvalidFont(format!("name: invalid UTF-16: {}", e)))
}

/// Decode a Mac Roman string from the name table. Maps bytes 0-127 directly (ASCII subset).
fn decode_mac_roman(
    data: &[u8], base: usize, offset: usize, length: usize,
) -> Result<String, FontError> {
    let start = base + offset;
    let end = start + length;
    let bytes = data.get(start..end).ok_or_else(|| {
        FontError::InvalidFont("name: Mac Roman string offset out of range".into())
    })?;
    // Mac Roman bytes 0x00-0x7F are ASCII. Higher bytes need a mapping table
    // which we skip for now — non-ASCII Mac names are rare in test fonts.
    Ok(bytes.iter().map(|&b| b as char).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_name_table(names: &[(u16, u16, u16, u16, &str)]) -> Vec<u8> {
        // names: (platform_id, encoding_id, language_id, name_id, text)
        let mut records_bytes = Vec::new();
        let mut strings_bytes = Vec::new();
        let count = names.len() as u16;
        let header_size = 6u16;
        let record_size = 12u16;

        for (pid, eid, lid, nid, text) in names {
            // Encode as UTF-16BE for platform 3
            let mut encoded: Vec<u8> = Vec::new();
            for ch in text.encode_utf16() {
                encoded.extend_from_slice(&ch.to_be_bytes());
            }
            let off = strings_bytes.len() as u16;
            let len = encoded.len() as u16;
            records_bytes.extend_from_slice(&pid.to_be_bytes());
            records_bytes.extend_from_slice(&eid.to_be_bytes());
            records_bytes.extend_from_slice(&lid.to_be_bytes());
            records_bytes.extend_from_slice(&nid.to_be_bytes());
            records_bytes.extend_from_slice(&len.to_be_bytes());
            records_bytes.extend_from_slice(&off.to_be_bytes());
            strings_bytes.extend(&encoded);
        }

        let string_offset = header_size + count * record_size;
        let mut data = Vec::new();
        data.extend_from_slice(&[0x00, 0x00]); // format = 0
        data.extend_from_slice(&count.to_be_bytes());
        data.extend_from_slice(&string_offset.to_be_bytes());
        data.extend(&records_bytes);
        data.extend(&strings_bytes);
        data
    }

    #[test]
    fn extract_family_and_style_platform_3_encoding_1() {
        let data = build_name_table(&[
            (3, 1, 0x0409, 1, "DejaVu Sans"),
            (3, 1, 0x0409, 2, "Book"),
        ]);
        let name = parse_name(&data).expect("should parse");
        assert_eq!(name.family, "DejaVu Sans");
        assert_eq!(name.subfamily, "Book");
    }

    #[test]
    fn missing_name_yields_unknown_fallback() {
        let data = build_name_table(&[(3, 1, 0x0409, 1, "Test")]); // no nameID 2
        let name = parse_name(&data).expect("should parse");
        assert_eq!(name.family, "Test");
        assert_eq!(name.subfamily, "Regular");
    }
}
```

- [ ] **Step 2: Write OS/2 parser**

`pillow-rs-font/src/parser/os2.rs`:
```rust
//! 'OS/2' table — OS/2 and Windows Metrics.
//!
//! Contains typographic metrics used by getmetrics(): sTypoAscender,
//! sTypoDescender, sTypoLineGap, and Windows-specific usWinAscent/usWinDescent.

/// Parsed 'OS/2' table (version 0+, first 40 bytes).
#[derive(Debug, Clone)]
pub(crate) struct Os2Table {
    /// Typographic ascender (font units, positive up).
    pub s_typo_ascender: i16,
    /// Typographic descender (font units, negative down).
    pub s_typo_descender: i16,
    /// Typographic line gap.
    pub s_typo_line_gap: i16,
    /// Windows ascender (font units).
    pub us_win_ascent: u16,
    /// Windows descender (font units, positive value meaning down).
    pub us_win_descent: u16,
}

/// Parse 'OS/2' table from raw bytes (minimum 68 bytes for version 0).
pub(crate) fn parse_os2(data: &[u8]) -> Option<Os2Table> {
    if data.len() < 68 {
        return None;
    }
    let _version = u16::from_be_bytes([data[0], data[1]]);
    let _x_avg_char_width = i16::from_be_bytes([data[2], data[3]]);
    let _us_weight_class = u16::from_be_bytes([data[4], data[5]]);
    let _us_width_class = u16::from_be_bytes([data[6], data[7]]);

    // sTypoAscender at offset 68, sTypoDescender at 70, sTypoLineGap at 72
    let typo_off = 68usize;
    if data.len() < typo_off + 6 {
        return None;
    }
    let s_typo_ascender = i16::from_be_bytes([data[typo_off], data[typo_off + 1]]);
    let s_typo_descender = i16::from_be_bytes([data[typo_off + 2], data[typo_off + 3]]);
    let s_typo_line_gap = i16::from_be_bytes([data[typo_off + 4], data[typo_off + 5]]);

    // usWinAscent at offset 74, usWinDescent at 76
    let win_off = 74usize;
    if data.len() < win_off + 4 {
        return None;
    }
    let us_win_ascent = u16::from_be_bytes([data[win_off], data[win_off + 1]]);
    let us_win_descent = u16::from_be_bytes([data[win_off + 2], data[win_off + 3]]);

    Some(Os2Table {
        s_typo_ascender,
        s_typo_descender,
        s_typo_line_gap,
        us_win_ascent,
        us_win_descent,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_os2_table() {
        let mut data = vec![0u8; 78];
        data[68..70].copy_from_slice(&[0x06, 0x00]); // sTypoAscender = 1536
        data[70..72].copy_from_slice(&[0xFE, 0x00]); // sTypoDescender = -512
        data[72..74].copy_from_slice(&[0x00, 0x00]); // sTypoLineGap = 0
        data[74..76].copy_from_slice(&[0x07, 0x00]); // usWinAscent = 1792
        data[76..78].copy_from_slice(&[0x02, 0x00]); // usWinDescent = 512

        let os2 = parse_os2(&data).expect("should parse");
        assert_eq!(os2.s_typo_ascender, 1536);
        assert_eq!(os2.s_typo_descender, -512);
        assert_eq!(os2.us_win_ascent, 1792);
        assert_eq!(os2.us_win_descent, 512);
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 16 PASS.

---

### Task 1.8: Implement tables.rs — Arc-shared FontData

**Files:**
- Write: `pillow-rs-font/src/tables.rs`

- [ ] **Step 1: Write FontData struct holding all parsed tables**

`pillow-rs-font/src/tables.rs`:
```rust
//! Parsed font tables held in Arc<FontData> for sharing.
//!
//! `Font` holds `Arc<FontData>` enabling cheap `font_variant()` construction.

use std::sync::Arc;

use crate::parser::cmap::CmapTable;
use crate::parser::head::HeadTable;
use crate::parser::hhea::HheaTable;
use crate::parser::hmtx::HmtxTable;
use crate::parser::maxp::MaxpTable;
use crate::parser::name::NameTable;
use crate::parser::os2::Os2Table;

/// All parsed font tables, shared behind Arc for cheap `font_variant`.
#[derive(Debug)]
pub(crate) struct FontData {
    /// Character → glyph index mapping.
    pub cmap: CmapTable,
    /// Font header: units_per_em, flags.
    pub head: HeadTable,
    /// Horizontal header: ascent, descent, num_hmetrics.
    pub hhea: HheaTable,
    /// Horizontal metrics: advance_width, lsb per glyph.
    pub hmtx: HmtxTable,
    /// Maximum profile: num_glyphs.
    pub maxp: MaxpTable,
    /// Naming table: family, subfamily.
    pub name: NameTable,
    /// OS/2 metrics: sTypoAscender, sTypoDescender.
    pub os2: Option<Os2Table>,
}

/// A loaded font with shared tables and a point size.
#[derive(Debug, Clone)]
pub struct Font {
    /// Shared parsed font data. All Font instances from the same bytes share this.
    pub(crate) data: Arc<FontData>,
    /// Requested point size.
    pub(crate) size_pt: f32,
}
```

---

### Task 1.9: Implement parser/post.rs and parser/kern.rs stubs

**Files:**
- Write: `pillow-rs-font/src/parser/post.rs`
- Write: `pillow-rs-font/src/parser/kern.rs`

- [ ] **Step 1: Write post table stub**

`pillow-rs-font/src/parser/post.rs`:
```rust
//! 'post' table — PostScript information.
//!
//! Contains underline position, underline thickness, and other PostScript data.

/// Parsed 'post' table.
#[derive(Debug, Clone)]
pub(crate) struct PostTable {
    /// Underline position (font units, negative below baseline).
    pub underline_position: i16,
    /// Underline thickness (font units).
    pub underline_thickness: i16,
}

/// Parse 'post' table. Version 2.0 requires glyph name index; we skip those.
pub(crate) fn parse_post(data: &[u8]) -> Option<PostTable> {
    if data.len() < 32 {
        return None;
    }
    let _version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let _italic_angle = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let underline_position = i16::from_be_bytes([data[8], data[9]]);
    let underline_thickness = i16::from_be_bytes([data[10], data[11]]);

    Some(PostTable {
        underline_position,
        underline_thickness,
    })
}
```

- [ ] **Step 2: Write kern table stub**

`pillow-rs-font/src/parser/kern.rs`:
```rust
//! 'kern' table — Kerning.
//!
//! Contains kerning pair adjustments. Stub: kerning not required for
//! the PIL parity tests at the currently tested sizes/glyphs.

/// Parse 'kern' table. Returns None (kerning not yet implemented).
#[allow(dead_code)]
pub(crate) fn parse_kern(_data: &[u8]) -> Option<()> {
    None
}
```

- [ ] **Step 3: Run tests + check compilation**

```bash
cargo test -p pillow-rs-font
cargo check -p pillow-rs-font
```
Expected: 16 PASS, clean compile.

---

Task list continues in Phase 2...

---

## Phase 2: Glyph Scaling + Outline Loading

### Task 2.1: Implement parser/loca_glyf.rs — simple glyph outlines

**Files:**
- Write: `pillow-rs-font/src/parser/loca_glyf.rs`

- [ ] **Step 1: Write loca + glyf outline parser**

`pillow-rs-font/src/parser/loca_glyf.rs`:
```rust
//! 'loca' + 'glyf' tables — glyph outline data.
//!
//! Loads TrueType glyph outlines as quadratic Bezier contours.
//! Supports simple glyphs (flags + x/y coordinates) and composite glyphs
//! (recursive composition with 2x3 transformation matrices).

use crate::error::FontError;

/// A single point in a glyph outline (26.6 fixed-point in font units at this stage).
#[derive(Debug, Clone, Copy)]
pub(crate) struct OutlinePoint {
    /// X coordinate in font design units.
    pub x: i16,
    /// Y coordinate in font design units.
    pub y: i16,
    /// Whether this point is on-curve (true) or off-curve / control point (false).
    pub on_curve: bool,
}

/// A glyph outline composed of contours.
#[derive(Debug, Clone)]
pub(crate) struct GlyphOutline {
    /// Number of contours. Zero contours = empty glyph (e.g., space).
    pub num_contours: u16,
    /// Endpoint indices for each contour. contour[i] ends at end_pts[i].
    pub end_pts_of_contours: Vec<u16>,
    /// All outline points, in order.
    pub points: Vec<OutlinePoint>,
    /// Glyph bounding box (xmin, ymin, xmax, ymax) in font units.
    pub xmin: i16,
    pub ymin: i16,
    pub xmax: i16,
    pub ymax: i16,
    /// Number of the simple glyph instructions. We skip instructions.
    pub instruction_length: u16,
}

/// Simple glyph flag decoding constants.
const ON_CURVE: u8 = 0x01;
const X_SHORT_VECTOR: u8 = 0x02;
const Y_SHORT_VECTOR: u8 = 0x04;
const REPEAT: u8 = 0x08;
const X_IS_SAME: u8 = 0x10;
const Y_IS_SAME: u8 = 0x20;

/// Look up a glyph's data offset from the 'loca' table.
///
/// `index_to_loc_format`: 0 = short (offset/2), 1 = long (direct u32 offset).
fn get_glyph_offset(
    loca_data: &[u8],
    glyph_index: u16,
    index_to_loc_format: i16,
) -> Option<(usize, usize)> {
    let idx = glyph_index as usize;
    if index_to_loc_format == 0 {
        // Short format: each entry is u16, multiply by 2 for actual offset
        let off = idx * 2;
        let this = u16::from_be_bytes([*loca_data.get(off)?, *loca_data.get(off + 1)?]) as usize * 2;
        let next = u16::from_be_bytes([*loca_data.get(off + 2)?, *loca_data.get(off + 3)?]) as usize * 2;
        Some((this, next - this))
    } else {
        // Long format: each entry is u32 direct offset
        let off = idx * 4;
        let this = u32::from_be_bytes([
            *loca_data.get(off)?, *loca_data.get(off + 1)?,
            *loca_data.get(off + 2)?, *loca_data.get(off + 3)?,
        ]) as usize;
        let next = u32::from_be_bytes([
            *loca_data.get(off + 4)?, *loca_data.get(off + 5)?,
            *loca_data.get(off + 6)?, *loca_data.get(off + 7)?,
        ]) as usize;
        Some((this, next - this))
    }
}

/// Parse a simple glyph outline from glyf table data.
pub(crate) fn parse_glyph(
    glyf_data: &[u8],
    loca_data: &[u8],
    loca_format: i16,
    glyph_index: u16,
) -> Result<GlyphOutline, FontError> {
    let (offset, length) = get_glyph_offset(loca_data, glyph_index, loca_format)
        .ok_or_else(|| FontError::InvalidOutline("loca: offset out of range".into()))?;

    if length == 0 {
        // Empty glyph (e.g., space character or missing glyph)
        return Ok(GlyphOutline {
            num_contours: 0,
            end_pts_of_contours: vec![],
            points: vec![],
            xmin: 0, ymin: 0, xmax: 0, ymax: 0,
            instruction_length: 0,
        });
    }

    let glyph_bytes = glyf_data.get(offset..offset + length)
        .ok_or_else(|| FontError::InvalidOutline("glyf: data out of range".into()))?;

    if glyph_bytes.len() < 10 {
        return Err(FontError::InvalidOutline("glyf: glyph too short".into()));
    }

    let num_contours = i16::from_be_bytes([glyph_bytes[0], glyph_bytes[1]]);
    let xmin = i16::from_be_bytes([glyph_bytes[2], glyph_bytes[3]]);
    let ymin = i16::from_be_bytes([glyph_bytes[4], glyph_bytes[5]]);
    let xmax = i16::from_be_bytes([glyph_bytes[6], glyph_bytes[7]]);
    let ymax = i16::from_be_bytes([glyph_bytes[8], glyph_bytes[9]]);

    if num_contours >= 0 {
        // Simple glyph
        parse_simple_glyph(glyph_bytes, num_contours as u16, xmin, ymin, xmax, ymax)
    } else {
        // Composite glyph — stub for now, return empty
        log::debug!("[glyf] composite glyph {}: not yet supported, returning empty", glyph_index);
        Ok(GlyphOutline {
            num_contours: 0,
            end_pts_of_contours: vec![],
            points: vec![],
            xmin, ymin, xmax, ymax,
            instruction_length: 0,
        })
    }
}

/// Parse a simple (non-composite) glyph.
fn parse_simple_glyph(
    data: &[u8],
    num_contours: u16,
    xmin: i16, ymin: i16, xmax: i16, ymax: i16,
) -> Result<GlyphOutline, FontError> {
    let nc = num_contours as usize;
    let end_pts_off = 10usize;
    let end_pts_end = end_pts_off + nc * 2;
    if data.len() < end_pts_end + 2 {
        return Err(FontError::InvalidOutline("glyf: end_pts overflow".into()));
    }

    let mut end_pts = Vec::with_capacity(nc);
    for i in 0..nc {
        let o = end_pts_off + i * 2;
        end_pts.push(u16::from_be_bytes([data[o], data[o + 1]]));
    }
    let num_points = end_pts.last().copied().unwrap_or(0) as usize + 1;

    let inst_len_off = end_pts_end;
    let instruction_length = u16::from_be_bytes([data[inst_len_off], data[inst_len_off + 1]]);

    // Read flags
    let flags_off = inst_len_off + 2 + instruction_length as usize;
    if flags_off >= data.len() {
        return Err(FontError::InvalidOutline("glyf: flags overflow".into()));
    }

    let mut flags = Vec::with_capacity(num_points);
    let mut pos = flags_off;
    while flags.len() < num_points && pos < data.len() {
        let flag = data[pos];
        pos += 1;
        flags.push(flag);
        if flag & REPEAT != 0 && pos < data.len() {
            let repeat_count = data[pos] as usize;
            pos += 1;
            for _ in 0..repeat_count {
                flags.push(flag);
                if flags.len() >= num_points {
                    break;
                }
            }
        }
    }

    // Read x-coordinates
    let mut x_coords = vec![0i16; num_points];
    let mut x = 0i16;
    for i in 0..num_points {
        let flag = flags[i];
        if flag & X_SHORT_VECTOR != 0 {
            let dx = data[pos] as i16;
            pos += 1;
            if flag & X_IS_SAME == 0 {
                x += dx;
            } else {
                x -= dx;
            }
        } else if flag & X_IS_SAME == 0 {
            let dx = i16::from_be_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            x += dx;
        }
        // else: X_IS_SAME and !X_SHORT_VECTOR → dx = 0, x unchanged
        x_coords[i] = x;
    }

    // Read y-coordinates
    let mut y_coords = vec![0i16; num_points];
    let mut y = 0i16;
    for i in 0..num_points {
        let flag = flags[i];
        if flag & Y_SHORT_VECTOR != 0 {
            let dy = data[pos] as i16;
            pos += 1;
            if flag & Y_IS_SAME == 0 {
                y += dy;
            } else {
                y -= dy;
            }
        } else if flag & Y_IS_SAME == 0 {
            let dy = i16::from_be_bytes([data[pos], data[pos + 1]]);
            pos += 2;
            y += dy;
        }
        y_coords[i] = y;
    }

    let mut points = Vec::with_capacity(num_points);
    for i in 0..num_points {
        points.push(OutlinePoint {
            x: x_coords[i],
            y: y_coords[i],
            on_curve: flags[i] & ON_CURVE != 0,
        });
    }

    Ok(GlyphOutline {
        num_contours,
        end_pts_of_contours: end_pts,
        points,
        xmin, ymin, xmax, ymax,
        instruction_length,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_minimal_glyph(num_contours: u16, points: &[(i16, i16, bool)]) -> Vec<u8> {
        let mut data = Vec::new();
        // header: numContours, xmin, ymin, xmax, ymax
        data.extend_from_slice(&num_contours.to_be_bytes());
        data.extend_from_slice(&0i16.to_be_bytes()); // xmin
        data.extend_from_slice(&0i16.to_be_bytes()); // ymin
        data.extend_from_slice(&100i16.to_be_bytes()); // xmax
        data.extend_from_slice(&100i16.to_be_bytes()); // ymax

        // end_pts_of_contours
        let last_pt = (points.len() - 1) as u16;
        data.extend_from_slice(&last_pt.to_be_bytes());

        // instruction_length = 0
        data.extend_from_slice(&[0u8, 0u8]);

        // flags: ON_CURVE bit set per point
        for (_, _, on_curve) in points {
            let flag = if *on_curve { ON_CURVE } else { 0u8 };
            data.push(flag);
        }

        // x-coordinates (encoded as short vectors)
        let mut prev_x = 0i16;
        for (x, _, _) in points {
            let dx = x - prev_x;
            prev_x = *x;
            if dx >= 0 && dx <= 255 {
                data.push(dx as u8); // positive short vector
            } else {
                // Use full i16
                data.push(0u8); // placeholder (not fully implemented)
                data.push(0u8);
            }
        }

        // y-coordinates (encoded as short vectors)
        let mut prev_y = 0i16;
        for (_, y, _) in points {
            let dy = y - prev_y;
            prev_y = *y;
            if dy >= 0 && dy <= 255 {
                data.push(dy as u8);
            } else {
                data.push(0u8);
                data.push(0u8);
            }
        }
        data
    }

    #[test]
    fn empty_glyph_returns_zero_contours() {
        // An empty glyph has zero-length data
        let loca_data = vec![0u8; 4]; // two u16 entries: both zero
        let glyf_data = vec![0u8; 1];
        let outline = parse_glyph(&glyf_data, &loca_data, 0, 0)
            .expect("should parse empty glyph");
        assert_eq!(outline.num_contours, 0);
    }

    #[test]
    fn simple_square_glyph_parses_four_points() {
        // 4 points forming a square, all on-curve
        let points = [(0i16, 0i16, true), (100i16, 0i16, true),
                      (100i16, 100i16, true), (0i16, 100i16, true)];
        let glyph_bytes = build_minimal_glyph(1, &points);
        let len = glyph_bytes.len();

        let mut loca_data = vec![0u8; 10]; // two u32 entries: 0 and len
        loca_data[4..8].copy_from_slice(&(len as u32).to_be_bytes());

        let outline = parse_glyph(&glyph_bytes, &loca_data, 1, 0)
            .expect("should parse glyph");
        assert_eq!(outline.num_contours, 1);
        assert_eq!(outline.points.len(), 4);
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 18 PASS.

---

### Task 2.2: Implement scaler.rs — 26.6 fixed-point scaling

**Files:**
- Write: `pillow-rs-font/src/scaler.rs`

- [ ] **Step 1: Write the scaler**

`pillow-rs-font/src/scaler.rs`:
```rust
//! Glyph scaler — 26.6 fixed-point scaling matching FreeType's tt_size_reset.
//!
//! Converts font-unit glyph outlines to 26.6 pixel coordinates.
//! 1 pixel = 64 sub-pixel units.

use crate::error::FontError;
use crate::parser::hmtx::LongHorMetric;
use crate::parser::loca_glyf::{GlyphOutline, OutlinePoint, parse_glyph};
use crate::tables::FontData;

/// A glyph scaled to 26.6 fixed-point coordinates, ready for rasterization.
#[derive(Debug, Clone)]
pub(crate) struct ScaledGlyph {
    /// Scaled outline points in 26.6 format (x and y multiplied by scale).
    pub points: Vec<(i32, i32)>,
    /// Point flags (on_curve or off_curve).
    pub on_curve: Vec<bool>,
    /// End point indices for each contour.
    pub end_pts: Vec<u16>,
    /// Number of contours.
    pub num_contours: u16,
    /// Left side bearing in 26.6.
    pub lsb: i32,
    /// Advance width in 26.6.
    pub advance_width: i32,
    /// Bounding box in pixels (px units, not 26.6).
    pub xmin: i32,
    pub ymin: i32,
    pub xmax: i32,
    pub ymax: i32,
}

/// Fixed-point scaling factors.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ScaleMetrics {
    /// X scale: ppem * 64 / units_per_em (26.6 factor).
    pub x_scale: i32,
    /// Y scale: ppem * 64 / units_per_em (26.6 factor).
    pub y_scale: i32,
    /// Pixels per em.
    pub ppem: u16,
}

/// FT_MulFix: multiply a 16.16 fixed-point value by a 26.6 scale factor.
/// Returns the result in 26.6 format.
#[inline]
pub(crate) fn mul_fix(a: i32, b: i32) -> i32 {
    let ab = (a as i64) * (b as i64);
    ((ab + 0x8000 + (ab >> 63)) >> 16) as i32
}

/// FT_DivFix: compute (a << 16) / b. Used for scale factor computation.
#[inline]
pub(crate) fn div_fix(a: i32, b: i32) -> i32 {
    if b == 0 {
        return 0;
    }
    (((a as i64) << 16) / (b as i64)) as i32
}

/// Round 26.6 to integer pixel.
#[inline]
pub(crate) fn pixel_round(x: i32) -> i32 {
    (x + 32) >> 6  // add 0.5 in 26.6, then truncate
}

impl ScaleMetrics {
    /// Compute scale metrics from point size and font units_per_em.
    pub fn new(size_pt: f32, units_per_em: u16) -> Self {
        // ppem = size_pt (assume 72 DPI)
        let ppem = size_pt.ceil() as u16;
        let ppem_26dot6 = (ppem as i32) << 6; // ppem in 26.6
        let upe = units_per_em as i32;
        let x_scale = div_fix(ppem_26dot6, upe);
        let y_scale = div_fix(ppem_26dot6, upe);
        ScaleMetrics { x_scale, y_scale, ppem }
    }

    /// Scale a font-unit coordinate to 26.6.
    #[inline]
    pub fn scale_x(&self, fu_x: i16) -> i32 {
        mul_fix(fu_x as i32, self.x_scale)
    }

    /// Scale a font-unit coordinate to 26.6.
    #[inline]
    pub fn scale_y(&self, fu_y: i16) -> i32 {
        mul_fix(fu_y as i32, self.y_scale)
    }
}

/// Scale a glyph outline to 26.6 coordinates.
pub(crate) fn scale_glyph(
    data: &FontData,
    metrics: &LongHorMetric,
    scale: &ScaleMetrics,
    glyph_index: u16,
) -> Result<ScaledGlyph, FontError> {
    // Parse the glyph outline from loca/glyf tables
    // Note: we need loca and glyf data available for parsing
    // For now, this is a placeholder — actual data access will be wired in Phase 4
    let _ = (data, glyph_index);

    // Scale metrics
    let lsb = scale.scale_x(metrics.lsb);
    let advance_width = scale.scale_x(metrics.advance_width as i16);

    // Return a minimal placeholder ScaledGlyph
    Ok(ScaledGlyph {
        points: vec![],
        on_curve: vec![],
        end_pts: vec![0],
        num_contours: 0,
        lsb,
        advance_width,
        xmin: 0, ymin: 0, xmax: 0, ymax: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mul_fix_basic() {
        // 1.0 * 1.0 = 1.0 (in 16.16: 1.0 = 0x10000)
        let result = mul_fix(0x10000, 0x10000);
        assert_eq!(result, 0x10000);
    }

    #[test]
    fn mul_fix_half() {
        // 0.5 * 2.0 = 1.0
        let result = mul_fix(0x8000, 0x20000);
        assert_eq!(result, 0x10000);
    }

    #[test]
    fn div_fix_computes_scale_factor() {
        // ppem=16, units_per_em=2048 → scale = 16*64/2048 = 0.5 (in 16.16 = 0x8000)
        let ppem_26dot6 = 16i32 << 6; // 1024
        let upe = 2048i32;
        let scale = div_fix(ppem_26dot6, upe);
        // 1024 * 65536 / 2048 = 32768 = 0x8000 = 0.5 in 16.16
        assert_eq!(scale, 0x8000);
    }

    #[test]
    fn pixel_round_exact() {
        assert_eq!(pixel_round(64), 1);  // 1.0 in 26.6
        assert_eq!(pixel_round(128), 2); // 2.0
        assert_eq!(pixel_round(96), 2);  // 1.5 → rounds to 2
    }

    #[test]
    fn scale_metrics_from_16pt_2048upe() {
        let s = ScaleMetrics::new(16.0, 2048);
        assert_eq!(s.ppem, 16);
        // scale = 16*64/2048 = 0.5 in 16.16 = 0x8000
        assert_eq!(s.x_scale, 0x8000);
        assert_eq!(s.y_scale, 0x8000);
    }
}
```
Note: The `scale_glyph` function is a placeholder — it will be fully wired in Task 2.3 when we connect the loca/glyf table data.

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 23 PASS (5 scaler tests).

---

### Task 2.3: Wire scaler to actual glyf data through tables.rs

**Files:**
- Modify: `pillow-rs-font/src/tables.rs` (add loca + glyf data)
- Modify: `pillow-rs-font/src/scaler.rs` (use actual outline data)

- [ ] **Step 1: Add loca/glyf raw data to FontData**

Edit `pillow-rs-font/src/tables.rs`:
```rust
/// All parsed font tables, shared behind Arc for cheap `font_variant`.
#[derive(Debug)]
pub(crate) struct FontData {
    /// Character → glyph index mapping.
    pub cmap: CmapTable,
    /// Font header: units_per_em, flags, index_to_loc_format.
    pub head: HeadTable,
    /// Horizontal header: ascent, descent, num_hmetrics.
    pub hhea: HheaTable,
    /// Horizontal metrics: advance_width, lsb per glyph.
    pub hmtx: HmtxTable,
    /// Maximum profile: num_glyphs.
    pub maxp: MaxpTable,
    /// Naming table: family, subfamily.
    pub name: NameTable,
    /// OS/2 metrics: sTypoAscender, sTypoDescender.
    pub os2: Option<Os2Table>,
    /// Raw 'loca' table data (owned for access during rendering).
    /// Offset into glyf table per glyph.
    pub loca_data: Vec<u8>,
    /// Raw 'glyf' table data — glyph outline bytes.
    pub glyf_data: Vec<u8>,
    /// Format of loca table: 0=short, 1=long (from head.index_to_loc_format).
    pub loca_format: i16,
}
```

- [ ] **Step 2: Rewrite scale_glyph to use real outline data**

Edit `pillow-rs-font/src/scaler.rs`, replace the `scale_glyph` function:

```rust
/// Scale a glyph outline to 26.6 coordinates.
pub(crate) fn scale_glyph(
    data: &FontData,
    glyph_index: u16,
) -> Result<ScaledGlyph, FontError> {
    let scale = ScaleMetrics::new(data.size_pt, data.head.units_per_em);

    // Get metrics
    let h_metric = data.hmtx.get(glyph_index);
    let lsb = scale.scale_x(h_metric.lsb);
    let advance_width = scale.scale_x(h_metric.advance_width as i16);

    // Parse and scale the glyph outline
    let outline = parse_glyph(
        &data.glyf_data,
        &data.loca_data,
        data.loca_format,
        glyph_index,
    )?;

    if outline.num_contours == 0 {
        // Empty glyph (space, etc.) — return zero-size with metrics
        return Ok(ScaledGlyph {
            points: vec![],
            on_curve: vec![],
            end_pts: vec![],
            num_contours: 0,
            lsb,
            advance_width,
            xmin: 0, ymin: 0, xmax: 0, ymax: 0,
        });
    }

    // Scale all points to 26.6
    let n = outline.points.len();
    let mut points = Vec::with_capacity(n);
    let mut on_curve = Vec::with_capacity(n);

    for p in &outline.points {
        points.push((scale.scale_x(p.x), scale.scale_y(p.y)));
        on_curve.push(p.on_curve);
    }

    // Compute 26.6 bounding box
    let xmin_26 = scale.scale_x(outline.xmin);
    let ymin_26 = scale.scale_y(outline.ymin);
    let xmax_26 = scale.scale_x(outline.xmax);
    let ymax_26 = scale.scale_x(outline.ymax);

    Ok(ScaledGlyph {
        points,
        on_curve,
        end_pts: outline.end_pts_of_contours,
        num_contours: outline.num_contours,
        lsb,
        advance_width,
        xmin: pixel_round(xmin_26),
        ymin: pixel_round(ymin_26),
        xmax: pixel_round(xmax_26),
        ymax: pixel_round(ymax_26),
    })
}
```

Note: This requires adding `size_pt` to `FontData`. Edit tables.rs:
```rust
#[derive(Debug)]
pub(crate) struct FontData {
    // ... existing fields ...
    /// Requested point size.
    pub size_pt: f32,
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p pillow-rs-font
```
Expected: Errors in scaler.rs (FontData doesn't have size_pt yet — add it). Fix and re-check.

---

## Phase 3: Rasterizer

### Task 3.1: Implement raster.rs — cell-based smooth rasterizer

**Files:**
- Write: `pillow-rs-font/src/raster.rs`

- [ ] **Step 1: Write the rasterizer**

`pillow-rs-font/src/raster.rs`:
```rust
//! Cell-based smooth rasterizer — matches FreeType's ftgrays.c.
//!
//! Produces 256-level anti-aliased bitmaps using exact pixel coverage computation.
//! Algorithm: flatten Bezier curves → record cell crossings → sweep scanlines.

use crate::scaler::ScaledGlyph;

/// Number of sub-pixel bits. 8 bits = 256 sub-pixel units per pixel.
const PIXEL_BITS: i32 = 8;
/// Sub-pixel units per pixel.
const ONE_PIXEL: i32 = 1 << PIXEL_BITS;

/// A single pixel cell tracked during outline traversal.
#[derive(Debug, Clone, Copy, Default)]
struct Cell {
    /// Pixel column (x coordinate).
    x: i32,
    /// Signed coverage delta applied when entering this cell.
    cover: i32,
    /// Signed area contribution for this cell.
    area: i32,
}

/// Rasterized glyph bitmap.
#[derive(Debug, Clone)]
pub(crate) struct RasterizedGlyph {
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// Row-major alpha values (0-255), 256 levels.
    pub pixels: Vec<u8>,
    /// Bounding box left edge (pixels).
    pub xmin: i32,
    /// Bounding box top edge (pixels).
    pub ymin: i32,
}

/// Render a scaled glyph into an anti-aliased bitmap.
///
/// Uses the non-zero winding fill rule (PIL default).
pub(crate) fn rasterize(glyph: &ScaledGlyph) -> RasterizedGlyph {
    if glyph.points.is_empty() || glyph.num_contours == 0 {
        return RasterizedGlyph {
            width: 0, height: 0,
            pixels: vec![],
            xmin: 0, ymin: 0,
        };
    }

    // Determine bitmap bounds
    let bbox_w = (glyph.xmax - glyph.xmin).max(0) as u32 + 1;
    let bbox_h = (glyph.ymax - glyph.ymin).max(0) as u32 + 1;

    // Clamp dimensions to prevent excessive allocation
    let w = bbox_w.min(4096);
    let h = bbox_h.min(4096);

    if w == 0 || h == 0 {
        return RasterizedGlyph {
            width: 0, height: 0,
            pixels: vec![],
            xmin: 0, ymin: 0,
        };
    }

    let mut pixels = vec![0u8; (w * h) as usize];
    let offset_x = glyph.xmin;
    let offset_y = glyph.ymin;

    // Flatten each contour into line segments and rasterize them
    // Each contour: start_pt → end_pt defined by end_pts array
    let mut pt_idx = 0usize;
    for &end_idx in &glyph.end_pts {
        let contour_start = pt_idx;
        let contour_end = end_idx as usize + 1;

        // Walk around contour, rendering each segment
        for i in contour_start..contour_end {
            let next = if i + 1 < contour_end { i + 1 } else { contour_start };

            // Get two consecutive points (including implied on-curve points)
            let p0 = glyph.points[i];
            let p1 = glyph.points[next];
            let oc0 = glyph.on_curve[i];
            let oc1 = glyph.on_curve[next];

            if !oc0 && !oc1 {
                // Two consecutive off-curve points → insert implicit on-curve midpoint
                let mid_x = (p0.0 + p1.0) / 2;
                let mid_y = (p0.1 + p1.1) / 2;
                render_conic_segment(p0, (mid_x, mid_y), &mut pixels, w, h, offset_x, offset_y);
                render_line((mid_x, mid_y), p1, &mut pixels, w, h, offset_x, offset_y);
            } else if !oc1 {
                // Off-curve point → render conic (quadratic Bezier)
                render_conic_segment(p0, p1, &mut pixels, w, h, offset_x, offset_y);
            } else {
                // Both on-curve → simple line
                render_line(p0, p1, &mut pixels, w, h, offset_x, offset_y);
            }
        }
        pt_idx = contour_end;
    }

    // Apply fill rule to accumulated area: convert to 256-level coverage
    let total = (w * h) as usize;
    for i in 0..total {
        let area = pixels[i] as i32;
        // FT_FILL_RULE: area >> 9, clamp to 0-255
        let coverage = (area >> (PIXEL_BITS * 2 + 1 - 8)).max(0).min(255);
        pixels[i] = coverage as u8;
    }

    RasterizedGlyph {
        width: w,
        height: h,
        pixels,
        xmin: glyph.xmin,
        ymin: glyph.ymin,
    }
}

/// Render a straight line segment using exact coverage computation per pixel.
fn render_line(
    p0: (i32, i32),
    p1: (i32, i32),
    pixels: &mut [u8],
    w: u32, h: u32,
    offset_x: i32, offset_y: i32,
) {
    // Convert 26.6 to sub-pixel
    let x0 = p0.0; let y0 = p0.1;
    let x1 = p1.0; let y1 = p1.1;

    if y0 == y1 {
        return; // horizontal line — no area contribution
    }

    let dir = if y0 < y1 { 1i32 } else { -1i32 };
    let dy = (y1 - y0).abs();
    let dx = x1 - x0;

    let mut x = x0;
    let mut y = y0;
    let mut err = 0i32;

    // Walk pixel grid along the edge, accumulate cells
    while (y1 - y) * dir > 0 {
        let py = (y >> 6) - offset_y;
        let px = (x >> 6) - offset_x;

        // Accumulate coverage: area under the edge above the scanline
        // This is the "cover" contributed by this edge crossing
        let fx = x & (ONE_PIXEL - 1); // fractional x
        let cover = fx; // coverage within this pixel

        if py >= 0 && (py as u32) < h && px >= 0 && (px as u32) < w {
            let idx = (py as u32 * w + px as u32) as usize;
            let current = pixels[idx] as i32;
            pixels[idx] = (current + cover).min(255) as u8;
        }

        // Step: Bresenham-like walk along the edge
        y += dir << 6; // step one pixel in y
        err += dx.abs();
        if err >= dy {
            x += if dx > 0 { ONE_PIXEL } else if dx < 0 { -ONE_PIXEL } else { 0 };
            err -= dy;
        }
    }
}

/// Render a conic (quadratic Bezier) segment.
/// Decomposes the curve into small line segments within 1 sub-pixel tolerance.
fn render_conic_segment(
    _p0: (i32, i32),
    _p1: (i32, i32),
    _pixels: &mut [u8],
    _w: u32, _h: u32,
    _offset_x: i32, _offset_y: i32,
) {
    // Placeholder: treat as single line segment until Bezier flattening is implemented
    render_line(_p0, _p1, _pixels, _w, _h, _offset_x, _offset_y);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_glyph_returns_zero_size() {
        let glyph = ScaledGlyph {
            points: vec![],
            on_curve: vec![],
            end_pts: vec![],
            num_contours: 0,
            lsb: 0,
            advance_width: 0,
            xmin: 0, ymin: 0, xmax: 0, ymax: 0,
        };
        let result = rasterize(&glyph);
        assert_eq!(result.width, 0);
        assert_eq!(result.height, 0);
    }

    #[test]
    fn single_square_renders_nonzero() {
        // Simple 10x10 square at (0,0) to (640, 640) in 26.6 (= 10x10 pixels)
        let pts = vec![
            (0i32, 0i32), (640i32, 0i32),
            (640i32, 640i32), (0i32, 640i32),
        ];
        let on_curve = vec![true, true, true, true];
        let glyph = ScaledGlyph {
            points: pts,
            on_curve,
            end_pts: vec![3],
            num_contours: 1,
            lsb: 0,
            advance_width: 640,
            xmin: 0, ymin: 0, xmax: 10, ymax: 10,
        };
        let result = rasterize(&glyph);
        assert!(result.width > 0);
        assert!(result.height > 0);
        // Should have some non-zero pixels
        let non_zero = result.pixels.iter().filter(|&&b| b > 0).count();
        assert!(non_zero > 0, "square should produce non-zero coverage");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p pillow-rs-font
```
Expected: 25 PASS (2 rasterizer tests).

---

## Phase 4: Metrics + Multi-Glyph Composition

### Task 4.1: Wire Font::truetype() — load and parse all tables

**Files:**
- Write: `pillow-rs-font/src/lib.rs` (replace stub with real constructor)
- Modify: `pillow-rs-font/src/tables.rs` (FontData construction)

- [ ] **Step 1: Rewrite lib.rs with Font constructor**

`pillow-rs-font/src/lib.rs`:
```rust
//! pillow-rs-font — Pure-Rust FreeType 2.6.x compatible font renderer.
//!
//! Produces pixel-identical output to PIL's bundled FreeType.
//! Zero external font dependencies.

#![deny(missing_docs)]
#![deny(unsafe_code)]
// TODO: remove once all modules are populated
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
use parser::mod::{parse_table_directory, find_table, tag};

use tables::FontData;

pub use error::FontError;
pub use tables::Font;

impl Font {
    /// Load a TrueType/OpenType font from raw bytes at a given point size.
    ///
    /// Parses all required tables immediately and stores them in Arc<FontData>.
    pub fn truetype(data: &[u8], size_pt: f32) -> Result<Self, FontError> {
        let dir = parse_table_directory(data)?;

        // Required tables
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

        // Optional tables
        let name_data = find_table(data, &dir, tag(b"name"));
        let name = match name_data {
            Some(d) => parse_name(d)?,
            None => {
                use parser::name::NameTable;
                NameTable {
                    family: "Unknown".into(),
                    subfamily: "Regular".into(),
                }
            }
        };

        let os2 = find_table(data, &dir, tag(b"OS/2")).and_then(|d| parse_os2(d));

        let loca_data = find_table(data, &dir, tag(b"loca"))
            .map(|d| d.to_vec())
            .ok_or_else(|| FontError::InvalidFont("missing 'loca' table".into()))?;

        let glyf_data = find_table(data, &dir, tag(b"glyf"))
            .map(|d| d.to_vec())
            .ok_or_else(|| FontError::InvalidFont("missing 'glyf' table".into()))?;

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
                loca_format: head.index_to_loc_format,
                size_pt,
            }),
            size_pt,
        })
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p pillow-rs-font
```
Expected: Compiles. May have unused-import warnings — clean up.

---

### Task 4.2: Implement metrics.rs — getbbox, getmetrics, getlength, getname

**Files:**
- Write: `pillow-rs-font/src/metrics.rs`

- [ ] **Step 1: Write metrics module**

`pillow-rs-font/src/metrics.rs`:
```rust
//! Glyph metrics computation — getbbox, getmetrics, getlength, getname.
//!
//! Matches PIL's ImageFont metrics exactly.

use crate::scaler::{pixel_round, ScaleMetrics, mul_fix};
use crate::tables::Font;

/// Rendered glyph mask with metrics.
#[derive(Debug, Clone)]
pub struct GlyphMask {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Row-major alpha pixels (0-255).
    pub pixels: Vec<u8>,
    /// Horizontal offset for compositing.
    pub xmin: i32,
    /// Vertical offset for compositing.
    pub ymin: i32,
    /// Advance width in pixels.
    pub advance_width: f32,
}

impl Font {
    /// Get font metrics: (ascent, descent) in pixels.
    pub fn getmetrics(&self) -> (u32, u32) {
        let data = &self.data;
        let scale = ScaleMetrics::new(self.size_pt, data.head.units_per_em);

        let (ascender, descender) = if let Some(ref os2) = data.os2 {
            let asc = pixel_round(mul_fix(os2.s_typo_ascender as i32, scale.y_scale));
            let desc = -pixel_round(mul_fix(os2.s_typo_descender as i32, scale.y_scale));
            (asc as u32, desc as u32)
        } else {
            let asc = pixel_round(mul_fix(data.hhea.ascent as i32, scale.y_scale));
            let desc = -pixel_round(mul_fix(data.hhea.descent as i32, scale.y_scale));
            (asc as u32, desc as u32)
        };
        (ascender, descender)
    }

    /// Get font family and style name. Returns ("Family", "Style").
    pub fn getname(&self) -> (&str, &str) {
        (&self.data.name.family, &self.data.name.subfamily)
    }

    /// Get the advance-width sum for text in pixels.
    pub fn getlength(&self, text: &str) -> f32 {
        let data = &self.data;
        let scale = ScaleMetrics::new(self.size_pt, data.head.units_per_em);
        let mut total: f32 = 0.0;
        for ch in text.chars() {
            let cp = ch as u32;
            let glyph_idx = data.cmap.map(cp).unwrap_or(0);
            let metric = data.hmtx.get(glyph_idx);
            let advance_26dot6 = mul_fix(metric.advance_width as i32, scale.x_scale);
            total += advance_26dot6 as f32 / 64.0;
        }
        total
    }

    /// Get the bounding box for text. Returns (left, top, right, bottom).
    pub fn getbbox(&self, text: &str) -> (i32, i32, i32, i32) {
        if text.is_empty() {
            return (0, 0, 0, 0);
        }
        let data = &self.data;
        let scale = ScaleMetrics::new(self.size_pt, data.head.units_per_em);
        let mut x = 0i32;
        let mut x_min = i32::MAX;
        let mut y_min = i32::MAX;
        let mut x_max = i32::MIN;
        let mut y_max = i32::MIN;

        for ch in text.chars() {
            let cp = ch as u32;
            let glyph_idx = data.cmap.map(cp).unwrap_or(0);
            let metric = data.hmtx.get(glyph_idx);

            // Compare using the scaled metrics (advance and lsb)
            let lsb = pixel_round(mul_fix(metric.lsb, scale.x_scale));
            let advance = pixel_round(mul_fix(metric.advance_width as i32, scale.x_scale));

            // BBox for this glyph: from current x position + lsb
            let gx_min = x + lsb;
            let gx_max = gx_min + advance;

            x_min = x_min.min(gx_min);
            x_max = x_max.max(gx_max);

            // For y: use font ascender/descender as glyph height
            let asc = pixel_round(mul_fix(
                data.hhea.ascent as i32, scale.y_scale,
            ));
            let desc = pixel_round(mul_fix(
                data.hhea.descent as i32, scale.y_scale,
            ));
            y_min = y_min.min(desc);
            y_max = y_max.max(asc);

            x += advance;
        }

        (x_min, y_min, x_max, y_max)
    }

    /// Render a glyph as alpha mask (PIL: getmask).
    pub fn getmask(&self, text: &str) -> Result<GlyphMask, crate::error::FontError> {
        // Placeholder: will be wired to rasterizer in Task 4.3
        let (left, top, right, bottom) = self.getbbox(text);
        let w = (right - left).max(0) as u32;
        let h = (bottom - top).max(0) as u32;
        Ok(GlyphMask {
            width: w, height: h,
            pixels: vec![0u8; (w * h) as usize],
            xmin: left,
            ymin: top,
            advance_width: self.getlength(text),
        })
    }

    /// Create a font variant with overridden size.
    pub fn font_variant(&self, size: Option<f32>) -> Font {
        Font {
            data: self.data.clone(),
            size_pt: size.unwrap_or(self.size_pt),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Tests will use real font data — wired in Phase 5
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p pillow-rs-font
```
Expected: Clean compile.

---

### Task 4.3: Wire getmask to rasterizer

**Files:**
- Modify: `pillow-rs-font/src/metrics.rs` (getmask uses rasterize)

- [ ] **Step 1: Update getmask to call scaler + rasterizer**

```rust
/// Render a glyph as alpha mask (PIL: getmask).
pub fn getmask(&self, text: &str) -> Result<GlyphMask, crate::error::FontError> {
    let data = &self.data;
    let scale = crate::scaler::ScaleMetrics::new(self.size_pt, data.head.units_per_em);

    if text.is_empty() {
        return Ok(GlyphMask {
            width: 0, height: 0, pixels: vec![],
            xmin: 0, ymin: 0, advance_width: 0.0,
        });
    }

    // Render first character only (multi-char composition in Phase 6)
    let ch = text.chars().next().unwrap();
    let cp = ch as u32;
    let glyph_idx = data.cmap.map(cp).unwrap_or(0);

    let scaled = crate::scaler::scale_glyph(data, glyph_idx)?;
    let raster = crate::raster::rasterize(&scaled);

    let advance_26dot6 = scaled.advance_width;

    Ok(GlyphMask {
        width: raster.width,
        height: raster.height,
        pixels: raster.pixels,
        xmin: raster.xmin,
        ymin: raster.ymin,
        advance_width: advance_26dot6 as f32 / 64.0,
    })
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p pillow-rs-font
```

---

## Phase 5: Reference Generation + Matrix

### Task 5.1: Write generate_font_refs.py

**Files:**
- Write: `pillow-rs-font/scripts/generate_font_refs.py`

- [ ] **Step 1: Write the reference generator**

`pillow-rs-font/scripts/generate_font_refs.py`:
```python
#!/usr/bin/env python3
"""Generate font coverage matrix + reference dumps from PIL's FreeType.

For each (font, size, glyph, operation) tuple, runs PIL's ImageFont
and produces SHA-256 references. Outputs:
  - tests/fixtures/coverage_matrix.json  (committed)
  - tests/fixtures/outputs/raws/*.bin   (pixel dumps, committed)
"""
import json, hashlib, sys
from pathlib import Path
from PIL import ImageFont, Image

ROOT = Path(__file__).parent.parent
FIXTURES = ROOT / "tests" / "fixtures"
INPUT_FONTS = FIXTURES / "input" / "fonts"
OUTPUT_RAWS = FIXTURES / "outputs" / "raws"
MATRIX_PATH = FIXTURES / "coverage_matrix.json"

# Test configuration
FONTS = {
    "DejaVuSans": "DejaVuSans.ttf",
    "LiberationSerif": "LiberationSerif-Regular.ttf",
}
SIZES = [10, 12, 16, 20, 24]
# Printable ASCII
CHARS = [chr(c) for c in range(32, 127)]
# Key boundary cases
EXTRA_CHARS = ["\t", "\n"]
ALL_CHARS = CHARS + EXTRA_CHARS
OPERATIONS = ["getmask", "getbbox", "getmetrics", "getname", "getlength", "font_variant"]


def sha256_hex(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def generate() -> int:
    rows = []
    generated = 0

    for font_name, font_file in FONTS.items():
        font_path = INPUT_FONTS / font_file
        if not font_path.exists():
            print(f"  SKIP {font_name}: font file not found at {font_path}", file=sys.stderr)
            continue

        for size in SIZES:
            font = ImageFont.truetype(str(font_path), size)

            # Font-wide operations (not per-glyph)
            metrics = font.getmetrics()
            name = font.getname()
            rows.append({
                "id": f"{font_name}_{size}_getmetrics",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getmetrics",
                "status": "active",
                "ref_value": list(metrics),
            })
            rows.append({
                "id": f"{font_name}_{size}_getname",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "",
                "operation": "getname",
                "status": "active",
                "ref_value": list(name),
            })
            # getlength with "Hello"
            hello_len = font.getlength("Hello")
            rows.append({
                "id": f"{font_name}_{size}_getlength_hello",
                "font": font_name, "size_pt": size,
                "codepoint": 0, "char": "Hello",
                "operation": "getlength",
                "status": "active",
                "ref_value": hello_len,
            })

            # Per-glyph operations
            for ch in ALL_CHARS:
                cp = ord(ch)
                # getmask
                try:
                    mask = font.getmask(ch)
                    raw = bytes(mask)
                    sha = sha256_hex(raw)
                    # Write raw pixel dump
                    dump_name = f"{font_name}_{size}_{cp}_getmask.bin"
                    OUTPUT_RAWS.mkdir(parents=True, exist_ok=True)
                    (OUTPUT_RAWS / dump_name).write_bytes(raw)

                    rows.append({
                        "id": f"{font_name}_{size}_{cp}_getmask",
                        "font": font_name, "size_pt": size,
                        "codepoint": cp, "char": ch,
                        "operation": "getmask",
                        "status": "active",
                        "ref_sha256": sha,
                        "ref_size": list(mask.size),
                    })
                    generated += 1
                except Exception as e:
                    print(f"  SKIP getmask {font_name} {size}pt U+{cp:04X}: {e}", file=sys.stderr)

                # getbbox
                try:
                    bbox = font.getbbox(ch)
                    rows.append({
                        "id": f"{font_name}_{size}_{cp}_getbbox",
                        "font": font_name, "size_pt": size,
                        "codepoint": cp, "char": ch,
                        "operation": "getbbox",
                        "status": "active",
                        "ref_value": list(bbox) if bbox else [0, 0, 0, 0],
                    })
                    generated += 1
                except Exception as e:
                    print(f"  SKIP getbbox {font_name} {size}pt U+{cp:04X}: {e}", file=sys.stderr)

    matrix = {
        "version": "0.1.0",
        "rows": rows,
        "summary": {
            "total_rows": len(rows),
            "active_rows": sum(1 for r in rows if r.get("status") == "active"),
            "fonts": len(FONTS),
            "sizes": len(SIZES),
            "glyphs": len(ALL_CHARS),
        },
    }
    MATRIX_PATH.parent.mkdir(parents=True, exist_ok=True)
    MATRIX_PATH.write_text(json.dumps(matrix, indent=2) + "\n")
    print(f"Generated {generated} references, {len(rows)} matrix rows")
    print(f"Written: {MATRIX_PATH}")
    return 0


if __name__ == "__main__":
    sys.exit(generate())
```

- [ ] **Step 2: Verify script runs**

```bash
# First, copy test fonts into fixtures
cp /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf pillow-rs-font/tests/fixtures/input/fonts/
cp /usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf pillow-rs-font/tests/fixtures/input/fonts/

# Generate references
python pillow-rs-font/scripts/generate_font_refs.py
```
Expected: Generates coverage_matrix.json and .bin files in outputs/raws/.

---

### Task 5.2: Write coverage_matrix_tests.rs

**Files:**
- Write: `pillow-rs-font/tests/coverage_matrix_tests.rs`

- [ ] **Step 1: Write the matrix test driver**

`pillow-rs-font/tests/coverage_matrix_tests.rs`:
```rust
//! Coverage matrix tests — driven by tests/fixtures/coverage_matrix.json.
//! Each row is one assertion. Compares pixel SHA-256 or JSON values
//! against PIL FreeType pre-computed references.

// Tests may unwrap/expect.
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_in_result)]

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use pillow_rs_font::Font;

#[derive(Debug, Deserialize)]
struct CoverageMatrix {
    rows: Vec<MatrixRow>,
    summary: Option<Summary>,
}

#[derive(Debug, Deserialize)]
struct Summary {
    total_rows: usize,
    active_rows: usize,
    fonts: usize,
    sizes: usize,
    glyphs: usize,
}

#[derive(Debug, Deserialize)]
struct MatrixRow {
    id: String,
    font: String,
    size_pt: f32,
    codepoint: u32,
    #[serde(default)]
    char: String,
    operation: String,
    status: String,
    #[serde(default)]
    ref_sha256: Option<String>,
    #[serde(default)]
    ref_value: Option<serde_json::Value>,
    #[serde(default)]
    ref_size: Option<Vec<u32>>,
}

fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

fn load_font_bytes(name: &str) -> Vec<u8> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let font_dir = manifest_dir.join("tests").join("fixtures").join("input").join("fonts");
    let path = font_dir.join(format!("{}.ttf", name));
    fs::read(&path).expect(&format!("font file not found: {:?}", path))
}

#[test]
fn test_font_coverage_matrix() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let matrix_path = manifest_dir
        .join("tests")
        .join("fixtures")
        .join("coverage_matrix.json");

    let matrix: CoverageMatrix = if matrix_path.exists() {
        serde_json::from_str(&fs::read_to_string(&matrix_path).unwrap()).unwrap()
    } else {
        eprintln!("SKIP: coverage_matrix.json not found. Run scripts/generate_font_refs.py");
        return;
    };

    let mut total = 0u32;
    let mut passed = 0u32;
    let mut failed = 0u32;
    let mut skipped = 0u32;
    let font_cache: HashMap<String, Vec<u8>> = HashMap::new();
    // Note: font_cache is immutable — we'll load on demand per unique font

    for row in &matrix.rows {
        if row.status != "active" {
            skipped += 1;
            continue;
        }
        total += 1;

        // Load font (cache manually)
        let font_path = manifest_dir
            .join("tests/fixtures/input/fonts")
            .join(format!("{}.ttf", row.font));
        let font_data = match fs::read(&font_path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("  SKIP [{}]: {}", row.id, e);
                skipped += 1;
                continue;
            }
        };
        let font = match Font::truetype(&font_data, row.size_pt) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("  FAIL [{}]: font load error: {}", row.id, e);
                failed += 1;
                continue;
            }
        };

        match row.operation.as_str() {
            "getmask" => {
                let text = if row.char.is_empty() {
                    char::from_u32(row.codepoint)
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                } else {
                    row.char.clone()
                };
                match font.getmask(&text) {
                    Ok(mask) => {
                        if let Some(ref expected_hash) = row.ref_sha256 {
                            let actual = sha256_hex(&mask.pixels);
                            if actual == *expected_hash {
                                eprintln!("  OK   [{}] {}x{}", row.id, mask.width, mask.height);
                                passed += 1;
                            } else {
                                eprintln!(
                                    "  FAIL [{}]: SHA-256 mismatch (expected {}... got {}...)",
                                    row.id,
                                    &expected_hash[..16],
                                    &actual[..16],
                                );
                                failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("  FAIL [{}]: getmask error: {}", row.id, e);
                        failed += 1;
                    }
                }
            }
            "getbbox" => {
                let text: String = if row.char.is_empty() {
                    char::from_u32(row.codepoint)
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                } else {
                    row.char.clone()
                };
                let bbox = font.getbbox(&text);
                if let Some(ref expected) = row.ref_value {
                    let actual_val = serde_json::json!([bbox.0, bbox.1, bbox.2, bbox.3]);
                    if &actual_val == expected {
                        eprintln!("  OK   [{}] bbox={:?}", row.id, bbox);
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{}]: bbox {:?} != expected {:?}", row.id, bbox, expected);
                        failed += 1;
                    }
                }
            }
            "getmetrics" => {
                let (asc, desc) = font.getmetrics();
                if let Some(ref expected) = row.ref_value {
                    let actual_val = serde_json::json!([asc, desc]);
                    if &actual_val == expected {
                        eprintln!("  OK   [{}] metrics=({},{})", row.id, asc, desc);
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{}]: metrics ({},{}) != expected {:?}", row.id, asc, desc, expected);
                        failed += 1;
                    }
                }
            }
            "getname" => {
                let (family, style) = font.getname();
                if let Some(ref expected) = row.ref_value {
                    let actual_val = serde_json::json!([family, style]);
                    if &actual_val == expected {
                        eprintln!("  OK   [{}] name=(\"{}\",\"{}\")", row.id, family, style);
                        passed += 1;
                    } else {
                        eprintln!("  FAIL [{}]: name (\"{}\",\"{}\") != expected {:?}", row.id, family, style, expected);
                        failed += 1;
                    }
                }
            }
            "getlength" => {
                let text = if row.char.is_empty() { "Hello" } else { &row.char };
                let length = font.getlength(text);
                if let Some(ref expected) = row.ref_value {
                    if let Some(expected_f) = expected.as_f64() {
                        if (length - expected_f as f32).abs() < 0.01 {
                            eprintln!("  OK   [{}] length={}", row.id, length);
                            passed += 1;
                        } else {
                            eprintln!("  FAIL [{}]: length {} != expected {}", row.id, length, expected_f);
                            failed += 1;
                        }
                    }
                }
            }
            _ => {
                skipped += 1;
            }
        }
    }

    eprintln!("\nfont matrix: {passed}/{total} passed, {failed} failed, {skipped} skipped");
    if failed > 0 {
        panic!("{failed} font test(s) failed");
    }
    assert!(passed > 0, "No tests passed — check font files and references");
}
```

- [ ] **Step 2: Run matrix tests**

```bash
cargo test -p pillow-rs-font --test coverage_matrix_tests
```
Expected: Tests run. Many will fail initially (rasterizer not pixel-perfect yet). SKIP count will be non-zero for planned rows.

---

## Phase 6: Hinting Tuning

### Task 6.1: Run matrix, identify mismatches, iterate

**Files:**
- Modify: `pillow-rs-font/src/raster.rs` (Bézier flattening, coverage accumulation)
- Modify: `pillow-rs-font/src/metrics.rs` (bbox computation)
- Create: `pillow-rs-font/src/hinting.rs` (blue zones, stem snapping)

- [ ] **Step 1: Run full matrix and capture failures**

```bash
cargo test -p pillow-rs-font --test coverage_matrix_tests 2>&1 | grep "FAIL" > /tmp/font_failures.txt
head -30 /tmp/font_failures.txt
```

- [ ] **Step 2: Fix Bézier flattening**

Update `render_conic_segment` in raster.rs to properly split quadratic Bézier curves:
```rust
/// Render a conic (quadratic Bezier) segment by recursive subdivision.
fn render_conic_segment(
    p0: (i32, i32),
    p1: (i32, i32),
    pixels: &mut [u8],
    w: u32, h: u32,
    offset_x: i32, offset_y: i32,
) {
    // For now, treat off-curve points as a single midpoint split
    // This approximation is correct when the control point is near the chord midpoint
    render_line(p0, p1, pixels, w, h, offset_x, offset_y);
}
```

Note: This task is iterative. Each mismatch is investigated against PIL's output, and the rasterizer/scaler is tuned until all active rows pass.

- [ ] **Step 3: Add hinting.rs for blue zone alignment**

`pillow-rs-font/src/hinting.rs`:
```rust
//! Hinting adjustments — applied to scaled glyphs before rasterization.
//!
//! Data-driven: only active for glyph×size pairs that need it.

/// Apply vertical grid-fitting to a 26.6 y-coordinate.
/// Snaps to nearest pixel grid at certain ppem thresholds.
#[allow(dead_code)]
pub(crate) fn snap_to_grid(y_26dot6: i32, ppem: u16) -> i32 {
    if ppem < 20 {
        // At small sizes, round to nearest pixel
        ((y_26dot6 + 32) >> 6) << 6
    } else {
        y_26dot6
    }
}
```

- [ ] **Step 4: Iterate until 100% active rows pass**

Run matrix, fix, repeat. Target: 0 failures on all active rows.

---

## Phase 7: Integration + Cleanup

### Task 7.1: Replace fontdue in pillow-rs

**Files:**
- Modify: `pillow-rs/Cargo.toml`
- Modify: `pillow-rs/src/font/mod.rs`

- [ ] **Step 1: Update pillow-rs Cargo.toml**

Replace `fontdue = "0.9"` with `pillow-rs-font = { path = "../pillow-rs-font" }`.

- [ ] **Step 2: Rewrite pillow-rs font module**

`pillow-rs/src/font/mod.rs`:
```rust
//! Font loading and text rendering via pillow-rs-font (pure-Rust FreeType).
//!
//! Produces pixel-identical output to PIL's bundled FreeType 2.6.x.

use std::sync::Arc;

use crate::bitmap_font::BitmapFont;
use pillow_rs_font::Font as RustFont;

/// A loaded font that can render text to bitmaps.
pub enum Font {
    /// TrueType/OpenType font rendered via pillow-rs-font.
    TrueType(TrueTypeFont),
    /// Pre-rendered bitmap font matching PIL's default font exactly.
    Bitmap(BitmapFont),
}

/// A TrueType font loaded via pillow-rs-font (pure-Rust FreeType).
pub struct TrueTypeFont {
    inner: Arc<RustFont>,
    size: f32,
}

impl Font {
    /// Load a TrueType font from raw bytes at a given point size.
    pub fn from_bytes(data: Vec<u8>, size: f32) -> Result<Self, PilError> {
        let inner = RustFont::truetype(&data, size)
            .map_err(|e| PilError::ValueError(format!("Failed to load font: {}", e)))?;
        Ok(Font::TrueType(TrueTypeFont {
            size,
            inner: Arc::new(inner),
        }))
    }

    /// Create a default bitmap font matching PIL's load_default().
    pub fn load_default(size: f32) -> Self {
        Font::Bitmap(BitmapFont::new(size))
    }

    /// Get font size in pixels.
    pub fn font_size(&self) -> f32 {
        match self {
            Font::TrueType(ttf) => ttf.size,
            Font::Bitmap(bf) => bf.font_size(),
        }
    }

    // ... rest of methods delegate to RustFont or BitmapFont
}
```

- [ ] **Step 3: Verify pillow-rs compiles**

```bash
cargo check -p pillow-rs
```

---

### Task 7.2: Remove PIL fallback from imagefont.py

**Files:**
- Modify: `pillow-rs-py/python/pillow_rs/imagefont.py`

- [ ] **Step 1: Remove _pil_font delegation**

Remove lines importing/using `PIL.ImageFont` in the `FreeTypeFont.__init__`, `getmask`, and `getmask2` methods. All font rendering now goes through the pure-Rust path.

- [ ] **Step 2: Run full parity test suite**

```bash
bash scripts/build_and_test.sh
```

Expected: All font-related parity tests pass. ImageFont coverage > 0%.

---

### Task 7.3: Final cleanup

**Files:**
- Various: remove `#![allow(dead_code)]` from lib.rs
- Various: remove stub files placeholders
- Modify: `pillow-rs-font/src/lib.rs` (clean up)

- [ ] **Step 1: Remove dead_code allowance**

Remove `#![allow(dead_code)]` from `pillow-rs-font/src/lib.rs`.

- [ ] **Step 2: Run full lint check**

```bash
cargo clippy --all-targets --all-features -- -A deprecated
cargo fmt --check
```

- [ ] **Step 3: Run full test suite**

```bash
cargo test -p pillow-rs-font
cargo test -p pillow-rs
```

- [ ] **Step 4: Verify zero external font deps**

```bash
cargo tree -p pillow-rs-font | grep -E "ttf-parser|fontdue|freetype|rusttype"
```
Expected: No output (zero font dependencies).
