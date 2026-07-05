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
        Ok(Face { font })
    }
}

/// A loaded font face.
#[derive(Clone)]
pub struct Face {
    font: Font,
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
        let metrics = if flags.contains(LoadFlags::NO_HINTING) {
            self.font.glyph_metrics_for_index_no_hinting(glyph_index)?
        } else if flags.contains(LoadFlags::FORCE_AUTOHINT) {
            self.font
                .glyph_metrics_for_index_force_autohint(glyph_index)?
        } else {
            self.font.glyph_metrics_for_index_default(glyph_index)?
        };

        let bitmap = if flags.contains(LoadFlags::RENDER) {
            let render_font = self.render_font(flags)?;
            Some(render_font.render_char_mode_for_index(glyph_index, flags.render_mode())?)
        } else {
            None
        };

        Ok(GlyphSlot::new(glyph_index, metrics, bitmap))
    }

    fn render_font(&self, flags: LoadFlags) -> Result<Font, FontError> {
        if flags.contains(LoadFlags::NO_HINTING) {
            return Err(FontError::UnsupportedLoadFlags(
                "NO_HINTING | RENDER".to_string(),
            ));
        }
        let load_mode = if flags.contains(LoadFlags::FORCE_AUTOHINT) {
            LoadMode::ForceAutoHint
        } else {
            LoadMode::Default
        };
        Font::truetype_face_with_load_mode(
            &self.font.data.raw_data,
            self.font.face_index(),
            self.font.size_pt,
            load_mode,
        )
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
    fn new(glyph_index: u16, metrics: GlyphSlotMetrics, bitmap: Option<RenderedBitmap>) -> Self {
        let format = if bitmap.is_some() {
            GlyphFormat::Bitmap
        } else if metrics.width == 0 && metrics.height == 0 {
            GlyphFormat::None
        } else {
            GlyphFormat::Outline
        };
        let (bitmap_left, bitmap_top) = bitmap
            .as_ref()
            .map_or((0, 0), |bitmap| (bitmap.left, bitmap.top));

        Self {
            glyph_index,
            metrics,
            advance: Vector {
                x: metrics.hori_advance,
                y: 0,
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
