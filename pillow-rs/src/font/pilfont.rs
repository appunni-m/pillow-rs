//! Pillow PILfont v1 bitmap-font parsing and rendering.
//!
//! A PILfont consists of a `.pil` metrics file and a sibling monochrome or
//! grayscale glyph image. Metrics are signed, big-endian 16-bit values for 256
//! Latin-1 glyphs. Rendering follows Pillow 12.2.0 `src/_imaging.c`
//! (`_font_new`, `textwidth`, and `_font_getmask`) exactly.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;

const GLYPH_COUNT: usize = 256;
const GLYPH_RECORD_LEN: usize = 20;
const METRICS_LEN: usize = GLYPH_COUNT * GLYPH_RECORD_LEN;
const MAX_STRING_LENGTH: usize = 1_000_000;

const DEFAULT_METRICS: &[u8] = include_bytes!("courb08.pil");
const DEFAULT_BITMAP: &[u8] = include_bytes!("courb08.png");

/// Native storage mode of a PILfont glyph image and its rendered masks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PilFontMode {
    /// One-bit black-and-white pixels.
    One,
    /// Eight-bit grayscale pixels.
    Luma,
}

/// Host-neutral text input for the legacy PILfont API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PilFontTextInput {
    /// Unicode text that must be representable in Pillow's Latin-1 font table.
    Text(String),
    /// Raw Pillow byte text.
    Bytes(Vec<u8>),
}

impl PilFontTextInput {
    fn into_bytes(self) -> Result<Vec<u8>, PilError> {
        match self {
            Self::Bytes(bytes) => Ok(bytes),
            Self::Text(text) => text
                .chars()
                .enumerate()
                .map(|(position, character)| {
                    u8::try_from(character as u32).map_err(|_| {
                        let codepoint = character as u32;
                        let escaped = if codepoint <= 0xffff {
                            format!("\\u{codepoint:04x}")
                        } else {
                            format!("\\U{codepoint:08x}")
                        };
                        PilError::UnicodeEncodeError {
                            message: format!("'latin-1' codec can't encode character '{escaped}' in position {position}: ordinal not in range(256)"),
                            encoding: "latin-1".into(),
                            object: text.clone(),
                            start: position,
                            end: position + 1,
                            reason: "ordinal not in range(256)".into(),
                        }
                    })
                })
                .collect(),
        }
    }
}

impl PilFontMode {
    /// Returns Pillow's mode name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::One => "1",
            Self::Luma => "L",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Glyph {
    dx: i32,
    dy: i32,
    dx0: i32,
    dy0: i32,
    dx1: i32,
    dy1: i32,
    sx0: i32,
    sy0: i32,
    sx1: i32,
    sy1: i32,
}

/// Parsed PILfont metrics and glyph bitmap.
#[derive(Debug, Clone)]
pub struct PilFont {
    glyphs: [Glyph; GLYPH_COUNT],
    bitmap: Vec<u8>,
    bitmap_width: u32,
    ysize: i32,
    baseline: i32,
    mode: PilFontMode,
    info: Vec<Vec<u8>>,
    render_error: Option<PilError>,
}

/// Glyph-image state used by Pillow PILfont loading.
#[derive(Debug, Clone)]
pub enum PilFontGlyphImage {
    /// Fully decoded glyph bitmap.
    Image(Image),
    /// Glyph image opened successfully, but rendering non-empty text raises.
    DeferredRenderError {
        /// Native bitmap mode.
        mode: PilFontMode,
        /// Bitmap width in pixels.
        width: u32,
        /// Bitmap height in pixels.
        height: u32,
        /// Pillow-compatible rendering error.
        error: PilError,
    },
}

/// A rendered PILfont mask with one byte per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PilFontMask {
    /// Mask width in pixels.
    pub width: u32,
    /// Mask height in pixels.
    pub height: u32,
    /// Native bitmap mode.
    pub mode: PilFontMode,
    /// Expanded pixels. Mode `"1"` uses `0` and `255`.
    pub pixels: Vec<u8>,
}

impl PilFontMask {
    /// Converts the mask to the core image representation.
    ///
    /// Mode `"1"` is packed MSB-first for [`Image::frombytes`], while
    /// [`PilFontMask::pixels`] remains expanded like Pillow's `ImagingCore`
    /// sequence.
    pub fn to_image(&self) -> Result<Image, PilError> {
        if self.width == 0 {
            return Image::new(self.width, self.height, self.mode.as_str(), (0, 0, 0, 0));
        }

        match self.mode {
            PilFontMode::Luma => Image::frombytes("L", (self.width, self.height), &self.pixels),
            PilFontMode::One => {
                let row_bytes = (self.width as usize).div_ceil(8);
                let packed_len = row_bytes
                    .checked_mul(self.height as usize)
                    .ok_or_else(|| PilError::DimensionError("PILfont mask size overflow".into()))?;
                let mut packed = vec![0u8; packed_len];
                for y in 0..self.height as usize {
                    for x in 0..self.width as usize {
                        if self.pixels[y * self.width as usize + x] != 0 {
                            packed[y * row_bytes + x / 8] |= 1 << (7 - x % 8);
                        }
                    }
                }
                Image::frombytes("1", (self.width, self.height), &packed)
            }
        }
    }
}

impl PilFont {
    /// Parses a PILfont metrics payload and its already-opened glyph image.
    ///
    /// The image must use Pillow mode `"1"` or `"L"`. The `.pil` header and
    /// 5,120-byte descriptor table are validated before the image is decoded,
    /// matching Pillow's error order.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::TypeError`] for an unsupported glyph-image mode,
    /// [`PilError::SyntaxError`] for a non-PILfont header, or
    /// [`PilError::ValueError`] for a truncated descriptor table.
    pub fn from_pilfont_data(data: &[u8], image: Image) -> Result<Self, PilError> {
        Self::from_pilfont_glyph_data(data, PilFontGlyphImage::Image(image))
    }

    /// Parses a PILfont metrics payload and Pillow-compatible glyph image state.
    pub fn from_pilfont_glyph_data(
        data: &[u8],
        glyph_image: PilFontGlyphImage,
    ) -> Result<Self, PilError> {
        let (mode, bitmap_width, bitmap_height, bitmap, render_error) = match glyph_image {
            PilFontGlyphImage::Image(image) => {
                let mode = match image.mode()?.as_str() {
                    "1" => PilFontMode::One,
                    "L" => PilFontMode::Luma,
                    _ => return Err(PilError::TypeError("invalid font image mode".into())),
                };
                let bitmap = image.materialize()?.to_luma8();
                let (bitmap_width, bitmap_height) = bitmap.dimensions();
                (mode, bitmap_width, bitmap_height, bitmap.into_raw(), None)
            }
            PilFontGlyphImage::DeferredRenderError {
                mode,
                width,
                height,
                error,
            } => {
                let dims = CheckedDims::new(width, height, 1)?;
                (mode, width, height, dims.alloc_buffer(), Some(error))
            }
        };
        let (info, metrics) = parse_metrics_file(data)?;

        let bitmap_width_i32 = i32::try_from(bitmap_width)
            .map_err(|_| PilError::DimensionError("PILfont bitmap width exceeds i32".into()))?;
        let bitmap_height_i32 = i32::try_from(bitmap_height)
            .map_err(|_| PilError::DimensionError("PILfont bitmap height exceeds i32".into()))?;

        let mut glyphs = [Glyph::default(); GLYPH_COUNT];
        let mut y0 = 0i32;
        let mut y1 = 0i32;
        for (glyph, record) in glyphs
            .iter_mut()
            .zip(metrics.chunks_exact(GLYPH_RECORD_LEN))
        {
            *glyph = Glyph {
                dx: signed_be(record, 0),
                dy: signed_be(record, 2),
                dx0: signed_be(record, 4),
                dy0: signed_be(record, 6),
                dx1: signed_be(record, 8),
                dy1: signed_be(record, 10),
                sx0: signed_be(record, 12),
                sy0: signed_be(record, 14),
                sx1: signed_be(record, 16),
                sy1: signed_be(record, 18),
            };

            // Pillow 12.2.0 src/_imaging.c::_font_new clips source rectangles
            // here and shifts the destination rectangle by the same amount.
            if glyph.sx0 < 0 {
                glyph.dx0 -= glyph.sx0;
                glyph.sx0 = 0;
            }
            if glyph.sy0 < 0 {
                glyph.dy0 -= glyph.sy0;
                glyph.sy0 = 0;
            }
            if glyph.sx1 > bitmap_width_i32 {
                glyph.dx1 -= glyph.sx1 - bitmap_width_i32;
                glyph.sx1 = bitmap_width_i32;
            }
            if glyph.sy1 > bitmap_height_i32 {
                glyph.dy1 -= glyph.sy1 - bitmap_height_i32;
                glyph.sy1 = bitmap_height_i32;
            }

            y0 = y0.min(glyph.dy0);
            y1 = y1.max(glyph.dy1);
        }

        Ok(Self {
            glyphs,
            bitmap,
            bitmap_width,
            ysize: y1 - y0,
            baseline: -y0,
            mode,
            info,
            render_error,
        })
    }

    /// Opens encoded PNG, GIF, or PBM glyph-image bytes for PILfont loading.
    ///
    /// PBM `P1` and `P4` are decoded locally because the workspace codec does
    /// not otherwise expose Netpbm. Pillow-generated `.pbm` files commonly
    /// contain PNG bytes and are detected by header before this fallback.
    pub fn open_pilfont_glyph_image(data: Vec<u8>) -> Result<PilFontGlyphImage, PilError> {
        match Image::open_bytes(data.clone()) {
            Ok(image) => Ok(PilFontGlyphImage::Image(image)),
            Err(original_error) => {
                if data.starts_with(b"P1") || data.starts_with(b"P4") {
                    decode_pbm_for_pilfont(&data)
                } else {
                    Err(original_error)
                }
            }
        }
    }

    /// Loads Pillow 12.2.0's embedded courB08 PILfont.
    ///
    /// The exact embedded payloads come from
    /// `src/PIL/ImageFont.py::load_default_imagefont`.
    ///
    /// - `.pil` bytes: 5,143; SHA-256
    ///   `5e85438582f0e790b6b115d1afd66fa439d9fc45875af5cc760af7034d67187a`
    /// - PNG bytes: 1,273; SHA-256
    ///   `afdc82adb778486c71c5cc9c6f88623b3c7e5044e80ff7b32d973663eff31ed0`
    pub fn load_default() -> Result<Self, PilError> {
        Self::from_pilfont_glyph_data(
            DEFAULT_METRICS,
            Self::open_pilfont_glyph_image(DEFAULT_BITMAP.to_vec())?,
        )
    }

    /// Returns the metadata lines between the PILfont descriptor and `DATA`.
    pub fn info(&self) -> &[Vec<u8>] {
        &self.info
    }

    /// Returns the font's native bitmap mode.
    pub fn mode(&self) -> PilFontMode {
        self.mode
    }

    /// Returns `(advance width, fixed font height)` for Latin-1 bytes.
    pub fn getsize(&self, text: &[u8]) -> Result<(i32, i32), PilError> {
        validate_text_length(text)?;
        Ok((self.textwidth(text)?, self.ysize))
    }

    /// Returns the size after applying the host text-input rules.
    pub fn getsize_input(&self, text: PilFontTextInput) -> Result<(i32, i32), PilError> {
        self.getsize(&text.into_bytes()?)
    }

    /// Returns the Pillow bitmap-font bounding box for Latin-1 bytes.
    pub fn getbbox(&self, text: &[u8]) -> Result<(i32, i32, i32, i32), PilError> {
        let (width, height) = self.getsize(text)?;
        Ok((0, 0, width, height))
    }

    /// Returns the bounding box after applying the host text-input rules.
    pub fn getbbox_input(&self, text: PilFontTextInput) -> Result<(i32, i32, i32, i32), PilError> {
        self.getbbox(&text.into_bytes()?)
    }

    /// Returns the Pillow bitmap-font horizontal advance for Latin-1 bytes.
    pub fn getlength(&self, text: &[u8]) -> Result<i32, PilError> {
        self.getsize(text).map(|(width, _)| width)
    }

    /// Returns the horizontal advance after applying the host text-input rules.
    pub fn getlength_input(&self, text: PilFontTextInput) -> Result<i32, PilError> {
        self.getlength(&text.into_bytes()?)
    }

    /// Renders Latin-1 bytes using Pillow's PILfont placement rules.
    pub fn getmask(&self, text: &[u8]) -> Result<PilFontMask, PilError> {
        validate_text_length(text)?;
        let width_i32 = self.textwidth(text)?;
        let width = u32::try_from(width_i32)
            .map_err(|_| PilError::ValueError("PILfont text width is negative".into()))?;
        let height = u32::try_from(self.ysize)
            .map_err(|_| PilError::ValueError("PILfont height is negative".into()))?;
        if width == 0 {
            return Ok(PilFontMask {
                width,
                height,
                mode: self.mode,
                pixels: Vec::new(),
            });
        }
        if height == 0 {
            return Err(PilError::SystemError(
                "<method 'getmask' of 'ImagingFont' objects> returned a result with an exception set"
                    .into(),
            ));
        }
        if let Some(error) = &self.render_error {
            return Err(error.clone());
        }

        let dims = CheckedDims::new(width, height, 1)?;
        let mut pixels = dims.alloc_buffer();
        let mut x = 0i32;
        let mut baseline = self.baseline;

        for &byte in text.iter().take_while(|&&byte| byte != 0) {
            let glyph = self.glyphs[byte as usize];
            self.paste_glyph(glyph, x, baseline, &dims, &mut pixels)?;
            x = x
                .checked_add(glyph.dx)
                .ok_or_else(|| PilError::DimensionError("PILfont pen x overflow".into()))?;
            baseline = baseline
                .checked_add(glyph.dy)
                .ok_or_else(|| PilError::DimensionError("PILfont pen y overflow".into()))?;
        }

        Ok(PilFontMask {
            width,
            height,
            mode: self.mode,
            pixels,
        })
    }

    /// Renders a mask after applying the host text-input rules.
    pub fn getmask_input(&self, text: PilFontTextInput) -> Result<PilFontMask, PilError> {
        self.getmask(&text.into_bytes()?)
    }

    fn textwidth(&self, text: &[u8]) -> Result<i32, PilError> {
        text.iter()
            .take_while(|&&byte| byte != 0)
            .try_fold(0i32, |width, &byte| {
                width
                    .checked_add(self.glyphs[byte as usize].dx)
                    .ok_or_else(|| PilError::DimensionError("PILfont text width overflow".into()))
            })
    }

    fn paste_glyph(
        &self,
        glyph: Glyph,
        pen_x: i32,
        baseline: i32,
        output: &CheckedDims,
        pixels: &mut [u8],
    ) -> Result<(), PilError> {
        let source_width = glyph.sx1 - glyph.sx0;
        let source_height = glyph.sy1 - glyph.sy0;
        if source_width < 0 || source_height < 0 {
            return Err(PilError::SystemError(
                "<method 'getmask' of 'ImagingFont' objects> returned a result with an exception set"
                    .into(),
            ));
        }
        if glyph.dx1 - glyph.dx0 != source_width || glyph.dy1 - glyph.dy0 != source_height {
            return Err(PilError::SystemError(
                "<method 'getmask' of 'ImagingFont' objects> returned a result with an exception set"
                    .into(),
            ));
        }

        let destination_x = pen_x + glyph.dx0;
        let destination_y = baseline + glyph.dy0;
        for source_y_offset in 0..source_height {
            for source_x_offset in 0..source_width {
                let x = destination_x + source_x_offset;
                let y = destination_y + source_y_offset;
                if x < 0 || x >= output.width as i32 {
                    continue;
                }

                let source_x = glyph.sx0 + source_x_offset;
                let source_y = glyph.sy0 + source_y_offset;
                // Pillow's `_font_new`-equivalent clipping above guarantees
                // these source coordinates are inside the glyph bitmap.
                let value =
                    self.bitmap[source_y as usize * self.bitmap_width as usize + source_x as usize];
                pixels[y as usize * output.row_stride() + x as usize] = value;
            }
        }
        Ok(())
    }
}

fn validate_text_length(text: &[u8]) -> Result<(), PilError> {
    if text.len() > MAX_STRING_LENGTH {
        return Err(PilError::ValueError("too many characters in string".into()));
    }
    Ok(())
}

fn parse_metrics_file(data: &[u8]) -> Result<(Vec<Vec<u8>>, &[u8]), PilError> {
    let Some(mut cursor) = data.strip_prefix(b"PILfont\n") else {
        return Err(PilError::SyntaxError("Not a PILfont file".into()));
    };

    let (_, remaining) = read_line(cursor);
    cursor = remaining;
    let mut info = Vec::new();
    loop {
        let (line, remaining) = read_line(cursor);
        cursor = remaining;
        if line.is_empty() || line == b"DATA\n" {
            break;
        }
        info.push(line.to_vec());
    }

    if cursor.len() < METRICS_LEN {
        return Err(PilError::ValueError(
            "descriptor table has wrong size".into(),
        ));
    }
    Ok((info, &cursor[..METRICS_LEN]))
}

fn read_line(data: &[u8]) -> (&[u8], &[u8]) {
    match data.iter().position(|&byte| byte == b'\n') {
        Some(index) => data.split_at(index + 1),
        None => (data, &[]),
    }
}

fn signed_be(record: &[u8], offset: usize) -> i32 {
    i16::from_be_bytes([record[offset], record[offset + 1]]) as i32
}

fn decode_pbm_for_pilfont(data: &[u8]) -> Result<PilFontGlyphImage, PilError> {
    let mut tokens = PbmTokens::new(data);
    let magic = tokens
        .next()
        .ok_or_else(|| PilError::ValueError("invalid PBM header".into()))?;
    let width = parse_pbm_dimension(tokens.next(), "width")?;
    let height = parse_pbm_dimension(tokens.next(), "height")?;
    let dims = CheckedDims::new(width, height, 1)?;
    let row_bytes = (width as usize).div_ceil(8);
    let packed_len = row_bytes
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::DimensionError("PBM raster size overflow".into()))?;
    let mut packed = vec![0u8; packed_len];

    match magic {
        b"P1" => {
            for index in 0..dims.total_pixels() {
                match tokens.next() {
                    Some(b"0") => {
                        let y = index / width as usize;
                        let x = index % width as usize;
                        packed[y * row_bytes + x / 8] |= 1 << (7 - x % 8);
                    }
                    Some(b"1") => {}
                    Some(token) => {
                        let token = String::from_utf8_lossy(token);
                        return Err(PilError::ValueError(format!(
                            "b'Invalid token for this mode: {token}'"
                        )));
                    }
                    None => {
                        return Err(PilError::ValueError("not enough image data".into()));
                    }
                }
            }
        }
        b"P4" => {
            let raster = tokens.binary_raster()?;
            if raster.bytes.len() < packed_len {
                if raster.had_crlf_separator {
                    return Ok(PilFontGlyphImage::DeferredRenderError {
                        mode: PilFontMode::One,
                        width,
                        height,
                        error: PilError::SystemError(
                            "<method 'getmask' of 'ImagingFont' objects> returned a result with an exception set"
                                .into(),
                        ),
                    });
                }
                return Err(PilError::IOError(
                    "image file is truncated (0 bytes not processed)".into(),
                ));
            }
            for (output, input) in packed.iter_mut().zip(raster.bytes) {
                // Netpbm uses 1 for black; Pillow mode "1" uses 0 for black.
                *output = !input;
            }
        }
        _ => return Err(PilError::ValueError("unsupported PBM format".into())),
    }
    Image::frombytes("1", (width, height), &packed).map(PilFontGlyphImage::Image)
}

fn parse_pbm_dimension(token: Option<&[u8]>, name: &str) -> Result<u32, PilError> {
    let token = token.ok_or_else(|| PilError::ValueError(format!("missing PBM {name}")))?;
    let value = std::str::from_utf8(token)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .ok_or_else(|| PilError::ValueError(format!("invalid PBM {name}")))?;
    if value == 0 {
        return Err(PilError::ValueError(format!("invalid PBM {name}")));
    }
    Ok(value)
}

struct PbmTokens<'a> {
    data: &'a [u8],
    position: usize,
}

struct PbmRaster<'a> {
    bytes: &'a [u8],
    had_crlf_separator: bool,
}

impl<'a> PbmTokens<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    fn next(&mut self) -> Option<&'a [u8]> {
        self.skip_spacing();
        let start = self.position;
        while self.position < self.data.len()
            && !self.data[self.position].is_ascii_whitespace()
            && self.data[self.position] != b'#'
        {
            self.position += 1;
        }
        (self.position > start).then(|| &self.data[start..self.position])
    }

    fn binary_raster(&mut self) -> Result<PbmRaster<'a>, PilError> {
        let Some(&separator) = self.data.get(self.position) else {
            return Err(PilError::IOError(
                "image file is truncated (0 bytes not processed)".into(),
            ));
        };
        if !separator.is_ascii_whitespace() {
            return Err(PilError::ValueError("invalid PBM raster separator".into()));
        }
        self.position += 1;
        let had_crlf_separator = separator == b'\r' && self.data.get(self.position) == Some(&b'\n');
        if had_crlf_separator {
            self.position += 1;
        }
        Ok(PbmRaster {
            bytes: &self.data[self.position..],
            had_crlf_separator,
        })
    }

    fn skip_spacing(&mut self) {
        loop {
            while self
                .data
                .get(self.position)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.position += 1;
            }
            if self.data.get(self.position) != Some(&b'#') {
                return;
            }
            while self.position < self.data.len() && self.data[self.position] != b'\n' {
                self.position += 1;
            }
        }
    }
}
