//! Safe Rust façade shaped after FreeType's common face/glyph-slot API.
//!
//! This module intentionally mirrors the concepts exposed by binding crates
//! such as Servo's `rust-freetype` without mirroring raw pointers or runtime
//! FFI.  A [`Face`] owns the parsed pure-Rust font, and [`GlyphSlot`] is an
//! immutable snapshot of the last loaded glyph data a C caller would inspect
//! through `FT_FaceRec::glyph`.

use crate::error::FontError;
use crate::font::{FaceInfo, Font, GlyphSlotMetrics, LoadMode, SizeMetrics};
use crate::render::{PixelMode, RenderMode, RenderedBitmap};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

/// FreeType-style 2D vector in 26.6 pixel units unless documented otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Vector {
    /// Horizontal coordinate.
    pub x: i32,
    /// Vertical coordinate.
    pub y: i32,
}

/// Glyph image format loaded into a [`GlyphSlot`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphFormat {
    /// No outline or bitmap is present.
    None,
    /// Scalable outline data is present.
    Outline,
    /// The glyph was rendered to a bitmap.
    Bitmap,
}

/// Bitflag-style glyph loading options aligned with common `FT_LOAD_*` flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LoadFlags(u32);

impl LoadFlags {
    /// Default native load behavior.
    pub const DEFAULT: Self = Self(0);
    /// Render the loaded glyph into [`GlyphSlot::bitmap`].
    pub const RENDER: Self = Self(1 << 0);
    /// Force the auto-hinter instead of native TrueType bytecode.
    pub const FORCE_AUTOHINT: Self = Self(1 << 1);
    /// Disable hinting for metrics and outline placement.
    pub const NO_HINTING: Self = Self(1 << 2);
    /// Render with monochrome coverage when [`Self::RENDER`] is also set.
    pub const TARGET_MONO: Self = Self(1 << 3);
    /// Render with horizontal LCD coverage when [`Self::RENDER`] is also set.
    pub const TARGET_LCD: Self = Self(1 << 4);
    /// Render with vertical LCD coverage when [`Self::RENDER`] is also set.
    pub const TARGET_LCD_V: Self = Self(1 << 5);
    /// Disable FreeType's auto-hinter while still allowing native hints.
    pub const NO_AUTOHINT: Self = Self(1 << 6);
    /// Load scalable glyphs in font units without scaling or rendering.
    pub const NO_SCALE: Self = Self(1 << 7);
    /// Use vertical layout advances for the loaded glyph slot.
    pub const VERTICAL_LAYOUT: Self = Self(1 << 8);

    /// Return true if all bits in `other` are set.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    fn render_mode(self) -> RenderMode {
        if self.contains(Self::TARGET_MONO) {
            RenderMode::Mono
        } else if self.contains(Self::TARGET_LCD) {
            RenderMode::Lcd
        } else if self.contains(Self::TARGET_LCD_V) {
            RenderMode::LcdV
        } else {
            RenderMode::Normal
        }
    }
}

impl std::ops::BitOr for LoadFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for LoadFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

/// A FreeType-style library handle.
///
/// The pure Rust implementation has no dynamic module registry, so this is a
/// zero-sized value used to align construction flow with `FT_Init_FreeType`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Library;

impl Library {
    /// Create a new pure-Rust library handle.
    pub fn init() -> Self {
        Self
    }

    /// Open a font face from memory, equivalent to `FT_New_Memory_Face`.
    pub fn new_memory_face(
        self,
        data: &[u8],
        face_index: usize,
        size_pt: f32,
    ) -> Result<Face, FontError> {
        let font = Font::truetype_face(data, face_index, size_pt)?;
        Ok(Face {
            font,
            render_fonts: RenderFontCache::default(),
        })
    }
}

/// A loaded font face.
#[derive(Clone)]
pub struct Face {
    font: Font,
    render_fonts: RenderFontCache,
}

#[derive(Clone, Default)]
struct RenderFontCache {
    fonts: Rc<RefCell<BTreeMap<RenderFontKey, Font>>>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RenderFontKey {
    load_mode: RenderLoadModeKey,
    size_pt_bits: u32,
    x_ppem: u16,
    y_ppem: u16,
    x_scale: i32,
    y_scale: i32,
    ascender: i32,
    descender: i32,
    height: i32,
    max_advance: i32,
    x_dpi: u32,
    y_dpi: u32,
    char_width: i32,
    char_height: i32,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum RenderLoadModeKey {
    Default,
    ForceAutoHint,
    NoHinting,
    NoAutoHint,
}

impl RenderFontCache {
    fn get_or_insert_with(&self, key: RenderFontKey, build: impl FnOnce() -> Font) -> Font {
        if let Some(font) = self.fonts.borrow().get(&key).cloned() {
            return font;
        }
        let font = build();
        self.fonts
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| font.clone())
            .clone()
    }
}

impl RenderFontKey {
    fn new(font: &Font, load_mode: LoadMode) -> Self {
        let metrics = font.size_metrics();
        Self {
            load_mode: RenderLoadModeKey::from(load_mode),
            size_pt_bits: font.size_pt.to_bits(),
            x_ppem: metrics.x_ppem,
            y_ppem: metrics.y_ppem,
            x_scale: metrics.x_scale,
            y_scale: metrics.y_scale,
            ascender: metrics.ascender,
            descender: metrics.descender,
            height: metrics.height,
            max_advance: metrics.max_advance,
            x_dpi: metrics.x_dpi,
            y_dpi: metrics.y_dpi,
            char_width: metrics.char_width,
            char_height: metrics.char_height,
        }
    }
}

impl From<LoadMode> for RenderLoadModeKey {
    fn from(load_mode: LoadMode) -> Self {
        match load_mode {
            LoadMode::Default => Self::Default,
            LoadMode::ForceAutoHint => Self::ForceAutoHint,
            LoadMode::NoHinting => Self::NoHinting,
            LoadMode::NoAutoHint => Self::NoAutoHint,
        }
    }
}

impl Face {
    /// Open a font face directly from bytes.
    pub fn from_memory(data: &[u8], face_index: usize, size_pt: f32) -> Result<Self, FontError> {
        Library::init().new_memory_face(data, face_index, size_pt)
    }

    /// Return the underlying high-level font object.
    pub fn font(&self) -> &Font {
        &self.font
    }

    /// Return scalar face metadata.
    pub fn info(&self) -> FaceInfo {
        self.font.face_info()
    }

    /// Return active size metrics.
    pub fn size_metrics(&self) -> SizeMetrics {
        self.font.size_metrics()
    }

    /// Set the active character size, equivalent to `FT_Set_Char_Size`.
    pub fn set_char_size(&mut self, char_width: i32, char_height: i32, x_dpi: u32, y_dpi: u32) {
        self.font
            .set_char_size(char_width, char_height, x_dpi, y_dpi);
    }

    /// Set the active pixel size, equivalent to `FT_Set_Pixel_Sizes`.
    pub fn set_pixel_sizes(&mut self, pixel_width: u32, pixel_height: u32) {
        self.font.set_pixel_sizes(pixel_width, pixel_height);
    }

    /// Return the glyph index for a Unicode scalar value.
    pub fn get_char_index(&self, char_code: u32) -> u16 {
        self.font.char_index(char_code)
    }

    /// Load a Unicode scalar value, equivalent to `FT_Load_Char`.
    pub fn load_char(&self, char_code: u32, flags: LoadFlags) -> Result<GlyphSlot, FontError> {
        self.load_glyph(self.get_char_index(char_code), flags)
    }

    /// Load a glyph index, equivalent to `FT_Load_Glyph`.
    pub fn load_glyph(&self, glyph_index: u16, flags: LoadFlags) -> Result<GlyphSlot, FontError> {
        let vertical_layout = flags.contains(LoadFlags::VERTICAL_LAYOUT);
        let metrics = if flags.contains(LoadFlags::NO_SCALE) {
            self.font.glyph_metrics_for_index_no_scale(glyph_index)?
        } else if flags.contains(LoadFlags::NO_HINTING) {
            self.font.glyph_metrics_for_index_no_hinting(glyph_index)?
        } else if flags.contains(LoadFlags::FORCE_AUTOHINT)
            && !flags.contains(LoadFlags::NO_AUTOHINT)
        {
            self.font
                .glyph_metrics_for_index_force_autohint_with_layout(glyph_index, vertical_layout)?
        } else if flags.contains(LoadFlags::NO_AUTOHINT) {
            self.font
                .glyph_metrics_for_index_no_autohint_with_layout(glyph_index, vertical_layout)?
        } else {
            self.font
                .glyph_metrics_for_index_default_with_layout(glyph_index, vertical_layout)?
        };

        let render_requested = flags.contains(LoadFlags::RENDER);
        let bitmap = if render_requested {
            let render_font = self.render_font(flags)?;
            let bitmap =
                render_font.render_char_mode_for_index(glyph_index, flags.render_mode())?;
            if bitmap.buffer.is_empty() {
                None
            } else {
                Some(bitmap)
            }
        } else {
            None
        };

        Ok(GlyphSlot::new(
            glyph_index,
            metrics,
            bitmap,
            vertical_layout,
            render_requested,
        ))
    }

    fn render_font(&self, flags: LoadFlags) -> Result<Font, FontError> {
        let load_mode = if flags.contains(LoadFlags::NO_HINTING) {
            LoadMode::NoHinting
        } else if flags.contains(LoadFlags::FORCE_AUTOHINT) {
            if flags.contains(LoadFlags::NO_AUTOHINT) {
                LoadMode::NoAutoHint
            } else {
                LoadMode::ForceAutoHint
            }
        } else if flags.contains(LoadFlags::NO_AUTOHINT) {
            LoadMode::NoAutoHint
        } else {
            LoadMode::Default
        };
        let key = RenderFontKey::new(&self.font, load_mode);
        Ok(self
            .render_fonts
            .get_or_insert_with(key, || self.font.clone_with_load_mode(load_mode)))
    }
}

/// Snapshot of the loaded glyph-slot fields callers normally read from
/// `FT_GlyphSlotRec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphSlot {
    /// Glyph index loaded into the slot.
    pub glyph_index: u16,
    /// Grid-fit glyph metrics in 26.6 pixels.
    pub metrics: GlyphSlotMetrics,
    /// Slot advance vector in 26.6 pixels.
    pub advance: Vector,
    /// Glyph image format currently present in the slot.
    pub format: GlyphFormat,
    /// Rendered bitmap if `LoadFlags::RENDER` was requested.
    pub bitmap: Option<RenderedBitmap>,
    /// Bitmap left bearing in pixels.
    pub bitmap_left: i32,
    /// Bitmap top bearing in pixels.
    pub bitmap_top: i32,
}

impl GlyphSlot {
    fn new(
        glyph_index: u16,
        metrics: GlyphSlotMetrics,
        bitmap: Option<RenderedBitmap>,
        vertical_layout: bool,
        rendered: bool,
    ) -> Self {
        let format = if rendered {
            GlyphFormat::Bitmap
        } else {
            GlyphFormat::Outline
        };
        let (bitmap_left, bitmap_top) = bitmap
            .as_ref()
            .map_or((0, 0), |bitmap| (bitmap.left, bitmap.top));

        Self {
            glyph_index,
            metrics,
            advance: if vertical_layout {
                Vector {
                    x: 0,
                    y: metrics.vert_advance,
                }
            } else {
                Vector {
                    x: metrics.hori_advance,
                    y: 0,
                }
            },
            format,
            bitmap,
            bitmap_left,
            bitmap_top,
        }
    }

    /// Return the rendered bitmap's pixel mode, if a bitmap is present.
    pub fn pixel_mode(&self) -> Option<PixelMode> {
        self.bitmap.as_ref().map(|bitmap| bitmap.pixel_mode)
    }
}
