//! FreeType-style glyph render modes and bitmap helpers.
//!
//! Normal and LCD modes use the smooth rasterizer. The pinned FreeType
//! configuration keeps `FT_CONFIG_OPTION_SUBPIXEL_RENDERING` disabled, so LCD
//! and LCD_V follow the Harmony renderer geometry from `ftsmooth.c`.

use crate::casts::{i32_from_usize, u32_from_usize, usize_from_i32, usize_from_i64};
use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_fix, ft_vector_length, ft_vector_norm_len};
use crate::font::Font;
use crate::grays;
use crate::outline::{
    OUTLINE_HIGH_PRECISION, OUTLINE_IGNORE_DROPOUTS, OUTLINE_INCLUDE_STUBS, OUTLINE_OVERLAP,
    OUTLINE_SINGLE_PASS, OUTLINE_SMART_DROPOUTS, Outline,
};
use crate::scaler;
use crate::tt::hinter::NativeHintMode;

const LCD_SUBPIXELS: [i32; 3] = [-21, 0, 21];
const FT_PIXEL_ONE: i32 = 64;
const SDF_SPREAD: i32 = 8;
const FT_INT_16D16_ONE: i32 = 1 << 16;

#[derive(Debug, Clone, Copy)]
struct MonoPrecision {
    bits: i32,
    precision: i32,
    half: i32,
    scale: i32,
    step: i32,
}

impl MonoPrecision {
    fn for_outline(outline: &Outline, width: usize, height: usize) -> Self {
        let high_precision = outline.flags & OUTLINE_HIGH_PRECISION != 0
            && width.saturating_sub(1) + height.saturating_sub(1) < 256;
        if high_precision {
            Self::high()
        } else {
            Self::low()
        }
    }

    fn low() -> Self {
        Self::new(6, 32)
    }

    fn high() -> Self {
        Self::new(12, 256)
    }

    fn new(bits: i32, step: i32) -> Self {
        let precision = 1 << bits;
        Self {
            bits,
            precision,
            half: precision >> 1,
            scale: precision >> 6,
            step,
        }
    }

    fn floor(self, value: i32) -> i32 {
        value & -self.precision
    }

    fn ceiling(self, value: i32) -> i32 {
        (value + self.precision - 1) & -self.precision
    }

    fn trunc(self, value: i32) -> i32 {
        value >> self.bits
    }

    fn scaled_coord(self, value: i32) -> i32 {
        (i64::from(value) * i64::from(self.scale) - i64::from(self.half)) as i32
    }

    fn pixel_ceil(self, value: i32) -> i32 {
        self.trunc(self.ceiling(value))
    }

    fn pixel_floor(self, value: i32) -> i32 {
        self.trunc(self.floor(value))
    }

    fn is_grid_aligned(self, value: i32) -> bool {
        value & (self.precision - 1) == 0
    }

    fn is_bottom_overshoot(self, value: i32) -> bool {
        self.ceiling(value) - value >= self.half
    }

    fn is_top_overshoot(self, value: i32) -> bool {
        value - self.floor(value) >= self.half
    }

    fn smart_dropout(self, x1: i32, x2: i32) -> i32 {
        let midpoint = (i64::from(x1) + i64::from(x2) + i64::from(self.precision * 63 / 64)) >> 1;
        self.floor(midpoint as i32)
    }
}

#[derive(Debug, Clone, Copy)]
struct MonoLineRange {
    min_y: i32,
    max_y: i32,
}

impl MonoLineRange {
    fn new(min_y: i32, max_y: i32) -> Self {
        Self { min_y, max_y }
    }
}

#[derive(Debug, Clone, Copy)]
struct MonoHorizontalSpanEdge {
    x: i32,
    y1: i32,
    y2: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderMode {
    Normal,
    Mono,
    Lcd,
    LcdV,
    Sdf,
}

impl RenderMode {
    pub fn fixture_name(self) -> &'static str {
        match self {
            RenderMode::Normal => "normal",
            RenderMode::Mono => "mono",
            RenderMode::Lcd => "lcd",
            RenderMode::LcdV => "lcd_v",
            RenderMode::Sdf => "sdf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelMode {
    Gray,
    Mono,
    Gray2,
    Gray4,
    Lcd,
    LcdV,
    Bgra,
}

impl PixelMode {
    pub fn num_grays(self) -> u16 {
        match self {
            PixelMode::Gray | PixelMode::Lcd | PixelMode::LcdV | PixelMode::Bgra => 256,
            PixelMode::Gray2 => 4,
            PixelMode::Gray4 => 16,
            // FreeType leaves `FT_Bitmap::num_grays` at 256 for rendered
            // monochrome slots even though the buffer is 1-bit packed.
            PixelMode::Mono => 256,
        }
    }

    pub fn fixture_name(self) -> &'static str {
        match self {
            PixelMode::Gray => "gray",
            PixelMode::Mono => "mono",
            PixelMode::Gray2 => "gray2",
            PixelMode::Gray4 => "gray4",
            PixelMode::Lcd => "lcd",
            PixelMode::LcdV => "lcd_v",
            PixelMode::Bgra => "bgra",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedBitmap {
    pub width: u32,
    pub rows: u32,
    pub pitch: i32,
    pub pixel_mode: PixelMode,
    pub num_grays: u16,
    pub left: i32,
    pub top: i32,
    pub buffer: Vec<u8>,
}

impl Font {
    /// Render the first character of `text` using a FreeType render mode.
    pub fn render_mode(&self, text: &str, mode: RenderMode) -> Result<RenderedBitmap, FontError> {
        let Some(ch) = text.chars().next() else {
            return Ok(RenderedBitmap {
                width: 0,
                rows: 0,
                pitch: 0,
                pixel_mode: PixelMode::Gray,
                num_grays: PixelMode::Gray.num_grays(),
                left: 0,
                top: 0,
                buffer: Vec::new(),
            });
        };
        self.render_char_mode(ch, mode)
    }

    pub fn render_char_mode(
        &self,
        ch: char,
        mode: RenderMode,
    ) -> Result<RenderedBitmap, FontError> {
        let glyph = self.data.cmap.char_index(ch as u32).unwrap_or(0);
        self.render_char_mode_for_index(glyph, mode)
    }

    pub(crate) fn render_char_mode_for_index(
        &self,
        glyph: u16,
        mode: RenderMode,
    ) -> Result<RenderedBitmap, FontError> {
        let native_hint_mode = match mode {
            RenderMode::Normal => NativeHintMode::Normal,
            RenderMode::Mono => NativeHintMode::Mono,
            RenderMode::Lcd => NativeHintMode::Lcd,
            RenderMode::LcdV => NativeHintMode::LcdV,
            RenderMode::Sdf => NativeHintMode::Normal,
        };
        self.render_char_mode_for_index_with_native_hint_mode(glyph, mode, native_hint_mode)
    }

    pub(crate) fn render_char_mode_for_index_with_native_hint_mode(
        &self,
        glyph: u16,
        mode: RenderMode,
        native_hint_mode: NativeHintMode,
    ) -> Result<RenderedBitmap, FontError> {
        let scaled = self.scale_glyph_for_load_mode_with_native_mode(glyph, native_hint_mode)?;

        if scaled.outline.n_contours == 0 && mode == RenderMode::Lcd {
            return render_empty_loaded_outline(mode);
        }

        if scaled.outline.n_contours == 0 {
            if mode == RenderMode::Mono {
                return Ok(RenderedBitmap {
                    width: 1,
                    rows: 1,
                    pitch: 2,
                    pixel_mode: PixelMode::Mono,
                    num_grays: PixelMode::Mono.num_grays(),
                    left: 0,
                    top: 1,
                    buffer: vec![0, 0],
                });
            }
            let pixel_mode = match mode {
                RenderMode::Mono => PixelMode::Mono,
                RenderMode::Lcd => PixelMode::Lcd,
                RenderMode::LcdV => PixelMode::LcdV,
                RenderMode::Sdf => PixelMode::Gray,
                RenderMode::Normal => PixelMode::Gray,
            };
            return Ok(RenderedBitmap {
                width: 0,
                rows: 0,
                pitch: 0,
                pixel_mode,
                num_grays: pixel_mode.num_grays(),
                left: 0,
                top: 0,
                buffer: Vec::new(),
            });
        }

        match mode {
            RenderMode::Normal => {
                render_scaled_normal(scaled, &mut self.raster_scratch.borrow_mut())
            }
            RenderMode::Mono => render_scaled_mono(scaled),
            RenderMode::Lcd => render_scaled_lcd(scaled),
            RenderMode::LcdV => render_scaled_lcd_v(scaled),
            RenderMode::Sdf => render_scaled_sdf(scaled),
        }
    }

    pub(crate) fn render_loaded_char_mode_for_index(
        &self,
        glyph: u16,
        mode: RenderMode,
    ) -> Result<RenderedBitmap, FontError> {
        let scaled = self.scale_glyph_for_load_mode(glyph)?;
        render_loaded_outline(
            scaled.outline,
            scaled.bbox_x_min,
            scaled.bbox_y_min,
            scaled.bbox_y_max,
            mode,
            &mut self.raster_scratch.borrow_mut(),
        )
    }
}

pub(crate) fn render_loaded_outline(
    outline: Outline,
    left: i32,
    bottom: i32,
    top: i32,
    mode: RenderMode,
    scratch: &mut crate::grays::RasterScratch,
) -> Result<RenderedBitmap, FontError> {
    // C `ft_smooth_render` presets normal-mode empty outlines, then exits on
    // zero rows/pitch (`ftsmooth.c:595-603`); keep non-normal modes on their
    // established empty-slot path because they expose different bitmap shapes.
    if outline.is_empty() && mode != RenderMode::Normal {
        return render_empty_loaded_outline(mode);
    }

    match mode {
        RenderMode::Normal => render_normal(outline, left, top, scratch),
        RenderMode::Mono => render_mono(outline, left, bottom),
        RenderMode::Lcd => render_lcd(outline, left, top),
        RenderMode::LcdV => render_lcd_v(outline, left, top),
        RenderMode::Sdf => render_sdf(outline, left, top, SDF_SPREAD),
    }
}

fn render_empty_loaded_outline(mode: RenderMode) -> Result<RenderedBitmap, FontError> {
    if mode == RenderMode::Mono {
        return Ok(RenderedBitmap {
            width: 1,
            rows: 1,
            pitch: 2,
            pixel_mode: PixelMode::Mono,
            num_grays: PixelMode::Mono.num_grays(),
            left: 0,
            top: 1,
            buffer: vec![0, 0],
        });
    }
    let pixel_mode = match mode {
        RenderMode::Mono => PixelMode::Mono,
        RenderMode::Lcd => PixelMode::Lcd,
        RenderMode::LcdV => PixelMode::LcdV,
        RenderMode::Sdf => PixelMode::Gray,
        RenderMode::Normal => PixelMode::Gray,
    };
    Ok(RenderedBitmap {
        width: 0,
        rows: 0,
        pitch: 0,
        pixel_mode,
        num_grays: pixel_mode.num_grays(),
        left: 0,
        top: 0,
        buffer: Vec::new(),
    })
}

fn render_scaled_normal(
    scaled: scaler::ScaledGlyph,
    scratch: &mut crate::grays::RasterScratch,
) -> Result<RenderedBitmap, FontError> {
    render_normal(
        scaled.outline,
        scaled.bbox_x_min,
        scaled.bbox_y_max,
        scratch,
    )
}

fn render_scaled_mono(scaled: scaler::ScaledGlyph) -> Result<RenderedBitmap, FontError> {
    render_mono(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_min)
}

fn render_scaled_lcd(scaled: scaler::ScaledGlyph) -> Result<RenderedBitmap, FontError> {
    render_lcd(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_max)
}

fn render_scaled_lcd_v(scaled: scaler::ScaledGlyph) -> Result<RenderedBitmap, FontError> {
    render_lcd_v(scaled.outline, scaled.bbox_x_min, scaled.bbox_y_max)
}

fn render_scaled_sdf(scaled: scaler::ScaledGlyph) -> Result<RenderedBitmap, FontError> {
    render_sdf(
        scaled.outline,
        scaled.bbox_x_min,
        scaled.bbox_y_max,
        SDF_SPREAD,
    )
}

fn render_normal(
    outline: Outline,
    left: i32,
    top: i32,
    scratch: &mut crate::grays::RasterScratch,
) -> Result<RenderedBitmap, FontError> {
    if outline.points.is_empty() || outline.n_contours == 0 {
        return Ok(RenderedBitmap {
            width: 0,
            rows: 0,
            pitch: 0,
            pixel_mode: PixelMode::Gray,
            num_grays: PixelMode::Gray.num_grays(),
            left,
            top,
            buffer: Vec::new(),
        });
    }
    let width = usize_from_i32(outline.cbox_x_max - outline.cbox_x_min);
    let height = usize_from_i32(outline.cbox_y_max - outline.cbox_y_min);
    if width == 0 || height == 0 {
        return Ok(RenderedBitmap {
            width: 0,
            rows: 0,
            pitch: 0,
            pixel_mode: PixelMode::Gray,
            num_grays: PixelMode::Gray.num_grays(),
            left,
            top,
            buffer: Vec::new(),
        });
    }
    if outline.flags & OUTLINE_OVERLAP != 0 {
        return render_normal_overlap(outline, left, top, width, height, scratch);
    }

    let mut target = vec![0u8; width * height];
    crate::grays::rasterize_shifted_in_box_to_with_scratch(
        &outline,
        0,
        0,
        width,
        height,
        &mut target,
        width,
        1,
        0,
        outline.cbox_x_min,
        outline.cbox_x_max,
        outline.cbox_y_min,
        outline.cbox_y_max,
        scratch,
    )?;
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(height),
        pitch: i32_from_usize(width),
        pixel_mode: PixelMode::Gray,
        num_grays: PixelMode::Gray.num_grays(),
        left,
        top,
        buffer: target,
    })
}

fn render_normal_overlap(
    mut outline: Outline,
    left: i32,
    top: i32,
    width: usize,
    height: usize,
    scratch: &mut crate::grays::RasterScratch,
) -> Result<RenderedBitmap, FontError> {
    const SCALE: i32 = 4;
    const SCALE_USIZE: usize = SCALE as usize;

    if width > 0x7FFF / SCALE_USIZE {
        return Err(FontError::RasterOverflow);
    }

    let oversampled_width = width
        .checked_mul(SCALE_USIZE)
        .ok_or(FontError::RasterOverflow)?;
    let oversampled_height = height
        .checked_mul(SCALE_USIZE)
        .ok_or(FontError::RasterOverflow)?;
    let oversampled_len = oversampled_width
        .checked_mul(oversampled_height)
        .ok_or(FontError::RasterOverflow)?;

    for point in &mut outline.points {
        point.x = point
            .x
            .checked_mul(SCALE)
            .ok_or(FontError::RasterOverflow)?;
        point.y = point
            .y
            .checked_mul(SCALE)
            .ok_or(FontError::RasterOverflow)?;
    }
    let cbox_x_min = outline
        .cbox_x_min
        .checked_mul(SCALE)
        .ok_or(FontError::RasterOverflow)?;
    let cbox_x_max = outline
        .cbox_x_max
        .checked_mul(SCALE)
        .ok_or(FontError::RasterOverflow)?;
    let cbox_y_min = outline
        .cbox_y_min
        .checked_mul(SCALE)
        .ok_or(FontError::RasterOverflow)?;
    let cbox_y_max = outline
        .cbox_y_max
        .checked_mul(SCALE)
        .ok_or(FontError::RasterOverflow)?;

    let mut oversampled = vec![0u8; oversampled_len];
    crate::grays::rasterize_shifted_in_box_to_with_scratch(
        &outline,
        0,
        0,
        oversampled_width,
        oversampled_height,
        &mut oversampled,
        oversampled_width,
        1,
        0,
        cbox_x_min,
        cbox_x_max,
        cbox_y_min,
        cbox_y_max,
        scratch,
    )?;

    let mut target = vec![0u8; width * height];
    downsample_overlap_bitmap(&oversampled, oversampled_width, &mut target, width);
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(height),
        pitch: i32_from_usize(width),
        pixel_mode: PixelMode::Gray,
        num_grays: PixelMode::Gray.num_grays(),
        left,
        top,
        buffer: target,
    })
}

fn downsample_overlap_bitmap(
    oversampled: &[u8],
    oversampled_width: usize,
    target: &mut [u8],
    width: usize,
) {
    const SCALE_USIZE: usize = 4;
    const SCALE_AREA: u16 = (SCALE_USIZE * SCALE_USIZE) as u16;
    const HALF_SCALE_AREA: u16 = SCALE_AREA / 2;

    for (sample_y, row) in oversampled.chunks(oversampled_width).enumerate() {
        let target_y = sample_y / SCALE_USIZE;
        let target_row = target_y * width;
        for (sample_x, &sample) in row.iter().enumerate() {
            let target_index = target_row + sample_x / SCALE_USIZE;
            let cover = (u16::from(sample) + HALF_SCALE_AREA) / SCALE_AREA;
            let sum = u16::from(target[target_index]) + cover;
            // ftsmooth.c:ft_smooth_overlap_spans accumulates 4x4 subpixel
            // coverage and maps the only possible overflow value, 256, to 255.
            target[target_index] = (sum - (sum >> 8)) as u8;
        }
    }
}

fn render_sdf(
    mut outline: Outline,
    left: i32,
    top: i32,
    spread: i32,
) -> Result<RenderedBitmap, FontError> {
    let base_width = outline.cbox_x_max - outline.cbox_x_min;
    let base_rows = outline.cbox_y_max - outline.cbox_y_min;
    // FreeType's SDF renderer first presets a normal bitmap via
    // `ft_glyphslot_preset_bitmap`, then exits before padding if that base
    // bitmap has zero rows or pitch (`ftobjs.c:374-539`, `ftsdfrend.c:295-305`).
    if base_width <= 0 || base_rows <= 0 {
        return Ok(RenderedBitmap {
            width: 0,
            rows: 0,
            pitch: 0,
            pixel_mode: PixelMode::Gray,
            num_grays: 255,
            left: 0,
            top: 0,
            buffer: Vec::new(),
        });
    }
    let width = base_width + spread * 2;
    let rows = base_rows + spread * 2;

    // C: `ft_sdf_render` pads the normal preset bitmap by the renderer spread,
    // then translates the outline by `-bitmap_left, rows - bitmap_top`
    // before `sdf_generate_subdivision` (`src/sdf/ftsdfrend.c:295-349`).
    translate_outline(&mut outline, spread * FT_PIXEL_ONE, spread * FT_PIXEL_ONE);
    let buffer = rasterize_sdf_outline(
        &outline,
        usize_from_i32(width),
        usize_from_i32(rows),
        spread,
    )?;
    Ok(RenderedBitmap {
        width: u32_from_usize(usize_from_i32(width)),
        rows: u32_from_usize(usize_from_i32(rows)),
        pitch: width,
        pixel_mode: PixelMode::Gray,
        num_grays: 255,
        left: left - spread,
        top: top + spread,
        buffer,
    })
}

fn render_mono(
    mut outline: Outline,
    bbox_x_min: i32,
    bbox_y_min: i32,
) -> Result<RenderedBitmap, FontError> {
    let mono_box = mono_preset_box(&outline, bbox_x_min, bbox_y_min);
    let width = usize_from_i32(mono_box.x_max - mono_box.x_min);
    let height = usize_from_i32(mono_box.y_max - mono_box.y_min);
    translate_outline(
        &mut outline,
        (bbox_x_min - mono_box.x_min) * 64,
        (bbox_y_min - mono_box.y_min) * 64,
    );
    let buffer = rasterize_mono_center(&outline, width, height)?;
    let pitch = mono_pitch(width);
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(height),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::Mono,
        num_grays: PixelMode::Mono.num_grays(),
        left: mono_box.x_min,
        top: mono_box.y_max,
        buffer,
    })
}

#[derive(Debug, Clone, Copy)]
struct PixelBox {
    x_min: i32,
    y_min: i32,
    x_max: i32,
    y_max: i32,
}

fn mono_preset_box(outline: &Outline, bbox_x_min: i32, bbox_y_min: i32) -> PixelBox {
    let mut x_min = outline.points[0].x;
    let mut y_min = outline.points[0].y;
    let mut x_max = outline.points[0].x;
    let mut y_max = outline.points[0].y;
    for point in &outline.points {
        x_min = x_min.min(point.x);
        y_min = y_min.min(point.y);
        x_max = x_max.max(point.x);
        y_max = y_max.max(point.y);
    }

    PixelBox {
        x_min: mono_round_min(bbox_x_min, x_min),
        y_min: mono_round_min(bbox_y_min, y_min),
        x_max: mono_round_max(bbox_x_min, x_max),
        y_max: mono_round_max(bbox_y_min, y_max),
    }
    .with_non_collapsed(x_min, y_min, x_max, y_max)
}

impl PixelBox {
    fn with_non_collapsed(mut self, x_min: i32, y_min: i32, x_max: i32, y_max: i32) -> Self {
        if self.x_min == self.x_max {
            if mono_collapse_bias(x_min, x_max) < 0 {
                self.x_min -= 1;
            } else {
                self.x_max += 1;
            }
        }
        if self.y_min == self.y_max {
            if mono_collapse_bias(y_min, y_max) < 0 {
                self.y_min -= 1;
            } else {
                self.y_max += 1;
            }
        }
        self
    }
}

fn mono_round_min(base: i32, value: i32) -> i32 {
    base + ((value + 31) >> 6)
}

fn mono_round_max(base: i32, value: i32) -> i32 {
    base + ((value + 32) >> 6)
}

fn mono_collapse_bias(min: i32, max: i32) -> i32 {
    (((min + 31) & 63) - 31) + (((max + 32) & 63) - 32)
}

#[derive(Debug, Clone, Copy)]
struct Segment {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

fn rasterize_mono_center(
    outline: &Outline,
    width: usize,
    height: usize,
) -> Result<Vec<u8>, FontError> {
    let pitch = mono_pitch(width);
    let mut buffer = vec![0u8; pitch * height];
    let precision = MonoPrecision::for_outline(outline, width, height);
    rasterize_mono_profiles(outline, &mut buffer, width, height, pitch, precision)?;
    if outline.flags & OUTLINE_SINGLE_PASS == 0 {
        rasterize_mono_horizontal_profiles(outline, &mut buffer, width, height, pitch, precision)?;
    }
    Ok(buffer)
}

fn dropout_control_from_outline_flags(flags: u32) -> u8 {
    let mut control = 0;
    if flags & OUTLINE_IGNORE_DROPOUTS != 0 {
        control |= 2;
    }
    if flags & OUTLINE_SMART_DROPOUTS != 0 {
        control |= 4;
    }
    if flags & OUTLINE_INCLUDE_STUBS == 0 {
        control |= 1;
    }
    control
}

const MONO_FLOW_UP: u8 = 0x08;
const MONO_OVERSHOOT_TOP: u8 = 0x10;
const MONO_OVERSHOOT_BOTTOM: u8 = 0x20;
const MONO_DROPOUT: u8 = 0x40;
const MONO_DROPOUT_CONTROL: u8 = 1;

#[derive(Debug, Clone)]
struct MonoProfile {
    xs: Vec<i32>,
    start: i32,
    offset: usize,
    height: usize,
    flags: u8,
    x: i32,
    link: Option<usize>,
    next: Option<usize>,
    contour: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MonoState {
    Unknown,
    Ascending,
    Descending,
}

fn rasterize_mono_profiles(
    outline: &Outline,
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    precision: MonoPrecision,
) -> Result<(), FontError> {
    if width == 0 || height == 0 || outline.is_empty() {
        return Ok(());
    }

    let profiles = MonoOutlineProfileBuilder::new(
        0,
        i32_from_usize(height - 1) * precision.precision,
        false,
        precision,
        dropout_control_from_outline_flags(outline.flags),
        &outline.contour_dropouts,
    )
    .build(outline)?;
    if profiles.is_empty() {
        return Ok(());
    }

    draw_mono_profile_sweep(profiles, buffer, width, height, pitch, precision);
    Ok(())
}

fn rasterize_mono_horizontal_profiles(
    outline: &Outline,
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    precision: MonoPrecision,
) -> Result<(), FontError> {
    if width == 0 || height == 0 || outline.is_empty() {
        return Ok(());
    }

    let profiles = MonoOutlineProfileBuilder::new(
        0,
        i32_from_usize(width - 1) * precision.precision,
        true,
        precision,
        dropout_control_from_outline_flags(outline.flags),
        &outline.contour_dropouts,
    )
    .build(outline)?;
    if profiles.is_empty() {
        return Ok(());
    }

    draw_mono_horizontal_profile_sweep(profiles, buffer, width, height, pitch, precision);
    Ok(())
}

struct MonoOutlineProfileBuilder {
    profiles: Vec<MonoProfile>,
    current: Option<usize>,
    contour_first: Option<usize>,
    contour_profiles: Vec<usize>,
    state: MonoState,
    last_x: i32,
    last_y: i32,
    min_y: i32,
    max_y: i32,
    flipped: bool,
    precision: MonoPrecision,
    dropout_control: u8,
    contour_dropouts: Vec<u8>,
}

impl MonoOutlineProfileBuilder {
    fn new(
        min_y: i32,
        max_y: i32,
        flipped: bool,
        precision: MonoPrecision,
        dropout_control: u8,
        contour_dropouts: &[u8],
    ) -> Self {
        Self {
            profiles: Vec::new(),
            current: None,
            contour_first: None,
            contour_profiles: Vec::new(),
            state: MonoState::Unknown,
            last_x: 0,
            last_y: 0,
            min_y,
            max_y,
            flipped,
            precision,
            dropout_control,
            contour_dropouts: contour_dropouts.to_vec(),
        }
    }

    fn build(mut self, outline: &Outline) -> Result<Vec<MonoProfile>, FontError> {
        self.decompose(
            &outline.points,
            &outline.tags,
            &outline.contours,
            outline.n_contours,
        )?;
        Ok(self.profiles)
    }

    fn move_to(&mut self, point: crate::outline::OutlinePoint) {
        let point = self.transform(point);
        self.move_to_scaled(point);
    }

    fn move_to_scaled(&mut self, point: Point) {
        self.last_x = point.x;
        self.last_y = point.y;
    }

    fn line_to_point(&mut self, point: crate::outline::OutlinePoint, contour: usize) {
        let point = self.transform(point);
        self.line_to_scaled(point, contour);
    }

    fn line_to_scaled(&mut self, point: Point, contour: usize) {
        self.line_to(point.x, point.y, contour);
    }

    fn conic_to_point(
        &mut self,
        control: crate::outline::OutlinePoint,
        point: crate::outline::OutlinePoint,
        contour: usize,
    ) {
        let control = self.transform(control);
        let point = self.transform(point);
        self.conic_to_scaled(control, point, contour);
    }

    fn conic_to_scaled(&mut self, control: Point, point: Point, contour: usize) {
        self.conic_to(control.x, control.y, point.x, point.y, contour);
    }

    fn cubic_to_point(
        &mut self,
        control1: crate::outline::OutlinePoint,
        control2: crate::outline::OutlinePoint,
        point: crate::outline::OutlinePoint,
        contour: usize,
    ) {
        let control1 = self.transform(control1);
        let control2 = self.transform(control2);
        let point = self.transform(point);
        self.cubic_to(control1, control2, point, contour);
    }

    fn transform(&self, point: crate::outline::OutlinePoint) -> Point {
        if self.flipped {
            Point {
                x: self.precision.scaled_coord(point.y),
                y: self.precision.scaled_coord(point.x),
            }
        } else {
            Point {
                x: self.precision.scaled_coord(point.x),
                y: self.precision.scaled_coord(point.y),
            }
        }
    }

    fn line_to(&mut self, x: i32, y: i32, contour: usize) {
        if y == self.last_y {
            self.last_x = x;
            self.last_y = y;
            return;
        }

        let state = if self.last_y < y {
            MonoState::Ascending
        } else {
            MonoState::Descending
        };
        self.ensure_profile_state(state, contour);

        let last_x = self.last_x;
        let last_y = self.last_y;
        let min_y = self.min_y;
        let max_y = self.max_y;
        let precision = self.precision;
        let range = MonoLineRange::new(min_y, max_y);
        if state == MonoState::Ascending {
            self.push_profile_xs_from(|xs| {
                line_up_into_precision(xs, last_x, last_y, x, y, range, precision);
            });
        } else {
            self.push_profile_xs_from(|xs| {
                line_down_into_precision(xs, last_x, last_y, x, y, range, precision);
            });
        }

        self.last_x = x;
        self.last_y = y;
    }

    fn conic_to(&mut self, cx: i32, cy: i32, x: i32, y: i32, contour: usize) {
        let mut stack = vec![[
            Point { x, y },
            Point { x: cx, y: cy },
            Point {
                x: self.last_x,
                y: self.last_y,
            },
        ]];

        while let Some(arc) = stack.pop() {
            let y1 = arc[2].y;
            let y2 = arc[1].y;
            let y3 = arc[0].y;
            let x3 = arc[0].x;
            let ymin = y1.min(y3);
            let ymax = y1.max(y3);

            if y2 < self.precision.floor(ymin) || y2 > self.precision.ceiling(ymax) {
                let (first, second) = split_conic_arc(arc);
                stack.push(second);
                stack.push(first);
                continue;
            }

            if y1 != y3 {
                let state = if y1 < y3 {
                    MonoState::Ascending
                } else {
                    MonoState::Descending
                };
                self.ensure_profile_state(state, contour);
                let min_y = self.min_y;
                let max_y = self.max_y;
                let precision = self.precision;
                if state == MonoState::Ascending {
                    self.push_profile_xs_from(|xs| {
                        bezier_up_2_into_precision(xs, arc, min_y, max_y, precision);
                    });
                } else {
                    self.push_profile_xs_from(|xs| {
                        bezier_down_2_into_precision(xs, arc, min_y, max_y, precision);
                    });
                }
            }

            self.last_x = x3;
            self.last_y = y3;
        }
    }

    fn cubic_to(&mut self, control1: Point, control2: Point, point: Point, contour: usize) {
        let mut stack = vec![[
            point,
            control2,
            control1,
            Point {
                x: self.last_x,
                y: self.last_y,
            },
        ]];

        while let Some(arc) = stack.pop() {
            let y1 = arc[3].y;
            let y2 = arc[2].y;
            let y3 = arc[1].y;
            let y4 = arc[0].y;
            let x4 = arc[0].x;
            let ymin1 = y1.min(y4);
            let ymax1 = y1.max(y4);
            let ymin2 = y2.min(y3);
            let ymax2 = y2.max(y3);

            if (ymin2 - self.precision.floor(ymin1)).min(self.precision.ceiling(ymax1) - ymax2) < 0
            {
                let (first, second) = split_cubic_arc(arc);
                stack.push(second);
                stack.push(first);
                continue;
            }

            if y1 != y4 {
                let state = if y1 < y4 {
                    MonoState::Ascending
                } else {
                    MonoState::Descending
                };
                self.ensure_profile_state(state, contour);
                let min_y = self.min_y;
                let max_y = self.max_y;
                let precision = self.precision;
                if state == MonoState::Ascending {
                    self.push_profile_xs_from(|xs| {
                        bezier_up_3_into_precision(xs, arc, min_y, max_y, precision);
                    });
                } else {
                    self.push_profile_xs_from(|xs| {
                        bezier_down_3_into_precision(xs, arc, min_y, max_y, precision);
                    });
                }
            }

            self.last_x = x4;
            self.last_y = y4;
        }
    }

    fn ensure_profile_state(&mut self, state: MonoState, contour: usize) {
        if self.state != state {
            if self.state != MonoState::Unknown {
                self.end_profile();
            }
            self.new_profile(state, contour);
        }
    }

    fn new_profile(&mut self, state: MonoState, contour: usize) {
        let mut flags = self
            .contour_dropouts
            .get(contour)
            .copied()
            .unwrap_or(self.dropout_control);
        let e = match state {
            MonoState::Ascending => {
                flags |= MONO_FLOW_UP;
                if self.precision.is_bottom_overshoot(self.last_y) {
                    flags |= MONO_OVERSHOOT_BOTTOM;
                }
                self.precision.ceiling(self.last_y)
            }
            MonoState::Descending => {
                if self.precision.is_top_overshoot(self.last_y) {
                    flags |= MONO_OVERSHOOT_TOP;
                }
                self.precision.floor(self.last_y)
            }
            MonoState::Unknown => unreachable!(),
        }
        .clamp(self.min_y, self.max_y);

        let mut xs = Vec::new();
        if self.last_y == e {
            xs.push(self.last_x);
        }
        let index = self.profiles.len();
        self.profiles.push(MonoProfile {
            xs,
            start: self.precision.trunc(e),
            offset: 0,
            height: 0,
            flags,
            x: self.last_x,
            link: None,
            next: self.contour_first,
            contour,
        });
        if self.contour_first.is_none() {
            self.contour_first = Some(index);
        }
        self.current = Some(index);
        self.state = state;
        self.contour_profiles.push(index);
    }

    fn end_profile(&mut self) {
        let Some(index) = self.current else {
            return;
        };
        let height = self.profiles[index].xs.len();
        if height == 0 {
            return;
        }

        if self.profiles[index].flags & MONO_FLOW_UP != 0 {
            if self.precision.is_top_overshoot(self.last_y) {
                self.profiles[index].flags |= MONO_OVERSHOOT_TOP;
            }
            self.profiles[index].offset = 0;
            self.profiles[index].x = self.profiles[index].xs[0];
        } else {
            if self.precision.is_bottom_overshoot(self.last_y) {
                self.profiles[index].flags |= MONO_OVERSHOOT_BOTTOM;
            }
            let top = self.profiles[index].start + 1;
            self.profiles[index].start = top - i32_from_usize(height);
            self.profiles[index].offset = height - 1;
            self.profiles[index].x = self.profiles[index].xs[height - 1];
        }
        self.profiles[index].height = height;
    }

    fn link_contour_profiles(&mut self) {
        self.contour_profiles
            .retain(|&profile| self.profiles[profile].height > 0);
        let len = self.contour_profiles.len();
        if len == 0 {
            return;
        }
        for idx in 0..len {
            let profile = self.contour_profiles[idx];
            let next = self.contour_profiles[(idx + 1) % len];
            self.profiles[profile].next = Some(next);
        }
    }

    fn push_profile_xs(&mut self, xs: Vec<i32>) {
        if let Some(index) = self.current {
            self.profiles[index].xs.extend(xs);
        }
    }

    fn push_profile_xs_from(&mut self, append: impl FnOnce(&mut Vec<i32>)) {
        if let Some(index) = self.current {
            append(&mut self.profiles[index].xs);
        }
    }

    fn decompose(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        tags: &[u8],
        contours: &[i16],
        n_contours: i32,
    ) -> Result<(), FontError> {
        let mut last: i64 = -1;
        for (contour, &contour_end) in contours.iter().take(usize_from_i32(n_contours)).enumerate()
        {
            self.state = MonoState::Unknown;
            self.current = None;
            self.contour_first = None;
            self.contour_profiles.clear();

            let first = usize_from_i64(last + 1);
            last = contour_end as i64;
            if last < first as i64 {
                return Err(FontError::InvalidOutline(
                    "outline: contour end before start".into(),
                ));
            }
            let limit = usize_from_i64(last);
            let mut v_start = pts[first];
            let v_last = pts[limit];
            let mut limit_eff = limit;
            let mut v_start_scaled = self.transform(v_start);

            let first_tag = curve_tag_at(pts, tags, first);
            if first_tag == CURVE_TAG_CUBIC {
                return Err(FontError::InvalidOutline(
                    "outline: contour starts with cubic".into(),
                ));
            }
            if first_tag == CURVE_TAG_CONIC {
                if curve_tag_at(pts, tags, limit) == CURVE_TAG_ON {
                    v_start = v_last;
                    v_start_scaled = self.transform(v_start);
                    limit_eff = limit.checked_sub(1).ok_or_else(|| {
                        FontError::InvalidOutline("outline: conic start underflow".into())
                    })?;
                } else {
                    v_start_scaled =
                        midpoint_trunc(self.transform(v_start), self.transform(v_last));
                }
            }

            self.move_to_scaled(v_start_scaled);
            let start = if first_tag == CURVE_TAG_CONIC {
                if first == 0 {
                    -1
                } else {
                    i32_from_usize(first) - 1
                }
            } else {
                i32_from_usize(first)
            };
            self.walk_contour(
                pts,
                tags,
                start,
                i32_from_usize(limit_eff),
                v_start_scaled,
                contour,
            )?;
            if self.contour_first.is_some() {
                if self.precision.is_grid_aligned(self.last_y)
                    && self.last_y >= self.min_y
                    && self.last_y <= self.max_y
                {
                    if let (Some(first), Some(current)) = (self.contour_first, self.current) {
                        if same_profile_flow(&self.profiles[first], &self.profiles[current]) {
                            self.profiles[current].xs.pop();
                        }
                    }
                }
                self.end_profile();
                self.link_contour_profiles();
            }
        }
        Ok(())
    }

    fn walk_contour(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        tags: &[u8],
        mut cursor: i32,
        limit: i32,
        v_start_scaled: Point,
        contour: usize,
    ) -> Result<(), FontError> {
        while cursor < limit {
            cursor += 1;
            let idx = usize_from_i32(cursor);
            match curve_tag_at(pts, tags, idx) {
                CURVE_TAG_ON => {
                    self.line_to_point(pts[idx], contour);
                }
                CURVE_TAG_CONIC => {
                    let mut control = pts[idx];
                    loop {
                        if cursor < limit {
                            cursor += 1;
                            let next = pts[usize_from_i32(cursor)];
                            let tag = curve_tag_at(pts, tags, usize_from_i32(cursor));
                            if tag == CURVE_TAG_ON {
                                self.conic_to_point(control, next, contour);
                                break;
                            }
                            if tag != CURVE_TAG_CONIC {
                                return Err(FontError::InvalidOutline(
                                    "outline: expected conic tag".into(),
                                ));
                            }
                            let mid = midpoint_trunc(self.transform(control), self.transform(next));
                            self.conic_to_scaled(self.transform(control), mid, contour);
                            control = next;
                            continue;
                        }
                        self.conic_to_scaled(self.transform(control), v_start_scaled, contour);
                        return Ok(());
                    }
                }
                CURVE_TAG_CUBIC => {
                    if cursor + 2 > limit
                        || curve_tag_at(pts, tags, usize_from_i32(cursor + 1)) != CURVE_TAG_CUBIC
                    {
                        return Err(FontError::InvalidOutline(
                            "outline: bad cubic tag sequence".into(),
                        ));
                    }
                    let control1 = pts[idx];
                    let control2 = pts[usize_from_i32(cursor + 1)];
                    cursor += 2;
                    if cursor <= limit {
                        let point = pts[usize_from_i32(cursor)];
                        self.cubic_to_point(control1, control2, point, contour);
                    } else {
                        let control1 = self.transform(control1);
                        let control2 = self.transform(control2);
                        self.cubic_to(control1, control2, v_start_scaled, contour);
                        return Ok(());
                    }
                }
                _ => unreachable!(),
            }
        }
        self.line_to_scaled(v_start_scaled, contour);
        Ok(())
    }
}

fn draw_mono_profile_sweep(
    mut profiles: Vec<MonoProfile>,
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    precision: MonoPrecision,
) {
    let mut waiting: Vec<usize> = (0..profiles.len())
        .filter(|&index| profiles[index].height > 0)
        .collect();
    waiting.sort_by_key(|&index| profiles[index].start);
    let Some(min_y) = waiting.first().map(|&index| profiles[index].start) else {
        return;
    };
    let max_y = profiles
        .iter()
        .filter(|profile| profile.height > 0)
        .map(|profile| profile.start + i32_from_usize(profile.height) - 1)
        .max()
        .unwrap_or(min_y);

    let mut draw_left: Vec<usize> = Vec::new();
    let mut draw_right: Vec<usize> = Vec::new();
    for y in min_y..=max_y {
        let mut index = 0;
        while index < waiting.len() {
            let profile = waiting[index];
            if profiles[profile].start == y {
                let profile = waiting.remove(index);
                if profiles[profile].flags & MONO_FLOW_UP != 0 {
                    insert_profile_sorted(&mut draw_left, profile, &profiles);
                } else {
                    insert_profile_sorted(&mut draw_right, profile, &profiles);
                }
            } else {
                index += 1;
            }
        }

        if y >= 0 && y < i32_from_usize(height) {
            let row = height - 1 - usize_from_i32(y);
            let row = &mut buffer[row * pitch..(row + 1) * pitch];
            let pair_count = draw_left.len().min(draw_right.len());
            let mut dropouts = Vec::new();
            for pair in 0..pair_count {
                let left = draw_left[pair];
                let right = draw_right[pair];
                let mut x1 = profiles[left].x;
                let mut x2 = profiles[right].x;
                if x1 > x2 {
                    std::mem::swap(&mut x1, &mut x2);
                }
                if precision.ceiling(x1) <= precision.floor(x2) {
                    fill_mono_span(
                        row,
                        width,
                        precision.pixel_ceil(x1),
                        precision.pixel_floor(x2),
                    );
                } else {
                    let should =
                        should_draw_profile_dropout(&profiles, left, right, x1, x2, precision);
                    if should {
                        let drop =
                            profile_dropout_pixels(x1, x2, precision, profiles[left].flags & 7);
                        profiles[left].x = drop.primary;
                        profiles[right].x = drop.secondary;
                        profiles[left].flags |= MONO_DROPOUT;
                        dropouts.push((left, right));
                    }
                }
            }

            for (left, right) in dropouts {
                if profiles[left].flags & MONO_DROPOUT != 0 {
                    set_mono_dropout_pixels(row, width, profiles[left].x, profiles[right].x);
                    profiles[left].flags &= !MONO_DROPOUT;
                }
            }
        }

        increment_profiles(&mut draw_left, &mut profiles, 1);
        increment_profiles(&mut draw_right, &mut profiles, -1);
    }
}

fn draw_mono_horizontal_profile_sweep(
    mut profiles: Vec<MonoProfile>,
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    precision: MonoPrecision,
) {
    let mut waiting: Vec<usize> = (0..profiles.len())
        .filter(|&index| profiles[index].height > 0)
        .collect();
    waiting.sort_by_key(|&index| profiles[index].start);
    let Some(min_y) = waiting.first().map(|&index| profiles[index].start) else {
        return;
    };
    let max_y = profiles
        .iter()
        .filter(|profile| profile.height > 0)
        .map(|profile| profile.start + i32_from_usize(profile.height) - 1)
        .max()
        .unwrap_or(min_y);

    let mut draw_left: Vec<usize> = Vec::new();
    let mut draw_right: Vec<usize> = Vec::new();
    for y in min_y..=max_y {
        let mut index = 0;
        while index < waiting.len() {
            let profile = waiting[index];
            if profiles[profile].start == y {
                let profile = waiting.remove(index);
                if profiles[profile].flags & MONO_FLOW_UP != 0 {
                    insert_profile_sorted(&mut draw_left, profile, &profiles);
                } else {
                    insert_profile_sorted(&mut draw_right, profile, &profiles);
                }
            } else {
                index += 1;
            }
        }

        if y >= 0 && y < i32_from_usize(width) {
            let pair_count = draw_left.len().min(draw_right.len());
            let mut dropouts = Vec::new();
            for pair in 0..pair_count {
                let left = draw_left[pair];
                let right = draw_right[pair];
                let mut x1 = profiles[left].x;
                let mut x2 = profiles[right].x;
                if x1 > x2 {
                    std::mem::swap(&mut x1, &mut x2);
                }
                if precision.ceiling(x1) <= precision.floor(x2) {
                    set_mono_horizontal_span_edges(
                        buffer,
                        width,
                        height,
                        pitch,
                        MonoHorizontalSpanEdge {
                            x: y,
                            y1: x1,
                            y2: x2,
                        },
                        precision,
                    );
                } else {
                    let should =
                        should_draw_profile_dropout(&profiles, left, right, x1, x2, precision);
                    if should {
                        let drop =
                            profile_dropout_pixels(x1, x2, precision, profiles[left].flags & 7);
                        profiles[left].x = drop.primary;
                        profiles[right].x = drop.secondary;
                        profiles[left].flags |= MONO_DROPOUT;
                        dropouts.push((left, right));
                    }
                }
            }

            for (left, right) in dropouts {
                if profiles[left].flags & MONO_DROPOUT != 0 {
                    set_mono_horizontal_dropout_pixel(
                        buffer,
                        width,
                        height,
                        pitch,
                        y,
                        profiles[left].x,
                        profiles[right].x,
                    );
                    profiles[left].flags &= !MONO_DROPOUT;
                }
            }
        }

        increment_profiles(&mut draw_left, &mut profiles, 1);
        increment_profiles(&mut draw_right, &mut profiles, -1);
    }
}

fn set_mono_horizontal_span_edges(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    span: MonoHorizontalSpanEdge,
    precision: MonoPrecision,
) {
    if span.y1 == precision.ceiling(span.y1) {
        set_mono_horizontal_pixel(
            buffer,
            width,
            height,
            pitch,
            span.x,
            precision.trunc(span.y1),
        );
    }
    if span.y2 == precision.floor(span.y2) {
        set_mono_horizontal_pixel(
            buffer,
            width,
            height,
            pitch,
            span.x,
            precision.trunc(span.y2),
        );
    }
}

fn set_mono_horizontal_dropout_pixel(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    mut primary: i32,
    secondary: i32,
) {
    if primary < 0 || primary >= i32_from_usize(height) {
        primary = secondary;
    } else if secondary >= 0
        && secondary < i32_from_usize(height)
        && mono_horizontal_pixel_is_set(buffer, width, height, pitch, x, secondary)
    {
        return;
    }

    set_mono_horizontal_pixel(buffer, width, height, pitch, x, primary);
}

fn mono_horizontal_pixel_is_set(
    buffer: &[u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y: i32,
) -> bool {
    let Some((row, col)) = mono_pixel_offset(width, height, pitch, x, y) else {
        return false;
    };
    buffer[row + col / 8] & (0x80 >> (col & 7)) != 0
}

fn set_mono_horizontal_pixel(
    buffer: &mut [u8],
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y: i32,
) {
    let Some((row, col)) = mono_pixel_offset(width, height, pitch, x, y) else {
        return;
    };
    buffer[row + col / 8] |= 0x80 >> (col & 7);
}

fn mono_pixel_offset(
    width: usize,
    height: usize,
    pitch: usize,
    x: i32,
    y: i32,
) -> Option<(usize, usize)> {
    if x < 0 || y < 0 || x >= i32_from_usize(width) || y >= i32_from_usize(height) {
        return None;
    }
    let row = height - 1 - usize_from_i32(y);
    Some((row * pitch, usize_from_i32(x)))
}

fn line_up_into_precision(
    out: &mut Vec<i32>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    range: MonoLineRange,
    precision: MonoPrecision,
) {
    let min_y = range.min_y;
    let max_y = range.max_y;
    if (i64::from(y2) - i64::from(min_y)).min(i64::from(max_y) - i64::from(y1)) < 0 {
        return;
    }
    let e2 = if y2 > max_y {
        max_y
    } else {
        precision.floor(y2)
    };
    let mut e = if y1 < min_y {
        min_y
    } else {
        precision.ceiling(y1)
    };
    if y1 == e {
        e += precision.precision;
    }
    if e2 < e {
        return;
    }

    let mut size = precision.trunc(e2 - e) + 1;
    let dx = x2 - x1;
    let dy = y2 - y1;
    out.reserve(usize_from_i32(size));
    if dx == 0 {
        let len = out.len();
        out.resize(len + usize_from_i32(size), x1);
        return;
    }

    let ix = mul_div_trunc(e - y1, dx, dy);
    let mut x = x1 + ix;
    out.push(x);
    size -= 1;
    if size > 0 {
        let dy_long = i64::from(dy);
        let mut ax = i64::from(dx) * i64::from(e - y1) - i64::from(dy) * i64::from(ix);
        let ix = mul_div_trunc(precision.precision, dx, dy);
        let mut rx = i64::from(dx) * i64::from(precision.precision) - i64::from(dy) * i64::from(ix);
        let mut step = 1;
        if x2 < x {
            ax = -ax;
            rx = -rx;
            step = -1;
        }
        while size > 0 {
            x += ix;
            ax += rx;
            if ax >= dy_long {
                ax -= dy_long;
                x += step;
            }
            out.push(x);
            size -= 1;
        }
    }
}

fn line_down_into_precision(
    out: &mut Vec<i32>,
    x1: i32,
    y1: i32,
    x2: i32,
    y2: i32,
    range: MonoLineRange,
    precision: MonoPrecision,
) {
    line_up_into_precision(
        out,
        x1,
        -y1,
        x2,
        -y2,
        MonoLineRange::new(-range.max_y, -range.min_y),
        precision,
    );
}

fn bezier_up_2_into_precision(
    out: &mut Vec<i32>,
    arc: [Point; 3],
    min_y: i32,
    max_y: i32,
    precision: MonoPrecision,
) {
    bezier_up_into_precision(out, arc, split_conic_arc, min_y, max_y, precision);
}

fn bezier_down_2_into_precision(
    out: &mut Vec<i32>,
    arc: [Point; 3],
    min_y: i32,
    max_y: i32,
    precision: MonoPrecision,
) {
    bezier_down_into_precision(out, arc, split_conic_arc, min_y, max_y, precision);
}

fn bezier_up_3_into_precision(
    out: &mut Vec<i32>,
    arc: [Point; 4],
    min_y: i32,
    max_y: i32,
    precision: MonoPrecision,
) {
    bezier_up_into_precision(out, arc, split_cubic_arc, min_y, max_y, precision);
}

fn bezier_down_3_into_precision(
    out: &mut Vec<i32>,
    arc: [Point; 4],
    min_y: i32,
    max_y: i32,
    precision: MonoPrecision,
) {
    bezier_down_into_precision(out, arc, split_cubic_arc, min_y, max_y, precision);
}

fn bezier_down_into_precision<const N: usize>(
    out: &mut Vec<i32>,
    mut arc: [Point; N],
    splitter: BezierSplitter<N>,
    min_y: i32,
    max_y: i32,
    precision: MonoPrecision,
) {
    for point in &mut arc {
        point.y = -point.y;
    }
    bezier_up_into_precision(out, arc, splitter, -max_y, -min_y, precision);
}

fn bezier_up_into_precision<const N: usize>(
    out: &mut Vec<i32>,
    mut arc: [Point; N],
    splitter: BezierSplitter<N>,
    min_y: i32,
    max_y: i32,
    precision: MonoPrecision,
) {
    // FreeType's black rasterizer uses the same `Bezier_Up` scanline
    // accumulator for conics and cubics (`src/raster/ftraster.c:1062-1157`).
    let degree = N - 1;
    let y1 = arc[degree].y;
    let y2 = arc[0].y;
    if (i64::from(y2) - i64::from(min_y)).min(i64::from(max_y) - i64::from(y1)) < 0 {
        return;
    }

    let e2 = if y2 > max_y {
        max_y
    } else {
        precision.floor(y2)
    };
    let mut e = if y1 < min_y {
        min_y
    } else {
        precision.ceiling(y1)
    };
    if y1 == e {
        e += precision.precision;
    }
    if e2 < e {
        return;
    }

    out.reserve(usize_from_i32(precision.trunc(e2 - e) + 1));
    let mut stack = Vec::new();
    while e <= e2 {
        let end_y = arc[0].y;
        let end_x = arc[0].x;
        if end_y > e {
            let dy = end_y - arc[degree].y;
            let dx = end_x - arc[degree].x;
            if dy.max(dx.abs()) > precision.step {
                let (first, second) = splitter(arc);
                stack.push(second);
                arc = first;
                continue;
            }
            out.push(end_x - mul_div_trunc(end_y - e, dx, dy));
            e += precision.precision;
        } else if end_y == e {
            out.push(end_x);
            e += precision.precision;
        }

        let Some(next) = stack.pop() else {
            break;
        };
        arc = next;
    }
}

fn split_conic_arc(arc: [Point; 3]) -> ([Point; 3], [Point; 3]) {
    let end = arc[0];
    let control = arc[1];
    let start = arc[2];
    let end_control = midpoint(end, control);
    let start_control = midpoint(control, start);
    let center = Point {
        x: (end.x + control.x + control.x + start.x) >> 2,
        y: (end.y + control.y + control.y + start.y) >> 2,
    };
    ([center, start_control, start], [end, end_control, center])
}

fn split_cubic_arc(arc: [Point; 4]) -> ([Point; 4], [Point; 4]) {
    let end = arc[0];
    let control2 = arc[1];
    let control1 = arc[2];
    let start = arc[3];

    // Port of `Split_Cubic` from `src/raster/ftraster.c:853-904`; the
    // right shifts intentionally preserve C's fixed-point subdivision.
    let split_axis = |end: i32, control2: i32, control1: i32, start: i32| {
        let mut a = end + control2;
        let b = control2 + control1;
        let mut c = control1 + start;
        let left_control1 = c >> 1;
        c += b;
        let left_control2 = c >> 2;
        let right_control2 = a >> 1;
        a += b;
        let right_control1 = a >> 2;
        let center = (a + c) >> 3;
        (
            left_control1,
            left_control2,
            center,
            right_control1,
            right_control2,
        )
    };

    let (left_c1_x, left_c2_x, center_x, right_c1_x, right_c2_x) =
        split_axis(end.x, control2.x, control1.x, start.x);
    let (left_c1_y, left_c2_y, center_y, right_c1_y, right_c2_y) =
        split_axis(end.y, control2.y, control1.y, start.y);

    let center = Point {
        x: center_x,
        y: center_y,
    };
    (
        [
            center,
            Point {
                x: left_c2_x,
                y: left_c2_y,
            },
            Point {
                x: left_c1_x,
                y: left_c1_y,
            },
            start,
        ],
        [
            end,
            Point {
                x: right_c2_x,
                y: right_c2_y,
            },
            Point {
                x: right_c1_x,
                y: right_c1_y,
            },
            center,
        ],
    )
}

fn insert_profile_sorted(list: &mut Vec<usize>, profile: usize, profiles: &[MonoProfile]) {
    let x = profiles[profile].x;
    let pos = list
        .iter()
        .position(|&current| profiles[current].x >= x)
        .unwrap_or(list.len());
    list.insert(pos, profile);
}

fn increment_profiles(list: &mut Vec<usize>, profiles: &mut [MonoProfile], flow: isize) {
    let mut index = 0;
    while index < list.len() {
        let profile = list[index];
        profiles[profile].height -= 1;
        if profiles[profile].height == 0 {
            list.remove(index);
            continue;
        }
        if flow > 0 {
            profiles[profile].offset += 1;
        } else {
            profiles[profile].offset -= 1;
        }
        profiles[profile].x = profiles[profile].xs[profiles[profile].offset];
        index += 1;
    }
    list.sort_by_key(|&profile| profiles[profile].x);
}

fn should_draw_profile_dropout(
    profiles: &[MonoProfile],
    left: usize,
    right: usize,
    x1: i32,
    x2: i32,
    precision: MonoPrecision,
) -> bool {
    let control = profiles[left].flags & 7;
    if control & 2 != 0 {
        return false;
    }
    if control & 1 == 0 {
        return true;
    }
    if profiles[left].contour == profiles[right].contour {
        if profiles[left].height == 1
            && profiles[left].next == Some(right)
            && (profiles[left].flags & MONO_OVERSHOOT_TOP == 0 || x2 - x1 < precision.half)
        {
            return false;
        }
        if profiles[left].offset == 0
            && profiles[right].next == Some(left)
            && (profiles[left].flags & MONO_OVERSHOOT_BOTTOM == 0 || x2 - x1 < precision.half)
        {
            return false;
        }
    }
    true
}

struct DropoutPixels {
    primary: i32,
    secondary: i32,
}

fn profile_dropout_pixels(
    x1: i32,
    x2: i32,
    precision: MonoPrecision,
    control: u8,
) -> DropoutPixels {
    if control & 4 != 0 {
        // C `ftraster.c` SMART mode chooses the nearest pixel center, then
        // records the adjacent pixel as the alternate dropout target.
        let primary = precision.smart_dropout(x1, x2);
        let secondary = if x1 > primary {
            primary + precision.precision
        } else {
            primary - precision.precision
        };
        return DropoutPixels {
            primary: precision.trunc(primary),
            secondary: precision.trunc(secondary),
        };
    }

    DropoutPixels {
        primary: precision.pixel_floor(x2),
        secondary: precision.pixel_ceil(x1),
    }
}

fn set_mono_dropout_pixels(row: &mut [u8], width: usize, mut primary: i32, secondary: i32) {
    if width == 0 {
        return;
    }
    if primary < 0 || primary >= i32_from_usize(width) {
        primary = secondary;
    } else if secondary >= 0 && secondary < i32_from_usize(width) {
        let secondary = usize_from_i32(secondary);
        if row[secondary / 8] & (0x80 >> (secondary & 7)) != 0 {
            return;
        }
    }

    if primary >= 0 && primary < i32_from_usize(width) {
        let x = usize_from_i32(primary);
        row[x / 8] |= 0x80 >> (x & 7);
    }
}

fn same_profile_flow(left: &MonoProfile, right: &MonoProfile) -> bool {
    (left.flags & MONO_FLOW_UP) == (right.flags & MONO_FLOW_UP)
}

fn mul_div_trunc(a: i32, b: i32, c: i32) -> i32 {
    ((a as i64 * b as i64) / c as i64) as i32
}

fn fill_mono_span(row: &mut [u8], width: usize, mut x1: i32, mut x2: i32) {
    if width == 0 {
        return;
    }
    x1 = x1.max(0);
    x2 = x2.min(i32_from_usize(width - 1));
    if x1 > x2 {
        return;
    }

    let start = usize_from_i32(x1);
    let end = usize_from_i32(x2);
    let start_byte = start / 8;
    let end_byte = end / 8;
    let start_bit = start & 7;
    let end_bit = end & 7;

    if start_byte == end_byte {
        row[start_byte] |= mono_span_mask(start_bit, end_bit);
        return;
    }

    row[start_byte] |= 0xFFu8 >> start_bit;
    if start_byte + 1 < end_byte {
        row[start_byte + 1..end_byte].fill(0xFF);
    }
    row[end_byte] |= 0xFFu8 << (7 - end_bit);
}

fn mono_span_mask(start_bit: usize, end_bit: usize) -> u8 {
    (0xFFu8 >> start_bit) & (0xFFu8 << (7 - end_bit))
}

fn flatten_outline(outline: &Outline) -> Result<Vec<Segment>, FontError> {
    let mut flattener = SdfFlattener {
        segments: Vec::new(),
        current_x: 0,
        current_y: 0,
    };
    flattener.decompose(
        &outline.points,
        &outline.tags,
        &outline.contours,
        outline.n_contours,
    )?;
    Ok(flattener.segments)
}

struct SdfFlattener {
    segments: Vec<Segment>,
    current_x: i32,
    current_y: i32,
}

impl SdfFlattener {
    fn move_to(&mut self, x: i32, y: i32) {
        self.current_x = x;
        self.current_y = y;
    }

    fn line_to(&mut self, x: i32, y: i32) {
        // FreeType's SDF subdivision keeps line edges verbatim
        // (`ftsdf.c:1263-1271`), including degenerate zero-length edges.
        self.segments.push(Segment {
            x0: self.current_x,
            y0: self.current_y,
            x1: x,
            y1: y,
        });
        self.current_x = x;
        self.current_y = y;
    }

    fn conic_to(&mut self, cx: i32, cy: i32, x: i32, y: i32) {
        let x0 = self.current_x;
        let y0 = self.current_y;
        let points = [
            Point { x: x0, y: y0 },
            Point { x: cx, y: cy },
            Point { x, y },
        ];
        if points[0].x == points[1].x && points[0].y == points[1].y
            || points[1].x == points[2].x && points[1].y == points[2].y
        {
            self.line_to(x, y);
            return;
        }

        // FreeType `split_sdf_shape` in `ftsdf.c:1263-1310` reduces the
        // conic deviation by four per bisection until it is below 1/8 px.
        let mut deviation = (points[2].x + points[0].x - 2 * points[1].x)
            .abs()
            .max((points[2].y + points[0].y - 2 * points[1].y).abs());
        let mut splits = 1u32;
        while deviation > FT_PIXEL_ONE / 8 {
            deviation >>= 2;
            splits <<= 1;
        }
        self.flatten_sdf_conic(points, splits);
    }

    fn cubic_to(&mut self, c1x: i32, c1y: i32, c2x: i32, c2y: i32, x: i32, y: i32) {
        let x0 = self.current_x;
        let y0 = self.current_y;
        self.flatten_sdf_cubic(
            [
                Point { x: x0, y: y0 },
                Point { x: c1x, y: c1y },
                Point { x: c2x, y: c2y },
                Point { x, y },
            ],
            32,
        );
    }

    fn flatten_sdf_conic(&mut self, points: [Point; 3], max_splits: u32) {
        // `split_conic` uses signed C division, which truncates toward zero.
        let left_mid = midpoint_trunc(points[0], points[1]);
        let right_mid = midpoint_trunc(points[1], points[2]);
        let center = Point {
            x: (points[0].x + 2 * points[1].x + points[2].x) / 4,
            y: (points[0].y + 2 * points[1].y + points[2].y) / 4,
        };

        if max_splits <= 2 {
            self.line_to(center.x, center.y);
            self.line_to(points[2].x, points[2].y);
            return;
        }

        self.flatten_sdf_conic([points[0], left_mid, center], max_splits / 2);
        self.flatten_sdf_conic([center, right_mid, points[2]], max_splits / 2);
    }

    fn flatten_sdf_cubic(&mut self, points: [Point; 4], max_splits: u32) {
        let flat_enough = (2 * points[0].x - 3 * points[1].x + points[3].x).abs()
            < FT_PIXEL_ONE / 4
            && (2 * points[0].y - 3 * points[1].y + points[3].y).abs() < FT_PIXEL_ONE / 4
            && (points[0].x - 3 * points[2].x + 2 * points[3].x).abs() < FT_PIXEL_ONE / 4
            && (points[0].y - 3 * points[2].y + 2 * points[3].y).abs() < FT_PIXEL_ONE / 4;

        // C `split_sdf_cubic` always performs one `split_cubic` before
        // appending two line segments (`src/sdf/ftsdf.c:1147-1215`).
        let split = split_sdf_cubic(points);
        if flat_enough || max_splits <= 2 {
            self.line_to(split[3].x, split[3].y);
            self.line_to(split[6].x, split[6].y);
            return;
        }

        self.flatten_sdf_cubic([split[0], split[1], split[2], split[3]], max_splits / 2);
        self.flatten_sdf_cubic([split[3], split[4], split[5], split[6]], max_splits / 2);
    }

    fn decompose(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        tags: &[u8],
        contours: &[i16],
        n_contours: i32,
    ) -> Result<(), FontError> {
        let mut last: i64 = -1;
        for &contour_end in contours.iter().take(usize_from_i32(n_contours)) {
            let first = usize_from_i64(last + 1);
            last = contour_end as i64;
            if last < first as i64 {
                return Err(FontError::InvalidOutline(
                    "outline: contour end before start".into(),
                ));
            }
            let limit = usize_from_i64(last);
            let mut v_start = pts[first];
            let v_last = pts[limit];
            let mut limit_eff = limit;

            let first_tag = curve_tag_at(pts, tags, first);
            if first_tag == CURVE_TAG_CUBIC {
                return Err(FontError::InvalidOutline(
                    "outline: contour starts with cubic".into(),
                ));
            }
            if first_tag == CURVE_TAG_CONIC {
                if curve_tag_at(pts, tags, limit) == CURVE_TAG_ON {
                    v_start = v_last;
                    limit_eff = limit.checked_sub(1).ok_or_else(|| {
                        FontError::InvalidOutline("outline: conic start underflow".into())
                    })?;
                } else {
                    v_start.x = (v_start.x + v_last.x) / 2;
                    v_start.y = (v_start.y + v_last.y) / 2;
                }
            }

            self.move_to(v_start.x, v_start.y);
            let start = if first_tag == CURVE_TAG_CONIC {
                if first == 0 {
                    -1
                } else {
                    i32_from_usize(first) - 1
                }
            } else {
                i32_from_usize(first)
            };
            self.walk_contour(pts, tags, start, i32_from_usize(limit_eff), v_start)?;
        }
        Ok(())
    }

    fn walk_contour(
        &mut self,
        pts: &[crate::outline::OutlinePoint],
        tags: &[u8],
        mut cursor: i32,
        limit: i32,
        v_start: crate::outline::OutlinePoint,
    ) -> Result<(), FontError> {
        while cursor < limit {
            cursor += 1;
            let idx = usize_from_i32(cursor);
            match curve_tag_at(pts, tags, idx) {
                CURVE_TAG_ON => {
                    let point = pts[idx];
                    self.line_to(point.x, point.y);
                }
                CURVE_TAG_CONIC => {
                    let mut control = pts[idx];
                    loop {
                        if cursor < limit {
                            cursor += 1;
                            let next = pts[usize_from_i32(cursor)];
                            let tag = curve_tag_at(pts, tags, usize_from_i32(cursor));
                            if tag == CURVE_TAG_ON {
                                self.conic_to(control.x, control.y, next.x, next.y);
                                break;
                            }
                            if tag != CURVE_TAG_CONIC {
                                return Err(FontError::InvalidOutline(
                                    "outline: expected conic tag".into(),
                                ));
                            }
                            let mid_x = (control.x + next.x) / 2;
                            let mid_y = (control.y + next.y) / 2;
                            self.conic_to(control.x, control.y, mid_x, mid_y);
                            control = next;
                            continue;
                        }
                        self.conic_to(control.x, control.y, v_start.x, v_start.y);
                        return Ok(());
                    }
                }
                CURVE_TAG_CUBIC => {
                    if cursor + 2 > limit
                        || curve_tag_at(pts, tags, usize_from_i32(cursor + 1)) != CURVE_TAG_CUBIC
                    {
                        return Err(FontError::InvalidOutline(
                            "outline: bad cubic tag sequence".into(),
                        ));
                    }
                    let control1 = pts[idx];
                    let control2 = pts[usize_from_i32(cursor + 1)];
                    cursor += 2;
                    if cursor <= limit {
                        let point = pts[usize_from_i32(cursor)];
                        self.cubic_to(
                            control1.x, control1.y, control2.x, control2.y, point.x, point.y,
                        );
                    } else {
                        self.cubic_to(
                            control1.x, control1.y, control2.x, control2.y, v_start.x, v_start.y,
                        );
                        return Ok(());
                    }
                }
                _ => unreachable!(),
            }
        }
        self.line_to(v_start.x, v_start.y);
        Ok(())
    }
}

const CURVE_TAG_ON: u8 = 1;
const CURVE_TAG_CONIC: u8 = 0;
const CURVE_TAG_CUBIC: u8 = 2;

fn curve_tag(on_curve: bool) -> u8 {
    if on_curve {
        CURVE_TAG_ON
    } else {
        CURVE_TAG_CONIC
    }
}

fn curve_tag_at(pts: &[crate::outline::OutlinePoint], tags: &[u8], index: usize) -> u8 {
    tags.get(index)
        .map_or_else(|| curve_tag(pts[index].on_curve), |tag| tag & 3)
}

#[derive(Debug, Clone, Copy)]
struct Point {
    x: i32,
    y: i32,
}

type BezierSplitter<const N: usize> = fn([Point; N]) -> ([Point; N], [Point; N]);

fn midpoint(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) >> 1,
        y: (a.y + b.y) >> 1,
    }
}

fn midpoint_trunc(a: Point, b: Point) -> Point {
    Point {
        x: (a.x + b.x) / 2,
        y: (a.y + b.y) / 2,
    }
}

fn split_sdf_cubic(points: [Point; 4]) -> [Point; 7] {
    let mut out = [Point { x: 0, y: 0 }; 7];
    out[0] = points[0];
    out[1] = points[1];
    out[2] = points[2];
    out[3] = points[3];
    split_sdf_cubic_axis(
        points[0].x,
        points[1].x,
        points[2].x,
        points[3].x,
        |index, value| out[index].x = value,
    );
    split_sdf_cubic_axis(
        points[0].y,
        points[1].y,
        points[2].y,
        points[3].y,
        |index, value| out[index].y = value,
    );
    out
}

fn split_sdf_cubic_axis(
    mut p0: i32,
    mut p1: i32,
    p2: i32,
    p3: i32,
    mut set: impl FnMut(usize, i32),
) {
    set(0, p0);
    set(6, p3);
    let mut a = p0 + p1;
    let b = p1 + p2;
    let mut c = p2 + p3;
    set(5, c / 2);
    c += b;
    set(4, c / 4);
    p1 = a / 2;
    set(1, p1);
    a += b;
    set(2, a / 4);
    p0 = (a + c) / 8;
    set(3, p0);
}

#[derive(Debug, Clone, Copy)]
struct SdfDistance {
    distance: i32,
    sign: i8,
    cross: i32,
}

impl SdfDistance {
    fn unset() -> Self {
        Self {
            distance: i32::MAX,
            sign: 0,
            cross: 0,
        }
    }
}

fn rasterize_sdf_outline(
    outline: &Outline,
    width: usize,
    rows: usize,
    spread: i32,
) -> Result<Vec<u8>, FontError> {
    if width == 0 || rows == 0 || outline.is_empty() {
        return Ok(Vec::new());
    }
    let segments = flatten_outline(outline)?;
    let fixed_spread = spread * FT_INT_16D16_ONE;
    let mut dists = vec![SdfDistance::unset(); width * rows];
    let fill_left = outline_orientation_fill_left(outline);

    for segment in &segments {
        let mut x_min = (segment.x0.min(segment.x1) - 63) / 64 - spread;
        let mut x_max = (segment.x0.max(segment.x1) + 63) / 64 + spread;
        let mut y_min = (segment.y0.min(segment.y1) - 63) / 64 - spread;
        let mut y_max = (segment.y0.max(segment.y1) + 63) / 64 + spread;
        x_min = x_min.max(0);
        y_min = y_min.max(0);
        x_max = x_max.min(i32_from_usize(width));
        y_max = y_max.min(i32_from_usize(rows));

        for y in y_min..y_max {
            for x in x_min..x_max {
                let point = Point {
                    x: x * FT_PIXEL_ONE + FT_PIXEL_ONE / 2,
                    y: y * FT_PIXEL_ONE + FT_PIXEL_ONE / 2,
                };
                let mut dist = sdf_line_distance(*segment, point);
                if fill_left {
                    dist.sign = -dist.sign;
                }
                if dist.distance > fixed_spread {
                    continue;
                }

                let row = rows - usize_from_i32(y) - 1;
                let index = row * width + usize_from_i32(x);
                if dists[index].sign == 0 {
                    dists[index] = dist;
                } else {
                    let diff = (dists[index].distance - dist.distance).abs();
                    if diff <= 32 {
                        dists[index] = resolve_sdf_corner(dists[index], dist);
                    } else if dists[index].distance > dist.distance {
                        dists[index] = dist;
                    }
                }
            }
        }
    }

    let mut buffer = vec![0u8; width * rows];
    for y in 0..rows {
        let mut current_sign = -1i8;
        for x in 0..width {
            let index = y * width + x;
            let mut dist = dists[index];
            if dist.sign == 0 {
                dist.distance = fixed_spread;
            } else {
                current_sign = dist.sign;
            }
            dist.distance = dist.distance.min(fixed_spread);
            let signed_distance = dist.distance * i32::from(current_sign);
            buffer[index] = map_fixed_to_sdf(signed_distance, fixed_spread);
        }
    }
    Ok(buffer)
}

fn sdf_line_distance(segment: Segment, point: Point) -> SdfDistance {
    let dx = segment.x1 - segment.x0;
    let dy = segment.y1 - segment.y0;
    if dx == 0 && dy == 0 {
        return SdfDistance::unset();
    }

    let px = point.x - segment.x0;
    let py = point.y - segment.y0;
    let sq_line_length = (((dx as i64 * dx as i64) / 64) + ((dy as i64 * dy as i64) / 64)) as i32;
    // C `get_min_distance_line` in `src/sdf/ftsdf.c:1932-1940` lets
    // `FT_DivFix` handle a truncated-zero squared length, then clamps the
    // sentinel factor below.  Tiny source-backed contours depend on that.
    let projection = (((px as i64 * dx as i64) / 64) + ((py as i64 * dy as i64) / 64)) as i32;
    let mut factor = ft_div_fix(projection, sq_line_length);
    factor = factor.clamp(0, FT_INT_16D16_ONE);

    let nearest_x = (segment.x0 * 1024).wrapping_add(ft_mul_fix(dx.wrapping_mul(1024), factor));
    let nearest_y = (segment.y0 * 1024).wrapping_add(ft_mul_fix(dy.wrapping_mul(1024), factor));
    let nearest_vector_x = nearest_x.wrapping_sub(point.x.wrapping_mul(1024));
    let nearest_vector_y = nearest_y.wrapping_sub(point.y.wrapping_mul(1024));
    let sign_cross =
        ft_mul_fix(nearest_vector_x, dy).wrapping_sub(ft_mul_fix(nearest_vector_y, dx));
    let distance = ft_vector_length(nearest_vector_x, nearest_vector_y);
    let cross = if factor != 0 && factor != FT_INT_16D16_ONE {
        FT_INT_16D16_ONE
    } else {
        normalized_cross_16d16(dx, dy, nearest_vector_x, nearest_vector_y)
    };

    SdfDistance {
        distance,
        sign: if sign_cross < 0 { 1 } else { -1 },
        cross,
    }
}

fn normalized_cross_16d16(dx: i32, dy: i32, vx: i32, vy: i32) -> i32 {
    if (dx == 0 && dy == 0) || (vx == 0 && vy == 0) {
        return 0;
    }
    let ((line_x, line_y), _) = ft_vector_norm_len(dx, dy);
    let ((vector_x, vector_y), _) = ft_vector_norm_len(vx, vy);
    ft_mul_fix(line_x, vector_y).wrapping_sub(ft_mul_fix(line_y, vector_x))
}

fn resolve_sdf_corner(left: SdfDistance, right: SdfDistance) -> SdfDistance {
    if left.cross.abs() > right.cross.abs() {
        left
    } else {
        right
    }
}

fn map_fixed_to_sdf(distance: i32, max_value: i32) -> u8 {
    let normalized = ft_div_fix(distance, max_value);
    let mut udist = normalized.unsigned_abs() >> 9;
    if normalized > 0 && udist > 127 {
        udist = 127;
    }
    if normalized < 0 && udist > 128 {
        udist = 128;
    }
    if normalized < 0 {
        128u8.saturating_sub(udist as u8)
    } else {
        (udist as u8).saturating_add(128)
    }
}

fn outline_orientation_fill_left(outline: &Outline) -> bool {
    let mut area = 0i64;
    let mut first = 0usize;
    for &last_i16 in outline
        .contours
        .iter()
        .take(usize_from_i32(outline.n_contours))
    {
        let last = usize_from_i32(i32::from(last_i16));
        if last < first || last >= outline.points.len() {
            return false;
        }
        let mut prev = outline.points[last];
        for index in first..=last {
            let current = outline.points[index];
            area = area.wrapping_add((current.y - prev.y) as i64 * (current.x + prev.x) as i64);
            prev = current;
        }
        first = last + 1;
    }
    area > 0
}

fn render_lcd(outline: Outline, left: i32, top: i32) -> Result<RenderedBitmap, FontError> {
    let box_ = lcd_pixel_box(&outline, RenderMode::Lcd);
    let width = usize_from_i32(box_.x_max - box_.x_min);
    let height = usize_from_i32(box_.y_max - box_.y_min);
    let row_width = width * 3;
    let pitch = pad_ceil(row_width, 4);
    let mut buffer = vec![0u8; pitch * height];
    let mut scratch = grays::RasterScratch::new();
    for (channel, sub_x) in LCD_SUBPIXELS.iter().enumerate() {
        grays::rasterize_shifted_in_box_to_with_scratch(
            &outline,
            -box_.x_min * FT_PIXEL_ONE - *sub_x,
            -box_.y_min * FT_PIXEL_ONE,
            width,
            height,
            &mut buffer,
            pitch,
            3,
            channel,
            0,
            i32_from_usize(width),
            0,
            i32_from_usize(height),
            &mut scratch,
        )?;
    }
    Ok(RenderedBitmap {
        width: u32_from_usize(row_width),
        rows: u32_from_usize(height),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::Lcd,
        num_grays: PixelMode::Lcd.num_grays(),
        left: left + box_.x_min,
        top: top - outline.cbox_y_max + box_.y_max,
        buffer,
    })
}

fn render_lcd_v(outline: Outline, left: i32, top: i32) -> Result<RenderedBitmap, FontError> {
    let box_ = lcd_pixel_box(&outline, RenderMode::LcdV);
    let width = usize_from_i32(box_.x_max - box_.x_min);
    let height = usize_from_i32(box_.y_max - box_.y_min);
    let rows = height * 3;
    let pitch = width;
    let mut buffer = vec![0u8; pitch * rows];
    let mut scratch = grays::RasterScratch::new();
    for (channel, sub_x) in LCD_SUBPIXELS.iter().enumerate() {
        // FreeType's Harmony LCD_V path translates into the preset bitmap box,
        // then applies rotated LCD geometry: (-sub.y, sub.x).  The default
        // geometry has sub.y = 0 and sub.x = [-21, 0, 21].
        grays::rasterize_shifted_in_box_to_with_scratch(
            &outline,
            -box_.x_min * FT_PIXEL_ONE,
            -box_.y_min * FT_PIXEL_ONE + *sub_x,
            width,
            height,
            &mut buffer,
            pitch * 3,
            1,
            channel * pitch,
            0,
            i32_from_usize(width),
            0,
            i32_from_usize(height),
            &mut scratch,
        )?;
    }
    Ok(RenderedBitmap {
        width: u32_from_usize(width),
        rows: u32_from_usize(rows),
        pitch: i32_from_usize(pitch),
        pixel_mode: PixelMode::LcdV,
        num_grays: PixelMode::LcdV.num_grays(),
        left: left + box_.x_min,
        top: top - outline.cbox_y_max + box_.y_max,
        buffer,
    })
}

pub fn mono_pitch(width: usize) -> usize {
    ((width + 15) >> 4) << 1
}

pub fn pad_ceil(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

pub fn unpack_mono_row(row: &[u8], width: usize) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width);
    for x in 0..width {
        let byte = row.get(x / 8).copied().unwrap_or(0);
        pixels.push(if (byte & (0x80 >> (x & 7))) != 0 {
            255
        } else {
            0
        });
    }
    pixels
}

fn translate_outline(outline: &mut Outline, dx: i32, dy: i32) {
    for point in &mut outline.points {
        point.x += dx;
        point.y += dy;
    }
}

fn lcd_pixel_box(outline: &Outline, mode: RenderMode) -> PixelBox {
    let mut cbox = outline_cbox_26_6(outline);
    let mut pbox = PixelBox {
        x_min: cbox.x_min >> 6,
        y_min: cbox.y_min >> 6,
        x_max: cbox.x_max >> 6,
        y_max: cbox.y_max >> 6,
    };

    cbox.x_min &= 63;
    cbox.y_min &= 63;
    cbox.x_max &= 63;
    cbox.y_max &= 63;
    lcd_padding(&mut cbox, mode);

    pbox.x_min += cbox.x_min >> 6;
    pbox.y_min += cbox.y_min >> 6;
    pbox.x_max += (cbox.x_max + 63) >> 6;
    pbox.y_max += (cbox.y_max + 63) >> 6;
    pbox
}

fn lcd_padding(cbox: &mut PixelBox, mode: RenderMode) {
    let min_sub = LCD_SUBPIXELS[0];
    let max_sub = LCD_SUBPIXELS[2];
    match mode {
        RenderMode::Lcd => {
            cbox.x_min -= max_sub;
            cbox.x_max -= min_sub;
        }
        RenderMode::LcdV => {
            cbox.y_min += min_sub;
            cbox.y_max += max_sub;
        }
        RenderMode::Normal | RenderMode::Mono | RenderMode::Sdf => {}
    }
}

fn outline_cbox_26_6(outline: &Outline) -> PixelBox {
    if outline.points.is_empty() {
        return PixelBox {
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
        };
    }

    let mut x_min = outline.points[0].x;
    let mut y_min = outline.points[0].y;
    let mut x_max = outline.points[0].x;
    let mut y_max = outline.points[0].y;

    for point in &outline.points {
        x_min = x_min.min(point.x);
        y_min = y_min.min(point.y);
        x_max = x_max.max(point.x);
        y_max = y_max.max(point.y);
    }

    PixelBox {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}
