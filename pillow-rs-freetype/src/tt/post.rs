//! 'post' table scalar metadata.
//!
//! Reference: `TT_Postscript` in FreeType's public TrueType table structs.

use super::read_i16;

/// Parsed 'post' table fields used by face metadata.
#[derive(Debug, Clone)]
pub struct PostTable {
    /// PostScript table format in 16.16 fixed-point form.
    pub format_type: u32,
    /// Underline position in font units.
    pub underline_position: i16,
    /// Underline thickness in font units.
    pub underline_thickness: i16,
    /// Non-zero if the face reports fixed-pitch advances.
    pub is_fixed_pitch: u32,
    /// Glyph-name indices for `post` formats 2.0 and 2.5.
    glyph_indices: Vec<u16>,
    /// Custom Pascal strings for `post` format 2.0 names above the Mac set.
    custom_names: Vec<String>,
}

/// Parse the 'post' table header fields used by `FT_FaceRec`.
pub fn parse_post(data: &[u8]) -> Option<PostTable> {
    if data.len() < 16 {
        return None;
    }

    let format_type = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let (glyph_indices, custom_names) = parse_glyph_names(format_type, data);

    Some(PostTable {
        format_type,
        underline_position: read_i16(data, 8),
        underline_thickness: read_i16(data, 10),
        is_fixed_pitch: u32::from_be_bytes([data[12], data[13], data[14], data[15]]),
        glyph_indices,
        custom_names,
    })
}

impl PostTable {
    /// Resolve a glyph's PostScript name using FreeType's `ttpost.c` rules.
    pub fn glyph_name(&self, glyph_index: usize, num_glyphs: u16) -> Option<&str> {
        // `tt_face_get_ps_name` repeats this service-level guard.  Public
        // `FT_Get_Glyph_Name` rejects the index first, while
        // `FT_Get_Name_Index` only scans valid glyph IDs.
        if glyph_index >= usize::from(num_glyphs) {
            return None;
        }
        if self.format_type == 0x0001_0000 {
            return if num_glyphs == 258 {
                mac_post_name(glyph_index).or(Some(".notdef"))
            } else {
                Some(".notdef")
            };
        }

        // `Font::glyph_name` applies FreeType's face-flag gate before service
        // dispatch.  The service itself initializes `.notdef`, then formats
        // 2.0 and 2.5 replace it when their name arrays contain this glyph.
        self.glyph_indices
            .get(glyph_index)
            .and_then(|index| {
                if *index < 258 {
                    mac_post_name(usize::from(*index))
                } else {
                    self.custom_names
                        .get(usize::from(*index - 258))
                        .map(String::as_str)
                        .or(Some(".notdef"))
                }
            })
            .or(Some(".notdef"))
    }
}

fn parse_glyph_names(format_type: u32, data: &[u8]) -> (Vec<u16>, Vec<String>) {
    match format_type {
        0x0002_0000 => parse_format_20_names(data).unwrap_or_default(),
        0x0002_5000 => (
            parse_format_25_indices(data).unwrap_or_default(),
            Vec::new(),
        ),
        _ => (Vec::new(), Vec::new()),
    }
}

fn parse_format_20_names(data: &[u8]) -> Option<(Vec<u16>, Vec<String>)> {
    if data.len() < 34 {
        return None;
    }
    let num_glyphs = usize::from(u16::from_be_bytes([data[32], data[33]]));
    if num_glyphs == 0 {
        return None;
    }
    let indices_end = 34usize.checked_add(num_glyphs.checked_mul(2)?)?;
    let indices = data.get(34..indices_end)?;
    let mut glyph_indices = Vec::with_capacity(num_glyphs);
    let mut max_index = 0u16;
    for chunk in indices.chunks_exact(2) {
        let index = u16::from_be_bytes([chunk[0], chunk[1]]);
        max_index = max_index.max(index);
        glyph_indices.push(index);
    }

    let custom_count = max_index.saturating_sub(257);
    let mut custom_names = Vec::with_capacity(usize::from(custom_count));
    let mut cursor = indices_end;
    for _ in 0..custom_count {
        let Some(&len) = data.get(cursor) else {
            custom_names.push(".notdef".to_string());
            continue;
        };
        cursor = cursor.saturating_add(1);
        let len = usize::from(len);
        let end = cursor.saturating_add(len).min(data.len());
        custom_names.push(String::from_utf8_lossy(&data[cursor..end]).into_owned());
        cursor = cursor.saturating_add(len).min(data.len());
    }

    Some((glyph_indices, custom_names))
}

fn parse_format_25_indices(data: &[u8]) -> Option<Vec<u16>> {
    if data.len() < 34 {
        return None;
    }
    let num_glyphs = usize::from(u16::from_be_bytes([data[32], data[33]]));
    if num_glyphs == 0 || num_glyphs > 258 + 128 {
        return None;
    }
    let deltas = data.get(34..34usize.checked_add(num_glyphs)?)?;
    let mut glyph_indices = Vec::with_capacity(num_glyphs);
    for (glyph_index, delta) in deltas.iter().copied().enumerate() {
        let index = glyph_index as i32 + i32::from(delta as i8);
        glyph_indices.push(if (0..=257).contains(&index) {
            index as u16
        } else {
            0
        });
    }
    Some(glyph_indices)
}

const MAC_POST_NAMES: [&str; 258] = [
    ".notdef",
    ".null",
    "nonmarkingreturn",
    "space",
    "exclam",
    "quotedbl",
    "numbersign",
    "dollar",
    "percent",
    "ampersand",
    "quotesingle",
    "parenleft",
    "parenright",
    "asterisk",
    "plus",
    "comma",
    "hyphen",
    "period",
    "slash",
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "colon",
    "semicolon",
    "less",
    "equal",
    "greater",
    "question",
    "at",
    "A",
    "B",
    "C",
    "D",
    "E",
    "F",
    "G",
    "H",
    "I",
    "J",
    "K",
    "L",
    "M",
    "N",
    "O",
    "P",
    "Q",
    "R",
    "S",
    "T",
    "U",
    "V",
    "W",
    "X",
    "Y",
    "Z",
    "bracketleft",
    "backslash",
    "bracketright",
    "asciicircum",
    "underscore",
    "grave",
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
    "g",
    "h",
    "i",
    "j",
    "k",
    "l",
    "m",
    "n",
    "o",
    "p",
    "q",
    "r",
    "s",
    "t",
    "u",
    "v",
    "w",
    "x",
    "y",
    "z",
    "braceleft",
    "bar",
    "braceright",
    "asciitilde",
    "Adieresis",
    "Aring",
    "Ccedilla",
    "Eacute",
    "Ntilde",
    "Odieresis",
    "Udieresis",
    "aacute",
    "agrave",
    "acircumflex",
    "adieresis",
    "atilde",
    "aring",
    "ccedilla",
    "eacute",
    "egrave",
    "ecircumflex",
    "edieresis",
    "iacute",
    "igrave",
    "icircumflex",
    "idieresis",
    "ntilde",
    "oacute",
    "ograve",
    "ocircumflex",
    "odieresis",
    "otilde",
    "uacute",
    "ugrave",
    "ucircumflex",
    "udieresis",
    "dagger",
    "degree",
    "cent",
    "sterling",
    "section",
    "bullet",
    "paragraph",
    "germandbls",
    "registered",
    "copyright",
    "trademark",
    "acute",
    "dieresis",
    "notequal",
    "AE",
    "Oslash",
    "infinity",
    "plusminus",
    "lessequal",
    "greaterequal",
    "yen",
    "mu",
    "partialdiff",
    "summation",
    "product",
    "pi",
    "integral",
    "ordfeminine",
    "ordmasculine",
    "Omega",
    "ae",
    "oslash",
    "questiondown",
    "exclamdown",
    "logicalnot",
    "radical",
    "florin",
    "approxequal",
    "Delta",
    "guillemotleft",
    "guillemotright",
    "ellipsis",
    "nonbreakingspace",
    "Agrave",
    "Atilde",
    "Otilde",
    "OE",
    "oe",
    "endash",
    "emdash",
    "quotedblleft",
    "quotedblright",
    "quoteleft",
    "quoteright",
    "divide",
    "lozenge",
    "ydieresis",
    "Ydieresis",
    "fraction",
    "currency",
    "guilsinglleft",
    "guilsinglright",
    "fi",
    "fl",
    "daggerdbl",
    "periodcentered",
    "quotesinglbase",
    "quotedblbase",
    "perthousand",
    "Acircumflex",
    "Ecircumflex",
    "Aacute",
    "Edieresis",
    "Egrave",
    "Iacute",
    "Icircumflex",
    "Idieresis",
    "Igrave",
    "Oacute",
    "Ocircumflex",
    "apple",
    "Ograve",
    "Uacute",
    "Ucircumflex",
    "Ugrave",
    "dotlessi",
    "circumflex",
    "tilde",
    "macron",
    "breve",
    "dotaccent",
    "ring",
    "cedilla",
    "hungarumlaut",
    "ogonek",
    "caron",
    "Lslash",
    "lslash",
    "Scaron",
    "scaron",
    "Zcaron",
    "zcaron",
    "brokenbar",
    "Eth",
    "eth",
    "Yacute",
    "yacute",
    "Thorn",
    "thorn",
    "minus",
    "multiply",
    "onesuperior",
    "twosuperior",
    "threesuperior",
    "onehalf",
    "onequarter",
    "threequarters",
    "franc",
    "Gbreve",
    "gbreve",
    "Idotaccent",
    "Scedilla",
    "scedilla",
    "Cacute",
    "cacute",
    "Ccaron",
    "ccaron",
    "dcroat",
];

fn mac_post_name(index: usize) -> Option<&'static str> {
    MAC_POST_NAMES.get(index).copied()
}
